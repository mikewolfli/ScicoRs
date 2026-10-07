// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Configurable retry with failure classification and backoff (Phase 38).
//!
//! Only failures explicitly classified as **transient** are retried. Deterministic
//! failures (bad input, model divergence) are never retried, so a retry policy
//! cannot mask a real model or input error.

use std::time::Duration;

/// Classification of a task failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// A temporary condition (e.g. a transient I/O error); safe to retry.
    Transient,
    /// A deterministic input error; retrying will not help.
    InvalidInput,
    /// The model did not converge or diverged; retrying identically will not help.
    NumericalFailure,
    /// A resource limit was hit; retrying may help once resources free up.
    ResourceExhausted,
    /// An unexpected internal error.
    Internal,
}

impl FailureKind {
    /// Whether failures of this kind may be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Transient | Self::ResourceExhausted)
    }
}

/// Backoff strategy between retries.
#[derive(Debug, Clone, PartialEq)]
pub enum Backoff {
    /// Fixed delay between attempts.
    Fixed(Duration),
    /// Exponential delay `base * factor^attempt`, capped at `max`.
    Exponential {
        /// Base delay.
        base: Duration,
        /// Multiplier applied per attempt.
        factor: f64,
        /// Maximum delay.
        max: Duration,
    },
    /// No delay between attempts.
    None,
}

impl Backoff {
    /// The delay before attempt number `attempt` (0-based).
    pub fn delay(&self, attempt: u32) -> Duration {
        match self {
            Self::None => Duration::ZERO,
            Self::Fixed(d) => *d,
            Self::Exponential { base, factor, max } => {
                let base_s = base.as_secs_f64();
                let scaled = base_s * factor.powi(attempt as i32);
                let capped = scaled.min(max.as_secs_f64());
                Duration::from_secs_f64(capped.max(0.0))
            }
        }
    }
}

/// A retry policy.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (not counting the initial attempt).
    pub max_retries: u32,
    /// Backoff strategy.
    pub backoff: Backoff,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff: Backoff::Exponential {
                base: Duration::from_millis(50),
                factor: 2.0,
                max: Duration::from_secs(5),
            },
        }
    }
}

/// The outcome of running a task under a retry policy.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryOutcome<T> {
    /// The successful value, if any.
    pub value: Option<T>,
    /// Number of attempts made (>= 1).
    pub attempts: u32,
    /// The last failure, if the task ultimately failed.
    pub last_failure: Option<(FailureKind, String)>,
    /// Total delay slept between attempts.
    pub total_delay: Duration,
}

impl<T> RetryOutcome<T> {
    /// Whether the task eventually succeeded.
    pub fn succeeded(&self) -> bool {
        self.value.is_some()
    }
}

/// Error returned by [`run_with_retry`] when the task ultimately failed or the
/// failure was non-retryable.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryError {
    /// Number of attempts made.
    pub attempts: u32,
    /// The final failure kind.
    pub kind: FailureKind,
    /// The final error message.
    pub message: String,
}

impl std::fmt::Display for RetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "task failed after {} attempt(s) [{:?}]: {}",
            self.attempts, self.kind, self.message
        )
    }
}

impl std::error::Error for RetryError {}

