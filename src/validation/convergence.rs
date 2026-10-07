// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Time-step and mesh refinement studies with observed-order estimation
//! (Phase 40).
//!
//! # The honest-verification rule
//!
//! A convergence study is only allowed to *report* an order when the data can
//! support one. Specifically, this module:
//!
//! * refuses to fit an order from fewer than two data points;
//! * detects non-monotonic refinement sequences and returns an explicit
//!   [`ConvergenceOutcome::NotAsymptotic`] instead of a fabricated slope;
//! * discriminates the asymptotic region from the pre-asymptotic one by
//!   requiring the per-pair local slopes to be reasonably consistent.
//!
//! The observed order `p` is estimated by a least-squares fit of
//! `log(error)` against `log(step_size)`: for a scheme whose error behaves like
//! `E = C * h^p`, this recovers `p` as the slope. The fit is performed on the
//! logarithms so that a spectrum of mesh sizes is weighted sensibly.

use crate::core::types::Scalar;

/// The refinement variable a study is refining.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RefinementKind {
    /// Time-step refinement (errors are functions of `dt`).
    TimeStep,
    /// Spatial mesh refinement (errors are functions of `h`).
    Mesh,
}

impl RefinementKind {
    /// Short machine-readable label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TimeStep => "time_step",
            Self::Mesh => "mesh",
        }
    }
}

/// One refinement level: a characteristic size and the measured error.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RefinementLevel {
    /// Characteristic size (`dt` for a time study, `h` for a mesh study).
    pub size: Scalar,
    /// Error measured at this size, in the study's norm.
    pub error: Scalar,
    /// Optional measured cost of the run (e.g. wall-clock seconds, or DOF
    /// count). Reported back but not used to determine the order.
    pub cost: Scalar,
}

impl RefinementLevel {
    /// Construct a level with a cost; use `cost = 0.0` when cost is unknown.
    pub fn new(size: Scalar, error: Scalar, cost: Scalar) -> Self {
        Self { size, error, cost }
    }
}

/// The verdict of a convergence study.
///
/// Only [`ConvergenceOutcome::Converged`] carries a numeric order. The other
/// variants exist so a caller can never mistake "we could not tell" for "the
/// order is fine".
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConvergenceOutcome {
    /// A trustworthy observed order was recovered.
    Converged {
        /// The estimated observed order `p`.
        observed_order: Scalar,
        /// Coefficient of determination of the log-log fit in [0, 1].
        r_squared: Scalar,
        /// Whether the top of the sequence sits in the asymptotic region.
        asymptotic: bool,
    },
    /// There was not enough data to attempt a fit.
    InsufficientData {
        /// Explanation of what was missing.
        detail: String,
    },
    /// The data were present but did not form a usable refinement sequence.
    NotAsymptotic {
        /// Explanation, e.g. which pair violated monotonicity.
        detail: String,
    },
}

impl ConvergenceOutcome {
    /// The observed order, if one was recovered.
    pub fn observed_order(&self) -> Option<Scalar> {
        match self {
            Self::Converged { observed_order, .. } => Some(*observed_order),
            _ => None,
        }
    }

    /// Whether the study yielded a trustworthy order.
    pub fn is_converged(&self) -> bool {
        matches!(self, Self::Converged { .. })
    }
}

/// A completed convergence study.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConvergenceStudy {
    /// What was refined.
    pub kind: RefinementKind,
    /// The norm the per-level errors are expressed in.
    pub norm: String,
    /// The levels, sorted by decreasing size (coarsest first).
    pub levels: Vec<RefinementLevel>,
    /// The verdict.
    pub outcome: ConvergenceOutcome,
    /// Local (pair-wise) orders, one fewer than `levels`.
    ///
    /// These are the observed slopes between consecutive levels; when they
    /// disagree strongly the study is flagged as not asymptotic.
    pub local_orders: Vec<Scalar>,
    /// Total cost over all levels.
    pub total_cost: Scalar,
}

/// Configuration controlling how strict a convergence study is.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConvergenceConfig {
    /// Minimum number of levels required before a fit is attempted.
    pub min_levels: usize,
    /// How consistent the local orders must be, as a fraction of the mean
    /// absolute order, before the sequence is accepted as asymptotic.
    pub order_consistency: Scalar,
    /// Minimum coefficient of determination for the log-log fit.
    pub min_r_squared: Scalar,
}

