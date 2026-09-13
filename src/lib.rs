//! Deterministic cloth and textile simulation experiments.
//!
//! `cloth-lab` owns cloth-specific solver behavior while it is still experimental. Reusable
//! rigid-body and collision authority remains in `physics-engine`; rendering remains consumer-owned.

#![forbid(unsafe_code)]

mod cloth;
mod math;

pub use cloth::{
    Cloth, ClothError, DistanceConstraint, FixedStepConfig, Particle, RectangularClothConfig,
    StepReport,
};
pub use math::Vec3;
