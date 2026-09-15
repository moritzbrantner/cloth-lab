//! Deterministic cloth and textile simulation experiments.
//!
//! `cloth-lab` owns cloth-specific solver behavior while it is still experimental. Reusable
//! rigid-body and collision authority remains in `physics-engine`; rendering remains consumer-owned.

#![forbid(unsafe_code)]

mod cloth;
mod garment;
mod material;
mod math;
mod self_collision;

pub use cloth::{
    BendingConstraint, CapsuleCollider, Cloth, ClothCollider, ClothError, ClothInteractionError,
    ContactConfig, DistanceConstraint, FixedStepConfig, Particle, ParticleDrag,
    RectangularClothConfig, SphereCollider, StepReport, TriangleMeshCloth, TriangleMeshClothConfig,
    TriangleMeshClothError,
};
pub use garment::{
    GarmentAsset, GarmentImportError, GarmentImporter, GarmentSourceFormat, ObjGarmentImporter,
};
pub use material::{TextileParameters, TextilePreset};
pub use math::Vec3;
pub use self_collision::{
    SelfCollisionConfig, SelfCollisionError, SelfCollisionParticle, SelfCollisionReport,
    solve_vertex_triangle_self_collision,
};
