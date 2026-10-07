# Changelog

All notable changes to SCIcoRS are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.1] — 2026-10-07

This release completes the **BLUE13** roadmap (phases 35–41) and turns SCIcoRS
into a mature scientific-computing platform rather than a set of physics models:
it adds the general-purpose numerical machinery that real research and
engineering workflows need — sparse large-scale linear algebra, parameter
estimation and uncertainty quantification, mesh/field contracts, resilient
reproducible execution, self-describing result datasets, an evidence-based
validation toolkit, and a user-facing toolchain. Every item below ships with
behavioural tests; the full record (including the defects found by two
independent audits and the honest non-findings) is in
`docs/log/log20261007-1.md`.

### Added

- **Phase 35 — Sparse linear algebra (`core::compute::sparse`).** Validated
  COO/CSR/CSC storage with format conversion, sparse add/sub/mul/transpose and
  submatrix selection, and Krylov solvers: CG (SPD, rejects asymmetric input),
  MINRES (Paige–Saunders, symmetric indefinite), restarted GMRES(m) with CGS2
  reorthogonalization, and BiCGSTAB. Every solver returns a solution **and**
  convergence statistics (`iterations`, initial/final/relative residual, residual
  history, and an explicit `StopReason`); hitting the iteration cap reports
  `MaxIterationsReached`, never convergence. Preconditioners: Jacobi, row and
  **symmetric** diagonal equilibration, and ILU(0). A matrix-free
  `LinearOperator` trait lets Krylov methods consume a user `apply`/`apply_transpose`
  pair without assembling a global matrix.
- **Phase 35 — Least squares (`core::compute::least_squares`).** Householder QR
  with column pivoting (rank-deficient input is rejected and directed to SVD) and
  a truncated one-sided Jacobi SVD that handles overdetermined, underdetermined
  and rank-deficient systems with a minimum-norm solution — without forming the
  normal equations as a general default. Both report rank, singular-value range,
  condition estimate and residual.
- **Phase 36 — Analysis layer (`analysis`).** Data contracts (`ParameterSpec`,
  `ObservationSet`, `ObjectiveSpec`) plus finite-difference sensitivity with
  gradient ranking, Levenberg–Marquardt parameter estimation with fit
  diagnostics and per-run traceable IDs, Monte-Carlo and Latin-hypercube
  uncertainty propagation with a seeded reproducible RNG and failure accounting,
  bounded Nelder–Mead optimization with cancellation, and full-factorial
  experiment design. Analysis is decoupled from execution through a
  `SimulationFunction` trait so the same code runs synthetic and real simulations.
- **Phase 37 — Mesh, regions and fields (`core::mesh`).** A common topology
  contract (dimension, units, node/element IDs, connectivity, boundary faces,
  region tags, source), named material/boundary regions, structural/quality
  validation (degenerate elements, extreme aspect ratios, isolated regions),
  node/cell/face fields with declared location and units, cross-mesh mapping with
  a measurable conservation error, and an error-indicator/mark/refine interface
  with a concrete triangle refiner. Bridged to the existing STEP/STL I/O with
  region names and unit metadata preserved.
- **Phase 38 — Checkpoints and elastic execution (`runtime::checkpoint`,
  `runtime::execution`).** Versioned, hash-validated, **crash-safe** atomic
  checkpoints (a crash between the two renames still recovers the last valid
  checkpoint from the backup), full state/event/RNG/recorder snapshots,
  model/solver/plugin compatibility checks that reject incompatible checkpoints
  with a difference list, six distinct run outcomes (completed / cancelled /
  resource-limit / numerical-failure / I/O-failure / timeout), cooperative
  cancellation tokens, resource budgets with RAII leases, and failure-classified
  retry with exponential backoff that never retries deterministic errors.
