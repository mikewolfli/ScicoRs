// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Parallel scheduler backed by a private rayon thread pool.
//!
//! Provides scheduling primitives for parallel task execution across pipeline
//! stages. Each [`ParallelScheduler`] owns its own `rayon::ThreadPool`, so the
//! `max_threads` requested at construction is the width actually used by
//! [`ParallelScheduler::execute_parallel`] — not the width of some unrelated
//! process-wide global pool.
//!
//! # Stage boundaries
//!
//! Two mechanisms enforce a stage boundary:
//!
//! 1. The join inside [`ParallelScheduler::execute_parallel`] — rayon's
//!    `par_iter` returns only once every task of the stage has completed, so no
//!    task in stage *n+1* can start before every task in stage *n* is finished.
//! 2. The optional [`BarrierSync`] registered per stage — a real count-down
//!    barrier with an optional timeout, used when workers must rendezvous
//!    *mid-stage* rather than only at the stage join.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use rayon::ThreadPool;

use super::stage::WorkflowStage;

/// Describes a synchronisation barrier at the end of a pipeline stage.
///
/// The struct is live configuration: registering it through
/// [`ParallelScheduler::add_barrier`] installs a real count-down barrier that
/// [`ParallelScheduler::barrier_wait`] blocks on until `expected_count`
/// participants have arrived, or until `timeout` elapses.
#[derive(Debug, Clone)]
pub struct BarrierSync {
    /// Identifier of the stage this barrier belongs to.
    pub stage_id: String,
    /// Number of participants that must reach this barrier.
    pub expected_count: usize,
    /// Optional timeout — if `Some`, a wait longer than this duration fails
    /// with an error instead of blocking forever.
    pub timeout: Option<Duration>,
}

/// Runtime state of one registered barrier.
#[derive(Debug)]
struct BarrierState {
    /// Configuration this state was created from.
    config: BarrierSync,
    /// Number of participants that have arrived for the current generation.
    arrived: usize,
    /// Generation counter; incremented every time the barrier trips so a
    /// re-used stage cannot observe stale arrivals.
    generation: u64,
}

/// Shared, cloneable barrier runtime.
///
/// Handed out by [`ParallelScheduler::barrier_handle`] so worker closures can
/// wait on the barrier without holding a borrow of the scheduler.
#[derive(Debug, Clone)]
pub struct BarrierHandle {
    state: Arc<(Mutex<BarrierState>, Condvar)>,
}

impl BarrierHandle {
    /// Wait until `expected_count` participants have arrived at this barrier.
    ///
    /// Returns the number of the generation that tripped, which lets callers
    /// thread one barrier through repeated stage rounds. If the barrier has a
    /// timeout and it expires first, returns an error describing the stage.
    pub fn wait(&self) -> Result<u64, String> {
        let (lock, cvar) = &*self.state;
        let mut state = lock.lock().map_err(|_| "barrier mutex poisoned")?;

        state.arrived += 1;
        let my_generation = state.generation;

        // Last arrival trips the barrier and wakes everyone else.
        if state.arrived >= state.config.expected_count.max(1) {
            state.arrived = 0;
            state.generation = state.generation.wrapping_add(1);
            cvar.notify_all();
            return Ok(my_generation);
        }

        let deadline = state.config.timeout.map(|t| Instant::now() + t);
        loop {
            if state.generation != my_generation {
                return Ok(my_generation);
            }

            match deadline {
                None => {
                    state = cvar.wait(state).map_err(|_| "barrier mutex poisoned")?;
                }
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        // Roll back this arrival: the participant gives up and the
                        // remaining participants keep waiting for their own quorum.
                        state.arrived = state.arrived.saturating_sub(1);
                        return Err(format!(
                            "barrier '{}' timed out after {:?} with {}/{} participants",
                            state.config.stage_id,
                            state.config.timeout.unwrap_or_default(),
                            state.arrived,
                            state.config.expected_count
                        ));
                    }
                    let (guard, timeout_result) = cvar
                        .wait_timeout(state, deadline - now)
                        .map_err(|_| "barrier mutex poisoned")?;
                    state = guard;
                    if timeout_result.timed_out() && state.generation == my_generation {
                        state.arrived = state.arrived.saturating_sub(1);
                        return Err(format!(
                            "barrier '{}' timed out after {:?}",
                            state.config.stage_id,
                            state.config.timeout.unwrap_or_default()
                        ));
                    }
                }
            }
        }
    }

    /// Number of participants currently waiting at this barrier.
    pub fn arrived_count(&self) -> usize {
        self.state.0.lock().map(|s| s.arrived).unwrap_or_default()
    }
}

