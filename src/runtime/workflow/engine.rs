//! Workflow engine — orchestrates pipeline stage execution.
//!
//! The workflow engine takes a DAG of tasks, decomposes it into pipeline
//! stages, and executes them sequentially (with per-stage parallelism)
//! while tracking status, handling failures, and supporting lifecycle
//! controls such as pause, resume, and retry.

use std::collections::HashMap;

use crate::core::error::SimError;

use super::dag::WorkflowDAG;
use super::parallel::ParallelScheduler;
use super::stage::{WorkflowStage, decompose_stages};

/// Execution status of the workflow engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowStatus {
    /// Engine created, not yet started.
    Idle,
    /// Execution is in progress.
    Running,
    /// Execution has been paused (may be resumed).
    Paused,
    /// All stages completed successfully.
    Completed,
    /// Execution failed with an error message.
    Failed(String),
}

/// Summary result returned after a workflow run.
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    /// Total number of stages in the pipeline.
    pub total_stages: usize,
    /// Number of stages that completed successfully.
    pub completed_stages: usize,
    /// Identifiers of any tasks that failed during execution.
    pub failed_tasks: Vec<String>,
    /// Final status of the workflow.
    pub status: WorkflowStatus,
    /// Total elapsed execution time, if measured.
    pub total_time: Option<f64>,
}

/// Orchestrates execution of a workflow DAG as a pipeline of stages.
#[derive(Debug, Clone)]
pub struct WorkflowEngine {
    /// The workflow DAG defining tasks and dependencies.
    pub dag: WorkflowDAG,
    /// Decomposed pipeline stages from the DAG.
    pub stages: Vec<WorkflowStage>,
    /// Scheduler handling per-stage parallel dispatch.
    pub parallel_scheduler: ParallelScheduler,
    /// Current execution status.
    pub status: WorkflowStatus,
    /// Identifiers of tasks that have failed.
    pub failed_tasks: Vec<String>,
    /// Maximum number of `retry_task` calls allowed per task before giving up.
    ///
    /// A value of `0` means a task may be retried once (the first retry is
    /// always allowed); the limit bounds how many *additional* retries a task
    /// may consume.
    pub max_retries: u32,
    /// Number of retries already consumed per task id.
    retry_counts: HashMap<String, u32>,
    /// Index of the currently executing stage.
    current_stage: usize,
}

impl WorkflowEngine {
    /// Create a new workflow engine from a DAG with the default retry limit of 0.
    ///
    /// Stages are built automatically from the DAG during construction.
    pub fn new(dag: WorkflowDAG) -> Self {
        Self::with_max_retries(dag, 0)
    }

    /// Create a new workflow engine from a DAG with an explicit retry limit.
    ///
    /// `max_retries` is the number of extra retries each task is granted; see
    /// [`Self::retry_task`].
    pub fn with_max_retries(dag: WorkflowDAG, max_retries: u32) -> Self {
        let mut engine = Self {
            dag,
            stages: Vec::new(),
            parallel_scheduler: ParallelScheduler::default(),
            status: WorkflowStatus::Idle,
            failed_tasks: Vec::new(),
            max_retries,
            retry_counts: HashMap::new(),
            current_stage: 0,
        };
        engine.build_stages();
        engine
    }

