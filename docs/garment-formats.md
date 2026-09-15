# Garment import formats

The import boundary should distinguish **mesh interchange**, **garment construction semantics**, and **textile material data**. A rendered garment mesh is useful, but it is not equivalent to a pattern/sewing description, and a texture is not equivalent to measured fabric physics.

## Priority

| Priority | Format | What it gives us | Cloth Lab strategy |
| --- | --- | --- | --- |
| 1 | OBJ (`.obj`) | Indexed 3D geometry; commonly paired with MTL material data | Support first as a geometry-only path into `GarmentAsset`. Never infer seams, patterns, or physical textile properties from OBJ. |
| 2 | glTF 2.0 / GLB (`.gltf`, `.glb`) | Open, web-friendly mesh/material/skin/animation interchange | Add next for browser uploads and asset preview. Keep standard glTF data separate from any vendor-specific garment metadata. Investigate CLO/Marvelous garment metadata in glTF `extras` as an optional adapter only if its schema is stable/documented. |
| 3 | DXF-AAMA / DXF-ASTM (`.dxf`, `.aam`) | Apparel-industry 2D pattern-piece interchange | Add once `GarmentAsset` has explicit pattern-piece and sewing concepts. Do not assume DXF alone contains enough information to reconstruct sewing relationships or complete product specifications. |
| 3 | U3M | Open fashion-oriented digital material data with PBR appearance plus physical measurements | Add alongside the richer textile model. Map measured source fields explicitly into cloth material parameters; never silently substitute visual material properties for simulation physics. |
| 4 | FBX (`.fbx`) and USD/USDZ (`.usd`, `.usda`, `.usdc`, `.usdz`) | Broad DCC scene interchange: meshes, materials, joints, animation and scene data | Useful compatibility adapters after OBJ/glTF. Imported mesh/rig data must remain distinct from cloth-specific semantics. |
| Integration | CLO / Marvelous garment/project (`.zpac`, `.zprj`) | Rich garment/project data including 2D patterns, sewing and fabrics | Valuable source data, but vendor-owned. Support only through an official/documented schema, SDK, metadata export, or user-side conversion path; do not build the core around reverse-engineering proprietary containers. |
| Integration | Browzwear garment (`.bw`; legacy `.vsgx`/`.vsp`) | Rich garment containers used by VStitcher | Treat like CLO/Marvelous project files: integrate through supported APIs/export paths rather than making a proprietary container the canonical asset format. |
| Low | Alembic / point caches | Baked vertex animation | Treat primarily as playback/reference evidence, not as an editable garment source for simulation. |

## Why this order

1. **OBJ proves the adapter and validation boundary cheaply.** It lets us validate arbitrary triangle topology before coupling the solver to richer authoring formats.
2. **glTF/GLB is the strongest next upload format for the web surface.** It is open and browser-friendly, and current CLO/Marvelous workflows can import/export it. The vendor tools can also emit additional garment metadata, which we can consume only as an optional extension rather than contaminating the canonical asset model.
3. **DXF is important for actual apparel patterns.** It should land after the normalized model can represent 2D pattern pieces and explicit seam pairings; otherwise we would either discard the useful semantics or invent them.
4. **U3M is important for actual textile data.** It belongs next to material calibration, not inside a renderer-only material layer, because it can carry physical measurements as well as appearance.
5. **FBX/USD are compatibility formats rather than the garment authority.** They help interoperate with DCC pipelines but should not define Cloth Lab's simulation model.
6. **Vendor project files are not our canonical format.** They can become high-value import adapters when a stable supported integration path exists.

## Normalized asset rule

Every parser produces the same repository-owned `GarmentAsset` boundary. Source-specific units, coordinate systems, metadata names, index conventions, and optional features are normalized before simulation state is created. Import must finish validation before any solver state is mutated.

The normalized asset fingerprint is based on canonical simulation data rather than UI state or source filename. Provenance can record the source format independently.

Material imports follow the same rule: source measurements are preserved as explicit provenance and converted through a deterministic, testable mapping. Unsupported measurements must remain unsupported rather than being guessed.

## Sources used for format selection

- CLO compatible formats: https://support.clo3d.com/hc/en-us/articles/115002000227-Compatible-File-Format
- CLO internal garment/project formats: https://support.clo3d.com/hc/en-us/articles/115000470688-CLO-File-Formats
- CLO glTF/GLB import/export: https://support.clo3d.com/hc/en-us/articles/360051525034-glTF-2-0-GLTF-GLB-File
- Marvelous Designer compatible formats: https://support.marvelousdesigner.com/hc/en-us/articles/47358199862553-Compatible-File-Format
- Marvelous Designer garment/project formats: https://support.marvelousdesigner.com/hc/en-us/articles/47358169252633-Marvelous-Designer-File-Format
- Marvelous Designer glTF/GLB import/export: https://support.marvelousdesigner.com/hc/en-us/articles/55686813557913-glTG-2-0-glTF-GLB-File-ver-2026-0
- Browzwear file types: https://help.browzwear.com/en/articles/13065254-browzwear-file-types
- U3M format: https://www.u3m.info/
- ASTM D6673 sewn-products pattern interchange: https://store.astm.org/d6673-04.html
