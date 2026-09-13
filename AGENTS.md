# Repository guidance

## Authority

- Rust simulation code in this repository is authoritative for cloth-specific behavior while it remains experimental.
- Do not move cloth-specific policy into `physics-engine` until the behavior is general-purpose and its deterministic contract is proven.
- Reuse compatible rigid-body, collision, spatial-query, and math concepts from `physics-engine` instead of creating a competing general physics authority.
- Rendering, UI, cameras, and game loops consume cloth state; they do not define simulation results.

## Determinism

- Advance simulation only through explicit fixed timesteps.
- Preserve canonical particle, topology, constraint, and contact ordering.
- Invalid simulation configuration must fail closed rather than silently substituting values.
- Any parallel solver work must first establish deterministic batching/coloring and replay evidence.
- Keep committed Cargo state frozen in validation.

## Evidence

- Every solver change needs focused deterministic tests.
- Performance changes require measurements without weakening correctness or replay guarantees.
- Do not claim real-world textile accuracy without corresponding material data and validation.
- Keep self-collision, tearing, sewing, GPU solving, and advanced material models out of earlier slices unless their prerequisites are complete.
