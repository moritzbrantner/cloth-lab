//! Deterministic cloth and textile simulation experiments.
//!
//! `cloth-lab` owns cloth-specific solver behavior while it is still experimental. Reusable
//! rigid-body and collision authority remains in `physics-engine`; rendering remains consumer-owned.

#![forbid(unsafe_code)]

mod cloth;
mod math;

pub use cloth::{
    BendingConstraint, CapsuleCollider, Cloth, ClothCollider, ClothError, ContactConfig,
    DistanceConstraint, FixedStepConfig, Particle, RectangularClothConfig, SphereCollider,
    StepReport,
};
pub use math::Vec3;
