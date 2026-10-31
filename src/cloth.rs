// Keep topology-specific adapters in the same module as the proven solver primitives so they can
// reuse one simulation authority without duplicating collision or constraint implementations.
mod collision;
mod error;
mod fingerprint;
mod geometry;
mod solver;
#[cfg(test)]
mod tests;
mod topology;
mod validation;

pub use error::ClothError;

include!("cloth/base.rs");
include!("cloth/particle_internal.rs");
include!("cloth/triangle_mesh.rs");
include!("cloth/interaction.rs");
