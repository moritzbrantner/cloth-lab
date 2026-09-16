use core::fmt;

use crate::{CapsuleCollider, ClothCollider, SphereCollider, Vec3};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClothObstacleKind {
    None,
    Sphere,
    Capsule,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClothObstacleConfig {
    pub kind: ClothObstacleKind,
    pub center: Vec3,
    pub radius: f64,
    pub thickness: f64,
    pub capsule_half_length: f64,
}

impl ClothObstacleConfig {
    #[must_use]
    pub const fn none() -> Self {
        Self {
            kind: ClothObstacleKind::None,
            center: Vec3::ZERO,
            radius: 0.28,
            thickness: 0.025,
            capsule_half_length: 0.52,
        }
    }

    pub fn colliders(self) -> Result<Vec<ClothCollider>, ClothObstacleError> {
        if self.kind == ClothObstacleKind::None {
            return Ok(Vec::new());
        }
        if !self.center.is_finite() {
            return Err(ClothObstacleError::InvalidCenter);
        }
        if !self.radius.is_finite() || self.radius <= 0.0 {
            return Err(ClothObstacleError::InvalidRadius);
        }
        if !self.thickness.is_finite()
            || self.thickness < 0.0
            || !(self.radius + self.thickness).is_finite()
        {
            return Err(ClothObstacleError::InvalidThickness);
        }

        match self.kind {
            ClothObstacleKind::None => unreachable!("none is returned before validation"),
            ClothObstacleKind::Sphere => Ok(vec![ClothCollider::Sphere(SphereCollider {
                center: self.center,
                radius: self.radius,
                thickness: self.thickness,
            })]),
            ClothObstacleKind::Capsule => {
                if !self.capsule_half_length.is_finite() || self.capsule_half_length <= 0.0 {
                    return Err(ClothObstacleError::InvalidCapsuleHalfLength);
                }
                let offset = Vec3::new(self.capsule_half_length, 0.0, 0.0);
                Ok(vec![ClothCollider::Capsule(CapsuleCollider {
                    start: self.center - offset,
                    end: self.center + offset,
                    radius: self.radius,
                    thickness: self.thickness,
                })])
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClothObstacleError {
    InvalidCenter,
    InvalidRadius,
    InvalidThickness,
    InvalidCapsuleHalfLength,
}

impl fmt::Display for ClothObstacleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidCenter => "cloth obstacle center must contain only finite components",
            Self::InvalidRadius => "cloth obstacle radius must be finite and positive",
            Self::InvalidThickness => {
                "cloth obstacle thickness must be finite, non-negative, and produce a finite shell"
            }
            Self::InvalidCapsuleHalfLength => {
                "cloth capsule half-length must be finite and positive"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ClothObstacleError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_produces_no_collision_authority() {
        assert!(ClothObstacleConfig::none().colliders().unwrap().is_empty());
    }

    #[test]
    fn sphere_maps_exactly_to_cloth_collider() {
        let config = ClothObstacleConfig {
            kind: ClothObstacleKind::Sphere,
            center: Vec3::new(0.2, -0.4, 0.8),
            radius: 0.3,
            thickness: 0.02,
            capsule_half_length: 9.0,
        };

        assert_eq!(
            config.colliders().unwrap(),
            vec![ClothCollider::Sphere(SphereCollider {
                center: config.center,
                radius: config.radius,
                thickness: config.thickness,
            })]
        );
    }

    #[test]
    fn capsule_is_centered_on_the_explicit_x_axis_span() {
        let config = ClothObstacleConfig {
            kind: ClothObstacleKind::Capsule,
            center: Vec3::new(1.0, -0.5, 0.75),
            radius: 0.25,
            thickness: 0.03,
            capsule_half_length: 0.4,
        };

        assert_eq!(
            config.colliders().unwrap(),
            vec![ClothCollider::Capsule(CapsuleCollider {
                start: Vec3::new(0.6, -0.5, 0.75),
                end: Vec3::new(1.4, -0.5, 0.75),
                radius: 0.25,
                thickness: 0.03,
            })]
        );
    }

    #[test]
    fn invalid_active_obstacles_fail_closed() {
        let base = ClothObstacleConfig {
            kind: ClothObstacleKind::Capsule,
            center: Vec3::ZERO,
            radius: 0.25,
            thickness: 0.02,
            capsule_half_length: 0.4,
        };

        assert_eq!(
            ClothObstacleConfig {
                center: Vec3::new(f64::NAN, 0.0, 0.0),
                ..base
            }
            .colliders()
            .unwrap_err(),
            ClothObstacleError::InvalidCenter
        );
        assert_eq!(
            ClothObstacleConfig {
                radius: 0.0,
                ..base
            }
            .colliders()
            .unwrap_err(),
            ClothObstacleError::InvalidRadius
        );
        assert_eq!(
            ClothObstacleConfig {
                thickness: -0.01,
                ..base
            }
            .colliders()
            .unwrap_err(),
            ClothObstacleError::InvalidThickness
        );
        assert_eq!(
            ClothObstacleConfig {
                capsule_half_length: 0.0,
                ..base
            }
            .colliders()
            .unwrap_err(),
            ClothObstacleError::InvalidCapsuleHalfLength
        );
    }

    #[test]
    fn identical_configuration_maps_deterministically() {
        let config = ClothObstacleConfig {
            kind: ClothObstacleKind::Capsule,
            center: Vec3::new(1.04, -0.58, 0.88),
            radius: 0.28,
            thickness: 0.025,
            capsule_half_length: 0.52,
        };

        assert_eq!(config.colliders().unwrap(), config.colliders().unwrap());
    }
}