/// Run a fallible task under a retry policy.
///
/// The task returns `Result<T, (FailureKind, String)>`. Non-retryable failures
/// abort immediately; retryable failures are retried up to `max_retries` with the
/// configured backoff. `sleep` is injected so tests can run without real delays.
pub fn run_with_retry<T, F, S>(
    policy: &RetryPolicy,
    mut task: F,
    mut sleep: S,
) -> Result<RetryOutcome<T>, RetryError>
where
    F: FnMut(u32) -> Result<T, (FailureKind, String)>,
    S: FnMut(Duration),
{
    let mut total_delay = Duration::ZERO;
    let mut attempt = 0u32;
    loop {
        match task(attempt) {
            Ok(value) => {
                return Ok(RetryOutcome {
                    value: Some(value),
                    attempts: attempt + 1,
                    last_failure: None,
                    total_delay,
                });
            }
            Err((kind, message)) => {
                let attempts_made = attempt + 1;
                if !kind.is_retryable() || attempt >= policy.max_retries {
                    return Err(RetryError {
                        attempts: attempts_made,
                        kind,
                        message,
                    });
                }
                let delay = policy.backoff.delay(attempt);
                if !delay.is_zero() {
                    sleep(delay);
                    total_delay += delay;
                }
                attempt += 1;
                let _ = attempts_made;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_failure_is_retried_then_succeeds() {
        let policy = RetryPolicy {
            max_retries: 3,
            backoff: Backoff::None,
        };
        let mut calls = 0;
        let result = run_with_retry(
            &policy,
            |attempt| {
                calls += 1;
                if attempt < 2 {
                    Err((FailureKind::Transient, "temporary".to_string()))
                } else {
                    Ok(42)
                }
            },
            |_| {},
        )
        .unwrap();
        assert!(result.succeeded());
        assert_eq!(result.value, Some(42));
        assert_eq!(result.attempts, 3);
        assert_eq!(calls, 3);
    }

    #[test]
    fn non_retryable_failure_aborts_immediately() {
        let policy = RetryPolicy::default();
        let mut calls = 0;
        let err = run_with_retry::<i32, _, _>(
            &policy,
            |_| {
                calls += 1;
                Err((FailureKind::InvalidInput, "bad param".to_string()))
            },
            |_| {},
        )
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(err.attempts, 1);
        assert_eq!(err.kind, FailureKind::InvalidInput);
    }

    #[test]
    fn numerical_failure_never_retried() {
        let policy = RetryPolicy::default();
        let err = run_with_retry::<i32, _, _>(
            &policy,
            |_| Err((FailureKind::NumericalFailure, "diverged".to_string())),
            |_| {},
        )
        .unwrap_err();
        assert_eq!(err.attempts, 1);
    }

    #[test]
    fn exhaustion_reports_attempt_count() {
        let policy = RetryPolicy {
            max_retries: 2,
            backoff: Backoff::None,
        };
        let err = run_with_retry::<i32, _, _>(
            &policy,
            |_| Err((FailureKind::Transient, "still failing".to_string())),
            |_| {},
        )
        .unwrap_err();
        // 1 initial + 2 retries.
        assert_eq!(err.attempts, 3);
    }

    #[test]
    fn exponential_backoff_is_capped() {
        let b = Backoff::Exponential {
            base: Duration::from_millis(100),
            factor: 2.0,
            max: Duration::from_millis(500),
        };
        assert_eq!(b.delay(0), Duration::from_millis(100));
        assert_eq!(b.delay(1), Duration::from_millis(200));
        assert_eq!(b.delay(2), Duration::from_millis(400));
        assert_eq!(b.delay(3), Duration::from_millis(500)); // capped
    }

    #[test]
    fn retryable_kinds_classification() {
        assert!(FailureKind::Transient.is_retryable());
        assert!(FailureKind::ResourceExhausted.is_retryable());
        assert!(!FailureKind::InvalidInput.is_retryable());
        assert!(!FailureKind::NumericalFailure.is_retryable());
        assert!(!FailureKind::Internal.is_retryable());
    }

    #[test]
    fn total_delay_is_accumulated() {
        let policy = RetryPolicy {
            max_retries: 3,
            backoff: Backoff::Fixed(Duration::from_millis(10)),
        };
        let mut slept = Vec::new();
        let _ = run_with_retry::<i32, _, _>(
            &policy,
            |_| Err((FailureKind::Transient, "x".to_string())),
            |d| slept.push(d),
        );
        assert_eq!(slept.len(), 3);
        assert!(slept.iter().all(|&d| d == Duration::from_millis(10)));
    }
}
