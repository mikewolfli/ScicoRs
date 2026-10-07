# SCIcoRS — Unified Simulation Kernel

[![中文文档](README.zh-CN.md)](README.zh-CN.md) | [Changelog](CHANGELOG.md) | [Checklist](docs/checklist/CHECKLIST.MD) | [Roadmap (Blueprint)](docs/blueprints/roadmap.md) | [Design Principles](docs/blueprints/principle.md)

> **SCI**entific **co**mputing & **R**eality **S**imulation — a universal simulation kernel that unifies engineering and scientific simulation across all disciplines, scales, and fields.

---

## Overview

SCIcoRS provides a **single architecture** for modeling, simulation, and data management, enabling seamless integration from the smallest chip to the largest cosmic system.

### Scale Coverage

| Scale | Examples |
|-------|---------|
| **Nanometer (10⁻⁹ m)** | Molecules, chip transistors, quantum dots |
| **Micrometer (10⁻⁶ m)** | Cells, MEMS devices, microfluidics |
| **Millimeter (10⁻³ m)** | PCB traces, biological organs, electronic components |
| **Meter (10⁰ m)** | Mechanical systems, human body, vehicles |
| **Kilometer (10³ m)** | Buildings, terrain, equipment installations |
| **Light-year (10¹⁶ m)** | Stars, galaxies, cosmic structures |

### Unified Architecture

- **One** coordinate system (1D/2D/3D, Cartesian/polar/cylindrical/spherical)
- **One** dimensional/unit system (7 SI base dimensions, automatic conversion)
- **One** solver engine (ODE/DAE, stiff/non-stiff, sparse, nonlinear)
- **One** extensible database (TOML data + SQLite indexing)

---

## Architecture (7 Layers)

```
┌─────────────────────────────────────────────────────────────┐
│  bindings/   — Python API, Plugin system, C ABI, Data I/O (STL/STEP/Mesh) │
│  cli/        — Command-line toolchain (check/run/status/results/validate) │
│  validation/ — Benchmarks, convergence studies, invariants, reports       │
│  postproc/   — Recording, Visualization, Reporting, Datasets, Batch, HIL  │
│  analysis/   — Sensitivity, Calibration, Uncertainty, Optimization        │
│  coupling/   — Multi-Physics Coupling Bus, Cross-Scale Mapping    │
│  domains/    — 19 Domain-Specific Simulation Modules              │
│  blocks/     — Standard Block Library (Sources, Math, Logic...)   │
│  runtime/    — Context, Engine, Solvers, Scheduler, Events, State,  │
│                Checkpoints, Elastic Execution                       │
│  core/       — Block, Port, Link, Diagram, Types, Coord, Units,    │
│                Compute (sparse/dense), Mesh                        │
└─────────────────────────────────────────────────────────────┘
```

### Layer Details

#### `core/` — Data Model Layer
The foundational "nouns" of simulation. Provides the building blocks for constructing all simulation models.

- **Block** — fundamental functional simulation unit with ports, parameters, and lifecycle
- **Port / Link** — typed I/O interfaces and directed signal connections
- **Diagram** — topology of interconnected blocks with serialization (JSON/TOML) and validation
- **Component** — reusable component template system
- **Signal** — continuous, discrete, event, and bus signal types
- **Tensor** — N-dimensional array type
- **State / IO / Dependency** — declarations for state variables, I/O specs, and inter-block deps
- **Coord** — 1D/2D/3D coordinate systems (Cartesian, polar, cylindrical, spherical) + `Transform4x4`
- **Units** — 7 SI base dimensions, derived dimensions, `Unit`/`Quantity` with automatic conversion
- **Compute** — unified math platform: matrix ops, vector ops, FFT, numerical integration, eigenvalue solvers (Jacobi, subspace iteration)

#### `runtime/` — Simulation Execution Layer
The "verbs" that make simulation happen. Drives execution on top of the data model.

| Sub-module | Description |
|------------|-------------|
| **Context** | Centralized time, mode (`Normal`/`RealTime`/`SingleStep`/`Breakpoint`), lifecycle & shared data |
| **Engine** | Top-level orchestrator: lifecycle, time advancement, block execution ordering |
| **State** | Unified continuous + discrete state management with snapshots |
| **Solvers** | Fixed-step (Euler/RK4/Heun/Midpoint), Adaptive (RK45/RK23/CashKarp), Stiff (BackwardEuler/Trapezoidal/BDF2), DAE (index-1), Nonlinear (Newton-Raphson), Linear (dense LU, sparse CSR) |
| **Scheduler** | Topological ordering, signal flow, hybrid continuous/discrete/event/multi-rate scheduling, clock domain isolation |
| **Workflow** | DAG-based task orchestration with parallel/serial stages, barrier sync, pipeline |
| **Event** | Time-sorted event queue, zero-crossing detection, external/conditional triggers |
| **Discrete** | Digital filters (FIR/IIR), integrators, counters, timers, PLC logic (AND/OR/NAND/NOR/XOR/NOT gates) |
| **Algebraic** | Algebraic loop detection, fixed-point/relaxation iteration, numerical guards |

