//! Unified scheduling layer (Phase 4-7)
//!
//! Covers all simulation orchestration concerns:
//!
//! Phase 4 — Execution engine primitives:
//!   Topological ordering, signal flow analysis
//!   Continuous/discrete/event/multi-rate hybrid scheduling
//!   Port signal propagation, caching, sync
//!   Block execution lifecycle (Init → Output → Deriv → Update → Event → Terminate)
//!   Multi-rate task scheduling, clock domain isolation
//!
//! Phase 5 — Computation workflow orchestration:
//!   DAG modeling (nodes = tasks, edges = data/signal/event deps)
//!   Parallel + serial hybrid scheduling with barrier sync
//!   Stage-based pipeline (preprocess → solve → sync → postprocess)
//!
//! Phase 6 — Event & trigger system:
//!   Time-sorted event queue, precise triggering
//!   Zero-crossing, rising/falling edge detection
//!   External/interrupt/conditional triggers
//!
//! Phase 7 — Multi-rate & discrete systems:
//!   Multi-rate sampling, sample-and-hold, resampling
//!   Digital filters, discrete integration
//!   Sync/async clocks, phase offset
//!   Embedded logic, counters, timers

pub mod hybrid;
pub mod multirate;
pub mod signal_flow;
pub mod signal_prop;
pub mod topo;
pub mod traits;

pub use hybrid::{
    BlockTaskType, ScheduleConfig, SchedulePhase, build_schedule, classify_blocks,
    execute_deriv_phase, execute_event_detection, execute_output_phase, execute_update_phase,
};
pub use multirate::{ClockDomain, MultiRateScheduler, build_multirate_schedule};
pub use signal_flow::{SignalFlowGraph, analyze_signal_flow, compute_propagation_layers};
pub use signal_prop::{SignalCache, extract_outputs, propagate_signals, update_inputs};
pub use topo::{CycleInfo, DiGraph, detect_cycles, has_cycles, topological_sort};
pub use traits::{ScheduleContext, ScheduleStepResult, Scheduler, SequentialScheduler};
