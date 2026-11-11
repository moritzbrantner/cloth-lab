use core::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClothError {
    DimensionsTooSmall,
    InvalidSpacing,
    InvalidParticleMass,
    InvalidCompliance,
    TopologyTooLarge,
    InvalidParticleIndex,
    InvalidTimeStep,
    InvalidGravity,
    InvalidSolverIterations,
    InvalidVelocityDamping,
    InvalidFrictionCoefficient,
    InvalidColliderCenter,
    InvalidColliderAxis,
    InvalidColliderRadius,
    InvalidCollisionThickness,
    DegenerateBendingStencil,
}

impl fmt::Display for ClothError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::DimensionsTooSmall => "cloth dimensions must be at least 2 by 2",
            Self::InvalidSpacing => "cloth spacing must be finite and positive",
            Self::InvalidParticleMass => "particle mass must be finite and positive",
            Self::InvalidCompliance => "constraint compliance must be finite and non-negative",
            Self::TopologyTooLarge => "cloth topology dimensions overflow addressable size",
            Self::InvalidParticleIndex => "particle index is outside the cloth",
            Self::InvalidTimeStep => "fixed timestep must be finite and positive",
            Self::InvalidGravity => "gravity must contain only finite components",
            Self::InvalidSolverIterations => "solver iteration count must be positive",
            Self::InvalidVelocityDamping => "velocity damping must be finite and between 0 and 1",
            Self::InvalidFrictionCoefficient => {
                "contact friction coefficient must be finite and between 0 and 1"
            }
            Self::InvalidColliderCenter => "collider center must contain only finite components",
            Self::InvalidColliderAxis => {
                "capsule endpoints must be finite and define a non-degenerate axis"
            }
            Self::InvalidColliderRadius => "collider radius must be finite and positive",
            Self::InvalidCollisionThickness => {
                "collision thickness must be finite, non-negative, and produce a finite shell"
            }
            Self::DegenerateBendingStencil => {
                "cloth bending stencil must be finite and contain two non-degenerate triangles"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ClothError {}
