use super::collision::{
    apply_contact_friction, closest_point_on_segment, solve_capsule_collision,
    solve_sphere_collision,
};
use super::geometry::dot;
use super::*;

fn cloth_fixture() -> Cloth {
    let mut cloth = Cloth::rectangular(RectangularClothConfig {
        columns: 6,
        rows: 6,
        spacing: 0.2,
        particle_mass: 1.0,
        stretch_compliance: 1.0e-7,
        shear_compliance: 2.5e-7,
        bending_compliance: 1.0e-3,
    })
    .expect("fixture cloth must be valid");
    cloth
        .pin_top_corners()
        .expect("fixture corner indices must be valid");
    cloth
}

fn shear_fixture(shear_compliance: f64) -> Cloth {
    Cloth::rectangular(RectangularClothConfig {
        columns: 3,
        rows: 3,
        spacing: 0.2,
        particle_mass: 1.0,
        stretch_compliance: 1.0e3,
        shear_compliance,
        bending_compliance: 1.0e3,
    })
    .expect("shear fixture must be valid")
}

fn bending_fixture(bending_compliance: f64) -> Cloth {
    Cloth::rectangular(RectangularClothConfig {
        columns: 2,
        rows: 2,
        spacing: 0.2,
        particle_mass: 1.0,
        stretch_compliance: 1.0e3,
        shear_compliance: 1.0e3,
        bending_compliance,
    })
    .expect("bending fixture must be valid")
}

fn sphere_fixture() -> ClothCollider {
    ClothCollider::Sphere(SphereCollider {
        center: Vec3::new(0.5, -0.5, 0.5),
        radius: 0.35,
        thickness: 0.02,
    })
}

fn capsule_fixture() -> ClothCollider {
    ClothCollider::Capsule(CapsuleCollider {
        start: Vec3::new(0.2, -0.5, 0.5),
        end: Vec3::new(0.8, -0.5, 0.5),
        radius: 0.3,
        thickness: 0.02,
    })
}

fn tangential_displacement(particle: Particle, normal: Vec3) -> f64 {
    let displacement = particle.position - particle.previous_position;
    (displacement - normal * dot(displacement, normal)).length()
}

#[test]
fn rectangular_topology_has_stable_ordering() {
    let cloth = cloth_fixture();
    assert_eq!(cloth.particles().len(), 36);
    assert_eq!(cloth.triangles().len(), 50);
    assert_eq!(cloth.stretch_constraints().len(), 60);
    assert_eq!(cloth.shear_constraints().len(), 50);
    assert_eq!(cloth.bending_constraints().len(), 65);
    assert_eq!(cloth.triangles()[0], [0, 6, 1]);
    assert_eq!(cloth.triangles()[49], [29, 34, 35]);
    assert_eq!(cloth.shear_constraints()[0].particle_a, 0);
    assert_eq!(cloth.shear_constraints()[0].particle_b, 7);
    assert_eq!(cloth.shear_constraints()[49].particle_a, 29);
    assert_eq!(cloth.shear_constraints()[49].particle_b, 34);
    let first_bend = cloth.bending_constraints()[0];
    assert_eq!((first_bend.opposite_a, first_bend.opposite_b), (0, 7));
    assert_eq!((first_bend.edge_a, first_bend.edge_b), (6, 1));
    let last_bend = cloth.bending_constraints()[64];
    assert_eq!((last_bend.opposite_a, last_bend.opposite_b), (28, 35));
    assert_eq!((last_bend.edge_a, last_bend.edge_b), (34, 29));
}

#[test]
fn step_replay_matches_recorded_fingerprints() {
    let mut free = cloth_fixture();
    for _ in 0..120 {
        free.step(FixedStepConfig::default()).unwrap();
    }
    assert_eq!(free.state_fingerprint(), 0xf89b_262e_95e3_b438);

    let contact = ContactConfig {
        friction_coefficient: 0.65,
    };
    let mut contacting = cloth_fixture();
    for _ in 0..120 {
        contacting
            .step_with_contacts(FixedStepConfig::default(), &[capsule_fixture()], contact)
            .unwrap();
    }
    assert_eq!(contacting.state_fingerprint(), 0x7ee2_163a_f44c_4fe2);
}

