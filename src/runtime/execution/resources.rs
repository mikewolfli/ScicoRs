// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Resource budgets and scheduling limits (Phase 38).
//!
//! Defines explicit upper bounds on CPU threads, concurrent simulations and
//! device selection, plus a [`ResourceTracker`] that records live usage and
//! signals when a budget would be exceeded. Parallel tasks are expected to run
//! in isolated simulation contexts; this module tracks the concurrency budget so
//! those contexts never share mutable state by accident.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A resource budget for a batch of concurrently scheduled simulations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBudget {
    /// Maximum worker threads allowed across the process.
    pub max_threads: usize,
    /// Maximum number of simulations running concurrently.
    pub max_concurrent_simulations: usize,
    /// Maximum total memory (bytes) the batch may use. `0` means unbounded.
    pub max_memory_bytes: usize,
    /// Optional device selector (e.g. "cpu", "gpu:0"). `None` = any.
    pub device: Option<String>,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Self {
            max_threads: threads,
            max_concurrent_simulations: threads.max(1),
            max_memory_bytes: 0,
            device: None,
        }
    }
}

impl ResourceBudget {
    /// Validate the budget: thread and concurrency limits must be non-zero.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_threads == 0 {
            return Err("max_threads must be >= 1".to_string());
        }
        if self.max_concurrent_simulations == 0 {
            return Err("max_concurrent_simulations must be >= 1".to_string());
        }
        Ok(())
    }

    /// The effective concurrency is the minimum of the thread and simulation
    /// limits.
    pub fn effective_concurrency(&self) -> usize {
        self.max_threads.min(self.max_concurrent_simulations).max(1)
    }
}

/// Error raised when a resource budget would be exceeded.
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceError {
    /// All concurrency slots are in use.
    ConcurrencyExhausted {
        /// Current number of running tasks.
        running: usize,
        /// The limit.
        limit: usize,
    },
    /// A memory request would exceed the budget.
    MemoryExceeded {
        /// Requested bytes.
        requested: usize,
        /// Currently used bytes.
        used: usize,
        /// The limit.
        limit: usize,
    },
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConcurrencyExhausted { running, limit } => {
                write!(
                    f,
                    "concurrency limit reached: {running}/{limit} tasks running"
                )
            }
            Self::MemoryExceeded {
                requested,
                used,
                limit,
            } => write!(
                f,
                "memory budget exceeded: requested {requested} + used {used} > limit {limit}"
            ),
        }
    }
}

impl std::error::Error for ResourceError {}

/// Tracks live resource usage against a [`ResourceBudget`].
///
/// Cloning shares the same counters, so a batch scheduler and its workers observe
/// one consistent view.
#[derive(Debug, Clone)]
pub struct ResourceTracker {
    budget: ResourceBudget,
    running: Arc<AtomicUsize>,
    memory_used: Arc<AtomicUsize>,
}

