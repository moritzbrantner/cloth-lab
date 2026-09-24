use core::fmt;

use three_d_animation::retarget::HumanoidBone;

use crate::{
    CapsuleCollider, ClothCollider, Vec3, garment::simulation_geometry_fingerprint,
    mannequin::{MannequinAnimator, MannequinAttachment},
};

const MIN_TEMPLATE_RESOLUTION: u32 = 6;
const MAX_TEMPLATE_RESOLUTION: u32 = 40;
const COLLISION_THICKNESS: f64 = 0.025;

/// Deterministic built-in garments used to exercise cloth behavior without importing an asset.
///
/// These are intentionally simulation fixtures rather than apparel pattern definitions. In
/// particular, the T-shirt, cape, skirt, dress, and poncho are single connected drape surfaces;
/// they do not invent sewing relationships before Cloth Lab has an explicit seam model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GarmentTemplate {
    Sheet,
    TShirt,
    Cape,
    Skirt,
    Dress,
    Poncho,
}

impl GarmentTemplate {
    pub const ALL: [Self; 6] = [
        Self::Sheet,
        Self::TShirt,
        Self::Cape,
        Self::Skirt,
        Self::Dress,
        Self::Poncho,
    ];

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Sheet => "Sheet",
            Self::TShirt => "T-shirt",
            Self::Cape => "Cape",
            Self::Skirt => "Skirt",
            Self::Dress => "Dress",
            Self::Poncho => "Poncho",
        }
    }

    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::Sheet => "Free sheet fixture with an editable capsule obstacle and no mannequin.",
            Self::TShirt => "Short-sleeve front drape over the mannequin torso and arms.",
            Self::Cape => "Back drape pinned at the shoulders against the mannequin upper body.",
            Self::Skirt => "Waist-pinned flared drape using pelvis and leg collision geometry.",
            Self::Dress => {
                "Long torso-to-knee drape combining a fitted waist with a flared lower section."
            }
            Self::Poncho => "Wide shoulder-pinned drape for broad folds across the upper body.",
        }
    }

    #[must_use]
    pub const fn uses_mannequin(self) -> bool {
        !matches!(self, Self::Sheet)
    }

    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Sheet => "sheet",
            Self::TShirt => "t-shirt",
            Self::Cape => "cape",
            Self::Skirt => "skirt",
            Self::Dress => "dress",
            Self::Poncho => "poncho",
        }
    }

    #[must_use]
    pub const fn source_kind(self) -> &'static str {
        match self {
            Self::Sheet => "template-sheet",
            Self::TShirt => "template-t-shirt",
            Self::Cape => "template-cape",
            Self::Skirt => "template-skirt",
            Self::Dress => "template-dress",
            Self::Poncho => "template-poncho",
        }
    }

    #[must_use]
    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "sheet" => Some(Self::Sheet),
            "t-shirt" => Some(Self::TShirt),
            "cape" => Some(Self::Cape),
            "skirt" => Some(Self::Skirt),
            "dress" => Some(Self::Dress),
            "poncho" => Some(Self::Poncho),
            _ => None,
        }
    }

    pub fn build(self, resolution: u32) -> Result<GarmentTemplateAsset, GarmentTemplateError> {
        if !(MIN_TEMPLATE_RESOLUTION..=MAX_TEMPLATE_RESOLUTION).contains(&resolution) {
            return Err(GarmentTemplateError::ResolutionOutOfRange);
        }

        let columns = resolution as usize;
        Ok(match self {
            Self::Sheet => build_sheet(columns),
            Self::TShirt => build_t_shirt(columns),
            Self::Cape => build_cape(columns),
            Self::Skirt => build_skirt(columns),
            Self::Dress => build_dress(columns),
            Self::Poncho => build_poncho(columns),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GarmentTemplateAsset {
    template: GarmentTemplate,
    positions: Vec<Vec3>,
    triangles: Vec<[usize; 3]>,
    pinned_indices: Vec<usize>,
    mannequin_attachments: Vec<MannequinAttachment>,
    editable_obstacle: Option<ClothCollider>,
}

impl GarmentTemplateAsset {
    #[must_use]
    pub const fn template(&self) -> GarmentTemplate {
        self.template
    }

    #[must_use]
    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    #[must_use]
    pub fn triangles(&self) -> &[[usize; 3]] {
        &self.triangles
    }

    #[must_use]
    pub fn pinned_indices(&self) -> &[usize] {
        &self.pinned_indices
    }

    #[must_use]
    pub fn mannequin_attachment_count(&self) -> usize {
        self.mannequin_attachments.len()
    }

    pub(crate) fn mannequin_attachments(&self) -> &[MannequinAttachment] {
        &self.mannequin_attachments
    }

    #[must_use]
    pub const fn editable_obstacle(&self) -> Option<ClothCollider> {
        self.editable_obstacle
    }

    /// Stable identity for the generated simulation geometry.
    #[must_use]
    pub fn simulation_fingerprint(&self) -> u64 {
        simulation_geometry_fingerprint(&self.positions, &self.triangles)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GarmentTemplateError {
    ResolutionOutOfRange,
}

impl fmt::Display for GarmentTemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResolutionOutOfRange => {
                formatter.write_str("garment template resolution must be between 6 and 40")
            }
        }
    }
}

impl std::error::Error for GarmentTemplateError {}

fn build_sheet(columns: usize) -> GarmentTemplateAsset {
    const WIDTH: f64 = 2.08;
    const ROWS_NUMERATOR: usize = 11;
    const ROWS_DENOMINATOR: usize = 13;
    const DEMO_CAPSULE: CapsuleCollider = CapsuleCollider {
        start: Vec3::new(0.52, -0.58, 0.88),
        end: Vec3::new(1.56, -0.58, 0.88),
        radius: 0.28,
        thickness: COLLISION_THICKNESS,
    };

    let rows = ((columns - 1) * ROWS_NUMERATOR + ROWS_DENOMINATOR / 2) / ROWS_DENOMINATOR + 1;
    let spacing = WIDTH / (columns - 1) as f64;
    let mut positions = Vec::with_capacity(columns * rows);
    for row in 0..rows {
        for column in 0..columns {
            positions.push(Vec3::new(
                column as f64 * spacing,
                0.0,
                row as f64 * spacing,
            ));
        }
    }

    GarmentTemplateAsset {
        template: GarmentTemplate::Sheet,
        triangles: regular_grid_triangles(columns, rows),
        pinned_indices: vec![0, columns - 1],
        positions,
        mannequin_attachments: Vec::new(),
        editable_obstacle: Some(ClothCollider::Capsule(DEMO_CAPSULE)),
    }
}

fn build_t_shirt(columns: usize) -> GarmentTemplateAsset {
    let rows = ((columns - 1) * 5 + 3) / 6 + 1;
    let (positions, triangles) = variable_width_panel(
        columns,
        rows,
        1.32,
        -0.72,
        0.18,
        |row_fraction| {
            if row_fraction <= 0.28 { 1.18 } else { 0.68 }
        },
        |x| {
            let neck_half_width = 0.34;
            if x.abs() >= neck_half_width {
                0.0
            } else {
                0.24 * (1.0 - x.abs() / neck_half_width)
            }
        },
    );
    let pinned_indices = vec![
        nearest_top_vertex(&positions, columns, -0.55),
        nearest_top_vertex(&positions, columns, 0.55),
    ];

    GarmentTemplateAsset {
        template: GarmentTemplate::TShirt,
        mannequin_attachments: shoulder_attachments(&positions, &pinned_indices),
        positions,
        triangles,
        pinned_indices,
        editable_obstacle: None,
    }
}}

fn build_cape(columns: usize) -> GarmentTemplateAsset {
    let rows = ((columns - 1) * 4 + 1) / 3 + 1;
    let (positions, triangles) = variable_width_panel(
        columns,
        rows,
        1.22,
        -1.12,
        -0.18,
        |row_fraction| 0.64 + 0.36 * row_fraction,
        |_| 0.0,
    );
    let pinned_indices = vec![
        nearest_top_vertex(&positions, columns, -0.48),
        nearest_top_vertex(&positions, columns, 0.48),
    ];

    GarmentTemplateAsset {
        template: GarmentTemplate::Cape,
        mannequin_attachments: shoulder_attachments(&positions, &pinned_indices),
        positions,
        triangles,
        pinned_indices,
        editable_obstacle: None,
    }
}}