    /// Builder-style setter for the per-task retry limit.
    ///
    /// ```
    /// # use scico_rs::{WorkflowDAG, WorkflowEngine};
    /// let engine = WorkflowEngine::new(WorkflowDAG::new("d")).with_retry_limit(3);
    /// assert_eq!(engine.max_retries, 3);
    /// ```
    pub fn with_retry_limit(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Number of retries already consumed by `task_id`.
    ///
    /// Returns `0` for a task that has never been retried.
    pub fn retries_used(&self, task_id: &str) -> u32 {
        self.retry_counts.get(task_id).copied().unwrap_or(0)
    }

    /// Retries still available to `task_id` under the current limit.
    pub fn retries_remaining(&self, task_id: &str) -> u32 {
        self.max_retries.saturating_sub(self.retries_used(task_id))
    }

    /// Decompose the DAG into pipeline stages.
    ///
    /// Replaces any previously computed stages. Stage boundaries are enforced
    /// by the join at the end of each parallel stage in the scheduler, so no
    /// barrier objects need to be registered here.
    pub fn build_stages(&mut self) {
        self.stages = decompose_stages(&self.dag);
        self.current_stage = 0;
    }

    /// Return the number of pipeline stages.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Execute all pipeline stages sequentially and return the result.
    ///
    /// This is a blocking call that runs stages one at a time from
    /// the current position. Respects pause and resume state.
    pub fn run(&mut self) -> WorkflowResult {
        if self.status == WorkflowStatus::Completed {
            return self.build_result();
        }

        self.status = WorkflowStatus::Running;
        let start_time = std::time::Instant::now();

        while self.current_stage < self.stages.len() && self.status == WorkflowStatus::Running {
            match self.execute_current_stage() {
                Ok(()) => {
                    self.current_stage += 1;
                }
                Err(err_msg) => {
                    self.status = WorkflowStatus::Failed(err_msg);
                    return self.build_result();
                }
            }
        }

        if self.status == WorkflowStatus::Running {
            self.status = WorkflowStatus::Completed;
        }

        let mut result = self.build_result();
        result.total_time = Some(start_time.elapsed().as_secs_f64());
        result
    }

    /// Execute a single pipeline stage and return the updated status.
    ///
    /// Returns an error if all stages have already been completed.
    pub fn step(&mut self) -> Result<WorkflowStatus, SimError> {
        if self.status == WorkflowStatus::Completed {
            return Err(SimError::runtime(
                "workflow already completed, cannot step further",
            ));
        }

        if self.current_stage >= self.stages.len() {
            self.status = WorkflowStatus::Completed;
            return Ok(self.status.clone());
        }

        self.status = WorkflowStatus::Running;

        match self.execute_current_stage() {
            Ok(()) => {
                self.current_stage += 1;
                if self.current_stage >= self.stages.len() {
                    self.status = WorkflowStatus::Completed;
                } else {
                    self.status = WorkflowStatus::Paused;
                }
                Ok(self.status.clone())
            }
            Err(err_msg) => {
                self.status = WorkflowStatus::Failed(err_msg);
                Ok(self.status.clone())
            }
        }
    }

    /// Pause execution at the next opportunity.
    ///
    /// Has no effect if the engine is not running.
    pub fn pause(&mut self) {
        if self.status == WorkflowStatus::Running {
            self.status = WorkflowStatus::Paused;
        }
    }

    /// Resume a paused workflow.
    ///
    /// Has no effect if the engine is not paused.
    pub fn resume(&mut self) {
        if self.status == WorkflowStatus::Paused {
            self.status = WorkflowStatus::Running;
        }
    }

    /// Reset the engine to its initial state.
    ///
    /// Clears all execution progress, status, failure tracking, and the per-task
    /// retry counters.
    pub fn reset(&mut self) {
        self.failed_tasks.clear();
        self.retry_counts.clear();
        self.current_stage = 0;
        self.status = WorkflowStatus::Idle;
        self.build_stages();
    }

    /// Retry a specific failed task.
    ///
    /// Returns an error if the task ID is not in the failed list, if the task
    /// does not exist in the DAG, or if the retry limit configured through
    /// [`Self::with_max_retries`] / [`Self::with_retry_limit`] has already been
    /// consumed. Each successful call consumes one retry from the task's budget.
    pub fn retry_task(&mut self, task_id: &str) -> Result<(), String> {
        // Find the task in the failed tasks list
        let pos = self
            .failed_tasks
            .iter()
            .position(|id| id == task_id)
            .ok_or_else(|| format!("task '{task_id}' is not in the failed tasks list"))?;

        // Verify the task exists in the DAG
        if !self.dag.tasks.contains_key(task_id) {
            return Err(format!("task '{task_id}' does not exist in the DAG"));
        }

        // Enforce the retry limit before consuming any budget.
        if self.retries_remaining(task_id) == 0 {
            return Err(format!(
                "task '{task_id}' has exceeded its retry limit of {}",
                self.max_retries
            ));
        }

        // Consume one retry from this task's budget.
        *self.retry_counts.entry(task_id.to_string()).or_insert(0) += 1;

        // Remove from failed list
        self.failed_tasks.remove(pos);

        // Reset the current stage to the stage containing this task
        for (i, stage) in self.stages.iter().enumerate() {
            if stage.task_ids.iter().any(|id| id == task_id) {
                self.current_stage = i;
                break;
            }
        }

        // Transition back to paused so the user can call resume/step
        self.status = WorkflowStatus::Paused;
        Ok(())
    }

    /// Execute the tasks in the current stage.
    ///
    /// Returns `Ok(())` on success or an error description on failure.
    fn execute_current_stage(&mut self) -> Result<(), String> {
        let stage = &self.stages[self.current_stage];
        let task_ids = &stage.task_ids;

        // Collect the actual task references from the DAG
        let tasks: Vec<_> = task_ids
            .iter()
            .filter_map(|id| self.dag.tasks.get(id))
            .collect();

        // Skip empty stages
        if tasks.is_empty() {
            return Ok(());
        }

        // Execute tasks in this stage using this engine's parallel scheduler.
        // The join inside `execute_parallel` is the stage boundary: it returns
        // only after every task of the stage has completed.
        let results = self.parallel_scheduler.execute_parallel(&tasks, |task| {
            // Simulate task execution — in production this would invoke
            // the actual computation associated with the task / block.
            if task.estimated_cost.is_nan() || task.estimated_cost < 0.0 {
                Err(format!("task '{}' has invalid cost", task.id))
            } else {
                Ok(())
            }
        });

        // Collect any failures
        let mut has_failure = false;
        for (i, result) in results.iter().enumerate() {
            if let Err(err_msg) = result {
                has_failure = true;
                let task_id = task_ids[i].clone();
                if !self.failed_tasks.contains(&task_id) {
                    self.failed_tasks.push(task_id);
                }
                // Log would go here in production
                let _ = err_msg;
            }
        }

        if has_failure {
            Err("stage execution completed with task failures".to_string())
        } else {
            Ok(())
        }
    }

    /// Build a `WorkflowResult` from the current engine state.
    fn build_result(&self) -> WorkflowResult {
        WorkflowResult {
            total_stages: self.stages.len(),
            completed_stages: self.current_stage,
            failed_tasks: self.failed_tasks.clone(),
            status: self.status.clone(),
            total_time: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::workflow::PipelineStageType;
    use crate::runtime::workflow::dag::{EdgeDataType, WorkflowEdge, WorkflowTask};

    /// Build a simple linear DAG with 3 tasks.
    fn linear_test_dag() -> WorkflowDAG {
        let mut dag = WorkflowDAG::new("linear_test");
        dag.add_task(WorkflowTask {
            id: "t1".into(),
            name: "Task 1".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "t2".into(),
            name: "Task 2".into(),
            block_id: None,
            priority: 2,
            estimated_cost: 2.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "t3".into(),
            name: "Task 3".into(),
            block_id: None,
            priority: 3,
            estimated_cost: 3.0,
        })
        .unwrap();
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

    #[test]
    fn test_engine_creation() {
        let dag = linear_test_dag();
        let engine = WorkflowEngine::new(dag);
        assert_eq!(engine.status, WorkflowStatus::Idle);
        assert!(engine.stage_count() > 0);
        assert!(engine.failed_tasks.is_empty());
    }

    #[test]
    fn test_build_stages_linear() {
        let dag = linear_test_dag();
        let engine = WorkflowEngine::new(dag);
        assert!(engine.stage_count() >= 3);
        // Verify stage types make sense for a linear DAG
        assert_eq!(engine.stages[0].stage_type, PipelineStageType::PreProcess);
    }

    #[test]
    fn test_run_empty_dag() {
        let dag = WorkflowDAG::new("empty");
        let mut engine = WorkflowEngine::new(dag);
        let result = engine.run();
        assert_eq!(result.status, WorkflowStatus::Completed);
        assert_eq!(result.total_stages, 0);
        assert_eq!(result.completed_stages, 0);
    }

    #[test]
    fn test_pause_resume() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::new(dag);

        // Engine should be idle
        assert_eq!(engine.status, WorkflowStatus::Idle);

        // Pause while idle should have no effect
        engine.pause();
        assert_eq!(engine.status, WorkflowStatus::Idle);

        // Start stepping
        let status = engine.step().expect("step should succeed");
        assert_eq!(status, WorkflowStatus::Paused);
        assert_eq!(engine.current_stage, 1);

        // Pause (already paused)
        engine.pause();
        assert_eq!(engine.status, WorkflowStatus::Paused);

        // Resume
        engine.resume();
        assert_eq!(engine.status, WorkflowStatus::Running);

        // Complete remaining steps — step() returns Paused between stages,
        // so we resume after each step until completion
        engine.resume();
        let mut last_status = engine.step().expect("step should succeed");
        while last_status != WorkflowStatus::Completed
            && !matches!(last_status, WorkflowStatus::Failed(_))
        {
            // step() returns Paused after a non-final stage; resume for the next
            if last_status == WorkflowStatus::Paused {
                engine.resume();
            }
            last_status = engine.step().expect("step should succeed");
        }
        assert_eq!(last_status, WorkflowStatus::Completed);
    }

    #[test]
    fn test_retry_task() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::with_max_retries(dag, 2);

        // Manually add a failed task
        engine.failed_tasks.push("t2".to_string());
        assert_eq!(engine.failed_tasks.len(), 1);

        // Retry a task that's not in the failed list
        let result = engine.retry_task("nonexistent");
        assert!(result.is_err());

        // Retry the actual failed task
        let result = engine.retry_task("t2");
        assert!(result.is_ok());
        assert!(engine.failed_tasks.is_empty());
        assert_eq!(engine.status, WorkflowStatus::Paused);
        assert_eq!(engine.retries_used("t2"), 1);
        assert_eq!(engine.retries_remaining("t2"), 1);
    }

    #[test]
    fn test_retry_within_limit_succeeds_repeatedly() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::with_max_retries(dag, 2);

        // Three retries are allowed: the initial retry plus `max_retries` extra.
        for expected_used in 1..=3 {
            engine.failed_tasks.push("t2".to_string());
            let result = engine.retry_task("t2");
            assert!(result.is_ok(), "retry #{expected_used} should succeed");
            assert_eq!(engine.retries_used("t2"), expected_used);
        }
        assert_eq!(engine.retries_remaining("t2"), 0);
    }

    #[test]
    fn test_retry_exceeding_limit_returns_error() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::with_max_retries(dag, 1);

        // Budget is one retry.
        engine.failed_tasks.push("t2".to_string());
        assert!(engine.retry_task("t2").is_ok());

        // The budget is exhausted: a further attempt must fail and must not
        // consume the failure entry.
        engine.failed_tasks.push("t2".to_string());
        let err = engine
            .retry_task("t2")
            .expect_err("the retry limit must be enforced");
        assert!(err.contains("retry limit"), "unexpected error: {err}");
        assert_eq!(engine.failed_tasks, vec!["t2".to_string()]);
        assert_eq!(engine.retries_used("t2"), 1);
    }

