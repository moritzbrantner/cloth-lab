use core::fmt;

use crate::Vec3;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// External source format used to create a normalized garment asset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GarmentSourceFormat {
    Obj,
}

/// Normalized geometry that can be validated independently of a UI or file parser.
///
/// This first slice deliberately contains only simulation surface geometry. Pattern pieces,
/// sewing relationships, fabric metadata, and attachments can be added as explicit normalized
/// data when importers that actually provide those semantics are introduced.
#[derive(Clone, Debug, PartialEq)]
pub struct GarmentAsset {
    source_format: GarmentSourceFormat,
    positions: Vec<Vec3>,
    triangles: Vec<[usize; 3]>,
}

impl GarmentAsset {
    #[must_use]
    pub const fn source_format(&self) -> GarmentSourceFormat {
        self.source_format
    }

    #[must_use]
    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    #[must_use]
    pub fn triangles(&self) -> &[[usize; 3]] {
        &self.triangles
    }

    /// Deterministic fingerprint of normalized simulation geometry.
    ///
    /// Source provenance is intentionally excluded so equivalent normalized geometry imported
    /// through different adapters can converge on the same simulation identity.
    #[must_use]
    pub fn simulation_fingerprint(&self) -> u64 {
        let mut hash = FNV_OFFSET_BASIS;
        hash_u64(&mut hash, self.positions.len() as u64);
        for position in &self.positions {
            hash_u64(&mut hash, position.x.to_bits());
            hash_u64(&mut hash, position.y.to_bits());
            hash_u64(&mut hash, position.z.to_bits());
        }
        hash_u64(&mut hash, self.triangles.len() as u64);
        for triangle in &self.triangles {
            for &index in triangle {
                hash_u64(&mut hash, index as u64);
            }
        }
        hash
    }
}

/// Adapter boundary for converting external garment data into `GarmentAsset`.
pub trait GarmentImporter {
    fn source_format(&self) -> GarmentSourceFormat;

    fn import(&self, bytes: &[u8]) -> Result<GarmentAsset, GarmentImportError>;
}

/// Geometry-only OBJ importer for the first garment-ingestion slice.
///
/// Vertex positions and polygon faces are normalized into an indexed triangle surface. Texture
/// coordinates, normals, material libraries, groups, and other rendering metadata are ignored;
/// they are not treated as simulation semantics.
#[derive(Clone, Copy, Debug, Default)]
pub struct ObjGarmentImporter;

impl GarmentImporter for ObjGarmentImporter {
    fn source_format(&self) -> GarmentSourceFormat {
        GarmentSourceFormat::Obj
    }

    fn import(&self, bytes: &[u8]) -> Result<GarmentAsset, GarmentImportError> {
        let source = core::str::from_utf8(bytes).map_err(|_| GarmentImportError::InvalidUtf8)?;
        let mut positions = Vec::new();
        let mut triangles = Vec::new();

        for (line_index, raw_line) in source.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut fields = line.split_whitespace();
            let Some(kind) = fields.next() else {
                continue;
            };

            match kind {
                "v" => positions.push(parse_vertex(fields, line_number)?),
                "f" => {
                    let face = fields
                        .map(|token| parse_face_index(token, positions.len(), line_number))
                        .collect::<Result<Vec<_>, _>>()?;
                    if face.len() < 3 {
                        return Err(GarmentImportError::InvalidFace { line: line_number });
                    }

                    for offset in 1..(face.len() - 1) {
                        let triangle = [face[0], face[offset], face[offset + 1]];
                        if triangle[0] == triangle[1]
                            || triangle[1] == triangle[2]
                            || triangle[0] == triangle[2]
                        {
                            return Err(GarmentImportError::DegenerateFace { line: line_number });
                        }
                        triangles.push(triangle);
                    }
                }
                _ => {}
            }
        }

        if positions.is_empty() {
            return Err(GarmentImportError::MissingVertices);
        }
        if triangles.is_empty() {
            return Err(GarmentImportError::MissingTriangles);
        }

