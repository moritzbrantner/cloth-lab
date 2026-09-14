use crate::cloth::{ContactConfig, RectangularClothConfig};

/// Named qualitative cloth presets used for deterministic comparison fixtures.
///
/// These presets are intentionally not calibrated measurements of real fabrics. They provide stable,
/// human-readable starting points over the raw XPBD/contact parameters so callers can compare behavior
/// without making claims about real-world material accuracy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TextilePreset {
    CottonLike,
    DenimLike,
    SilkLike,
    LeatherLike,
}

impl TextilePreset {
    pub const ALL: [Self; 4] = [
        Self::CottonLike,
        Self::DenimLike,
        Self::SilkLike,
        Self::LeatherLike,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CottonLike => "cotton-like",
            Self::DenimLike => "denim-like",
            Self::SilkLike => "silk-like",
            Self::LeatherLike => "leather-like",
        }
    }

    #[must_use]
    pub const fn parameters(self) -> TextileParameters {
        match self {
            Self::CottonLike => TextileParameters {
                stretch_compliance: 1.5e-7,
                shear_compliance: 3.0e-7,
                bending_compliance: 1.5e-3,
                friction_coefficient: 0.45,
            },
            Self::DenimLike => TextileParameters {
                stretch_compliance: 6.0e-8,
                shear_compliance: 1.2e-7,
                bending_compliance: 4.0e-4,
                friction_coefficient: 0.60,
            },
            Self::SilkLike => TextileParameters {
                stretch_compliance: 5.0e-7,
                shear_compliance: 9.0e-7,
                bending_compliance: 7.5e-3,
                friction_coefficient: 0.20,
            },
            Self::LeatherLike => TextileParameters {
                stretch_compliance: 3.0e-8,
                shear_compliance: 5.0e-8,
                bending_compliance: 1.0e-4,
                friction_coefficient: 0.75,
            },
        }
    }
}

/// Raw solver/contact parameters behind a named qualitative textile preset.
///
/// Topology, spacing, and particle mass stay explicit at the call site. This keeps material naming from
/// silently deciding discretization or mass while still providing one stable mapping to cloth-specific
/// compliance and contact parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextileParameters {
    pub stretch_compliance: f64,
    pub shear_compliance: f64,
    pub bending_compliance: f64,
    pub friction_coefficient: f64,
}

impl TextileParameters {
    #[must_use]
    pub const fn rectangular_config(
        self,
        columns: usize,
        rows: usize,
        spacing: f64,
        particle_mass: f64,
    ) -> RectangularClothConfig {
        RectangularClothConfig {
            columns,
            rows,
            spacing,
            particle_mass,
            stretch_compliance: self.stretch_compliance,
            shear_compliance: self.shear_compliance,
            bending_compliance: self.bending_compliance,
        }
    }

