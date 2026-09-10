//! Pipeline stages — decomposition of a workflow DAG into execution stages.
//!
//! A pipeline stage groups tasks that can (optionally) execute in parallel.
//! Stages are classified by their role in the simulation pipeline:
//! pre-process, solve, coupling synchronisation, or post-process.

use super::dag::WorkflowDAG;

/// Classification of a stage within the simulation pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStageType {
    /// Pre-processing tasks (input loading, initialisation, setup).
    PreProcess,
    /// Core solve tasks (numerical computation, block execution).
    Solve,
    /// Coupling synchronisation between solver domains.
    CouplingSync,
    /// Post-processing tasks (output logging, results export, cleanup).
    PostProcess,
}

/// A single stage within a workflow execution pipeline.
#[derive(Debug, Clone)]
pub struct WorkflowStage {
    /// The role this stage plays in the pipeline.
    pub stage_type: PipelineStageType,
    /// Ordered list of task identifiers assigned to this stage.
    pub task_ids: Vec<String>,
    /// If true, tasks within this stage may execute in parallel.
    pub parallel: bool,
    /// If true, a synchronisation barrier is required after this stage.
    pub barrier_required: bool,
}

/// Decompose a workflow DAG into pipeline stages.
///
/// Uses the DAG's `parallel_stages` analysis to produce a sequence
/// of pipeline stages, distributing topological levels across the
/// four pipeline phase types.
///
/// Returns an empty vector if the DAG contains cycles or has no tasks.
pub fn decompose_stages(dag: &WorkflowDAG) -> Vec<WorkflowStage> {
    let levels = dag.parallel_stages();
    if levels.is_empty() {
        return Vec::new();
    }

    let num_levels = levels.len();
    let mut stages: Vec<WorkflowStage> = Vec::new();

    for (i, level) in levels.into_iter().enumerate() {
        // Distribute levels across pipeline types heuristically
        let (stage_type, parallel, barrier) = classify_level(i, num_levels);
        stages.push(WorkflowStage {
            stage_type,
            task_ids: level,
            parallel,
            barrier_required: barrier,
        });
    }

    stages
}

/// Determine the pipeline classification for a given topological level index.
///
/// Heuristic distribution:
/// - First level  → PreProcess (parallel, barrier before solve)
/// - Middle levels → Solve (parallel between coupling points)
/// - If >= 4 levels, one middle level is CouplingSync
/// - Last level   → PostProcess (parallel, barrier at end)
fn classify_level(level_index: usize, total_levels: usize) -> (PipelineStageType, bool, bool) {
    if total_levels == 0 {
        return (PipelineStageType::Solve, false, false);
    }

    if total_levels == 1 {
        // Single stage: it's the core solve phase
        return (PipelineStageType::Solve, true, false);
    }

    let last = total_levels - 1;

    if level_index == 0 {
        // First stage: pre-process with barrier before solve
        (PipelineStageType::PreProcess, true, true)
    } else if level_index == last {
        // Last stage: post-process with barrier
        (PipelineStageType::PostProcess, true, true)
    } else if total_levels >= 4 && level_index == total_levels / 2 {
        // Midpoint: coupling synchronisation with barrier
        (PipelineStageType::CouplingSync, false, true)
    } else {
        // Core solve stage
        (PipelineStageType::Solve, true, false)
    }
}

