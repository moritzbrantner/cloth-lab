use gltf::{Node, Scene, buffer::Source, mesh::Mode};

use super::{GarmentAsset, GarmentImportError, GarmentImporter, GarmentSourceFormat};
use crate::Vec3;

const MAX_NODE_DEPTH: usize = 256;
type Matrix4 = [[f64; 4]; 4];

const IDENTITY_MATRIX: Matrix4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// Geometry-only importer for a self-contained binary glTF (`.glb`) garment.
///
/// Static triangle meshes from the selected scene are flattened into one normalized simulation
/// surface after applying node hierarchy transforms. Rendering-only attributes are ignored.
/// Animation, skins, morph targets, external geometry buffers, and non-triangle primitives are
/// rejected explicitly rather than being approximated or silently discarded.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlbGarmentImporter;

impl GarmentImporter for GlbGarmentImporter {
    fn source_format(&self) -> GarmentSourceFormat {
        GarmentSourceFormat::Glb
    }

    fn import(&self, bytes: &[u8]) -> Result<GarmentAsset, GarmentImportError> {
        if !bytes.starts_with(b"glTF") {
            return Err(GarmentImportError::InvalidGlb);
        }

        let gltf = gltf::Gltf::from_slice(bytes).map_err(|_| GarmentImportError::InvalidGlb)?;
        if gltf.animations().next().is_some() {
            return Err(GarmentImportError::UnsupportedGlbAnimation);
        }
        if gltf
            .buffers()
            .any(|buffer| matches!(buffer.source(), Source::Uri(_)))
        {
            return Err(GarmentImportError::ExternalGlbBuffer);
        }
        let blob = gltf
            .blob
            .as_deref()
            .ok_or(GarmentImportError::MissingGlbBinaryChunk)?;
        let scene = selected_scene(&gltf)?;

        let mut positions = Vec::new();
        let mut triangles = Vec::new();
        for node in scene.nodes() {
            append_node(
                node,
                IDENTITY_MATRIX,
                blob,
                &mut positions,
                &mut triangles,
                0,
            )?;
        }

        if positions.is_empty() {
            return Err(GarmentImportError::MissingVertices);
        }
        if triangles.is_empty() {
            return Err(GarmentImportError::MissingTriangles);
        }

        Ok(GarmentAsset {
            source_format: GarmentSourceFormat::Glb,
            positions,
            triangles,
        })
    }
}

fn selected_scene<'a>(gltf: &'a gltf::Gltf) -> Result<Scene<'a>, GarmentImportError> {
    if let Some(scene) = gltf.default_scene() {
        return Ok(scene);
    }

    let mut scenes = gltf.scenes();
    let scene = scenes.next().ok_or(GarmentImportError::MissingGlbScene)?;
    if scenes.next().is_some() {
        return Err(GarmentImportError::AmbiguousGlbScene);
    }
    Ok(scene)
}

fn append_node(
    node: Node<'_>,
    parent_transform: Matrix4,
    blob: &[u8],
    positions: &mut Vec<Vec3>,
    triangles: &mut Vec<[usize; 3]>,
    depth: usize,
) -> Result<(), GarmentImportError> {
    if depth > MAX_NODE_DEPTH {
        return Err(GarmentImportError::GlbHierarchyTooDeep);
    }
    if node.skin().is_some() {
        return Err(GarmentImportError::UnsupportedGlbSkin);
    }
    if node.weights().is_some() {
        return Err(GarmentImportError::UnsupportedGlbMorphTargets);
    }

    let world_transform = multiply_matrices(
        parent_transform,
        matrix_to_f64(node.transform().matrix()),
    );
    if !matrix_is_finite(world_transform) {
        return Err(GarmentImportError::NonFiniteGlbGeometry);
    }

    if let Some(mesh) = node.mesh() {
        if mesh.weights().is_some() {
            return Err(GarmentImportError::UnsupportedGlbMorphTargets);
        }
        for primitive in mesh.primitives() {
            append_primitive(primitive, world_transform, blob, positions, triangles)?;
        }
    }

    for child in node.children() {
        append_node(
            child,
            world_transform,
            blob,
            positions,
            triangles,
            depth + 1,
        )?;
    }
    Ok(())
}