/// A scheduler that distributes work across the threads of its own pool.
///
/// The private pool is shared through an `Arc`, so cloning a scheduler yields
/// another handle to the same fixed-width pool — the requested `max_threads` is
/// never silently replaced by a different pool's width.
#[derive(Debug, Clone)]
pub struct ParallelScheduler {
    /// Maximum number of worker threads for this scheduler's private pool.
    pub max_threads: usize,
    /// Private rayon pool built with exactly `max_threads` worker threads.
    ///
    /// `None` means the pool could not be created for this instance, in which
    /// case execution falls back to rayon's global pool (and `max_threads` is
    /// still honoured for chunk sizing).
    pool: Option<Arc<ThreadPool>>,
    /// Registered synchronisation barriers keyed by stage id.
    barriers: HashMap<String, BarrierHandle>,
}

impl ParallelScheduler {
    /// Create a new parallel scheduler owning a pool of `max_threads` workers.
    ///
    /// `max_threads` is clamped to at least `1`. The pool is private to this
    /// instance: two schedulers created with different limits run with their
    /// own widths, independent of each other and of rayon's global pool.
    pub fn new(max_threads: usize) -> Self {
        let max_threads = max_threads.max(1);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(max_threads)
            .thread_name(move |i| format!("scico-pool-{max_threads}-{i}"))
            .build()
            .ok()
            .map(Arc::new);

        Self {
            max_threads,
            pool,
            barriers: HashMap::new(),
        }
    }

    /// Number of worker threads in this scheduler's private pool.
    ///
    /// Returns `None` when the pool could not be created.
    pub fn pool_thread_count(&self) -> Option<usize> {
        self.pool.as_ref().map(|pool| pool.current_num_threads())
    }

    /// Compute chunk sizes for distributing tasks across workers.
    ///
    /// Returns a vector of chunk sizes (number of tasks per worker)
    /// based on the stage's parallelism flag and the available threads.
    /// Serial stages return a single chunk containing all tasks.
    ///
    /// Chunking is independent of barrier registration: the stage join in
    /// [`Self::execute_parallel`] always enforces the boundary, and a registered
    /// [`BarrierSync`] adds an explicit rendezvous when workers need one.
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

    /// Execute tasks in parallel on this scheduler's private pool.
    ///
    /// Every task runs inside `pool.install(...)`, so the pool width is the
    /// `max_threads` requested at construction. Blocks until all tasks have
    /// completed, which is what enforces the stage boundary. Results are
    /// returned in the same order as the input tasks.
    pub fn execute_parallel<T, F>(&self, tasks: &[T], f: F) -> Vec<Result<(), String>>
    where
        T: Send + Sync,
        F: Fn(&T) -> Result<(), String> + Send + Sync,
    {
        use rayon::prelude::*;

        let run = || tasks.par_iter().map(&f).collect();
        match &self.pool {
            Some(pool) => pool.install(run),
            None => run(),
        }
    }

    /// Execute tasks in parallel with a scope-based loop on this scheduler's pool.
    ///
    /// Better suited for CPU-bound numerical work where each task is
    /// independent. Blocks until every task has run.
    pub fn execute_parallel_scoped<T, F>(&self, tasks: &[T], f: F)
    where
        T: Send + Sync,
        F: Fn(&T) + Send + Sync,
    {
        use rayon::prelude::*;

        let run = || tasks.par_iter().for_each(&f);
        match &self.pool {
            Some(pool) => pool.install(run),
            None => run(),
        }
    }

