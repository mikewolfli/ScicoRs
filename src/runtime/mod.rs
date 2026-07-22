//! Simulation Runtime Layer
//!
//! The runtime layer drives simulation execution on top of the core data model.
//! It manages time, state, lifecycle, and the execution engine — the "verbs"
//! that make simulation happen.
//!
//! - **Context**: centralized simulation context (time, mode, lifecycle, logs)
//! - **State**: unified continuous + discrete state management
//! - **Engine**: simulation execution engine orchestrator
//! - **Event**: time-sorted event queue, trigger system, zero-crossing detection
//! - **Workflow** (Phase 5): DAG-based computation workflow orchestration
//! - **Discrete** (Phase 7): digital filters, integrators, counters, PLC logic
//! - **Solver** (Phase 3): numerical ODE/DAE solvers
//! - **Scheduler** (Phase 4): unified scheduling layer
//! - **Algebraic** (Phase 8): algebraic loop detection

pub mod context;
pub mod discrete;
pub mod engine;
pub mod event;
pub mod state;
pub mod workflow;

pub use context::{LogEntry, LogLevel, SimContext, SimLifecycle, SimRunMode, TimeConfig};
pub use discrete::{
    Counter, CounterDirection, DFlipFlop, DiscreteIntegrator, EdgeDetector, FIRFilter, HazardType,
    IIRFilter, IntegrationMethod, MovingAverage, RSFlipFlop, SampleHold, Timer, TimingAnalysis,
    and_gate, linear_interpolate, nand_gate, nor_gate, not_gate, or_gate, resample, xor_gate,
};
pub use engine::{SimEngine, SimStepResult, SimSummary};
pub use event::{
    EdgeType, Event, EventQueue, EventStatistics, EventTriggerManager, EventType, TriggerCondition,
    ZeroCrossingDetector,
};
pub use state::{ContinuousState, DiscreteState, SimStateManager, StateSnapshot};
pub use workflow::{
    BarrierSync, EdgeDataType, ParallelScheduler, PipelineStageType, WorkflowDAG, WorkflowEdge,
    WorkflowEngine, WorkflowResult, WorkflowStage, WorkflowStatus, WorkflowTask, decompose_stages,
    stage_from_task_ids,
};

// Future module placeholders (declared for architecture completeness)
pub mod algebraic;
pub mod scheduler;
pub mod solver;
