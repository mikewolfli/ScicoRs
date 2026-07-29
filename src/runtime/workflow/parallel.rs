//! Parallel scheduler with barrier synchronisation using rayon.
//!
//! Provides scheduling primitives for parallel task execution with
//! optional barrier synchronisation between stages. Uses rayon's
//! work-stealing thread pool for automatic load balancing.

use std::time::Duration;

use super::stage::WorkflowStage;

/// Describes a synchronisation barrier at the end of a pipeline stage.
#[derive(Debug, Clone)]
pub struct BarrierSync {
    /// Identifier of the stage this barrier belongs to.
    pub stage_id: String,
    /// Number of participants that must reach this barrier.
    pub expected_count: usize,
    /// Optional timeout — if `Some`, the barrier may fail after this duration.
    pub timeout: Option<Duration>,
}

/// A scheduler that distributes work across threads with barrier support.
#[derive(Debug, Clone)]
pub struct ParallelScheduler {
    /// Maximum number of worker threads (logical cores).
    pub max_threads: usize,
    /// Registered synchronisation barriers keyed by stage.
    pub barriers: Vec<BarrierSync>,
    // Number of threads actively in a barrier at each stage.
    barrier_counts: Vec<usize>,
}

impl ParallelScheduler {
    /// Create a new parallel scheduler with the given thread limit.
    pub fn new(max_threads: usize) -> Self {
        if max_threads > 1 {
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(max_threads)
                .build_global();
        }
        Self {
            max_threads: max_threads.max(1),
            barriers: Vec::new(),
            barrier_counts: Vec::new(),
        }
    }

    /// Compute chunk sizes for distributing tasks across workers.
    ///
    /// Returns a vector of chunk sizes (number of tasks per worker)
    /// based on the stage's parallelism flag and the available threads.
    /// Serial stages return a single chunk containing all tasks.
    pub fn schedule_stage(&self, stage: &WorkflowStage) -> Vec<usize> {
        if stage.task_ids.is_empty() {
            return Vec::new();
        }

        if !stage.parallel || self.max_threads <= 1 {
            // Serial execution — one chunk with all tasks
            return vec![stage.task_ids.len()];
        }

        let num_workers = self.max_threads.min(stage.task_ids.len());
        let base = stage.task_ids.len() / num_workers;
        let remainder = stage.task_ids.len() % num_workers;

        let mut chunks: Vec<usize> = Vec::with_capacity(num_workers);
        for i in 0..num_workers {
            let chunk_size = base + if i < remainder { 1 } else { 0 };
            chunks.push(chunk_size);
        }
        chunks
    }

    /// Execute tasks in parallel using rayon's work-stealing thread pool.
    ///
    /// Tasks are dispatched across available threads automatically.
    /// Returns results in the same order as the input tasks.
    pub fn execute_parallel<T, F>(tasks: &[T], f: F) -> Vec<Result<(), String>>
    where
        T: Send + Sync,
        F: Fn(&T) -> Result<(), String> + Send + Sync,
    {
        use rayon::prelude::*;
        tasks.par_iter().map(f).collect()
    }

    /// Execute tasks with a scope-based parallel for loop.
    /// Better suited for CPU-bound numerical work where each task is independent.
    pub fn execute_parallel_scoped<T, F>(tasks: &[T], f: F)
    where
        T: Send + Sync,
        F: Fn(&T) + Send + Sync,
    {
        use rayon::prelude::*;
        tasks.par_iter().for_each(f);
    }

    /// Register a synchronisation barrier for a pipeline stage.
    pub fn add_barrier(&mut self, barrier: BarrierSync) {
        if let Some(pos) = self
            .barriers
            .iter()
            .position(|b| b.stage_id == barrier.stage_id)
        {
            self.barriers[pos] = barrier;
        } else {
            self.barriers.push(barrier);
        }
        self.barrier_counts.resize(self.barriers.len(), 0);
    }

    /// Check whether a barrier with the given stage ID exists.
    pub fn has_barrier(&self, stage_id: &str) -> bool {
        self.barriers.iter().any(|b| b.stage_id == stage_id)
    }

    /// Reset all barrier tracking counters.
    pub fn reset_barriers(&mut self) {
        for count in &mut self.barrier_counts {
            *count = 0;
        }
    }
}

impl Default for ParallelScheduler {
    fn default() -> Self {
        Self::new(num_cpus())
    }
}