    /// Register (or replace) a synchronisation barrier for a pipeline stage.
    ///
    /// Registering the same `stage_id` again replaces the barrier and its
    /// runtime state, so a re-used stage never inherits stale arrivals.
    pub fn add_barrier(&mut self, barrier: BarrierSync) {
        let state = BarrierState {
            config: barrier.clone(),
            arrived: 0,
            generation: 0,
        };
        self.barriers.insert(
            barrier.stage_id.clone(),
            BarrierHandle {
                state: Arc::new((Mutex::new(state), Condvar::new())),
            },
        );
    }

    /// Check whether a barrier with the given stage ID exists.
    pub fn has_barrier(&self, stage_id: &str) -> bool {
        self.barriers.contains_key(stage_id)
    }

    /// Configuration snapshot of the registered barrier for `stage_id`.
    pub fn barrier(&self, stage_id: &str) -> Option<BarrierSync> {
        self.barriers
            .get(stage_id)
            .and_then(|h| h.state.0.lock().ok().map(|s| s.config.clone()))
    }

    /// Number of registered barriers.
    pub fn barrier_count(&self) -> usize {
        self.barriers.len()
    }

    /// Number of participants currently waiting at `stage_id`'s barrier.
    ///
    /// Returns `None` when no barrier is registered for the stage.
    pub fn barrier_arrived_count(&self, stage_id: &str) -> Option<usize> {
        self.barriers.get(stage_id).map(|h| h.arrived_count())
    }

    /// Wait at `stage_id`'s barrier until `expected_count` participants arrive.
    ///
    /// Returns an error when no barrier is registered for the stage, or when the
    /// barrier's timeout expires before the quorum is reached.
    pub fn barrier_wait(&self, stage_id: &str) -> Result<u64, String> {
        let handle = self
            .barriers
            .get(stage_id)
            .ok_or_else(|| format!("no barrier registered for stage '{stage_id}'"))?;
        handle.wait()
    }

    /// Get a cloneable handle for `stage_id`'s barrier.
    ///
    /// Use this to let worker closures (which cannot borrow the scheduler)
    /// rendezvous on the barrier from inside `execute_parallel_scoped`.
    pub fn barrier_handle(&self, stage_id: &str) -> Option<BarrierHandle> {
        self.barriers.get(stage_id).cloned()
    }

