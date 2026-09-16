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

**Status:** complete for the deterministic vertex/triangle reference scope. Self-collision uses explicit positive thickness, the pinned `rust-kernels` sweep-and-prune broad phase already exercised by `collision-lab`, conservative f32 AABBs for candidate generation, f64 vertex/triangle narrow phase, inverse-mass-weighted projection, deterministic one-ring mesh exclusions, and degenerate-triangle fallbacks. `TriangleMeshCloth` caches static exclusion topology and runs the kernel in-place during every solver iteration without cloning particle state. Folded-surface, coplanar/inverted-layer, and dense-contact fixtures cover deterministic replay, finite state, and actual projection. Broader optimization and parallelism belong to the performance stage rather than changing the proven reference semantics here.

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

- **OBJ:** supported for deterministic geometry-only ingestion; it does not imply pattern/sewing/material semantics.
- **GLB:** self-contained static triangle geometry is supported next to OBJ, including deterministic scene-node transforms. Unsupported animations, skins, morph targets, external buffers, ambiguous scenes, and non-triangle primitives fail closed. Loose `.gltf` plus sidecars remains future work if multi-file browser upload is justified.
- **DXF-AAMA/ASTM after the normalized pattern model exists:** preserve 2D apparel pattern pieces instead of flattening them into a generic mesh; require explicit sewing information where DXF does not provide enough to reconstruct it.
- **U3M alongside material calibration:** consume fashion-oriented PBR plus measured physical material data through explicit, testable mappings into solver parameters; do not infer simulation physics from visual material properties.
- **FBX and USD/USDZ later:** compatibility adapters for broader DCC pipelines, not cloth-specific authorities.
- **CLO/Marvelous `.zpac`/`.zprj` and Browzwear `.bw` via supported integration only:** these are semantically rich but vendor-owned; use documented schemas, SDKs, or export paths rather than reverse-engineering proprietary containers.
- **Alembic and point caches remain low priority for ingestion:** useful as baked animation/reference evidence, not as editable simulation assets.

See [docs/garment-formats.md](docs/garment-formats.md) for the format rationale and source references.

**Progress:** the import foundation now has a parser-independent `GarmentAsset`, deterministic simulation-geometry fingerprints, fail-closed OBJ and self-contained GLB geometry adapters, and a `TriangleMeshCloth` path for arbitrary indexed surfaces. Structural constraints come only from real mesh edges and reuse the existing mesh-general bending/collision/contact solver primitives. Plain mesh formats intentionally do not manufacture pattern-space shear, seam, or fabric semantics they do not contain.

**Acceptance:** a supported uploaded garment can be converted into a stable `GarmentAsset`, round-tripped/reloaded without changing its simulation fingerprint, and used as the initial state for deterministic cloth simulation. Parser and topology failures never partially mutate simulation state.

## 6. Interactive mode and rendering

- Add a wgpu viewer over simulation snapshots.
- Update render vertices from authoritative simulation positions.
- Add an interactive mode that can load normalized garment assets and run them directly in the cloth simulation.
- Allow explicit user interaction with the simulation, including grabbing/dragging/releasing cloth and editing supported pins or attachment inputs, without giving the rendering layer direct authority over solver state.
- Add pause/resume, deterministic single-step, reset/replay, and controlled simulation-parameter inspection so interactive experiments remain reproducible.
- Surface garment-upload/import validation and parsing errors clearly before simulation begins.
- Add camera and inspection controls without coupling them to the solver.
- Add debug views for constraints, collisions, normals, self-collision contacts, and solver error.

**Progress:** the Pages viewer runs generated falling sheets and normalized OBJ/self-contained GLB assets through a live Rust/WASM session. It supports mesh-density changes, textile presets, gravity/solver-iteration/velocity-damping controls, pause/play/single-step/reset, ordinary vertex dragging, persistent movable pins, editable none/sphere/capsule collision scenery, renderer-only orbit/pan/zoom camera controls, and deterministic replay coverage. The inspector exposes normalized upload fingerprints, constraint topology/error, rigid-contact/friction counts, and aggregate self-collision candidate/test/projection counts from Rust. Surface, wireframe, constraint, and sampled-normal views remain renderer-derived consumers of authoritative positions/topology. Generated reference snapshots remain a fail-closed fallback/evidence path. Remaining interactive work is primarily opt-in self-collision contact-location visualization, richer attachment editing, and eventually the planned wgpu viewer when it provides concrete value over the current canvas consumer.

**Acceptance:** a user can upload a supported garment, inspect the normalized asset, start/pause/step/reset its simulation, and manipulate supported interactive inputs while replay from the same asset and recorded inputs remains deterministic.

## 7. Performance and scale

- Profile integration, constraint projection, collision detection, and self-collision independently.
- Introduce deterministic graph coloring/batching before parallel constraint solving.
- Evaluate SIMD and GPU solving only against exact or explicitly bounded correctness evidence.
- Add cloth LOD experiments only after a stable high-quality reference solver exists.

**Progress:** a deterministic reference performance profile now keeps five stable lanes: one- versus eight-iteration reference stepping, rigid collision against the same reference sheet, and a layered self-collision workload paired with an identical no-self-collision control. Every timing sample starts from the same fixture and must replay to the same state fingerprint. CI runs only a small functional smoke of these workloads; wall-clock measurements remain release-mode evidence rather than pass/fail gates.

**Next:** capture and compare release-mode baseline measurements on a stable environment, identify the strongest measured cost rather than assuming the hotspot, and make the first optimization against that lane while preserving the paired control, replay fingerprint, and existing correctness fixtures.

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