    #[must_use]
    pub const fn contact_config(self) -> ContactConfig {
        ContactConfig {
            friction_coefficient: self.friction_coefficient,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CapsuleCollider, Cloth, ClothCollider, FixedStepConfig, Vec3};

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct FixtureEvidence {
        fingerprint: u64,
        bottom_middle_y: f64,
        max_stretch_error: f64,
        max_shear_error: f64,
        max_bending_error: f64,
        collision_projections: usize,
        friction_corrections: usize,
    }

    fn run_qualitative_fixture(preset: TextilePreset) -> FixtureEvidence {
        let parameters = preset.parameters();
        let mut cloth = Cloth::rectangular(parameters.rectangular_config(6, 6, 0.2, 1.0))
            .expect("preset fixture must build");
        cloth
            .pin_top_corners()
            .expect("preset fixture corner pins must be valid");

        let colliders = [ClothCollider::Capsule(CapsuleCollider {
            start: Vec3::new(0.2, -0.5, 0.5),
            end: Vec3::new(0.8, -0.5, 0.5),
            radius: 0.3,
            thickness: 0.02,
        })];
        let step = FixedStepConfig::default();
        let contact = parameters.contact_config();
        let mut report = cloth
            .step_with_contacts(step, &colliders, contact)
            .expect("first preset fixture step must succeed");
        let mut collision_projections = report.collision_projections;
        let mut friction_corrections = report.friction_corrections;

        for _ in 1..240 {
            report = cloth
                .step_with_contacts(step, &colliders, contact)
                .expect("preset fixture step must succeed");
            collision_projections += report.collision_projections;
            friction_corrections += report.friction_corrections;
        }

        let bottom_middle = (cloth.rows() - 1) * cloth.columns() + cloth.columns() / 2;
        FixtureEvidence {
            fingerprint: cloth.state_fingerprint(),
            bottom_middle_y: cloth.particles()[bottom_middle].position().y,
            max_stretch_error: report.max_stretch_error,
            max_shear_error: report.max_shear_error,
            max_bending_error: report.max_bending_error,
            collision_projections,
            friction_corrections,
        }
    }

    #[test]
    fn preset_names_and_parameter_ordering_are_stable() {
        assert_eq!(
            TextilePreset::ALL.map(TextilePreset::name),
            ["cotton-like", "denim-like", "silk-like", "leather-like"]
        );

        let cotton = TextilePreset::CottonLike.parameters();
        let denim = TextilePreset::DenimLike.parameters();
        let silk = TextilePreset::SilkLike.parameters();
        let leather = TextilePreset::LeatherLike.parameters();

        assert!(silk.stretch_compliance > cotton.stretch_compliance);
        assert!(cotton.stretch_compliance > denim.stretch_compliance);
        assert!(denim.stretch_compliance > leather.stretch_compliance);
        assert!(silk.shear_compliance > cotton.shear_compliance);
        assert!(cotton.shear_compliance > denim.shear_compliance);
        assert!(denim.shear_compliance > leather.shear_compliance);
        assert!(silk.bending_compliance > cotton.bending_compliance);
        assert!(cotton.bending_compliance > denim.bending_compliance);
        assert!(denim.bending_compliance > leather.bending_compliance);
        assert!(silk.friction_coefficient < cotton.friction_coefficient);
        assert!(cotton.friction_coefficient < denim.friction_coefficient);
        assert!(denim.friction_coefficient < leather.friction_coefficient);
    }

    #[test]
    fn preset_mapping_keeps_topology_spacing_and_mass_explicit() {
        let parameters = TextilePreset::DenimLike.parameters();
        let config = parameters.rectangular_config(5, 7, 0.3, 2.5);

        assert_eq!(config.columns, 5);
        assert_eq!(config.rows, 7);
        assert_eq!(config.spacing, 0.3);
        assert_eq!(config.particle_mass, 2.5);
        assert_eq!(config.stretch_compliance, parameters.stretch_compliance);
        assert_eq!(config.shear_compliance, parameters.shear_compliance);
        assert_eq!(config.bending_compliance, parameters.bending_compliance);
        assert_eq!(
            parameters.contact_config().friction_coefficient,
            parameters.friction_coefficient
        );
    }

    #[test]
    fn qualitative_material_fixtures_are_deterministic_and_distinct() {
        let first = TextilePreset::ALL.map(run_qualitative_fixture);
        let replay = TextilePreset::ALL.map(run_qualitative_fixture);
        assert_eq!(first, replay);

        for evidence in first {
            assert!(evidence.bottom_middle_y.is_finite());
            assert!(evidence.max_stretch_error.is_finite());
            assert!(evidence.max_shear_error.is_finite());
            assert!(evidence.max_bending_error.is_finite());
            assert!(evidence.collision_projections > 0);
        }

        for left in 0..first.len() {
            for right in (left + 1)..first.len() {
                assert_ne!(first[left].fingerprint, first[right].fingerprint);
            }
        }

        assert!(
            first
                .iter()
                .any(|evidence| evidence.friction_corrections > 0)
        );
    }
}