#[test]
fn pinned_corners_remain_exactly_fixed() {
    let mut cloth = cloth_fixture();
    let left = cloth.particles()[0].position();
    let right = cloth.particles()[cloth.columns() - 1].position();
    for _ in 0..240 {
        cloth.step(FixedStepConfig::default()).unwrap();
    }
    assert_eq!(cloth.particles()[0].position(), left);
    assert_eq!(cloth.particles()[cloth.columns() - 1].position(), right);
}

#[test]
fn repeated_runs_produce_identical_state() {
    let mut first = cloth_fixture();
    let mut second = cloth_fixture();
    for _ in 0..240 {
        assert_eq!(
            first.step(FixedStepConfig::default()).unwrap(),
            second.step(FixedStepConfig::default()).unwrap()
        );
    }
    assert_eq!(first.particles(), second.particles());
    assert_eq!(first.state_fingerprint(), second.state_fingerprint());
}

#[test]
fn hanging_sheet_falls_and_keeps_constraint_errors_bounded() {
    let mut cloth = cloth_fixture();
    let bottom_middle = (cloth.rows() - 1) * cloth.columns() + cloth.columns() / 2;
    let initial_height = cloth.particles()[bottom_middle].position().y;
    for _ in 0..360 {
        cloth.step(FixedStepConfig::default()).unwrap();
    }
    assert!(cloth.particles()[bottom_middle].position().y < initial_height - 0.1);
    assert!(cloth.max_stretch_error() < 0.02);
    assert!(cloth.max_shear_error() < 0.03);
    assert!(cloth.max_bending_error().is_finite());
}

#[test]
fn shear_constraints_reduce_diagonal_distortion() {
    let mut resistant = shear_fixture(1.0e-8);
    let mut compliant = shear_fixture(1.0e3);
    for cloth in [&mut resistant, &mut compliant] {
        cloth.particles[4].position += Vec3::new(0.12, 0.0, 0.05);
        cloth.particles[4].previous_position = cloth.particles[4].position;
    }
    let step = FixedStepConfig {
        gravity: Vec3::ZERO,
        solver_iterations: 20,
        velocity_damping: 0.0,
        ..FixedStepConfig::default()
    };
    resistant.step(step).unwrap();
    compliant.step(step).unwrap();
    assert!(compliant.max_shear_error() > 0.01);
    assert!(resistant.max_shear_error() < compliant.max_shear_error() * 0.25);
}

#[test]
fn bending_constraints_reduce_fold_energy() {
    let mut resistant = bending_fixture(1.0e-8);
    let mut compliant = bending_fixture(1.0e3);
    for cloth in [&mut resistant, &mut compliant] {
        cloth.particles[3].position += Vec3::new(0.0, 0.15, 0.0);
        cloth.particles[3].previous_position = cloth.particles[3].position;
    }
    let step = FixedStepConfig {
        gravity: Vec3::ZERO,
        solver_iterations: 20,
        velocity_damping: 0.0,
        ..FixedStepConfig::default()
    };
    resistant.step(step).unwrap();
    compliant.step(step).unwrap();
    assert!(compliant.max_bending_error() > 0.5);
    assert!(resistant.max_bending_error() < compliant.max_bending_error() * 0.01);
}

#[test]
fn sphere_collision_preserves_shell_and_replay() {
    let collider = sphere_fixture();
    let mut first = cloth_fixture();
    let mut second = cloth_fixture();
    let mut observed_collision = false;
    for _ in 0..240 {
        let first_report = first
            .step_with_colliders(FixedStepConfig::default(), &[collider])
            .unwrap();
        let second_report = second
            .step_with_colliders(FixedStepConfig::default(), &[collider])
            .unwrap();
        assert_eq!(first_report, second_report);
        observed_collision |= first_report.collision_projections > 0;
    }
    assert!(observed_collision);
    assert_eq!(first.particles(), second.particles());
    let ClothCollider::Sphere(sphere) = collider else {
        unreachable!()
    };
    let minimum_distance = sphere.effective_radius() - 1.0e-9;
    for particle in first
        .particles()
        .iter()
        .filter(|particle| !particle.is_pinned())
    {
        assert!((particle.position() - sphere.center).length() >= minimum_distance);
    }
}