fn append_primitive(
    primitive: gltf::Primitive<'_>,
    transform: Matrix4,
    blob: &[u8],
    positions: &mut Vec<Vec3>,
    triangles: &mut Vec<[usize; 3]>,
) -> Result<(), GarmentImportError> {
    if primitive.mode() != Mode::Triangles {
        return Err(GarmentImportError::UnsupportedGlbPrimitiveMode);
    }
    if primitive.morph_targets().next().is_some() {
        return Err(GarmentImportError::UnsupportedGlbMorphTargets);
    }

    let reader = primitive.reader(|buffer| match buffer.source() {
        Source::Bin => Some(blob),
        Source::Uri(_) => None,
    });
    let primitive_positions = reader
        .read_positions()
        .ok_or(GarmentImportError::MissingGlbPositions)?
        .map(|position| transform_position(transform, position))
        .collect::<Result<Vec<_>, _>>()?;
    if primitive_positions.is_empty() {
        return Err(GarmentImportError::MissingGlbPositions);
    }

    let local_indices = if let Some(indices) = reader.read_indices() {
        indices
            .into_u32()
            .map(|index| usize::try_from(index).map_err(|_| GarmentImportError::InvalidGlbIndices))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        (0..primitive_positions.len()).collect::<Vec<_>>()
    };
    if local_indices.is_empty() || local_indices.len() % 3 != 0 {
        return Err(GarmentImportError::InvalidGlbIndices);
    }

    let base_index = positions.len();
    for indices in local_indices.chunks_exact(3) {
        let triangle = [indices[0], indices[1], indices[2]];
        if triangle
            .into_iter()
            .any(|index| index >= primitive_positions.len())
        {
            return Err(GarmentImportError::InvalidGlbIndices);
        }
        if triangle[0] == triangle[1]
            || triangle[1] == triangle[2]
            || triangle[0] == triangle[2]
            || triangle_area_squared(&primitive_positions, triangle) <= f64::EPSILON
        {
            return Err(GarmentImportError::DegenerateGlbTriangle);
        }

        triangles.push([
            base_index
                .checked_add(triangle[0])
                .ok_or(GarmentImportError::InvalidGlbIndices)?,
            base_index
                .checked_add(triangle[1])
                .ok_or(GarmentImportError::InvalidGlbIndices)?,
            base_index
                .checked_add(triangle[2])
                .ok_or(GarmentImportError::InvalidGlbIndices)?,
        ]);
    }
    positions.extend(primitive_positions);
    Ok(())
}

fn matrix_to_f64(matrix: [[f32; 4]; 4]) -> Matrix4 {
    matrix.map(|column| column.map(f64::from))
}

fn multiply_matrices(left: Matrix4, right: Matrix4) -> Matrix4 {
    let mut product = [[0.0; 4]; 4];
    for column in 0..4 {
        for row in 0..4 {
            product[column][row] = (0..4)
                .map(|index| left[index][row] * right[column][index])
                .sum();
        }
    }
    product
}

fn matrix_is_finite(matrix: Matrix4) -> bool {
    matrix
        .into_iter()
        .flatten()
        .all(|component| component.is_finite())
}

fn transform_position(
    matrix: Matrix4,
    [x, y, z]: [f32; 3],
) -> Result<Vec3, GarmentImportError> {
    let [x, y, z] = [f64::from(x), f64::from(y), f64::from(z)];
    let position = Vec3::new(
        matrix[0][0] * x + matrix[1][0] * y + matrix[2][0] * z + matrix[3][0],
        matrix[0][1] * x + matrix[1][1] * y + matrix[2][1] * z + matrix[3][1],
        matrix[0][2] * x + matrix[1][2] * y + matrix[2][2] * z + matrix[3][2],
    );
    if !position.is_finite() {
        return Err(GarmentImportError::NonFiniteGlbGeometry);
    }
    Ok(position)
}