        Ok(GarmentAsset {
            source_format: GarmentSourceFormat::Obj,
            positions,
            triangles,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GarmentImportError {
    InvalidUtf8,
    MissingVertices,
    MissingTriangles,
    InvalidVertex { line: usize },
    NonFiniteVertex { line: usize },
    InvalidFace { line: usize },
    InvalidFaceIndex { line: usize },
    FaceIndexOutOfBounds { line: usize },
    DegenerateFace { line: usize },
}

impl fmt::Display for GarmentImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 => formatter.write_str("garment input is not valid UTF-8"),
            Self::MissingVertices => formatter.write_str("garment contains no vertices"),
            Self::MissingTriangles => formatter.write_str("garment contains no triangle surface"),
            Self::InvalidVertex { line } => write!(formatter, "invalid OBJ vertex at line {line}"),
            Self::NonFiniteVertex { line } => {
                write!(formatter, "non-finite OBJ vertex at line {line}")
            }
            Self::InvalidFace { line } => write!(formatter, "invalid OBJ face at line {line}"),
            Self::InvalidFaceIndex { line } => {
                write!(formatter, "invalid OBJ face index at line {line}")
            }
            Self::FaceIndexOutOfBounds { line } => {
                write!(formatter, "OBJ face index is out of bounds at line {line}")
            }
            Self::DegenerateFace { line } => {
                write!(formatter, "degenerate OBJ face at line {line}")
            }
        }
    }
}

impl std::error::Error for GarmentImportError {}

fn parse_vertex<'a>(
    mut fields: impl Iterator<Item = &'a str>,
    line: usize,
) -> Result<Vec3, GarmentImportError> {
    let x = parse_coordinate(fields.next(), line)?;
    let y = parse_coordinate(fields.next(), line)?;
    let z = parse_coordinate(fields.next(), line)?;
    let position = Vec3::new(x, y, z);
    if !position.is_finite() {
        return Err(GarmentImportError::NonFiniteVertex { line });
    }
    Ok(position)
}

fn parse_coordinate(value: Option<&str>, line: usize) -> Result<f64, GarmentImportError> {
    value
        .ok_or(GarmentImportError::InvalidVertex { line })?
        .parse::<f64>()
        .map_err(|_| GarmentImportError::InvalidVertex { line })
}

fn parse_face_index(
    token: &str,
    vertex_count: usize,
    line: usize,
) -> Result<usize, GarmentImportError> {
    let raw_index = token
        .split('/')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or(GarmentImportError::InvalidFaceIndex { line })?;
    let parsed = raw_index
        .parse::<i64>()
        .map_err(|_| GarmentImportError::InvalidFaceIndex { line })?;
    if parsed == 0 {
        return Err(GarmentImportError::InvalidFaceIndex { line });
    }

    let resolved = if parsed > 0 {
        usize::try_from(parsed - 1)
            .map_err(|_| GarmentImportError::FaceIndexOutOfBounds { line })?
    } else {
        let offset = usize::try_from(parsed.unsigned_abs())
            .map_err(|_| GarmentImportError::FaceIndexOutOfBounds { line })?;
        vertex_count
            .checked_sub(offset)
            .ok_or(GarmentImportError::FaceIndexOutOfBounds { line })?
    };

    if resolved >= vertex_count {
        return Err(GarmentImportError::FaceIndexOutOfBounds { line });
    }
    Ok(resolved)
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GarmentImportError, GarmentImporter, GarmentSourceFormat, ObjGarmentImporter,
    };
    use crate::Vec3;

    #[test]
    fn imports_triangle_surface() {
        let source = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";

        let asset = ObjGarmentImporter.import(source).expect("valid OBJ");

        assert_eq!(asset.source_format(), GarmentSourceFormat::Obj);
        assert_eq!(
            asset.positions(),
            &[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ]
        );
        assert_eq!(asset.triangles(), &[[0, 1, 2]]);
    }

    #[test]
    fn triangulates_polygons_in_source_order() {
        let source = b"v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n";

        let asset = ObjGarmentImporter.import(source).expect("valid OBJ");

        assert_eq!(asset.triangles(), &[[0, 1, 2], [0, 2, 3]]);
    }

    #[test]
    fn supports_relative_obj_indices() {
        let source = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3/-1 -2/-1 -1/-1\n";

        let asset = ObjGarmentImporter.import(source).expect("valid OBJ");

        assert_eq!(asset.triangles(), &[[0, 1, 2]]);
    }

    #[test]
    fn rejects_non_finite_vertices_before_asset_creation() {
        let source = b"v NaN 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";

        let error = ObjGarmentImporter.import(source).expect_err("must fail closed");

        assert_eq!(error, GarmentImportError::NonFiniteVertex { line: 1 });
    }

    #[test]
    fn rejects_out_of_bounds_faces() {
        let source = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 4\n";

        let error = ObjGarmentImporter.import(source).expect_err("must fail closed");

        assert_eq!(error, GarmentImportError::FaceIndexOutOfBounds { line: 4 });
    }

    #[test]
    fn normalized_geometry_has_stable_fingerprint() {
        let source = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";

        let first = ObjGarmentImporter.import(source).expect("valid OBJ");
        let second = ObjGarmentImporter.import(source).expect("valid OBJ");

        assert_eq!(first.simulation_fingerprint(), second.simulation_fingerprint());
    }
}