- **Phase 39 — Self-describing datasets (`postproc::dataset`).** A versioned
  manifest (simulation id, model signature, config hash, parameters + units,
  solver/tolerances, RNG seed, backend, library/plugin versions, status, error
  summary), variables carrying unit/dimensions/sampling location/missing-value
  encoding, **real** unit conversion (values are transformed, not relabelled),
  chunked append with atomic completion marking, a self-contained on-disk
  container (no new dependencies), a hand-written CRC-32, schema migration, and
  CSV/JSON adapters that report the metadata they cannot preserve.
- **Phase 40 — Numerical validation (`validation`).** Versioned benchmarks with
  sourced, precision-tagged reference values and **mandatory justified**
  tolerances; convergence studies that estimate the observed order from log-log
  least squares and refuse to fabricate an order from insufficient or
  non-monotonic data; registrable physical invariants with declared
  applicability and source/sink terms; result comparison that separates alignment
  error from solver error; and a machine-readable report that tracks passed /
  failed / skipped / not-applicable **separately** (an un-executed item never
  counts as passed).
- **Phase 41 — Toolchain, C ABI and plugin compatibility (`cli`, `bindings`).**
  A library CLI with stable exit codes for check/run/status/results/cancel/
  resume/validate, a stable C ABI (opaque handles, fixed-width types, explicit
  `scico_free`, stable error codes, with double-free and use-after-free guards),
  plugin manifests declaring API version/entry/capabilities/platform/dependencies/
  signature-hash with pre-load compatibility checks and staged, isolated loading,
  and a documented deprecation policy. Dynamic-library plugins are documented as
  running in-process and **not** a security sandbox.
- **`scico` binary (`src/bin/scico.rs`).** A real command-line entry point over
  the CLI library, exercised end-to-end (check → run → status → results →
  validate dataset) with correct non-zero exit codes.
- **`examples/validation_study.rs`.** Wires the real `HeatConduction2D` solver
  into the validation toolkit: a mesh-refinement convergence study against the
  analytic steady state, a sourced physics benchmark, and a registered invariant,
  assembled into a `ValidationReport`.

### Changed

- **Crate version raised to `0.2.1`.**
- **Publishing metadata added** to `Cargo.toml`: `description`, `license`
  (`MIT OR Apache-2.0`), `repository`, `homepage`, `documentation`, `readme`,
  `rust-version`, `keywords`, `categories`, and an explicit `include` list.
- **`LICENSE-APACHE` added.** The README and `Cargo.toml` both claim dual
  `MIT OR Apache-2.0` licensing; only `LICENSE-MIT` existed before, so the Apache
  text is now present to make the declared license accurate.

### Fixed

Fixes discovered by two independent read-only audits of the new code:

- **Checkpoint atomic-write crash window.** Renaming the checkpoint directory to
  a backup and then renaming the staging directory into place left a window in
  which the checkpoint directory was absent; a crash there lost the checkpoint.
  `load_checkpoint` now falls back to the backup, and a stale backup is recovered
  before the next write. Regression test `crash_between_renames_still_loads_backup`.
- **Backup path collision.** `Path::with_extension("old")` made `results.01` and
  `results.02` map to the same backup; suffixes are now appended
  (`results.01.old` / `results.02.old`).
- **Plugin deprecation note was dropped.** An empty branch acknowledged a
  deprecated-but-compatible decision without recording its migration note; the
  note is now surfaced through `PluginLoader::notices()`.
- **`cg_with_scaling` broke symmetry.** The first version applied *row* scaling,
  which is asymmetric and made CG reject the system; it now uses symmetric
  equilibration `D·A·D` and recomputes the residual against the original system.
- Dead helpers (`DiagonalScaling::unscale_solution`, `ScicoOpaqueTag`,
  `ScicoHandlePtr`) removed or wired into real call paths; `MeshTopology` ID
  accessors and `CheckpointStore::with_max_payload_bytes` are covered by tests.

### Verification

```
cargo fmt --check                                          → clean
cargo clippy --all-targets -- -D warnings                  → 0 warnings
cargo test --all-targets --quiet                           → 2529 passed; 0 failed; 0 ignored
cargo test --doc                                           → 5 passed; 0 ignored
cargo clippy --features gpu --all-targets -- -D warnings   → 0 warnings
cargo test --features gpu --lib                            → 2563 passed; 0 failed; 0 ignored
cargo run --example validation_study                       → 3 passed; 0 failed; 1 not-applicable
cargo run --bin scico -- version                           → exit 0
```