#### `blocks/` — Standard Block Library
Built-in simulation blocks for rapid model construction.

- **sources** — const, sine, square, step, pulse, noise
- **math** — adder, subtractor, multiplier, divider, gain, trig, matrix multiply
- **logic** — AND/OR/NOT/XOR gates, comparator, multiplexer, switch, saturation
- **continuous** — integrator, PID controller, transfer function, state-space
- **discrete_ctrl** — unit delay, discrete filter, discrete PID
- **sinks** — scope, chart buffer, data recorder, numeric display

#### `domains/` — 19 Domain-Specific Simulation Modules

| # | Domain | Module | Coverage |
|---|--------|--------|----------|
| 13 | **TCAD** | `tcad/` | MOSFET/BJT models, drift-diffusion, doping profiles, CV/IV curves, mobility models, oxidation |
| 14 | **Analog** | `analog/` | MNA matrix, R/L/C/D/Diode/OpAmp/MOSFET stamps, DC op/sweep, AC sweep, transient, noise analysis |
| 15 | **Digital** | `digital/` | Logic gates, flip-flops, ALU, decoder, multiplier, shift register, CPU pipeline, timing analysis |
| 16 | **Molecular Dynamics** | `molbio/` | Force fields (LJ, Harmonic), integrators, energy minimization, RMSD, hydrogen bonds, dihedral angles |
| 17 | **Cell/Tissue** | `cellbio/` | Cell model, population dynamics, bioreactor (batch/fed-batch/continuous), growth kinetics |
| 18 | **Optics** | `optical/` | Ray tracing, Gaussian beams, Jones/Mueller matrices, gratings, fibers, waveguides, solar cells |
| 19 | **Acoustics** | `acoustic/` | SPL, RT60, room modes, transmission loss, BEM, loudspeaker, microphone, accelerometer |
| 20 | **PCB** | `pcb/` | Transmission lines (microstrip/stripline/CPW), PDN impedance, eye diagram, thermal, S2P/T-params |
| 21 | **Power Electronics** | `powerelec/` | Buck/Boost/Single-phase/Three-phase converters, motors (DC/Induction/PMSM/Stepper), FOC, IGBT |
| 22 | **EM/RF** | `emag/` | 1D/3D FDTD (Yee), electrostatics, antenna (dipole, arrays), RCS, Smith chart, skin depth |
| 23 | **Biomedical** | `bio_medical/` | Hodgkin-Huxley, Windkessel, compartment PK/PD, tissue mechanics, diffusion, tumor model |
| 24 | **Chemical** | `chemical/` | Batch/CSTR/PFR reactors, reaction kinetics, equilibrium, distillation, heat exchanger NTU |
| 25 | **Structural** | `structural/` | FEA (truss/beam/shell/solid), nonlinear FEA (Newton-Raphson), SDOF, fatigue, explicit dynamics |
| 26 | **Thermal** | `thermal/` | 1D/2D/3D heat conduction (ADI/SOR), convection, radiation, phase change, heat pipes |
| 27 | **Fluid (CFD)** | `fluid/` | 2D/3D Navier-Stokes (projection), 2D compressible NS (Roe), turbulence (k-ε RANS, Smagorinsky LES), multiphase VOF |
| 28 | **Multibody** | `multibody/` | Rigid bodies, constraints, collision detection/response, quaternions, AABB |
| 29 | **Aerospace** | `aerospace/` | 6-DOF aircraft, ISA/High-altitude atmosphere, aerodynamics, thermal protection, rocket thrust |
| 30 | **Quantum** | `quantum/` | State vectors, density matrices, MPS (tensor networks), VQE/QAOA/Grover/HHL/QFT, Lindblad master eq |
| 31 | **Astrophysics** | `astrophysics/` | N-body, ΛCDM cosmology, 2D MHD (HLL Riemann solver) |

#### `coupling/` — Multi-Physics Coupling Bus
Unified coupling bus enabling cross-domain and cross-scale co-simulation.

