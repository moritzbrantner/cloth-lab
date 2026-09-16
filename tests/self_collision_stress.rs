use cloth_lab::{
    ContactConfig, FixedStepConfig, SelfCollisionConfig, TriangleMeshCloth,
    TriangleMeshClothConfig, Vec3,
};

const SELF_COLLISION: SelfCollisionConfig = SelfCollisionConfig { thickness: 0.08 };

fn solver_step() -> FixedStepConfig {
    FixedStepConfig {
        delta_seconds: 1.0 / 60.0,
        gravity: Vec3::ZERO,
        solver_iterations: 8,
        velocity_damping: 1.0,
    }
}

fn cloth_config() -> TriangleMeshClothConfig {
    TriangleMeshClothConfig {
        particle_mass: 1.0,
        stretch_compliance: 1.0e-7,
        bending_compliance: 1.0e-3,
    }
}

fn layered_triangle_fixture(offsets: &[f64], reverse_alternate_layers: bool) -> TriangleMeshCloth {
    let mut positions = Vec::with_capacity(offsets.len() * 3);
    let mut triangles = Vec::with_capacity(offsets.len());

    for (layer, &height) in offsets.iter().enumerate() {
        let base = positions.len();
        positions.extend([
            Vec3::new(-0.35, height, -0.2),
            Vec3::new(0.35, height, -0.2),
            Vec3::new(0.0, height, 0.4),
        ]);
        if reverse_alternate_layers && layer % 2 == 1 {
            triangles.push([base, base + 1, base + 2]);
        } else {
            triangles.push([base, base + 2, base + 1]);
        }
    }

    TriangleMeshCloth::new(&positions, &triangles, cloth_config())
        .expect("stress fixture topology must be valid")
}

fn assert_finite(cloth: &TriangleMeshCloth) {
    assert!(
        cloth
            .particles()
            .iter()
            .all(|particle| particle.position().is_finite()),
        "self-collision stress fixture produced a non-finite particle"
    );
}

#[test]
fn coplanar_inverted_layers_resolve_deterministically() {
    // Two disconnected faces start exactly coplanar with opposite winding. This drives the
    // zero-distance branch where self-collision must choose a deterministic separation normal.
    let mut first = layered_triangle_fixture(&[0.0, 0.0], true);
    let mut second = first.clone();
    let mut total_projections = 0;

    for _ in 0..6 {
        let first_report = first
            .step_with_contacts_and_self_collision(
                solver_step(),
                &[],
                ContactConfig::default(),
                SELF_COLLISION,
            )
            .expect("coplanar inversion stress step must succeed");
        let second_report = second
            .step_with_contacts_and_self_collision(
                solver_step(),
                &[],
                ContactConfig::default(),
                SELF_COLLISION,
            )
            .expect("replayed coplanar inversion stress step must succeed");

        assert_eq!(first_report, second_report);
        total_projections += first_report.self_collision.projections;
        assert_finite(&first);
        assert_finite(&second);
    }

    assert!(
        total_projections > 0,
        "coplanar inversion must exercise projection"
    );
    assert_eq!(first.particles(), second.particles());
    assert_eq!(first.state_fingerprint(), second.state_fingerprint());
}

#[test]
fn dense_contact_stack_stays_finite_and_replays_exactly() {
    // Every adjacent layer starts inside the configured thickness. Keeping the faces disconnected
    // avoids adjacency exclusions so this fixture stresses a dense set of legitimate contacts.
    let mut first = layered_triangle_fixture(&[0.0, 0.015, 0.03, 0.045, 0.06], false);
    let mut second = first.clone();
    let mut total_candidates = 0;
    let mut total_tests = 0;
    let mut total_projections = 0;

    for _ in 0..12 {
        let first_report = first
            .step_with_contacts_and_self_collision(
                solver_step(),
                &[],
                ContactConfig::default(),
                SELF_COLLISION,
            )
            .expect("dense-contact stress step must succeed");
        let second_report = second
            .step_with_contacts_and_self_collision(
                solver_step(),
                &[],
                ContactConfig::default(),
                SELF_COLLISION,
            )
            .expect("replayed dense-contact stress step must succeed");

        assert_eq!(first_report, second_report);
        total_candidates += first_report.self_collision.vertex_triangle_candidates;
        total_tests += first_report.self_collision.narrow_phase_tests;
        total_projections += first_report.self_collision.projections;
        assert_finite(&first);
        assert_finite(&second);
    }

    assert!(
        total_candidates > 0,
        "dense fixture must reach the broad/narrow seam"
    );
    assert!(
        total_tests > 0,
        "dense fixture must execute narrow-phase tests"
    );
    assert!(
        total_projections > 0,
        "dense fixture must exercise projection"
    );
    assert!(total_candidates >= total_tests);
    assert!(total_tests >= total_projections);
    assert_eq!(first.particles(), second.particles());
    assert_eq!(first.state_fingerprint(), second.state_fingerprint());
}