impl Default for ConvergenceConfig {
    fn default() -> Self {
        Self {
            min_levels: 3,
            order_consistency: 0.35,
            min_r_squared: 0.95,
        }
    }
}

/// Estimate the observed order of a refinement study.
///
/// The levels are sorted by decreasing size (coarsest first). The function never
/// fabricates an order: see the module docs for the exact rules.
pub fn estimate_order(levels: &[RefinementLevel], config: ConvergenceConfig) -> ConvergenceStudy {
    let mut sorted: Vec<RefinementLevel> = levels.to_vec();
    // Coarsest (largest size) first, so refinement shrinks `size`.
    sorted.sort_by(|a, b| {
        b.size
            .partial_cmp(&a.size)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_cost: Scalar = sorted.iter().map(|l| l.cost).sum();
    let mut study = ConvergenceStudy {
        kind: RefinementKind::Mesh,
        norm: String::new(),
        levels: sorted.clone(),
        outcome: ConvergenceOutcome::InsufficientData {
            detail: String::new(),
        },
        local_orders: Vec::new(),
        total_cost,
    };

    // Validate the raw data before doing anything else.
    for (i, lvl) in sorted.iter().enumerate() {
        if !lvl.size.is_finite() || lvl.size <= 0.0 {
            study.outcome = ConvergenceOutcome::NotAsymptotic {
                detail: format!("level {i} has non-positive step size {}", lvl.size),
            };
            return study;
        }
        if !lvl.error.is_finite() || lvl.error <= 0.0 {
            study.outcome = ConvergenceOutcome::NotAsymptotic {
                detail: format!("level {i} has non-positive error {}", lvl.error),
            };
            return study;
        }
    }

    if sorted.len() < config.min_levels {
        study.outcome = ConvergenceOutcome::InsufficientData {
            detail: format!(
                "need at least {} levels, got {}",
                config.min_levels,
                sorted.len()
            ),
        };
        return study;
    }

    // Local pair-wise orders p_i = log(e_{i-1}/e_i) / log(h_{i-1}/h_i).
    let mut local_orders = Vec::with_capacity(sorted.len() - 1);
    for pair in sorted.windows(2) {
        let coarse = pair[0];
        let fine = pair[1];
        let size_ratio = (coarse.size / fine.size).ln();
        let err_ratio = (coarse.error / fine.error).ln();
        if size_ratio <= 0.0 {
            study.outcome = ConvergenceOutcome::NotAsymptotic {
                detail: format!(
                    "step sizes not decreasing: {} -> {}",
                    coarse.size, fine.size
                ),
            };
            return study;
        }
        // A refinement that does not reduce the error is a monotonicity
        // failure: reporting an order here would be fabrication.
        if err_ratio <= 0.0 {
            study.outcome = ConvergenceOutcome::NotAsymptotic {
                detail: format!(
                    "error did not decrease under refinement: {:.3e} -> {:.3e}",
                    coarse.error, fine.error
                ),
            };
            return study;
        }
        local_orders.push(err_ratio / size_ratio);
    }

    // Consistency of the local orders: reject strongly non-constant slopes.
    let mean_abs = if local_orders.is_empty() {
        0.0
    } else {
        local_orders.iter().map(|p| p.abs()).sum::<Scalar>() / local_orders.len() as Scalar
    };
    let max_spread = local_orders
        .iter()
        .map(|p| (p - mean_abs).abs())
        .fold(0.0 as Scalar, Scalar::max);
    let asymptotic = mean_abs > 0.0 && (max_spread / mean_abs) <= config.order_consistency;

    study.local_orders = local_orders;

    // Least-squares slope of log(error) vs log(size).
    match log_log_slope(&sorted) {
        Some((slope, r_squared)) => {
            if r_squared < config.min_r_squared {
                study.outcome = ConvergenceOutcome::NotAsymptotic {
                    detail: format!(
                        "log-log fit too poor for an order estimate (R^2 = {r_squared:.3})"
                    ),
                };
            } else if !asymptotic {
                study.outcome = ConvergenceOutcome::NotAsymptotic {
                    detail: format!(
                        "local orders inconsistent (spread {:.2e} exceeds {:.2} of mean order)",
                        max_spread, config.order_consistency
                    ),
                };
            } else {
                study.outcome = ConvergenceOutcome::Converged {
                    observed_order: slope,
                    r_squared,
                    asymptotic: true,
                };
            }
        }
        None => {
            study.outcome = ConvergenceOutcome::InsufficientData {
                detail: "log-log fit is singular (all sizes identical)".to_string(),
            };
        }
    }

    study
}

/// Least-squares slope and R² of `log(error)` against `log(size)`.
///
/// Returns `None` when the design matrix is singular (all sizes equal).
fn log_log_slope(levels: &[RefinementLevel]) -> Option<(Scalar, Scalar)> {
    let n = levels.len() as Scalar;
    if n < 2.0 {
        return None;
    }
    let xs: Vec<Scalar> = levels.iter().map(|l| l.size.ln()).collect();
    let ys: Vec<Scalar> = levels.iter().map(|l| l.error.ln()).collect();
    let mean_x = xs.iter().sum::<Scalar>() / n;
    let mean_y = ys.iter().sum::<Scalar>() / n;

    let mut sxx = 0.0 as Scalar;
    let mut sxy = 0.0 as Scalar;
    for (x, y) in xs.iter().zip(ys.iter()) {
        sxx += (x - mean_x) * (x - mean_x);
        sxy += (x - mean_x) * (y - mean_y);
    }
    if sxx <= 0.0 {
        return None;
    }
    let slope = sxy / sxx;
    let intercept = mean_y - slope * mean_x;

    let mut ss_res = 0.0 as Scalar;
    let mut ss_tot = 0.0 as Scalar;
    for (x, y) in xs.iter().zip(ys.iter()) {
        let pred = intercept + slope * x;
        ss_res += (y - pred) * (y - pred);
        ss_tot += (y - mean_y) * (y - mean_y);
    }
    let r_squared = if ss_tot <= 0.0 {
        1.0
    } else {
        1.0 - ss_res / ss_tot
    };
    Some((slope, r_squared))
}

/// Construct a geometrically spaced mesh-size sequence.
///
/// Useful for building a study from a single characteristic size and a
/// refinement factor, e.g. `sizes = geometric_sizes(0.4, 0.5, 5)` gives
/// `[0.4, 0.2, 0.1, 0.05, 0.025]`.
pub fn geometric_sizes(coarsest: Scalar, ratio: Scalar, count: usize) -> Vec<Scalar> {
    let mut out = Vec::with_capacity(count);
    let mut s = coarsest;
    for _ in 0..count {
        out.push(s);
        s *= ratio;
    }
    out
}

/// Build a study for an `error = C * h^p` analytic model.
///
/// This is a convenience for verification tests and for producing synthetic
/// convergence data whose order is known exactly.
pub fn analytic_levels(
    coefficient: Scalar,
    order: Scalar,
    sizes: &[Scalar],
    cost_exponent: Scalar,
) -> Vec<RefinementLevel> {
    sizes
        .iter()
        .map(|&h| {
            let error = coefficient * h.powf(order);
            // Cost grows as refinement shrinks `h`.
            let cost = (1.0 / h).powf(cost_exponent);
            RefinementLevel::new(h, error, cost)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn recovers_second_order_from_analytic_data() {
        // error = 3.0 * h^2 exactly.
        let sizes = geometric_sizes(0.4, 0.5, 5);
        let levels = analytic_levels(3.0, 2.0, &sizes, 1.0);
        let study = estimate_order(&levels, ConvergenceConfig::default());
        let p = study.outcome.observed_order().unwrap();
        assert!((p - 2.0).abs() < 1e-9, "recovered order {p}");
        assert!(study.outcome.is_converged());
    }

    #[test]
    fn recovers_first_order_from_analytic_data() {
        let sizes = geometric_sizes(0.1, 0.5, 5);
        let levels = analytic_levels(1.0, 1.0, &sizes, 1.0);
        let study = estimate_order(&levels, ConvergenceConfig::default());
        let p = study.outcome.observed_order().unwrap();
        assert!((p - 1.0).abs() < 1e-9, "recovered order {p}");
    }

    #[test]
    fn recovers_order_with_floating_point_noise() {
        // Perturb the exact data slightly; the fit should still land near 2.
        let sizes = geometric_sizes(0.4, 0.5, 6);
        let mut levels = analytic_levels(3.0, 2.0, &sizes, 1.0);
        for (i, lvl) in levels.iter_mut().enumerate() {
            let wobble = 1.0 + 0.001 * ((i as Scalar) * PI).sin();
            lvl.error *= wobble;
        }
        let study = estimate_order(&levels, ConvergenceConfig::default());
        let p = study.outcome.observed_order().unwrap();
        assert!((p - 2.0).abs() < 0.05, "recovered order {p} too far from 2");
    }

    #[test]
    fn insufficient_data_is_reported_not_fabricated() {
        let levels = vec![
            RefinementLevel::new(0.1, 1e-2, 1.0),
            RefinementLevel::new(0.05, 2.5e-3, 2.0),
        ];
        // Default min_levels = 3.
        let study = estimate_order(&levels, ConvergenceConfig::default());
        assert!(!study.outcome.is_converged());
        assert!(matches!(
            study.outcome,
            ConvergenceOutcome::InsufficientData { .. }
        ));
        assert!(study.outcome.observed_order().is_none());
    }

    #[test]
    fn non_monotonic_errors_are_rejected() {
        // Error goes UP from coarse to fine at the second pair.
        let levels = vec![
            RefinementLevel::new(0.4, 1e-1, 1.0),
            RefinementLevel::new(0.2, 2.0e-2, 2.0),
            RefinementLevel::new(0.1, 5.0e-2, 3.0),
            RefinementLevel::new(0.05, 1.0e-2, 4.0),
            RefinementLevel::new(0.025, 2.0e-3, 5.0),
        ];
        let study = estimate_order(&levels, ConvergenceConfig::default());
        assert!(matches!(
            study.outcome,
            ConvergenceOutcome::NotAsymptotic { .. }
        ));
        assert!(study.outcome.observed_order().is_none());
    }

    #[test]
    fn inconsistent_local_orders_flagged_as_not_asymptotic() {
        // Errors chosen so consecutive local orders differ wildly but still
        // decrease monotonically, i.e. pre-asymptotic behaviour.
        let levels = vec![
            RefinementLevel::new(0.8, 0.5, 1.0),
            RefinementLevel::new(0.4, 0.1, 2.0), // local order ~2.3
            RefinementLevel::new(0.2, 0.05, 3.0), // local order ~1.0
            RefinementLevel::new(0.1, 0.03, 4.0), // local order ~0.7
            RefinementLevel::new(0.05, 0.02, 5.0), // local order ~0.6
        ];
        let study = estimate_order(&levels, ConvergenceConfig::default());
        assert!(!study.outcome.is_converged());
        assert!(matches!(
            study.outcome,
            ConvergenceOutcome::NotAsymptotic { .. }
        ));
    }

    #[test]
    fn non_positive_step_size_is_rejected() {
        let levels = vec![
            RefinementLevel::new(0.4, 1e-1, 1.0),
            RefinementLevel::new(0.0, 1e-2, 2.0),
            RefinementLevel::new(0.1, 1e-3, 3.0),
        ];
        let study = estimate_order(&levels, ConvergenceConfig::default());
        assert!(matches!(
            study.outcome,
            ConvergenceOutcome::NotAsymptotic { .. }
        ));
    }

    #[test]
    fn local_orders_and_cost_are_reported() {
        let sizes = geometric_sizes(0.2, 0.5, 4);
        let levels = analytic_levels(1.0, 2.0, &sizes, 2.0);
        let study = estimate_order(&levels, ConvergenceConfig::default());
        assert_eq!(study.local_orders.len(), 3);
        for p in &study.local_orders {
            assert!((p - 2.0).abs() < 1e-9, "local order {p}");
        }
        // total cost = sum over sizes of (1/h)^2 = 25 + 100 + 400 + 1600.
        assert!((study.total_cost - 2125.0).abs() < 1e-6);
        // Levels are stored coarsest first.
        assert!(study.levels[0].size > study.levels[1].size);
    }

    #[test]
    fn analytic_levels_have_exact_expected_error() {
        let sizes = [0.5, 0.25];
        let levels = analytic_levels(4.0, 2.0, &sizes, 1.0);
        assert!((levels[0].error - 4.0 * 0.25).abs() < 1e-12);
        assert!((levels[1].error - 4.0 * 0.0625).abs() < 1e-12);
    }
}