- Physics field registry, field mapping/interpolation between meshes (RBF)
- Cross-scale coupling (nano → micro → meter → cosmic) with RVE homogenization
- Convergence control (fixed-point, relaxation, Aitken)
- Time synchronization and coupling iteration scheduling

#### `postproc/` — Post-Processing & Visualization
- **Data Recording** — streaming data recorder/replayer, 3D field snapshots, offline analysis (RMS, FFT)
- **Visualization** — charts, contours, iso-surfaces, vector fields, volume slices
- **Reporting** — simulation report with sections and tables, data export (CSV, JSON, HDF5, VTK, XLSX)
- **Batch** — parameter sweeps, optimization loops, solver benchmarking
- **HIL Support** — hardware-in-the-loop I/O channels and runner

#### `bindings/` — Cross-Platform & Extension System
- **Python** scripting stubs — run simulation, read signals, register custom blocks, query library
- **Plugin** system — block, solver, and post-processor registries with manifest loading, API-version compatibility checks and staged, isolated loading
- **C ABI** — stable `extern "C"` facade with opaque handles, explicit `scico_free`, and stable error codes
- **Data I/O** — STEP/STL mesh import/export, general mesh formats
- **Platform** — OS detection, normalized paths, cloud/distributed runner

#### `core::mesh/` — Mesh, Regions & Field Workflow
- Topology contract (dimension, units, IDs, connectivity, boundary faces, region tags)
- Named material/boundary regions with structural and quality validation
- Node/cell/face scalar, vector and tensor fields with conservation-checked mapping
- Error-indicator / mark / refine interface with a concrete triangle refiner
- Lossless bridge to the existing STEP/STL/generic mesh I/O (region names + units preserved)

#### `analysis/` — Sensitivity, Calibration, Uncertainty & Optimization
- Data contracts: `ParameterSpec`, `ObservationSet`, `ObjectiveSpec`, `AnalysisResult`
- Finite-difference local sensitivity with gradient ranking
- Levenberg–Marquardt parameter estimation with fit diagnostics and run tracing
- Monte-Carlo / Latin-hypercube uncertainty with a seeded reproducible RNG
- Bounded Nelder–Mead optimization and full-factorial experiment design

#### `validation/` — Numerical Validation & Credibility Baseline
- Versioned benchmarks with sourced, precision-tagged reference values and justified tolerances
- Convergence studies estimating the observed order (refuses to fabricate one)
- Registrable physical invariants with declared applicability and source/sink terms
- Result comparison separating alignment error from solver error
- Machine-readable reports tracking passed / failed / skipped / not-applicable separately

#### `runtime` — Checkpoints & Elastic Execution
- Versioned, crash-safe atomic checkpoints with state/event/RNG/recorder snapshots
- Model/solver/plugin compatibility checks that reject incompatible checkpoints
- Six run outcomes (completed/cancelled/resource-limit/numerical-failure/I-O-failure/timeout)
- Cooperative cancellation, resource budgets, and failure-classified retry with backoff

---

## Compute Platform

All domain modules delegate mathematics to the unified `core::compute` module:

| Operation | Implementations |
|-----------|----------------|
| **Matrix** | Multiply, transpose, determinant, inverse, LU/Cholesky decomposition |
| **Sparse** | COO/CSR/CSC, SpMV, CG/MINRES/GMRES(m)/BiCGSTAB, Jacobi/ILU(0) preconditioners |
| **Least squares** | Householder QR, truncated SVD, minimum-norm solutions, rank/condition diagnostics |
| **Vector** | Dot, cross, norm, normalization, linear/spline interpolation |
| **FFT** | Base-2 Cooley-Tukey FFT for spectral analysis |
| **Integration** | Trapezoidal, Simpson, Gauss-Legendre quadrature |
| **Eigenvalues** | Jacobi method, subspace iteration |
| **Parallel** | `rayon`-based parallelism for compute-intensive loops |
| **GPU** | wgpu 30.0.1 compute shaders (optional `gpu` feature) |

This eliminated 5 copies of Gaussian elimination that existed across domain modules.

### GPU Acceleration (wgpu 30.0.1)

Enable the optional `gpu` feature to compile in a real GPU compute backend built on
wgpu 30.0.1 compute shaders:

```sh
cargo build --release --features gpu
cargo run  --release --features gpu --example gpu_bench
```

```rust
// Opt in once at start-up; every adaptive_* call then dispatches large
// workloads to the device automatically.
scico_rs::bindings::platform::enable_gpu_acceleration()?;
println!("{}", scico_rs::bindings::platform::compute_acceleration_report());
```

