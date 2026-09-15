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

**Status:** complete for the initial qualitative textile scope. Stretch, shear, mesh-general isometric bending, and deterministic cloth/rigid contact friction remain independent raw solver/contact parameters. `TextilePreset` maps cotton-, denim-, silk-, and leather-like names onto those raw parameters while topology, spacing, and particle mass remain explicit. Deterministic fixtures record per-preset fingerprints, sag position, constraint errors, collision projections, and friction corrections. These presets are comparison fixtures, not calibrated real-world material models.

## 4. Self-collision

- Add cloth thickness and vertex/triangle self-collision.
- Reuse/prove spatial acceleration structures from the existing physics/collision work where appropriate.
- Exclude adjacent mesh features deterministically.
- Stress folding, inversion, and dense-contact cases.

**Progress:** the first self-collision kernel slice is implemented independently of the main cloth step loop. It uses explicit positive thickness, the pinned `rust-kernels` sweep-and-prune broad phase already exercised by `collision-lab`, conservative f32 AABBs for candidate generation, f64 vertex/triangle narrow phase, inverse-mass-weighted projection, deterministic one-ring mesh exclusions, and degenerate-triangle fallbacks. The next slice wires this proven kernel into every cloth solver iteration and adds folding/inversion stress fixtures before broader self-collision optimization.

## 5. Garments and asset ingestion

- Support attachment constraints to animated bodies.
- Add cape/skirt-style garment fixtures.
- Add a user-uploaded clothing import pipeline rather than requiring garments to be authored as code fixtures.
- Parse each supported source format through an adapter into a normalized `GarmentAsset` containing simulation topology/rest geometry plus explicit material, seam, pattern, and attachment metadata where the source actually provides them.
- Keep the normalized garment asset independent of the viewer/session so imported clothes can be stored, replayed, tested, and simulated again without reparsing UI state.
- Canonicalize and fingerprint imported assets deterministically; identical supported input and import settings must produce identical simulation assets.
- Validate imports before simulation and fail closed on malformed topology, unsupported features, invalid indices, non-finite geometry, or ambiguous required metadata rather than silently repairing them differently between runs.
- Keep garment-specific parsing and normalization outside the solver loop. Reuse generic asset provenance/build infrastructure from `asset-tooling` when that becomes useful instead of creating a second general-purpose asset pipeline here.
- Validate fast-moving attachment points and character collision.
- Keep animation authority outside the cloth solver.

### Format sequence

- **OBJ first:** prove arbitrary mesh ingestion and deterministic normalization without pretending that a plain mesh contains pattern/sewing semantics.
- **glTF/GLB next:** use the open, web-friendly mesh/material format for uploads and previews; treat CLO/Marvelous garment metadata in `extras` as an optional vendor extension only when its schema is stable enough to validate explicitly.
- **DXF-AAMA/ASTM after the normalized pattern model exists:** preserve 2D apparel pattern pieces instead of flattening them into a generic mesh; require explicit sewing information where DXF does not provide enough to reconstruct it.
- **FBX and USD/USDZ later:** compatibility adapters for broader DCC pipelines, not cloth-specific authorities.
- **CLO/Marvelous `.zpac`/`.zprj` via supported integration only:** these are semantically rich but vendor-owned; use a documented schema/SDK/export path rather than reverse-engineering proprietary containers.
- **Alembic and point caches remain low priority for ingestion:** useful as baked animation/reference evidence, not as editable simulation assets.

See [docs/garment-formats.md](docs/garment-formats.md) for the format rationale and source references.

**Progress:** the first import slice defines a parser-independent `GarmentAsset`, deterministic simulation-geometry fingerprinting, and a fail-closed OBJ adapter with deterministic polygon triangulation. This slice intentionally imports only surface geometry; richer garment semantics are deferred until the canonical asset model can represent them explicitly.

**Acceptance:** a supported uploaded garment can be converted into a stable `GarmentAsset`, round-tripped/reloaded without changing its simulation fingerprint, and used as the initial state for deterministic cloth simulation. Parser failures never partially mutate simulation state.

## 6. Interactive mode and rendering

- Add a wgpu viewer over simulation snapshots.
- Update render vertices from authoritative simulation positions.
- Add an interactive mode that can load normalized garment assets and run them directly in the cloth simulation.
- Allow explicit user interaction with the simulation, including grabbing/dragging/releasing cloth and editing supported pins or attachment inputs, without giving the rendering layer direct authority over solver state.
- Add pause/resume, deterministic single-step, reset/replay, and controlled simulation-parameter inspection so interactive experiments remain reproducible.
- Surface garment-upload/import validation and parsing errors clearly before simulation begins.
- Add camera and inspection controls without coupling them to the solver.
- Add debug views for constraints, collisions, normals, self-collision contacts, and solver error.

**Acceptance:** a user can upload a supported garment, inspect the normalized asset, start/pause/step/reset its simulation, and manipulate supported interactive inputs while replay from the same asset and recorded inputs remains deterministic.

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