---

## [0.2.0] — 2026-09-10

This release is the result of two multi-round deep-and-broad audits of the whole
crate. Its theme is **removing silent failure**: a large class of defects were
found where code compiled, produced no warning, and passed its tests, yet
produced wrong results or discarded user data without saying so. Every fix below
is backed by a regression test, and the audits' full records (including the
claims that were disproven along the way) are in `docs/log/log20260910-1.md` and
`docs/log/log20260910-2.md`.

### Added

- **Real GPU acceleration on wgpu 30.0.1**, behind the optional `gpu` feature.
  The previous implementation was an interface shell: it accepted work and
  produced answers without executing any compute shader. The new backend ships
  WGSL kernels, a buffer pool, and a `wgsl-validate` feature that statically
  parses and validates every shader with `naga`.
- **`Link::delay`** — transport delay on links was previously accepted and
  silently ignored, so a configured delay had no effect on the simulation. It
  is now modelled with a per-link delay line.
- **A real zero-crossing event chain.** Crossing detection now feeds the
  scheduler's event queue, and `Scheduler::run_event_phase` is shared between
  the engine's Phase 6 and `SequentialScheduler::step`, so an event observed by
  one path is dispatched by both. New observables: `crossings_detected()` and
  `events_dispatched()`.
- **`all_finite`** (`core::param`) — shared element-wise finiteness validation
  for blocks whose configuration is a coefficient or state vector.
- **Structured event routing** via `Event::target`, plus a
  `log_ignored_parameters` diagnostic so a saved parameter a block does not
  accept is reported instead of dropped in silence.

### Changed

- **`coupling::convergence` reports non-convergence as an error.**
  `fixed_point_iteration`, `jacobi_coupling` and `gauss_seidel_coupling` now
  return `Err` when the iteration budget is exhausted instead of returning
  `Ok`. This is a **behavioural contract change**: a converged-looking `Ok` used
  to hide drift. `converged()`'s documentation now states that the single-step
  criterion cannot distinguish "settled" from "constant drift", which is why
  exhaustion is reported as failure rather than success. In-tree callers were
  tests only, and they were updated.
- **`DataRecorder` streaming is no longer allowed to drop data.** The previous
  strategy froze the CSV column set at the first flush, which silently discarded
  every signal that appeared later. Streaming now writes each block to its own
  segment file and the final flush merges them, taking the **union** of all
  signals seen; rows where a signal is absent read back as `NaN`. Memory stays
  bounded by `max_samples`, and nothing is lost in either direction (a signal
  appearing *or* disappearing mid-run keeps its column).
- **VTK mesh set names round-trip losslessly.** Names colliding with a reserved
  keyword used to be rewritten with a `set_` prefix — a silent rename. They are
  now encoded reversibly (`~n<escaped>`) and decoded on import; unreserved
  foreign `FIELD` arrays keep their literal name as node sets.
- **`BlockFactory::reconstruct` restores through
  `configuration`/`apply_configuration`** instead of the generic parameter bag,
  so a block's typed fields are populated rather than only its mirrored
  parameters.
- **`hybrid::execute_update_phase` now runs the Update phase.** It previously
  validated blocks and returned without ever calling `block.update()`. The
  original (validation-only) entry point is retained as
  `validate_update_phase`, and both are exported.
- Configuration de-serialization is **hand-written for every block** rather than
  generated by a `scico_config!` macro. The macro was removed along with
  `src/scico_config.rs`; see "Removed" below for the reasoning.
- Crate version raised to `0.2.0`.

### Fixed

