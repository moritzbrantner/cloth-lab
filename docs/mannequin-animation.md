# Animated mannequin integration

Cloth Lab reuses renderer-independent animation semantics from
`moritzbrantner/3d-lab` at the revision pinned in `Cargo.toml`.

## Ownership

- `three-d-animation` owns transform composition, quaternion interpolation,
  keyframe/clip sampling, loop semantics, and parent-before-child hierarchy evaluation.
- Cloth Lab owns cloth simulation, garment fixtures, cloth/body collision adaptation,
  and the mapping from garment attachment particles to semantic humanoid joints.
- The Pages browser renders collider descriptors and forwards animation controls. It
  does not evaluate animation poses or author collision results.

The current mannequin is a Cloth Lab collision approximation of a humanoid rig rather
than a duplicate renderer model. Its hierarchy follows the reusable humanoid semantics
already exercised in 3d-lab, while capsule/sphere dimensions remain cloth-contact policy.

## Fixed-step contract

Mannequin animation advances exactly once before each cloth step using the same explicit
`FixedStepConfig::delta_seconds`. The resulting joint pose is converted into:

1. a stable ordered set of cloth colliders; and
2. attachment targets for garment particles still bound to mannequin joints.

The fixed collider count and ordering do not change while a session runs. User-added
obstacles remain after the fixed mannequin prefix.

Moving or removing a mannequin-controlled pin explicitly detaches that particle from
the mannequin. This prevents animation from silently overriding direct editing.

## Initial motion set

- **Rest** — bind/rest pose with no clip sampling.
- **Walk** — repeating opposing arm/leg swing plus a small hips translation.
- **Wave** — repeating upper/lower arm motion with a small chest rotation.

These are deterministic interaction fixtures, not claims of production-quality
locomotion or biomechanical accuracy.

## Evidence

Unit tests cover animation fail-closed validation, deterministic collider replay,
attachment motion, and reset behavior. Garment integration tests run animated T-shirt
fixtures through the real cloth contact solver and require both actual contact
projections and identical replay fingerprints.