fn build_skirt(columns: usize) -> GarmentTemplateAsset {
    let rows = ((columns - 1) * 3 + 2) / 4 + 1;
    let (positions, triangles) = variable_width_panel(
        columns,
        rows,
        0.22,
        -1.18,
        0.12,
        |row_fraction| 0.56 + 0.4 * row_fraction,
        |_| 0.0,
    );
    let pinned_indices = vec![
        nearest_top_vertex(&positions, columns, -0.42),
        nearest_top_vertex(&positions, columns, 0.42),
    ];

    GarmentTemplateAsset {
        template: GarmentTemplate::Skirt,
        mannequin_attachments: hip_attachments(&positions, &pinned_indices),
        positions,
        triangles,
        pinned_indices,
        editable_obstacle: None,
    }
}}

fn build_dress(columns: usize) -> GarmentTemplateAsset {
    let rows = ((columns - 1) * 7 + 3) / 5 + 1;
    let (positions, triangles) = variable_width_panel(
        columns,
        rows,
        1.34,
        -1.32,
        0.16,
        |row_fraction| {
            if row_fraction <= 0.18 {
                1.04
            } else if row_fraction <= 0.5 {
                0.66
            } else {
                0.66 + 0.5 * ((row_fraction - 0.5) / 0.5)
            }
        },
        |x| {
            let neck_half_width = 0.3;
            if x.abs() >= neck_half_width {
                0.0
            } else {
                0.2 * (1.0 - x.abs() / neck_half_width)
            }
        },
    );
    let pinned_indices = vec![
        nearest_top_vertex(&positions, columns, -0.5),
        nearest_top_vertex(&positions, columns, 0.5),
    ];

    GarmentTemplateAsset {
        template: GarmentTemplate::Dress,
        mannequin_attachments: shoulder_attachments(&positions, &pinned_indices),
        positions,
        triangles,
        pinned_indices,
        editable_obstacle: None,
    }
}}

