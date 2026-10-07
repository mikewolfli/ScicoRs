// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Elastic execution subsystem (Phase 38).
//!
//! Provides cooperative cancellation ([`cancellation`]), resource budgets
//! ([`resources`]) and failure-classified retry ([`retry`]) for long-running
//! simulation tasks. Together with the [`super::checkpoint`] subsystem these let
//! expensive runs pause, resume and recover without corrupting state.

pub mod cancellation;
pub mod resources;
pub mod retry;

pub use cancellation::{CancellationToken, reason};
pub use resources::{ResourceBudget, ResourceError, ResourceLease, ResourceTracker};
pub use retry::{Backoff, FailureKind, RetryError, RetryOutcome, RetryPolicy, run_with_retry};
