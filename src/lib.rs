//! Deterministic cloth and textile simulation experiments.
//!
//! `cloth-lab` owns cloth-specific solver behavior while it is still experimental. Reusable
//! rigid-body and collision authority remains in `physics-engine`; rendering remains consumer-owned.

#![forbid(unsafe_code)]

mod cloth;
mod garment;
mod material;
mod math;
mod obstacle;
mod self_collision;
mod template;
#[cfg(target_arch = "wasm32")]
mod web;

pub use cloth::{
    BendingConstraint, CapsuleCollider, Cloth, ClothCollider, ClothError, ClothInteractionError,
    ContactConfig, DistanceConstraint, FixedStepConfig, Particle, ParticleDrag,
    RectangularClothConfig, SphereCollider, StepReport, TriangleMeshCloth, TriangleMeshClothConfig,
    TriangleMeshClothError, TriangleMeshStepError, TriangleMeshStepReport,
};
pub use garment::{
    GarmentAsset, GarmentImportError, GarmentImporter, GarmentSourceFormat, GlbGarmentImporter,
    ObjGarmentImporter,
};
pub use material::{TextileParameters, TextilePreset};
pub use math::Vec3;
pub use obstacle::{ClothObstacleConfig, ClothObstacleError, ClothObstacleKind};
pub use self_collision::{
    SelfCollisionConfig, SelfCollisionError, SelfCollisionParticle, SelfCollisionReport,
    solve_vertex_triangle_self_collision,
};
pub use template::{GarmentTemplate, GarmentTemplateAsset, GarmentTemplateError};
#[cfg(target_arch = "wasm32")]
pub use web::{BrowserClothSession, garment_template_catalog};