fn build_poncho(columns: usize) -> GarmentTemplateAsset {
    let rows = ((columns - 1) * 6 + 2) / 5 + 1;
    let (positions, triangles) = variable_width_panel(
        columns,
        rows,
        1.28,
        -0.92,
        -0.12,
        |row_fraction| 0.72 + 0.6 * row_fraction,
        |x| {
            let neck_half_width = 0.28;
            if x.abs() >= neck_half_width {
                0.0
            } else {
                0.16 * (1.0 - x.abs() / neck_half_width)
            }
        },
    );
    let pinned_indices = vec![
        nearest_top_vertex(&positions, columns, -0.44),
        nearest_top_vertex(&positions, columns, 0.44),
    ];

    GarmentTemplateAsset {
        template: GarmentTemplate::Poncho,
        mannequin_attachments: shoulder_attachments(&positions, &pinned_indices),
        positions,
        triangles,
        pinned_indices,
        editable_obstacle: None,
    }
}}

fn variable_width_panel(
    columns: usize,
    rows: usize,
    top_y: f64,
    bottom_y: f64,
    z: f64,
    half_width_for_row: impl Fn(f64) -> f64,
    top_edge_drop: impl Fn(f64) -> f64,
) -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let mut positions = Vec::with_capacity(columns * rows);
    for row in 0..rows {
        let row_fraction = row as f64 / (rows - 1) as f64;
        let half_width = half_width_for_row(row_fraction);
        let base_y = top_y + (bottom_y - top_y) * row_fraction;
        for column in 0..columns {
            let column_fraction = column as f64 / (columns - 1) as f64;
            let x = -half_width + 2.0 * half_width * column_fraction;
            let y = if row == 0 {
                base_y - top_edge_drop(x)
            } else {
                base_y
            };
            positions.push(Vec3::new(x, y, z));
        }
    }
    let triangles = regular_grid_triangles(columns, rows);
    (positions, triangles)
}

fn regular_grid_triangles(columns: usize, rows: usize) -> Vec<[usize; 3]> {
    let mut triangles = Vec::with_capacity((columns - 1) * (rows - 1) * 2);
    for row in 0..rows - 1 {
        for column in 0..columns - 1 {
            let top_left = row * columns + column;
            let top_right = top_left + 1;
            let bottom_left = (row + 1) * columns + column;
            let bottom_right = bottom_left + 1;
            triangles.push([top_left, bottom_left, top_right]);
            triangles.push([top_right, bottom_left, bottom_right]);
        }
    }
    triangles
}

fn nearest_top_vertex(positions: &[Vec3], columns: usize, target_x: f64) -> usize {
    positions[..columns]
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (left.x - target_x)
                .abs()
                .total_cmp(&(right.x - target_x).abs())
        })
        .map_or(0, |(index, _)| index)
}

fn shoulder_attachments(
    positions: &[Vec3],
    pinned_indices: &[usize],
) -> Vec<MannequinAttachment> {
    vec![
        MannequinAttachment::from_rest_position(
            pinned_indices[0],
            HumanoidBone::LeftUpperArm,
            positions[pinned_indices[0]],
        ),
        MannequinAttachment::from_rest_position(
            pinned_indices[1],
            HumanoidBone::RightUpperArm,
            positions[pinned_indices[1]],
        ),
    ]
}

