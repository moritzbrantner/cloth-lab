# Performance evidence

`cloth-lab` keeps performance measurements separate from correctness gates. The reference profiler is intentionally deterministic in topology, fixed-step inputs, solver settings, and replay state, but it does not impose wall-clock pass/fail thresholds in ordinary CI.

Run the profile with:

```text
cargo run --locked --release --example performance_profile
```

The output is a small Markdown table with median/min/max nanoseconds per simulation step plus the final deterministic state fingerprint for every lane. Each timing sample starts from a fresh copy of the same fixture, and all samples for one lane must end on the same fingerprint or the profiler fails.

## Stable lanes

- `reference-1-iteration`: one integration step plus one stretch/bending projection pass on the reference sheet. This is the lowest-cost whole-step baseline; it is not described as pure integration because the public solver correctly keeps integration and constraint projection inside one authoritative step.
- `reference-8-iterations`: the same sheet and step inputs with eight solver iterations. Comparing this with the one-iteration lane makes repeated constraint-projection cost visible without inventing a second benchmark-only solver path.
- `rigid-collision`: the same reference sheet and eight iterations, with one intersecting sphere collider. Compare it with `reference-8-iterations` to observe the incremental rigid-contact path.
- `layered-no-self-collision`: two close, disconnected sheets with eight iterations and self-collision disabled. This is the control workload for the self-collision lane.
- `layered-self-collision`: the identical layered fixture with vertex/triangle self-collision enabled. Compare it with the control lane to observe the incremental broad-phase, narrow-phase, and projection cost.

The paired lanes are deliberate. They keep normal production APIs authoritative and avoid exposing partially-stepped solver phases merely for benchmarking. If future profiling shows a real hot path that needs a narrower API, that API should be justified by production computation boundaries rather than by the benchmark harness.

## Comparability rules

Keep fixture dimensions, topology construction, fixed timestep, solver iteration counts, collision geometry, and sample/step counts stable when comparing commits. If a benchmark workload itself must change, treat that as a new lane or record the change explicitly rather than silently replacing historical evidence.

Use release builds and compare measurements on the same machine/environment when evaluating an optimization. Absolute timings from different hosts are not directly comparable. Correctness tests and deterministic fingerprints remain authoritative even when a faster implementation is under evaluation.