- **Non-finite values can no longer be injected through a saved diagram.**
  Blocks with vector- or matrix-valued configuration (`TransferFunction`,
  `StateSpaceSystem`, `MatrixMultiply`, `DiscreteFilter`) previously stored the
  incoming value directly, so one hand-edited `NaN` coefficient would poison
  every subsequent evaluation of that block. Each now validates element-wise and
  rejects the value instead of storing it.
- **`DiscreteFilter` no longer panics on a hostile file.** `IIRFilter::new`
  asserts `a[0] != 0`; an `a` vector with a zero leading coefficient is now
  rejected during configuration instead of reaching that assertion.
- **Zero-crossing semantics.** A crossing is a sign change:
  - an exact zero no longer fires when the sign is unchanged (a signal parked at
    zero used to fire every step);
  - a non-finite sample now clears that index's history, so `-1 → NaN → +1` no
    longer reports a crossing across the gap;
  - history for signals that vanish is pruned.
- **`NaN` residuals no longer pass the convergence test.** `0.0.max(NaN)`
  returns `0.0`, so a `NaN` residual was silently treated as converged. Residual
  accumulation uses `finite_max_propagating`, keeping `NaN` sticky.
- **Delay-line capacity** is `steps + 1`, not `steps`: the push happens before
  the read. Eager allocation was also replaced with lazy growth, so a legal
  `delay = 1 s` at `dt = 1e-6` no longer reserves tens of MiB per link.
- **Float step counting** snaps the delay step count to the nearest integer
  within a relative epsilon before ceiling, because `3.0 * 0.1 / 0.1` is
  `3.0000000000000004` and would otherwise round up to four steps.
- **STEP `B_SPLINE_CURVE_WITH_KNOTS`** is dispatched on the parsed keyword
  rather than `str::contains("LINE")`.
- **`import_mesh`**, `BlockFactory::reconstruct` and the recorder each had a
  first "fix" that was itself a no-op or actively harmful (the recorder's first
  fix reintroduced silent data loss); all three are corrected and covered.
- Test suites that executed **zero assertions** were repaired, including
  `domains::analog::mna::test_mna_singular_detection` and
  `domains::acoustic::bem_acoustic::test_far_field`.
- Pre-existing `clippy -D warnings` blockers cleared, including a
  `question_mark` lint in `bindings::data_io::mesh_io`.

### Removed

- **`src/scico_config.rs` and the `scico_config!` macro.** The macro generated
  `configuration`/`apply_configuration` from a declarative scalar field list.
  An attempt to extend it to non-scalar fields (vectors, matrices, strings,
  enums) failed for structural reasons that were verified experimentally:
  `macro_rules!` cannot simultaneously match a parameterised step such as
  `pad_to(4)` (which needs `tt`, an indivisible token) and pass an expression
  alongside it (`expr` may not be followed by `[`); a forwarded `$($rest)*` tail
  collapses into a single opaque token; and a bare brace group as the first
  token of a `tt` repetition makes rustc report a spurious recursion-limit
  error. The blocks that needed this are better served by direct
  implementations, so all 14 call sites were expanded by hand and the macro was
  deleted. The failure analysis is recorded in `docs/log/log20260910-2.md` §13
  so it need not be rediscovered.

### Verification

```
cargo test --all-targets --quiet                          → 2111 passed; 0 failed; 0 ignored
cargo clippy --all-targets -- -D warnings                 → 0 warnings
cargo test --doc                                          → 4 passed; 0 ignored
cargo test --features gpu --quiet                         → 2145 passed; 0 failed; 0 ignored
cargo clippy --all-targets --features gpu -- -D warnings  → 0 warnings
```

---

## [0.1.0] — Initial release

First public version: the seven-layer architecture (`core`, `runtime`,
`blocks`, `domains`, `coupling`, `postproc`, `bindings`) with 19 domain modules,
a unit system built on the 7 SI base dimensions, an ODE/DAE solver engine, a
TOML + SQLite data layer, and Python/plugin bindings.

[0.2.1]: https://github.com/mikewolfli/ScicoRs/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/mikewolfli/ScicoRs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/mikewolfli/ScicoRs/releases/tag/v0.1.0