**Kernels** (WGSL, in `src/core/compute/gpu/`): dense GEMM with 16×16 shared-memory
tiling, element-wise add/sub/mul, scalar-vector scale, AXPY, and two-stage
dot/sum tree reductions.

**Precision.** WGSL `f64` storage buffers require `Features::SHADER_F64`, which is
*not* universal — Apple Silicon Metal reports `false` (verified on an M4). The
backend probes the adapter once and selects the matching kernel set, so GPU
acceleration works on every WebGPU-capable adapter:

- **`f64` kernels** on devices that advertise `SHADER_F64`.
- **`f32` kernels** elsewhere, reported as `GpuPrecision::F32`. The dispatcher
then keeps the crate's `f64` workloads on the CPU (`GpuBackend::supports_f64`)
instead of silently returning a lower-precision result.

**Adaptive dispatch.** Workloads below `gpu_threshold` stay on the CPU, so the
GPU is only used when the O(n³) work justifies the dispatch. `ComputeConfig`
provides calibrated presets: `integrated_gpu()` (conservative, for shared-memory
parts) and `discrete_gpu()` (aggressive, for dedicated-memory parts).
Measurement on Apple M4: dispatch latency ≈ 0.8 ms, CPU rayon reaches
~115 GFLOPS sustained, GPU reaches ~85 GFLOPS at 1024³ — so an integrated GPU
loses at these sizes, which is exactly what the thresholds encode.

All GPU results are verified against the CPU reference in tests
(`cargo test --features gpu`), never silently trusted.

---

## Project Stats

| Metric | Value |
|--------|-------|
| Rust source files | 300+ |
| Lines of code | ~90,000 |
| Tests | **2529 passing** (default) / **2563** (`--features gpu`) ✅ |
| Test failures | **0** ✅ |
| Ignored tests | **0** ✅ |
| Clippy warnings | **0** (`-D warnings`), both with and without `gpu` ✅ |
| Build profile | Release with LTO fat, codegen-units=1 |
| Default dependency graph | **no `wgpu`** (GPU is fully optional) ✅ |
| Documentation files | 34 (blueprints, checklist, logs) |

---

## Scientific-Computing Toolkit (BLUE13, phases 35–41)

Beyond the domain models, SCIcoRS ships the general-purpose numerical machinery
a real research workflow needs. Everything below is implemented, tested, and
observable — not a stub.

| Capability | Module | What it provides |
|-----------|--------|------------------|
| **Sparse linear algebra** | `core::compute::sparse` | COO/CSR/CSC storage, SpMV, CG / MINRES / GMRES(m) / BiCGSTAB with explicit stop reasons, Jacobi / diagonal / ILU(0) preconditioners, matrix-free `LinearOperator` |
| **Least squares** | `core::compute::least_squares` | Householder QR with column pivoting, truncated one-sided Jacobi SVD, minimum-norm solutions, rank/condition diagnostics |
| **Analysis & UQ** | `analysis` | Finite-difference sensitivity, Levenberg–Marquardt calibration, Monte-Carlo / Latin-hypercube uncertainty, bounded Nelder–Mead optimization, factorial experiment design |
| **Mesh & fields** | `core::mesh` | Topology, named regions, quality validation, node/cell/face fields, conservation-checked mapping, adaptive refinement, STEP/STL bridge |
| **Checkpoints & resilience** | `runtime::checkpoint`, `runtime::execution` | Crash-safe atomic checkpoints, compatibility checks, six-state run outcomes, cancellation tokens, resource budgets, retry with backoff |
| **Self-describing datasets** | `postproc::dataset` | Versioned manifests, real unit conversion, chunked atomic writes, CRC-32, schema migration, CSV/JSON adapters |
| **Numerical validation** | `validation` | Sourced benchmarks with justified tolerances, convergence-order studies, registrable invariants, alignment-aware comparison, four-state reports |
| **Toolchain & bindings** | `cli`, `bindings` | `scico` CLI with stable exit codes, stable C ABI, plugin compatibility & diagnostics |

### Robust numerical solvers

Iterative Krylov solvers never report success on failure: hitting the iteration
cap returns `StopReason::MaxIterationsReached`, and a solve that breaks down or
stagnates says so. Wrong-algorithm/input combinations (e.g. CG on an asymmetric
matrix) return a specific error instead of silently switching methods.

### Credibility by construction

The validation toolkit refuses to fabricate confidence: a convergence order is
only reported when the data support one (insufficient or non-monotonic
data yields an explicit "not asymptotic"), a benchmark tolerance must carry a
stated reason, and un-executed checks are tracked separately from passed ones.
See `examples/validation_study.rs` for a real solver wired through the whole
workflow.