#[test]
fn capsule_collision_preserves_shell_and_replay() {
    let collider = capsule_fixture();
    let mut first = cloth_fixture();
    let mut second = cloth_fixture();
    let mut observed_collision = false;
    for _ in 0..240 {
        let first_report = first
            .step_with_colliders(FixedStepConfig::default(), &[collider])
            .unwrap();
        let second_report = second
            .step_with_colliders(FixedStepConfig::default(), &[collider])
            .unwrap();
        assert_eq!(first_report, second_report);
        observed_collision |= first_report.collision_projections > 0;
    }
    assert!(observed_collision);
    assert_eq!(first.particles(), second.particles());
    let ClothCollider::Capsule(capsule) = collider else {
        unreachable!()
    };
    let minimum_distance = capsule.effective_radius() - 1.0e-9;
    for particle in first
        .particles()
        .iter()
        .filter(|particle| !particle.is_pinned())
    {
        let axis_point = closest_point_on_segment(particle.position(), capsule.start, capsule.end);
        assert!((particle.position() - axis_point).length() >= minimum_distance);
    }
}

#[test]
fn swept_sphere_collision_blocks_tunneling() {
    let collider = SphereCollider {
        center: Vec3::ZERO,
        radius: 0.25,
        thickness: 0.05,
    };
    let mut particle = Particle {
        position: Vec3::new(1.0, 0.0, 0.0),
        previous_position: Vec3::new(-1.0, 0.0, 0.0),
        inverse_mass: 1.0,
    };
    assert!(solve_sphere_collision(&mut particle, collider).is_some());
    assert!((particle.position.x + collider.effective_radius()).abs() < 1.0e-12);
    assert!(particle.position.y.abs() < 1.0e-12);
    assert!(particle.position.z.abs() < 1.0e-12);
}

#[test]
fn swept_capsule_collision_blocks_tunneling() {
    let collider = CapsuleCollider {
        start: Vec3::new(0.0, -0.5, 0.0),
        end: Vec3::new(0.0, 0.5, 0.0),
        radius: 0.25,
        thickness: 0.05,
    };
    let mut particle = Particle {
        position: Vec3::new(1.0, 0.0, 0.0),
        previous_position: Vec3::new(-1.0, 0.0, 0.0),
        inverse_mass: 1.0,
    };
    assert!(solve_capsule_collision(&mut particle, collider).is_some());
    assert!((particle.position.x + collider.effective_radius()).abs() < 1.0e-10);
    assert!(particle.position.y.abs() < 1.0e-10);
    assert!(particle.position.z.abs() < 1.0e-10);
}

#[test]
fn coulomb_bounded_friction_reduces_tangential_slip_without_moving_contact() {
    let collider = SphereCollider {
        center: Vec3::ZERO,
        radius: 1.0,
        thickness: 0.0,
    };
    let seed = Particle {
        position: Vec3::new(0.8, 0.0, 0.2),
        previous_position: Vec3::new(0.9, 0.0, -0.2),
        inverse_mass: 1.0,
    };
    let mut frictionless = seed;
    let mut frictional = seed;
    let frictionless_projection = solve_sphere_collision(&mut frictionless, collider).unwrap();
    let frictional_projection = solve_sphere_collision(&mut frictional, collider).unwrap();
    assert_eq!(frictionless.position, frictional.position);
    let before = tangential_displacement(frictional, frictional_projection.normal);
    assert!(!apply_contact_friction(
        &mut frictionless,
        frictionless_projection,
        0.0
    ));
    assert!(apply_contact_friction(
        &mut frictional,
        frictional_projection,
        1.0
    ));
    let after = tangential_displacement(frictional, frictional_projection.normal);
    assert!(after < before);
    assert_eq!(frictionless.position, frictional.position);
    assert!(before - after <= frictional_projection.normal_correction + 1.0e-12);
}

