# Roadmap

`cloth-lab` develops cloth and textile simulation independently, with stable reusable pieces promoted to `physics-engine` only after their boundaries are proven.

## 1. Deterministic hanging cloth

- Build a rectangular triangle-grid cloth from deterministic topology.
- Integrate particles with a fixed timestep and gravity.
- Pin selected particles without special-casing the solver loop.
- Solve XPBD stretch constraints with explicit compliance.
- Prove replay stability with deterministic state fingerprints/tests.
- Keep rendering out of the simulation authority boundary.

**Acceptance:** a pinned rectangular sheet settles under gravity, preserves its rest-edge lengths within a bounded tolerance, and repeated runs from identical initial state produce identical particle state.

## 2. Rigid collider interaction

- Add sphere and capsule cloth collision.
- Reuse compatible collision/math concepts from `physics-engine` rather than duplicating general rigid-body authority.
- Add collision thickness and deterministic contact ordering.
- Cover tunneling-prone fixtures before expanding collider support.

**Status:** complete for the initial sphere/capsule scope. Both collider types share the deterministic cloth contact path, validate before state mutation, include explicit collision thickness, and have replay plus tunneling fixtures. Broader rigid-body collision authority remains outside this repository.

## 3. Textile behavior

- Add shear resistance.
- Add bending constraints.
- Add cloth/rigid friction.
- Separate solver parameters from named material presets.
- Establish measurable cotton-, denim-, silk-, and leather-like qualitative fixtures without claiming real-world material accuracy prematurely.

**Progress:** shear resistance, mesh-general isometric bending, and deterministic cloth/rigid contact friction are implemented independently. Contact friction uses an explicit raw coefficient, preserves the zero-friction collision path, and bounds tangential velocity correction by the normal contact projection in the same Coulomb-style spirit as `physics-engine`. Named material presets and their qualitative fixtures remain.

## 4. Self-collision

- Add cloth thickness and vertex/triangle self-collision.
- Reuse/prove spatial acceleration structures from the existing physics/collision work where appropriate.
- Exclude adjacent mesh features deterministically.
- Stress folding, inversion, and dense-contact cases.

## 5. Garments

- Support attachment constraints to animated bodies.
- Add cape/skirt-style garment fixtures.
- Validate fast-moving attachment points and character collision.
- Keep animation authority outside the cloth solver.

## 6. Rendering and interaction

- Add a wgpu viewer over simulation snapshots.
- Update render vertices from authoritative simulation positions.
- Add camera and inspection controls without coupling them to the solver.
- Add debug views for constraints, collisions, normals, and solver error.

## 7. Performance and scale

- Profile integration, constraint projection, collision detection, and self-collision independently.
- Introduce deterministic graph coloring/batching before parallel constraint solving.
- Evaluate SIMD and GPU solving only against exact or explicitly bounded correctness evidence.
- Add cloth LOD experiments only after a stable high-quality reference solver exists.

## 8. Advanced textiles

Only after the core solver is reliable:

- Sewing and seams.
- Cutting and tearing.
- Plastic deformation.
- Anisotropic/weave-aware material models.
- Soft bodies and volumetric extensions.
- Cloth/fluid interaction.

## Promotion rule

A component moves into `physics-engine` only when it is demonstrably general-purpose, its deterministic contract is defined, and doing so does not make cloth-specific policy authoritative over rigid-body behavior.