impl ResourceTracker {
    /// Create a tracker for the given budget.
    pub fn new(budget: ResourceBudget) -> Result<Self, String> {
        budget.validate()?;
        Ok(Self {
            budget,
            running: Arc::new(AtomicUsize::new(0)),
            memory_used: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// The budget this tracker enforces.
    pub fn budget(&self) -> &ResourceBudget {
        &self.budget
    }

    /// Current number of running simulations.
    pub fn running(&self) -> usize {
        self.running.load(Ordering::SeqCst)
    }

    /// Current tracked memory usage in bytes.
    pub fn memory_used(&self) -> usize {
        self.memory_used.load(Ordering::SeqCst)
    }

    /// Try to acquire a simulation slot and a memory reservation.
    ///
    /// On success returns a [`ResourceLease`] that releases both when dropped.
    pub fn acquire(&self, memory_bytes: usize) -> Result<ResourceLease, ResourceError> {
        // Concurrency check with a compare-exchange reservation loop.
        loop {
            let cur = self.running.load(Ordering::SeqCst);
            if cur >= self.budget.effective_concurrency() {
                return Err(ResourceError::ConcurrencyExhausted {
                    running: cur,
                    limit: self.budget.effective_concurrency(),
                });
            }
            if self
                .running
                .compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
        }
        // Memory check (0 limit = unbounded).
        if self.budget.max_memory_bytes > 0 {
            let used = self.memory_used.load(Ordering::SeqCst);
            if used + memory_bytes > self.budget.max_memory_bytes {
                // Roll back the concurrency reservation.
                self.running.fetch_sub(1, Ordering::SeqCst);
                return Err(ResourceError::MemoryExceeded {
                    requested: memory_bytes,
                    used,
                    limit: self.budget.max_memory_bytes,
                });
            }
            self.memory_used.fetch_add(memory_bytes, Ordering::SeqCst);
        }
        Ok(ResourceLease {
            tracker: self.clone(),
            memory_bytes,
        })
    }
}

/// A lease on a concurrency slot and memory reservation; releases on drop.
#[derive(Debug)]
pub struct ResourceLease {
    tracker: ResourceTracker,
    memory_bytes: usize,
}

impl ResourceLease {
    /// Bytes reserved by this lease.
    pub fn memory_bytes(&self) -> usize {
        self.memory_bytes
    }
}

impl Drop for ResourceLease {
    fn drop(&mut self) {
        self.tracker.running.fetch_sub(1, Ordering::SeqCst);
        if self.tracker.budget.max_memory_bytes > 0 && self.memory_bytes > 0 {
            self.tracker
                .memory_used
                .fetch_sub(self.memory_bytes, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_validation() {
        let mut b = ResourceBudget::default();
        assert!(b.validate().is_ok());
        b.max_threads = 0;
        assert!(b.validate().is_err());
    }

    #[test]
    fn effective_concurrency_is_minimum() {
        let b = ResourceBudget {
            max_threads: 8,
            max_concurrent_simulations: 3,
            max_memory_bytes: 0,
            device: None,
        };
        assert_eq!(b.effective_concurrency(), 3);
    }

    #[test]
    fn lease_acquires_and_releases() {
        let b = ResourceBudget {
            max_threads: 4,
            max_concurrent_simulations: 2,
            max_memory_bytes: 0,
            device: None,
        };
        let t = ResourceTracker::new(b).unwrap();
        let l1 = t.acquire(0).unwrap();
        let l2 = t.acquire(0).unwrap();
        assert_eq!(t.running(), 2);
        // Third acquisition exceeds the limit.
        let err = t.acquire(0).unwrap_err();
        assert!(matches!(err, ResourceError::ConcurrencyExhausted { .. }));
        drop(l1);
        assert_eq!(t.running(), 1);
        drop(l2);
        assert_eq!(t.running(), 0);
    }

    #[test]
    fn memory_budget_enforced_and_released() {
        let b = ResourceBudget {
            max_threads: 4,
            max_concurrent_simulations: 4,
            max_memory_bytes: 1000,
            device: None,
        };
        let t = ResourceTracker::new(b).unwrap();
        let l1 = t.acquire(600).unwrap();
        assert_eq!(t.memory_used(), 600);
        // 600 + 600 > 1000 → rejected, and the concurrency slot is rolled back.
        let err = t.acquire(600).unwrap_err();
        assert!(matches!(err, ResourceError::MemoryExceeded { .. }));
        assert_eq!(t.running(), 1);
        drop(l1);
        assert_eq!(t.memory_used(), 0);
    }

    #[test]
    fn tracker_is_shared_across_clones() {
        let b = ResourceBudget {
            max_threads: 2,
            max_concurrent_simulations: 1,
            max_memory_bytes: 0,
            device: None,
        };
        let t = ResourceTracker::new(b).unwrap();
        let t2 = t.clone();
        let _lease = t.acquire(0).unwrap();
        assert_eq!(t2.running(), 1);
        assert!(t2.acquire(0).is_err());
    }
}
