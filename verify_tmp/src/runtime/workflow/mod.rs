//! Workflow module — DAG modelling, pipeline stages, parallel scheduling, and engine.
//!
//! This module provides the orchestration layer for simulation workflows:
//!
//! - **dag**: Workflow DAG with topological sort, critical path, and validation.
//! - **stage**: Pipeline stage decomposition (pre-process → solve → sync → post-process).
//! - **parallel**: Parallel scheduler with barrier synchronisation.
//! - **engine**: Workflow engine orchestrating full pipeline execution.

pub mod dag;
pub mod engine;
pub mod parallel;
pub mod stage;

// ── dag ──
pub use dag::{
    EdgeDataType, WorkflowDAG, WorkflowEdge, WorkflowTask,
};

// ── stage ──
pub use stage::{
    decompose_stages, stage_from_task_ids, PipelineStageType, WorkflowStage,
};

// ── parallel ──
pub use parallel::{BarrierSync, ParallelScheduler};

// ── engine ──
pub use engine::{WorkflowEngine, WorkflowResult, WorkflowStatus};