/// Return the number of available logical CPUs.
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::workflow::stage::{PipelineStageType, WorkflowStage};

    #[test]
    fn test_scheduler_create() {
        let sched = ParallelScheduler::new(4);
        assert_eq!(sched.max_threads, 4);
        assert!(sched.barriers.is_empty());
    }

    #[test]
    fn test_scheduler_create_min_threads() {
        let sched = ParallelScheduler::new(0);
        assert_eq!(sched.max_threads, 1);
    }

    #[test]
    fn test_scheduler_default() {
        let sched = ParallelScheduler::default();
        assert!(sched.max_threads >= 1);
    }

    #[test]
    fn test_schedule_stage_parallel() {
        let sched = ParallelScheduler::new(4);
        let stage = WorkflowStage {
            stage_type: PipelineStageType::Solve,
            task_ids: vec![
                "a".into(),
                "b".into(),
                "c".into(),
                "d".into(),
                "e".into(),
                "f".into(),
            ],
            parallel: true,
            barrier_required: true,
        };

        let chunks = sched.schedule_stage(&stage);
        // 6 tasks, 4 workers => 2, 2, 1, 1
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks.iter().sum::<usize>(), 6);
        // First workers get larger chunks
        assert_eq!(chunks[0], 2);
        assert_eq!(chunks[1], 2);
        assert_eq!(chunks[2], 1);
        assert_eq!(chunks[3], 1);
    }

    #[test]
    fn test_schedule_stage_serial() {
        let sched = ParallelScheduler::new(4);
        let stage = WorkflowStage {
            stage_type: PipelineStageType::CouplingSync,
            task_ids: vec!["a".into(), "b".into()],
            parallel: false,
            barrier_required: true,
        };

        let chunks = sched.schedule_stage(&stage);
        assert_eq!(chunks, vec![2]);
    }

    #[test]
    fn test_schedule_stage_single_worker() {
        let sched = ParallelScheduler::new(1);
        let stage = WorkflowStage {
            stage_type: PipelineStageType::Solve,
            task_ids: vec!["a".into(), "b".into(), "c".into()],
            parallel: true,
            barrier_required: false,
        };

        let chunks = sched.schedule_stage(&stage);
        assert_eq!(chunks, vec![3]);
    }

    #[test]
    fn test_empty_tasks() {
        let sched = ParallelScheduler::new(4);
        let stage = WorkflowStage {
            stage_type: PipelineStageType::PreProcess,
            task_ids: vec![],
            parallel: true,
            barrier_required: false,
        };

        let chunks = sched.schedule_stage(&stage);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_single_task() {
        let sched = ParallelScheduler::new(4);
        let stage = WorkflowStage {
            stage_type: PipelineStageType::Solve,
            task_ids: vec!["only".into()],
            parallel: true,
            barrier_required: false,
        };

        let chunks = sched.schedule_stage(&stage);
        assert_eq!(chunks, vec![1]);
    }

    #[test]
    fn test_add_barrier() {
        let mut sched = ParallelScheduler::new(4);
        let barrier = BarrierSync {
            stage_id: "stage1".into(),
            expected_count: 4,
            timeout: Some(Duration::from_secs(30)),
        };
        sched.add_barrier(barrier);
        assert_eq!(sched.barriers.len(), 1);
        assert!(sched.has_barrier("stage1"));
    }

    #[test]
    fn test_replace_barrier() {
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "s1".into(),
            expected_count: 2,
            timeout: None,
        });
        sched.add_barrier(BarrierSync {
            stage_id: "s1".into(),
            expected_count: 4,
            timeout: Some(Duration::from_secs(10)),
        });
        assert_eq!(sched.barriers.len(), 1);
        assert_eq!(sched.barriers[0].expected_count, 4);
    }

    #[test]
    fn test_has_barrier() {
        let sched = ParallelScheduler::new(2);
        assert!(!sched.has_barrier("nonexistent"));
    }

    #[test]
    fn test_reset_barriers() {
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "s1".into(),
            expected_count: 2,
            timeout: None,
        });
        sched.barrier_counts[0] = 2;
        sched.reset_barriers();
        assert_eq!(sched.barrier_counts[0], 0);
    }

    #[test]
    fn test_execute_parallel_empty() {
        let results = ParallelScheduler::execute_parallel::<i32, _>(&[], |_| Ok(()));
        assert!(results.is_empty());
    }

    #[test]
    fn test_execute_parallel_success() {
        let tasks = vec![1, 2, 3];
        let results = ParallelScheduler::execute_parallel(&tasks, |x| {
            if *x > 0 {
                Ok(())
            } else {
                Err("negative".into())
            }
        });
        assert_eq!(results.len(), 3);
        for r in &results {
            assert!(r.is_ok());
        }
    }

    #[test]
    fn test_execute_parallel_failure() {
        let tasks = vec![1, 0, 3];
        let results = ParallelScheduler::execute_parallel(&tasks, |x| {
            if *x != 0 {
                Ok(())
            } else {
                Err(format!("invalid value: {x}"))
            }
        });
        assert!(results[0].is_ok());
        assert!(results[1].is_err());
        assert_eq!(results[1].as_ref().unwrap_err(), "invalid value: 0");
        assert!(results[2].is_ok());
    }

    #[test]
    fn test_schedule_stage_uneven_workers() {
        let sched = ParallelScheduler::new(3);
        // 10 tasks distributed across 3 workers => 4, 3, 3
        let stage = WorkflowStage {
            stage_type: PipelineStageType::PostProcess,
            task_ids: (0..10).map(|i| format!("t{i}")).collect(),
            parallel: true,
            barrier_required: true,
        };
        let chunks = sched.schedule_stage(&stage);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks.iter().sum::<usize>(), 10);
        assert_eq!(chunks[0], 4);
        assert_eq!(chunks[1], 3);
        assert_eq!(chunks[2], 3);
    }
}
