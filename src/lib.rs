//! SCIcoRS — Unified Simulation Kernel for All Humanity
//!
//! A universal simulation kernel designed to unify all engineering and
//! scientific simulation scenarios across every discipline, scale, and field.
//! It provides a single architecture for modeling, simulation, and data
//! management, enabling seamless integration from the smallest chip to the
//! largest cosmic system.
//!
//! ## Architecture
//!
//! - **core** — Block/Port/Link/Diagram model kernel
//! - **solver** — ODE, DAE, stiff, nonlinear, sparse solvers
//! - **engine** — Scheduling and execution engine
//! - **event** — Event and trigger system
//! - **discrete** — Discrete and multi-rate systems
//! - **algebra** — Algebraic loop detection and numerical stability
//! - **math** — Math, signal, and control library
//! - **coordinate** — Unified coordinate system
//! - **unit** — Unified dimension and unit system
//! - **database** — TOML + SQLite database system
//! - **modules** — Domain-specific simulation modules
//! - **physics** — Physics simulation building blocks
//! - **coupling** — Multi-physics coupling bus
//! - **visualization** — Data recording and visualization
//! - **platform** — Cross-platform and system integration
//! - **scripting** — Embedded scripting ecosystem
//! - **ext** — Extension and plugin system
//! - **utils** — General-purpose utilities

pub mod core;
pub mod solver;
pub mod engine;
pub mod event;
pub mod discrete;
pub mod algebra;
pub mod math;
pub mod coordinate;
pub mod unit;
pub mod database;
pub mod modules;
pub mod physics;
pub mod coupling;
pub mod visualization;
pub mod platform;
pub mod scripting;
pub mod ext;
pub mod utils;

// Re-export commonly used types at the crate root for convenience.
pub use core::types::{Scalar, Time, SignalValue, SignalType, PortDirection};
pub use core::block::{Block, BlockError, SimpleBlock};
pub use core::diagram::Diagram;
pub use core::error::{ErrorCode, SimError};
pub use core::param::{Parameter, ParameterSet};
pub use core::port::Port;
pub use core::link::Link;
