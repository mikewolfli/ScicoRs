//! SCIcoRS — Unified Simulation Kernel for All Humanity
//!
//! A universal simulation kernel designed to unify all engineering and
//! scientific simulation scenarios across every discipline, scale, and field.
//!
//! Architecture (7 layers):
//!   core/      — Data model layer: Block, Port, Link, Diagram, Types
//!   runtime/   — Simulation runtime: Context, State, Engine, Solvers
//!   blocks/    — Standard block library: sources, math, logic, sinks
//!   domains/   — Domain-specific simulations: circuits, fluid, quantum, etc.
//!   coupling/  — Multi-physics coupling bus
//!   postproc/  — Post-processing & visualization
//!   bindings/  — Python bindings & plugin system

pub mod core;
pub mod runtime;

// Future module placeholders (declared for architecture completeness)
pub mod bindings;
pub mod blocks;
pub mod coupling;
pub mod db;
pub mod domains;
pub mod postproc;

// Re-export commonly used types at the crate root for convenience.
pub use core::block::{Block, BlockError, SimpleBlock};
pub use core::diagram::Diagram;
pub use core::error::{ErrorCode, SimError};
pub use core::link::Link;
pub use core::param::{Parameter, ParameterSet};
pub use core::port::Port;
pub use core::types::{PortDirection, Scalar, SignalType, SignalValue, Time};
pub use runtime::context::{LogLevel, SimContext, SimLifecycle, SimRunMode, TimeConfig};
pub use runtime::discrete::{
    Counter, CounterDirection, DiscreteIntegrator, EdgeDetector, FIRFilter, IIRFilter,
    MovingAverage, RSFlipFlop, SampleHold, Timer,
};
pub use runtime::engine::{SimEngine, SimStepResult, SimSummary};
pub use runtime::event::{Event, EventQueue, EventTriggerManager, EventType, ZeroCrossingDetector};
pub use runtime::state::{ContinuousState, DiscreteState, SimStateManager, StateSnapshot};
pub use runtime::workflow::{WorkflowDAG, WorkflowEdge, WorkflowEngine, WorkflowTask};