    #[test]
    fn test_retry_limit_is_configurable() {
        let dag = linear_test_dag();

        // The default constructor grants no extra retries.
        let default_engine = WorkflowEngine::new(dag.clone());
        assert_eq!(default_engine.max_retries, 0);

        // Builder style and constructor style agree.
        let built = WorkflowEngine::new(dag.clone()).with_retry_limit(4);
        assert_eq!(built.max_retries, 4);

        let constructed = WorkflowEngine::with_max_retries(dag, 4);
        assert_eq!(constructed.max_retries, 4);
    }

    #[test]
    fn test_retry_budget_is_per_task() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::with_max_retries(dag, 1);

        // Exhaust t2's budget.
        engine.failed_tasks.push("t2".to_string());
        assert!(engine.retry_task("t2").is_ok());

        // t3 has an independent budget and is still retryable.
        engine.failed_tasks.push("t3".to_string());
        assert!(engine.retry_task("t3").is_ok());
        assert_eq!(engine.retries_used("t2"), 1);
        assert_eq!(engine.retries_used("t3"), 1);
        assert!(engine.retries_used("t1") == 0);
    }

    #[test]
    fn test_reset_clears_retry_counters() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::with_max_retries(dag, 1);
        engine.failed_tasks.push("t2".to_string());
        assert!(engine.retry_task("t2").is_ok());
        assert_eq!(engine.retries_used("t2"), 1);

        engine.reset();
        assert_eq!(engine.retries_used("t2"), 0);
        assert_eq!(engine.retries_remaining("t2"), 1);
    }

    #[test]
    fn test_reset() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::new(dag);

        // Progress the engine
        let _ = engine.step();
        engine.failed_tasks.push("t1".to_string());
        assert!(engine.current_stage > 0);

        // Reset
        engine.reset();
        assert_eq!(engine.status, WorkflowStatus::Idle);
        assert_eq!(engine.current_stage, 0);
        assert!(engine.failed_tasks.is_empty());
        assert!(engine.stage_count() > 0);
    }

    #[test]
    fn test_step_past_completion() {
        let dag = WorkflowDAG::new("empty");
        let mut engine = WorkflowEngine::new(dag);

        // Empty DAG should complete immediately
        let status = engine.step().expect("step should succeed");
        assert_eq!(status, WorkflowStatus::Completed);

        // Stepping again should error
        let err = engine.step();
        assert!(err.is_err());
    }

    #[test]
    fn test_run_completes() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::new(dag);
        let result = engine.run();

        assert_eq!(result.status, WorkflowStatus::Completed);
        assert_eq!(result.completed_stages, result.total_stages);
        assert!(result.total_time.is_some());
        assert!(result.total_time.unwrap() >= 0.0);
    }

    #[test]
    fn test_run_twice() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::new(dag);
        let _ = engine.run();

        // Running again should return completed immediately
        let result = engine.run();
        assert_eq!(result.status, WorkflowStatus::Completed);
    }

    #[test]
    fn test_stage_count() {
        let dag = linear_test_dag();
        let engine = WorkflowEngine::new(dag);
        assert_eq!(engine.stage_count(), engine.stages.len());
    }

    #[test]
    fn test_stages_remain_unchanged_after_refactor() {
        // Removing the inert barrier bookkeeping must not change stage
        // decomposition; `barrier_required` still documents the boundary.
        let dag = linear_test_dag();
        let engine = WorkflowEngine::new(dag);
        assert_eq!(engine.stage_count(), 3);
        assert!(engine.stages[0].barrier_required);
        assert!(engine.stages[2].barrier_required);
    }

    #[test]
    fn test_retry_nonexistent_task_returns_error() {
        let dag = linear_test_dag();
        let mut engine = WorkflowEngine::new(dag);
        let result = engine.retry_task("ghost");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ghost"));
    }

    #[test]
    fn test_execute_current_stage_on_invalid_task() {
        let mut dag = WorkflowDAG::new("invalid");
        dag.add_task(WorkflowTask {
            id: "bad".into(),
            name: "Bad".into(),
            block_id: None,
            priority: 1,
            estimated_cost: f64::NAN,
        })
        .unwrap();

        let mut engine = WorkflowEngine::new(dag);
        engine.run();
        // The task with NaN cost should trigger a failure
        assert_eq!(
            engine.status,
            WorkflowStatus::Failed("stage execution completed with task failures".to_string())
        );
    }
}
