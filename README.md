# SCIcoRS — Unified Simulation Kernel for All Humanity

[![中文文档](README.zh-CN.md)](README.zh-CN.md) | [Checklist](docs/checklist/CHECKLIST.MD) | [Roadmap (Blueprint)](docs/blueprints/roadmap.md) | [Design Principles](docs/blueprints/principle.md)

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
│  bindings/   — Python API, Plugin system, Data I/O (STL/STEP/Mesh) │
│  postproc/   — Recording, Visualization, Reporting, Batch, HIL    │
│  coupling/   — Multi-Physics Coupling Bus, Cross-Scale Mapping    │
│  domains/    — 19 Domain-Specific Simulation Modules              │
│  blocks/     — Standard Block Library (Sources, Math, Logic...)   │
│  runtime/    — Context, Engine, Solvers, Scheduler, Events, State │
│  core/       — Block, Port, Link, Diagram, Types, Coord, Units   │
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
- **Plugin** system — block, solver, and post-processor registries with manifest loading
- **Data I/O** — STEP/STL mesh import/export, general mesh formats
- **Platform** — OS detection, normalized paths, cloud/distributed runner

---

## Compute Platform

All domain modules delegate mathematics to the unified `core::compute` module:

| Operation | Implementations |
|-----------|----------------|
| **Matrix** | Multiply, transpose, determinant, inverse, LU/Cholesky decomposition |
| **Vector** | Dot, cross, norm, normalization, linear/spline interpolation |
| **FFT** | Base-2 Cooley-Tukey FFT for spectral analysis |
| **Integration** | Trapezoidal, Simpson, Gauss-Legendre quadrature |
| **Eigenvalues** | Jacobi method, subspace iteration |
| **Parallel** | `rayon`-based parallelism for compute-intensive loops |

This eliminated 5 copies of Gaussian elimination that existed across domain modules.

---

## Project Stats

| Metric | Value |
|--------|-------|
| Rust source files | 269 |
| Lines of code | ~70,900 |
| Tests | **1811 passing** ✅ |
| Test failures | **0** ✅ |
| Ignored tests | **0** ✅ |
| Clippy warnings | **0** (`-D warnings`) ✅ |
| Build profile | Release with LTO fat, codegen-units=1 |
| Documentation files | 32 (blueprints, checklist, logs) |

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `serde` / `serde_json` | 1.x | Serialization |
| `toml` | 1.1 | Human-readable data storage |
| `rusqlite` | 0.40 | SQLite indexing & query |
| `num-complex` | 0.4 | Complex number support |
| `rayon` | 1.x | Data parallelism |

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

Dual-licensed under either:

- [MIT License](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT)
- [Apache License, Version 2.0](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0)

at your option.

---

[中文文档](README.zh-CN.md) | [Checklist](docs/checklist/CHECKLIST.MD) | [Roadmap](docs/blueprints/roadmap.md) | [Design Principles](docs/blueprints/principle.md) | [Dev Logs](docs/log/)