/// Construct a single pipeline stage from a list of task IDs.
///
/// This is a convenience constructor for manual stage assembly
/// without going through automatic DAG decomposition.
pub fn stage_from_task_ids(
    stage_type: PipelineStageType,
    task_ids: &[String],
    parallel: bool,
) -> WorkflowStage {
    WorkflowStage {
        stage_type,
        task_ids: task_ids.to_vec(),
        parallel,
        barrier_required: parallel, // parallel stages default to requiring a barrier
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::workflow::dag::{EdgeDataType, WorkflowDAG, WorkflowEdge, WorkflowTask};

    fn linear_dag() -> WorkflowDAG {
        let mut dag = WorkflowDAG::new("linear");
        for i in 1..=3 {
            dag.add_task(WorkflowTask {
                id: format!("t{i}"),
                name: format!("Task {i}"),
                block_id: None,
                priority: i,
                estimated_cost: i as f64,
            })
            .unwrap();
        }
        dag.add_edge(WorkflowEdge {
            source: "t1".into(),
            destination: "t2".into(),
            data_type: EdgeDataType::Data,
            delay: None,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "t2".into(),
            destination: "t3".into(),
            data_type: EdgeDataType::Data,
            delay: None,
        })
        .unwrap();
        dag
    }

    fn parallel_dag() -> WorkflowDAG {
        let mut dag = WorkflowDAG::new("parallel");
        // Three independent tasks with no dependencies
        for i in 1..=3 {
            dag.add_task(WorkflowTask {
                id: format!("t{i}"),
                name: format!("Task {i}"),
                block_id: None,
                priority: i,
                estimated_cost: i as f64,
            })
            .unwrap();
        }
        dag
    }

    #[test]
    fn test_decompose_linear() {
        let dag = linear_dag();
        let stages = decompose_stages(&dag);
        // Linear 3-task DAG has 3 parallel stages (each task at its own depth)
        assert_eq!(stages.len(), 3);
        assert_eq!(stages[0].stage_type, PipelineStageType::PreProcess);
        assert_eq!(stages[1].stage_type, PipelineStageType::Solve);
        assert_eq!(stages[2].stage_type, PipelineStageType::PostProcess);
    }

    #[test]
    fn test_decompose_parallel() {
        let dag = parallel_dag();
        let stages = decompose_stages(&dag);
        // All three tasks are independent, so they form a single parallel stage
        assert_eq!(stages.len(), 1);
        assert_eq!(stages[0].task_ids.len(), 3);
        assert!(stages[0].parallel);
    }

    #[test]
    fn test_stage_creation() {
        let stage = stage_from_task_ids(PipelineStageType::Solve, &["a".into(), "b".into()], true);
        assert_eq!(stage.stage_type, PipelineStageType::Solve);
        assert_eq!(stage.task_ids.len(), 2);
        assert!(stage.parallel);
        assert!(stage.barrier_required);
    }

    #[test]
    fn test_empty_dag() {
        let dag = WorkflowDAG::new("empty");
        let stages = decompose_stages(&dag);
        assert!(stages.is_empty());
    }

    #[test]
    fn test_decompose_non_parallel_stage() {
        let stage = stage_from_task_ids(PipelineStageType::CouplingSync, &["sync1".into()], false);
        assert_eq!(stage.stage_type, PipelineStageType::CouplingSync);
        assert!(!stage.parallel);
        assert!(!stage.barrier_required);
    }

    #[test]
    fn test_decompose_large_dag() {
        // Create a DAG with enough levels to trigger CouplingSync
        let mut dag = WorkflowDAG::new("large");
        for i in 1..=8 {
            dag.add_task(WorkflowTask {
                id: format!("t{i}"),
                name: format!("Task {i}"),
                block_id: None,
                priority: i,
                estimated_cost: i as f64,
            })
            .unwrap();
        }
        // Chain: t1 -> t2 -> t3 -> t4 -> t5 -> t6 -> t7 -> t8
        for i in 1..8 {
            dag.add_edge(WorkflowEdge {
                source: format!("t{i}"),
                destination: format!("t{}", i + 1),
                data_type: EdgeDataType::Data,
                delay: None,
            })
            .unwrap();
        }

        let stages = decompose_stages(&dag);
        assert_eq!(stages.len(), 8);
        // Level 4 (total=8, 8/2=4) should be CouplingSync
        assert_eq!(stages[4].stage_type, PipelineStageType::CouplingSync);
    }

    #[test]
    fn test_cyclical_dag_returns_empty() {
        let mut dag = WorkflowDAG::new("cyclic");
        dag.add_task(WorkflowTask {
            id: "a".into(),
            name: "A".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "b".into(),
            name: "B".into(),
            block_id: None,
            priority: 2,
            estimated_cost: 2.0,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "a".into(),
            destination: "b".into(),
            data_type: EdgeDataType::Control,
            delay: None,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "b".into(),
            destination: "a".into(),
            data_type: EdgeDataType::Control,
            delay: None,
        })
        .unwrap();

        let stages = decompose_stages(&dag);
        assert!(stages.is_empty());
    }

    #[test]
    fn test_single_task_dag_is_solve() {
        let mut dag = WorkflowDAG::new("single");
        dag.add_task(WorkflowTask {
            id: "only".into(),
            name: "Only".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();

        let stages = decompose_stages(&dag);
        assert_eq!(stages.len(), 1);
        assert_eq!(stages[0].stage_type, PipelineStageType::Solve);
    }
}
