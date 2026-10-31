use super::{
    CapsuleCollider, ClothCollider, ClothError, ContactConfig, FixedStepConfig,
    RectangularClothConfig, SphereCollider,
};

pub(super) fn validate_rectangular_config(
    config: RectangularClothConfig,
) -> Result<(), ClothError> {
    if config.columns < 2 || config.rows < 2 {
        return Err(ClothError::DimensionsTooSmall);
    }
    if !config.spacing.is_finite() || config.spacing <= 0.0 {
        return Err(ClothError::InvalidSpacing);
    }
    if !config.particle_mass.is_finite() || config.particle_mass <= 0.0 {
        return Err(ClothError::InvalidParticleMass);
    }
    if [
        config.stretch_compliance,
        config.shear_compliance,
        config.bending_compliance,
    ]
    .into_iter()
    .any(|compliance| !compliance.is_finite() || compliance < 0.0)
    {
        return Err(ClothError::InvalidCompliance);
    }
    Ok(())
}

pub(super) fn validate_step_config(config: FixedStepConfig) -> Result<(), ClothError> {
    if !config.delta_seconds.is_finite() || config.delta_seconds <= 0.0 {
        return Err(ClothError::InvalidTimeStep);
    }
    if !config.gravity.is_finite() {
        return Err(ClothError::InvalidGravity);
    }
    if config.solver_iterations == 0 {
        return Err(ClothError::InvalidSolverIterations);
    }
    if !config.velocity_damping.is_finite() || !(0.0..=1.0).contains(&config.velocity_damping) {
        return Err(ClothError::InvalidVelocityDamping);
    }
    Ok(())
}

pub(super) fn validate_contact_config(config: ContactConfig) -> Result<(), ClothError> {
    if !config.friction_coefficient.is_finite()
        || !(0.0..=1.0).contains(&config.friction_coefficient)
    {
        return Err(ClothError::InvalidFrictionCoefficient);
    }
    Ok(())
}

pub(super) fn validate_colliders(colliders: &[ClothCollider]) -> Result<(), ClothError> {
    for collider in colliders {
        match collider {
            ClothCollider::Sphere(sphere) => validate_sphere_collider(*sphere)?,
            ClothCollider::Capsule(capsule) => validate_capsule_collider(*capsule)?,
        }
    }
    Ok(())
}

fn validate_sphere_collider(collider: SphereCollider) -> Result<(), ClothError> {
    if !collider.center.is_finite() {
        return Err(ClothError::InvalidColliderCenter);
    }
    validate_radius_and_thickness(collider.radius, collider.thickness)
}

fn validate_capsule_collider(collider: CapsuleCollider) -> Result<(), ClothError> {
    let axis = collider.end - collider.start;
    if !collider.start.is_finite()
        || !collider.end.is_finite()
        || axis.length_squared() <= f64::EPSILON
    {
        return Err(ClothError::InvalidColliderAxis);
    }
    validate_radius_and_thickness(collider.radius, collider.thickness)
}

fn validate_radius_and_thickness(radius: f64, thickness: f64) -> Result<(), ClothError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(ClothError::InvalidColliderRadius);
    }
    if !thickness.is_finite() || thickness < 0.0 || !(radius + thickness).is_finite() {
        return Err(ClothError::InvalidCollisionThickness);
    }
    Ok(())
}