fn triangle_area_squared(positions: &[Vec3], triangle: [usize; 3]) -> f64 {
    let edge_ab = positions[triangle[1]] - positions[triangle[0]];
    let edge_ac = positions[triangle[2]] - positions[triangle[0]];
    let cross_x = edge_ab.y * edge_ac.z - edge_ab.z * edge_ac.y;
    let cross_y = edge_ab.z * edge_ac.x - edge_ab.x * edge_ac.z;
    let cross_z = edge_ab.x * edge_ac.y - edge_ab.y * edge_ac.x;
    cross_x * cross_x + cross_y * cross_y + cross_z * cross_z
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TriangleMeshCloth, TriangleMeshClothConfig};

    #[test]
    fn imports_transformed_static_triangle_from_glb() {
        let bytes = triangle_glb(4, true, true);
        let asset = GlbGarmentImporter.import(&bytes).expect("valid GLB garment");

        assert_eq!(asset.source_format(), GarmentSourceFormat::Glb);
        assert_eq!(
            asset.positions(),
            &[
                Vec3::new(2.0, 3.0, 4.0),
                Vec3::new(3.0, 3.0, 4.0),
                Vec3::new(2.0, 4.0, 4.0),
            ]
        );
        assert_eq!(asset.triangles(), &[[0, 1, 2]]);
    }

    #[test]
    fn imports_unindexed_triangle_mode() {
        let bytes = triangle_glb(4, false, true);
        let asset = GlbGarmentImporter.import(&bytes).expect("valid unindexed GLB garment");

        assert_eq!(asset.triangles(), &[[0, 1, 2]]);
    }

    #[test]
    fn imported_glb_enters_triangle_mesh_simulation_deterministically() {
        let bytes = triangle_glb(4, true, true);
        let asset = GlbGarmentImporter.import(&bytes).expect("valid GLB garment");
        let config = TriangleMeshClothConfig {
            particle_mass: 1.0,
            stretch_compliance: 1.0e-7,
            bending_compliance: 1.0e-3,
        };
        let first = TriangleMeshCloth::new(asset.positions(), asset.triangles(), config)
            .expect("GLB garment must initialize cloth");
        let second = TriangleMeshCloth::new(asset.positions(), asset.triangles(), config)
            .expect("same GLB garment must initialize cloth");

        assert_eq!(first.state_fingerprint(), second.state_fingerprint());
        assert_eq!(
            GlbGarmentImporter.import(&bytes).unwrap().simulation_fingerprint(),
            asset.simulation_fingerprint()
        );
    }

    #[test]
    fn rejects_non_triangle_primitive_mode() {
        let bytes = triangle_glb(1, true, true);
        let error = GlbGarmentImporter
            .import(&bytes)
            .expect_err("line mode must not be interpreted as cloth triangles");

        assert_eq!(error, GarmentImportError::UnsupportedGlbPrimitiveMode);
    }

    #[test]
    fn rejects_external_geometry_buffer_and_plain_json_gltf() {
        let external = triangle_glb(4, true, false);
        assert_eq!(
            GlbGarmentImporter.import(&external).unwrap_err(),
            GarmentImportError::ExternalGlbBuffer
        );
        assert_eq!(
            GlbGarmentImporter
                .import(br#"{"asset":{"version":"2.0"}}"#)
                .unwrap_err(),
            GarmentImportError::InvalidGlb
        );
    }

    fn triangle_glb(mode: u32, indexed: bool, embedded_buffer: bool) -> Vec<u8> {
        let mut binary = Vec::new();
        for component in [
            0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0,
        ] {
            binary.extend_from_slice(&component.to_le_bytes());
        }
        if indexed {
            for index in [0_u16, 1, 2] {
                binary.extend_from_slice(&index.to_le_bytes());
            }
        }

        let indices_view = if indexed {
            r#",{"buffer":0,"byteOffset":36,"byteLength":6}"#
        } else {
            ""
        };
        let indices_accessor = if indexed {
            r#",{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}"#
        } else {
            ""
        };
        let primitive_indices = if indexed { r#","indices":1"# } else { "" };
        let buffer = if embedded_buffer {
            format!(r#"{{"byteLength":{}}}"#, binary.len())
        } else {
            format!(r#"{{"byteLength":{},"uri":"mesh.bin"}}"#, binary.len())
        };
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"buffers":[{buffer}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}}{indices_view}],"accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}}{indices_accessor}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}}{primitive_indices},"mode":{mode}}}]}}],"nodes":[{{"mesh":0,"translation":[2,3,4]}}],"scenes":[{{"nodes":[0]}}],"scene":0}}"#,
        );

        if embedded_buffer {
            build_glb(&json, Some(&binary))
        } else {
            build_glb(&json, None)
        }
    }

    fn build_glb(json: &str, binary: Option<&[u8]>) -> Vec<u8> {
        let mut json_chunk = json.as_bytes().to_vec();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(b' ');
        }
        let mut binary_chunk = binary.unwrap_or_default().to_vec();
        while binary_chunk.len() % 4 != 0 {
            binary_chunk.push(0);
        }

        let binary_chunk_size = binary.map_or(0, |_| 8 + binary_chunk.len());
        let total_length = 12 + 8 + json_chunk.len() + binary_chunk_size;
        let mut output = Vec::with_capacity(total_length);
        output.extend_from_slice(b"glTF");
        output.extend_from_slice(&2_u32.to_le_bytes());
        output.extend_from_slice(&(total_length as u32).to_le_bytes());
        output.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
        output.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
        output.extend_from_slice(&json_chunk);
        if binary.is_some() {
            output.extend_from_slice(&(binary_chunk.len() as u32).to_le_bytes());
            output.extend_from_slice(&0x004e_4942_u32.to_le_bytes());
            output.extend_from_slice(&binary_chunk);
        }
        output
    }
}
