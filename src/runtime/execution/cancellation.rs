// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Cooperative cancellation tokens and safe stop points (Phase 38).
//!
//! A [`CancellationToken`] is a cloneable handle that a long-running task polls
//! at safe boundaries (e.g. between simulation steps). Cancellation is
//! cooperative — the task decides where it is safe to stop — so state remains
//! consistent and a checkpoint can be written at the stop point.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// A cloneable, cooperative cancellation token.
///
/// Cloning shares the same underlying flag, so all clones observe the same
/// cancellation. Cancellation is one-way: once cancelled it stays cancelled
/// (unless [`Self::reset`] is called for reuse).
#[derive(Debug, Clone)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
    /// Monotonic reason code (0 = not cancelled).
    reason: Arc<AtomicU64>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    /// Create a fresh, un-cancelled token.
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            reason: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Request cancellation with a reason code (non-zero).
    pub fn cancel(&self, reason: u64) {
        self.reason.store(reason, Ordering::SeqCst);
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// The cancellation reason code, if cancelled.
    pub fn reason(&self) -> Option<u64> {
        if self.is_cancelled() {
            Some(self.reason.load(Ordering::SeqCst))
        } else {
            None
        }
    }

    /// Clear the cancellation state so the token can be reused for a new run.
    pub fn reset(&self) {
        self.flag.store(false, Ordering::SeqCst);
        self.reason.store(0, Ordering::SeqCst);
    }

    /// Return an error if cancellation has been requested, otherwise `Ok(())`.
    /// Call this at safe stop points.
    pub fn check(&self) -> Result<(), String> {
        if self.is_cancelled() {
            Err(format!(
                "cancelled (reason code {})",
                self.reason().unwrap_or(0)
            ))
        } else {
            Ok(())
        }
    }
}

/// Common cancellation reason codes.
pub mod reason {
    /// The user requested cancellation.
    pub const USER: u64 = 1;
    /// A resource budget was exceeded.
    pub const RESOURCE_LIMIT: u64 = 2;
    /// The task exceeded its wall-clock timeout.
    pub const TIMEOUT: u64 = 3;
    /// The task was superseded by a newer run.
    pub const SUPERSEDED: u64 = 4;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_observed_by_clones() {
        let a = CancellationToken::new();
        let b = a.clone();
        assert!(!b.is_cancelled());
        a.cancel(reason::USER);
        assert!(b.is_cancelled());
        assert_eq!(b.reason(), Some(reason::USER));
    }

    #[test]
    fn check_returns_error_after_cancel() {
        let t = CancellationToken::new();
        assert!(t.check().is_ok());
        t.cancel(reason::TIMEOUT);
        assert!(t.check().is_err());
    }

    #[test]
    fn reset_clears_state() {
        let t = CancellationToken::new();
        t.cancel(reason::USER);
        assert!(t.is_cancelled());
        t.reset();
        assert!(!t.is_cancelled());
        assert_eq!(t.reason(), None);
    }

    #[test]
    fn cancellation_is_visible_across_threads() {
        let t = CancellationToken::new();
        let t2 = t.clone();
        let handle = std::thread::spawn(move || {
            // Spin until cancelled.
            while !t2.is_cancelled() {
                std::hint::spin_loop();
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(10));
        t.cancel(reason::USER);
        handle.join().unwrap();
        assert!(t.is_cancelled());
    }
}