#[test]
fn frictional_contact_replay_is_deterministic() {
    let collider = capsule_fixture();
    let contact = ContactConfig {
        friction_coefficient: 0.65,
    };
    let mut first = cloth_fixture();
    let mut second = cloth_fixture();
    for cloth in [&mut first, &mut second] {
        for particle in cloth
            .particles
            .iter_mut()
            .filter(|particle| !particle.is_pinned())
        {
            particle.previous_position.z -= 0.02;
        }
    }
    let mut observed_friction = false;
    for _ in 0..240 {
        let first_report = first
            .step_with_contacts(FixedStepConfig::default(), &[collider], contact)
            .unwrap();
        let second_report = second
            .step_with_contacts(FixedStepConfig::default(), &[collider], contact)
            .unwrap();
        assert_eq!(first_report, second_report);
        observed_friction |= first_report.friction_corrections > 0;
    }
    assert!(observed_friction);
    assert_eq!(first.particles(), second.particles());
}

#[test]
fn invalid_contact_friction_fails_closed_before_advancing() {
    let mut cloth = cloth_fixture();
    let fingerprint = cloth.state_fingerprint();
    let error = cloth
        .step_with_contacts(
            FixedStepConfig::default(),
            &[capsule_fixture()],
            ContactConfig {
                friction_coefficient: 1.01,
            },
        )
        .expect_err("out-of-range friction must be rejected");
    assert_eq!(error, ClothError::InvalidFrictionCoefficient);
    assert_eq!(cloth.state_fingerprint(), fingerprint);
}

#[test]
fn invalid_collider_fails_closed_before_advancing() {
    let mut cloth = cloth_fixture();
    let fingerprint = cloth.state_fingerprint();
    let invalid = ClothCollider::Sphere(SphereCollider {
        center: Vec3::ZERO,
        radius: 0.5,
        thickness: -0.1,
    });
    let error = cloth
        .step_with_colliders(FixedStepConfig::default(), &[invalid])
        .expect_err("negative collision thickness must be rejected");
    assert_eq!(error, ClothError::InvalidCollisionThickness);
    assert_eq!(cloth.state_fingerprint(), fingerprint);
}

#[test]
fn invalid_capsule_axis_fails_closed_before_advancing() {
    let mut cloth = cloth_fixture();
    let fingerprint = cloth.state_fingerprint();
    let invalid = ClothCollider::Capsule(CapsuleCollider {
        start: Vec3::ZERO,
        end: Vec3::ZERO,
        radius: 0.5,
        thickness: 0.0,
    });
    let error = cloth
        .step_with_colliders(FixedStepConfig::default(), &[invalid])
        .expect_err("degenerate capsule axis must be rejected");
    assert_eq!(error, ClothError::InvalidColliderAxis);
    assert_eq!(cloth.state_fingerprint(), fingerprint);
}

#[test]
fn invalid_shear_compliance_is_rejected() {
    let error = Cloth::rectangular(RectangularClothConfig {
        columns: 2,
        rows: 2,
        spacing: 0.2,
        particle_mass: 1.0,
        stretch_compliance: 0.0,
        shear_compliance: -1.0,
        bending_compliance: 0.0,
    })
    .expect_err("negative shear compliance must be rejected");
    assert_eq!(error, ClothError::InvalidCompliance);
}

#[test]
fn invalid_bending_compliance_is_rejected() {
    let error = Cloth::rectangular(RectangularClothConfig {
        columns: 2,
        rows: 2,
        spacing: 0.2,
        particle_mass: 1.0,
        stretch_compliance: 0.0,
        shear_compliance: 0.0,
        bending_compliance: -1.0,
    })
    .expect_err("negative bending compliance must be rejected");
    assert_eq!(error, ClothError::InvalidCompliance);
}

#[test]
fn invalid_step_configuration_fails_closed() {
    let mut cloth = cloth_fixture();
    let error = cloth
        .step(FixedStepConfig {
            delta_seconds: 0.0,
            ..FixedStepConfig::default()
        })
        .expect_err("zero timestep must be rejected");
    assert_eq!(error, ClothError::InvalidTimeStep);
}
