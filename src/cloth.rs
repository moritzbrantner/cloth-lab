// Keep topology-specific adapters in the same module as the proven solver primitives so they can
// reuse one simulation authority without duplicating collision or constraint implementations.
include!("cloth/base.rs");
include!("cloth/triangle_mesh.rs");