    /// Reset the arrival counters of every registered barrier to zero.
    ///
    /// Wakes any waiters by advancing each barrier's generation, so a stalled
    /// stage cannot leave the scheduler permanently blocked.
    pub fn reset_barriers(&mut self) {
        for handle in self.barriers.values() {
            if let Ok(mut state) = handle.state.0.lock() {
                state.arrived = 0;
                state.generation = state.generation.wrapping_add(1);
            }
            handle.state.1.notify_all();
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn stage(parallel: bool, tasks: &[&str]) -> WorkflowStage {
        WorkflowStage {
            stage_type: PipelineStageType::Solve,
            task_ids: tasks.iter().map(|t| (*t).to_string()).collect(),
            parallel,
            barrier_required: parallel,
        }
    }

    #[test]
    fn test_scheduler_create() {
        let sched = ParallelScheduler::new(4);
        assert_eq!(sched.max_threads, 4);
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
    fn test_pool_has_requested_width() {
        // Every scheduler owns a pool sized exactly to `max_threads`.
        for width in [1usize, 2, 3, 8] {
            let sched = ParallelScheduler::new(width);
            assert_eq!(
                sched.pool_thread_count(),
                Some(width),
                "pool for max_threads={width} should have {width} threads"
            );
        }
    }

    #[test]
    fn test_two_schedulers_use_their_own_widths() {
        let small = ParallelScheduler::new(2);
        let large = ParallelScheduler::new(6);

        assert_eq!(small.pool_thread_count(), Some(2));
        assert_eq!(large.pool_thread_count(), Some(6));

        // Observe the width from *inside* the parallel work.
        let observed_small = Arc::new(Mutex::new(None));
        let observed_large = Arc::new(Mutex::new(None));

        let record = |slot: &Arc<Mutex<Option<usize>>>| {
            *slot.lock().expect("lock") = Some(rayon::current_num_threads());
        };

        let tasks: Vec<usize> = (0..4).collect();
        let sm = Arc::clone(&observed_small);
        let lg = Arc::clone(&observed_large);

        small.execute_parallel(&tasks, |_| {
            record(&sm);
            Ok(())
        });
        large.execute_parallel(&tasks, |_| {
            record(&lg);
            Ok(())
        });

        assert_eq!(
            *observed_small.lock().expect("lock"),
            Some(2),
            "the 2-thread scheduler must execute on a 2-thread pool"
        );
        assert_eq!(
            *observed_large.lock().expect("lock"),
            Some(6),
            "the 6-thread scheduler must execute on a 6-thread pool"
        );
    }

    #[test]
    fn test_pool_width_is_not_the_global_pool() {
        // A one-thread scheduler must observe exactly one thread even though the
        // process-wide rayon pool (built lazily elsewhere) is wider.
        let sched = ParallelScheduler::new(1);
        let observed = Arc::new(Mutex::new(usize::MAX));
        let slot = Arc::clone(&observed);

        let tasks = vec![0usize; 8];
        sched.execute_parallel(&tasks, |_| {
            let n = rayon::current_num_threads();
            let mut guard = slot.lock().expect("lock");
            *guard = (*guard).min(n);
            Ok(())
        });

        assert_eq!(*observed.lock().expect("lock"), 1);
    }

    #[test]
    fn test_execute_parallel_serialized_on_one_thread() {
        // With a single worker, no two tasks may overlap.
        let sched = ParallelScheduler::new(1);
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&concurrent);
        let m = Arc::clone(&max_concurrent);

        let tasks = vec![0usize; 16];
        sched.execute_parallel(&tasks, move |_| {
            let now = c.fetch_add(1, Ordering::SeqCst) + 1;
            m.fetch_max(now, Ordering::SeqCst);
            std::thread::yield_now();
            c.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        });

        assert_eq!(max_concurrent.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_schedule_stage_parallel() {
        let sched = ParallelScheduler::new(4);
        let stage = stage(true, &["a", "b", "c", "d", "e", "f"]);

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
        let chunks = sched.schedule_stage(&stage(false, &["a", "b"]));
        assert_eq!(chunks, vec![2]);
    }

    #[test]
    fn test_schedule_stage_single_worker() {
        let sched = ParallelScheduler::new(1);
        let chunks = sched.schedule_stage(&stage(true, &["a", "b", "c"]));
        assert_eq!(chunks, vec![3]);
    }

    #[test]
    fn test_empty_tasks() {
        let sched = ParallelScheduler::new(4);
        let chunks = sched.schedule_stage(&stage(true, &[]));
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_single_task() {
        let sched = ParallelScheduler::new(4);
        let chunks = sched.schedule_stage(&stage(true, &["only"]));
        assert_eq!(chunks, vec![1]);
    }

    #[test]
    fn test_schedule_stage_uneven_workers() {
        let sched = ParallelScheduler::new(3);
        // 10 tasks distributed across 3 workers => 4, 3, 3
        let task_ids: Vec<String> = (0..10).map(|i| format!("t{i}")).collect();
        let refs: Vec<&str> = task_ids.iter().map(String::as_str).collect();
        let chunks = sched.schedule_stage(&stage(true, &refs));
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks.iter().sum::<usize>(), 10);
        assert_eq!(chunks[0], 4);
        assert_eq!(chunks[1], 3);
        assert_eq!(chunks[2], 3);
    }

    #[test]
    fn test_execute_parallel_empty() {
        let sched = ParallelScheduler::new(2);
        let results = sched.execute_parallel::<i32, _>(&[], |_| Ok(()));
        assert!(results.is_empty());
    }

    #[test]
    fn test_execute_parallel_success() {
        let sched = ParallelScheduler::new(2);
        let tasks = vec![1, 2, 3];
        let results = sched.execute_parallel(&tasks, |x| {
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
        let sched = ParallelScheduler::new(2);
        let tasks = vec![1, 0, 3];
        let results = sched.execute_parallel(&tasks, |x| {
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
    fn test_execute_parallel_joins_all_tasks() {
        // The join at the end of `execute_parallel` is the stage barrier: every
        // task must have run before the call returns.
        let sched = ParallelScheduler::new(4);
        let finished = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&finished);

        let tasks: Vec<usize> = (0..64).collect();
        let results = sched.execute_parallel(&tasks, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        assert_eq!(results.len(), 64);
        assert_eq!(
            finished.load(Ordering::SeqCst),
            64,
            "the stage boundary must be a real join"
        );
    }

    #[test]
    fn test_execute_parallel_scoped_runs_every_task() {
        let sched = ParallelScheduler::new(3);
        let count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&count);

        let tasks: Vec<usize> = (0..32).collect();
        sched.execute_parallel_scoped(&tasks, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        assert_eq!(count.load(Ordering::SeqCst), 32);
    }

    #[test]
    fn test_execute_parallel_on_scheduler_without_pool() {
        // A scheduler whose pool failed to build (simulated by clearing it) must
        // still execute tasks correctly, just on the fallback pool.
        let mut sched = ParallelScheduler::new(2);
        sched.pool = None;
        let tasks = vec![1, 2, 3];
        let results = sched.execute_parallel(&tasks, |_| Ok(()));
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(Result::is_ok));
    }

    #[test]
    fn test_add_barrier_registers_stage() {
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "stage1".into(),
            expected_count: 4,
            timeout: Some(Duration::from_secs(30)),
        });
        assert_eq!(sched.barrier_count(), 1);
        assert!(sched.has_barrier("stage1"));
        assert_eq!(sched.barrier_arrived_count("stage1"), Some(0));
    }

    #[test]
    fn test_replace_barrier_resets_state() {
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "s1".into(),
            expected_count: 2,
            timeout: None,
        });

        // Trip the barrier with exactly its quorum (2 arrivals). Calling
        // `wait()` once here would block forever on a 2-participant barrier,
        // so the arrival counter is driven through the public handle by two
        // participants, exactly as `execute_parallel_scoped` would.
        let handle = sched.barrier_handle("s1").expect("handle");
        let h1 = handle.clone();
        let waiter = std::thread::spawn(move || h1.wait());
        let first = handle.wait().expect("second arrival trips the barrier");
        let second = waiter.join().expect("worker joins").expect("arrive");
        assert_eq!(first, second, "both participants observe one generation");
        // A tripped barrier resets its arrival count for the next generation.
        assert_eq!(sched.barrier_arrived_count("s1"), Some(0));

        // Replacing the barrier must discard the (now empty) runtime state and
        // install the new configuration.
        sched.add_barrier(BarrierSync {
            stage_id: "s1".into(),
            expected_count: 4,
            timeout: Some(Duration::from_secs(10)),
        });
        assert_eq!(sched.barrier_count(), 1);
        assert_eq!(sched.barrier("s1").expect("barrier").expected_count, 4);
        assert_eq!(sched.barrier_arrived_count("s1"), Some(0));
    }

    #[test]
    fn test_barrier_without_quorum_times_out_instead_of_hanging() {
        // A barrier that never reaches quorum must fail fast via its timeout
        // rather than blocking the test suite forever.
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "lonely".into(),
            expected_count: 2,
            timeout: Some(Duration::from_millis(20)),
        });
        let handle = sched.barrier_handle("lonely").expect("handle");
        let err = handle.wait().expect_err("a lone arrival must time out");
        assert!(err.contains("timed out"), "unexpected error: {err}");
        // The timed-out participant is rolled back, leaving no stale arrival.
        assert_eq!(sched.barrier_arrived_count("lonely"), Some(0));
    }

    #[test]
    fn test_has_barrier() {
        let sched = ParallelScheduler::new(2);
        assert!(!sched.has_barrier("nonexistent"));
    }

    #[test]
    fn test_barrier_wait_unblocks_at_quorum() {
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "sync".into(),
            expected_count: 3,
            timeout: Some(Duration::from_secs(5)),
        });

        let handles: Vec<BarrierHandle> = (0..3)
            .map(|_| sched.barrier_handle("sync").expect("handle"))
            .collect();
        let barrier = Arc::new(std::sync::Barrier::new(3));

        let workers: Vec<_> = handles
            .into_iter()
            .map(|handle| {
                let rendezvous = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    // Line up all three participants before any of them waits,
                    // so the quorum is guaranteed to be reached.
                    rendezvous.wait();
                    handle.wait()
                })
            })
            .collect();

        for worker in workers {
            let generation = worker.join().expect("worker panicked").expect("wait");
            assert_eq!(generation, 0, "the first round trips generation 0");
        }

        // The barrier reset itself for the next generation.
        assert_eq!(sched.barrier_arrived_count("sync"), Some(0));
    }

    #[test]
    fn test_barrier_records_partial_arrivals() {
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "sync".into(),
            expected_count: 2,
            timeout: None,
        });

        let handle = sched.barrier_handle("sync").expect("handle");
        let waiter = std::thread::spawn(move || handle.wait());

        // Spin until the first participant has parked, then send the second.
        let mut spins = 0;
        while sched.barrier_arrived_count("sync") != Some(1) {
            std::thread::yield_now();
            spins += 1;
            assert!(spins < 10_000_000, "participant never arrived");
        }

        sched.barrier_wait("sync").expect("second arrival trips it");
        assert!(waiter.join().expect("join").is_ok());
        assert_eq!(sched.barrier_arrived_count("sync"), Some(0));
    }

    #[test]
    fn test_barrier_timeout_returns_error() {
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "slow".into(),
            // Nobody else will ever arrive.
            expected_count: 2,
            timeout: Some(Duration::from_millis(20)),
        });

        let err = sched
            .barrier_wait("slow")
            .expect_err("a lone participant must time out");
        assert!(err.contains("slow"), "unexpected error: {err}");
        assert!(err.contains("timed out"), "unexpected error: {err}");
    }

    #[test]
    fn test_barrier_wait_without_registration_errors() {
        let sched = ParallelScheduler::new(2);
        let err = sched
            .barrier_wait("missing")
            .expect_err("unregistered barrier must error");
        assert!(err.contains("missing"));
        assert!(sched.barrier_handle("missing").is_none());
    }

    #[test]
    fn test_reset_barriers_releases_waiters() {
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "s1".into(),
            expected_count: 2,
            timeout: None,
        });

        let handle = sched.barrier_handle("s1").expect("handle");
        let waiter = std::thread::spawn(move || handle.wait());

        let mut spins = 0;
        while sched.barrier_arrived_count("s1") != Some(1) {
            std::thread::yield_now();
            spins += 1;
            assert!(spins < 10_000_000, "participant never arrived");
        }

        sched.reset_barriers();
        assert_eq!(sched.barrier_arrived_count("s1"), Some(0));
        // The reset advances the generation, so the parked waiter is released.
        assert!(waiter.join().expect("join").is_ok());
    }

    #[test]
    fn test_barrier_used_from_inside_parallel_execution() {
        // Real rendezvous mid-stage: every task arrives at the barrier and only
        // continues once all of them have arrived.
        let mut sched = ParallelScheduler::new(4);
        sched.add_barrier(BarrierSync {
            stage_id: "midstage".into(),
            expected_count: 4,
            timeout: Some(Duration::from_secs(10)),
        });
        let handle = sched.barrier_handle("midstage").expect("handle");

        let tasks: Vec<usize> = (0..4).collect();
        let before_last_arrival = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&before_last_arrival);

        let results = sched.execute_parallel(&tasks, move |_| {
            let generation = handle.wait().map_err(|e| e.to_string())?;
            // Nothing may observe a completed barrier before the quorum, so the
            // counter is only ever incremented post-wait.
            counter.fetch_add(1, Ordering::SeqCst);
            assert_eq!(generation, 0);
            Ok(())
        });

        assert!(results.iter().all(Result::is_ok), "results: {results:?}");
        assert_eq!(before_last_arrival.load(Ordering::SeqCst), 4);
        assert_eq!(sched.barrier_arrived_count("midstage"), Some(0));
    }
}