fn hip_attachments(
    positions: &[Vec3],
    pinned_indices: &[usize],
) -> Vec<MannequinAttachment> {
    pinned_indices
        .iter()
        .copied()
        .map(|particle_index| {
            MannequinAttachment::from_rest_position(
                particle_index,
                HumanoidBone::Hips,
                positions[particle_index],
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FixedStepConfig, TextilePreset, TriangleMeshCloth};

    #[test]
    fn all_templates_build_as_valid_triangle_meshes() {
        for template in GarmentTemplate::ALL {
            let asset = template.build(14).expect("template must build");
            assert!(!asset.positions().is_empty());
            assert!(!asset.triangles().is_empty());
            assert!(
                asset
                    .pinned_indices()
                    .iter()
                    .all(|&index| index < asset.positions().len())
            );
            TriangleMeshCloth::new(
                asset.positions(),
                asset.triangles(),
                TextilePreset::CottonLike
                    .parameters()
                    .triangle_mesh_config(1.0),
            )
            .expect("template mesh must satisfy solver topology requirements");
        }
    }

    #[test]
    fn t_shirt_has_sleeves_neckline_and_mannequin_collision() {
        let asset = GarmentTemplate::TShirt.build(18).expect("T-shirt template");

        assert!(
            asset
                .positions()
                .iter()
                .any(|position| position.x.abs() > 1.0),
            "sleeves should extend beyond the torso"
        );
        let top = &asset.positions()[..18];
        let center = top[18 / 2];
        let shoulder = top[3];
        assert!(
            center.y < shoulder.y,
            "neckline should dip below the shoulder edge"
        );
        assert_eq!(asset.mannequin_attachment_count(), 2);
        assert_eq!(asset.pinned_indices().len(), 2);
    }

    fn mannequin_contact_evidence(template: GarmentTemplate) -> (usize, u64) {
        let asset = template.build(18).expect("mannequin garment template");
        let parameters = TextilePreset::CottonLike.parameters();
        let mut cloth = TriangleMeshCloth::new(
            asset.positions(),
            asset.triangles(),
            parameters.triangle_mesh_config(1.0),
        )
        .expect("template mesh");
        for &index in asset.pinned_indices() {
            cloth.pin(index).expect("template pin");
        }

        let animator = MannequinAnimator::new();
        let mut collision_projections = 0;
        for _ in 0..12 {
            let report = cloth
                .step_with_contacts(
                    FixedStepConfig::default(),
                    animator.colliders(),
                    parameters.contact_config(),
                )
                .expect("template step");
            collision_projections += report.collision_projections;
        }
        (collision_projections, cloth.state_fingerprint())
    }

    #[test]
    fn mannequin_templates_contact_and_replay_deterministically() {
        for template in GarmentTemplate::ALL
            .into_iter()
            .filter(|template| template.uses_mannequin())
        {
            let first = mannequin_contact_evidence(template);
            let second = mannequin_contact_evidence(template);

            assert!(
                first.0 > 0,
                "{template:?} fixture must actually contact the mannequin"
            );
            assert_eq!(
                first, second,
                "{template:?} mannequin drape replay must be deterministic"
            );
        }
    }

    #[test]
    fn mannequin_attachments_start_at_authored_garment_vertices() {
        for template in GarmentTemplate::ALL
            .into_iter()
            .filter(|template| template.uses_mannequin())
        {
            let asset = template.build(18).expect("mannequin garment template");
            let animator = MannequinAnimator::new();
            for &attachment in asset.mannequin_attachments() {
                let expected = asset.positions()[attachment.particle_index];
                let actual = animator.attachment_target(attachment);
                assert!((actual.x - expected.x).abs() < 1.0e-5);
                assert!((actual.y - expected.y).abs() < 1.0e-5);
                assert!((actual.z - expected.z).abs() < 1.0e-5);
            }
        }
    }

    #[test]
    fn generated_geometry_fingerprint_is_stable() {
        let first = GarmentTemplate::TShirt.build(16).expect("first template");
        let second = GarmentTemplate::TShirt.build(16).expect("second template");

        assert_eq!(
            first.simulation_fingerprint(),
            second.simulation_fingerprint()
        );
        assert_eq!(first.positions(), second.positions());
        assert_eq!(first.triangles(), second.triangles());
    }

    #[test]
    fn template_catalog_metadata_round_trips() {
        let mut keys = GarmentTemplate::ALL
            .into_iter()
            .map(GarmentTemplate::key)
            .collect::<Vec<_>>();
        let advertised_count = keys.len();
        keys.sort_unstable();
        keys.dedup();

        assert_eq!(keys.len(), advertised_count, "template keys must be unique");
        for template in GarmentTemplate::ALL {
            assert_eq!(GarmentTemplate::from_key(template.key()), Some(template));
            assert!(!template.display_name().is_empty());
            assert!(!template.summary().is_empty());
        }
    }

    #[test]
    fn unknown_template_keys_are_rejected() {
        assert_eq!(GarmentTemplate::from_key("hoodie"), None);
    }

    #[test]
    fn resolution_is_bounded() {
        assert_eq!(
            GarmentTemplate::TShirt.build(5),
            Err(GarmentTemplateError::ResolutionOutOfRange)
        );
        assert_eq!(
            GarmentTemplate::TShirt.build(41),
            Err(GarmentTemplateError::ResolutionOutOfRange)
        );
    }
}