### Resilience by construction

Checkpoints are written through a temporary directory and an atomic, crash-safe
swap; a crash in the swap window still recovers the last valid checkpoint. A
resumed run reproduces an uninterrupted run exactly (verified end-to-end against
a real `SimEngine`).

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `serde` / `serde_json` | 1.x | Serialization |
| `toml` | 1.1 | Human-readable data storage |
| `rusqlite` | 0.40 | SQLite indexing & query |
| `num-complex` | 0.4 | Complex number support |
| `rayon` | 1.x | Data parallelism |
| `matrixmultiply` | 0.3 | Pure-Rust SIMD GEMM kernels |
| `wgpu` (optional) | 30.0.1 | GPU compute backend (`gpu` feature) |

---

## Development Phases (Checklist)

See the [full checklist](docs/checklist/CHECKLIST.MD) for detailed progress on all 33 phases.

**Phases 1-7 (Core Framework):** ✅ 100% Complete
- Core Model Kernel (Block/Port/Link/Diagram)
- Simulation Context & Time System
- General Numerical Solver System (ODE/DAE/Nonlinear)
- Scheduling & Execution Engine
- Workflow Orchestration (DAG)
- Event & Trigger System
- Discrete & Multi-Rate Systems

**Domain Phases (13-31):** All 19 domains fully implemented with computation, tests, and zero warnings.

**Integration Phases (32-34):** Coupling bus, post-processing, bindings — fully implemented.

**Scientific-Computing Phases (35-41, BLUE13):** Sparse linear algebra, sensitivity/
calibration/uncertainty/optimization, mesh & field workflows, checkpoints & elastic
execution, self-describing datasets, numerical validation, and the CLI/C-ABI/plugin
toolchain — fully implemented. See the [changelog](CHANGELOG.md) and
[development log](docs/log/log20261007-1.md) for details.

---

## Command-Line Tool

The `scico` binary drives the CLI library (stable, non-zero exit codes on error):

```sh
cargo build --release --bin scico

scico check   project.json      # validate project config, model and output paths
scico run     project.json      # one-shot bounded run → result dataset
scico status  out/              # query run status / resume point
scico results out/              # inspect a result dataset's manifest
scico cancel  out/ 0.5          # cancel a run, recording a resume point
scico resume  project.json      # resume a cancelled run
scico validate model diagram.json
scico validate plugin manifest.json 1.0
scico version
```

## Examples

```sh
cargo run --example validation_study   # real solver through benchmark + convergence + invariants
cargo run --example gpu_bench          # CPU vs GPU GEMM benchmark
cargo run --example compute_bench      # compute-primitive micro-benchmarks
```

---

## Data & Extensibility

- **Database:** TOML for human-readable data, SQLite for fast indexing and search
- **Libraries:** Materials, celestial bodies, fluids, sections, electrical, logic gates, chips, board-level, optics, acoustics, chemicals, biomolecules, cells, culture media, semiconductor process
- **Extensible:** Public/private libraries, custom data, import/export, versioning
- **LibraryManager** — full CRUD operations, TOML bulk import, category listing, keyword search

---

## Quick Start

```rust
use scico_rs::*;

// Create a diagram
let mut diagram = Diagram::new("my_simulation");

// Add blocks
let src = SineSource::new("src", 1.0, 60.0);  // 60 Hz sine wave
let gain = Gain::new("gain", 2.0);
let scope = Scope::new("scope", 1024);

diagram.add_block(Box::new(src));
diagram.add_block(Box::new(gain));
diagram.add_block(Box::new(scope));

// Connect blocks
diagram.connect("src:output", "gain:input").unwrap();
diagram.connect("gain:output", "scope:input").unwrap();

// Create simulation context and run
let ctx = SimContext::new(TimeConfig::new(0.0, 1.0, 1e-4));
let mut engine = SimEngine::new(diagram, ctx);
let summary = engine.run().unwrap();
println!("Completed {} steps in {} time units", summary.total_steps, summary.final_time);
```

---

## License

Dual-licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option. The SPDX expression is `MIT OR Apache-2.0`.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work, as defined in the Apache-2.0 license, shall be
dual-licensed as above, without any additional terms or conditions.

---

[中文文档](README.zh-CN.md) | [Changelog](CHANGELOG.md) | [Checklist](docs/checklist/CHECKLIST.MD) | [Roadmap](docs/blueprints/roadmap.md) | [Design Principles](docs/blueprints/principle.md) | [Dev Logs](docs/log/)
