// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Bounded, derivative-free single-objective optimization (Phase 36).
//!
//! The default algorithm is a bounded-restart Nelder–Mead simplex method, which
//! needs no derivatives and handles box constraints by projection. It is
//! adequate for the expensive, possibly noisy black-box objective produced by a
//! simulation run. A caller may supply multiple starting points for reproducible
//! multi-start optimization.

use crate::analysis::objective::ParameterSpec;
use crate::core::types::Scalar;

/// Why an optimization run stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizerStopReason {
    /// The simplex converged (spread below tolerance).
    Converged,
    /// The evaluation budget was exhausted.
    MaxEvaluations,
    /// The iteration budget was exhausted.
    MaxIterations,
    /// A stopping criterion detected no further progress.
    Stagnation,
    /// The user cancelled the run.
    Cancelled,
}

impl OptimizerStopReason {
    /// Whether this reason represents a normal, converged finish.
    pub fn is_converged(&self) -> bool {
        matches!(self, Self::Converged)
    }

    /// A short stable identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Converged => "converged",
            Self::MaxEvaluations => "max-evaluations",
            Self::MaxIterations => "max-iterations",
            Self::Stagnation => "stagnation",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Configuration for a bounded optimizer.
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    /// Maximum number of objective evaluations.
    pub max_evaluations: usize,
    /// Maximum number of simplex iterations.
    pub max_iterations: usize,
    /// Convergence tolerance on the simplex function spread.
    pub ftol: Scalar,
    /// Convergence tolerance on the simplex parameter spread.
    pub xtol: Scalar,
    /// Number of consecutive non-improving iterations before declaring
    /// stagnation (`0` disables the check).
    pub stagnation_limit: usize,
    /// Optional external cancellation flag. When it returns `true`, the
    /// optimizer stops at the next safe point.
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            max_evaluations: 2000,
            max_iterations: 1000,
            ftol: 1e-10,
            xtol: 1e-8,
            stagnation_limit: 100,
            cancel: None,
        }
    }
}

impl OptimizerConfig {
    /// Validate tolerances and budgets.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_evaluations == 0 {
            return Err("max_evaluations must be >= 1".to_string());
        }
        if self.max_iterations == 0 {
            return Err("max_iterations must be >= 1".to_string());
        }
        if !(self.ftol.is_finite() && self.ftol > 0.0) {
            return Err(format!("invalid ftol: {}", self.ftol));
        }
        if !(self.xtol.is_finite() && self.xtol > 0.0) {
            return Err(format!("invalid xtol: {}", self.xtol));
        }
        Ok(())
    }

    fn is_cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
    }
}

/// The result of an optimization run.
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizerResult {
    /// Best parameter vector found.
    pub best_params: Vec<Scalar>,
    /// Objective value at `best_params`.
    pub best_value: Scalar,
    /// Number of objective evaluations used.
    pub evaluations: usize,
    /// Number of iterations performed.
    pub iterations: usize,
    /// Why the run stopped.
    pub reason: OptimizerStopReason,
    /// Whether any box constraint was active at the solution.
    pub at_bound: Vec<bool>,
    /// The starting point used (for reproducibility records).
    pub start: Vec<Scalar>,
}

/// A bounded objective function `f: [lo, hi]^n → R`.
pub trait BoundedOptimizer {
    /// Minimize `objective` over the box defined by the parameter specs.
    fn minimize(
        &self,
        objective: &mut dyn FnMut(&[Scalar]) -> Scalar,
        initial: &[Scalar],
        specs: &[ParameterSpec],
        config: &OptimizerConfig,
    ) -> Result<OptimizerResult, String>;
}

/// Project `x` onto the box defined by `specs`.
fn project(x: &[Scalar], specs: &[ParameterSpec]) -> Vec<Scalar> {
    x.iter()
        .enumerate()
        .map(|(i, &v)| specs.get(i).map(|s| s.clamp(v)).unwrap_or(v))
        .collect()
}

/// Run bounded Nelder–Mead from a single starting point.
///
/// The simplex is initialised by perturbing each coordinate by 5% of its range
/// (or 0.00025 when unbounded), then all vertices are projected onto the box.
pub fn nelder_mead(
    objective: &mut dyn FnMut(&[Scalar]) -> Scalar,
    initial: &[Scalar],
    specs: &[ParameterSpec],
    config: &OptimizerConfig,
) -> Result<OptimizerResult, String> {
    config.validate()?;
    let n = initial.len();
    if n == 0 {
        return Err("optimizer requires at least one parameter".to_string());
    }
    if specs.len() != n {
        return Err(format!("expected {n} parameter specs, got {}", specs.len()));
    }

    let eval_count = std::cell::Cell::new(0usize);
    let mut eval = |x: &[Scalar]| -> Scalar {
        eval_count.set(eval_count.get() + 1);
        objective(x)
    };

    // Build the initial simplex: n+1 vertices.
    let mut simplex: Vec<Vec<Scalar>> = Vec::with_capacity(n + 1);
    simplex.push(project(initial, specs));
    for i in 0..n {
        let mut v = initial.to_vec();
        let range = match (specs[i].lower, specs[i].upper) {
            (Some(lo), Some(hi)) => hi - lo,
            _ => 1.0,
        };
        let delta = if range > 0.0 { 0.05 * range } else { 0.00025 };
        v[i] += delta;
        simplex.push(project(&v, specs));
    }
    let mut values: Vec<Scalar> = simplex.iter().map(|v| eval(v)).collect();

    let alpha = 1.0;
    let gamma = 2.0;
    let rho = 0.5;
    let sigma = 0.5;

    let mut iterations = 0usize;
    let mut stagnant = 0usize;
    let mut best = values.iter().cloned().fold(Scalar::INFINITY, Scalar::min);

    let reason = loop {
        if config.is_cancelled() {
            break OptimizerStopReason::Cancelled;
        }
        // Order vertices by value.
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|&a, &b| {
            values[a]
                .partial_cmp(&values[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let sorted_simplex: Vec<Vec<Scalar>> = order.iter().map(|&i| simplex[i].clone()).collect();
        let sorted_values: Vec<Scalar> = order.iter().map(|&i| values[i]).collect();
        simplex = sorted_simplex;
        values = sorted_values;

        let f_best = values[0];
        let f_worst = values[n];
        let f_second_worst = values[n - 1];

        // Convergence tests.
        let fx_spread = (f_worst - f_best).abs();
        let mut max_x_spread = 0.0 as Scalar;
        for v in simplex.iter().skip(1) {
            for i in 0..n {
                max_x_spread = max_x_spread.max((v[i] - simplex[0][i]).abs());
            }
        }
        if fx_spread <= config.ftol && max_x_spread <= config.xtol {
            break OptimizerStopReason::Converged;
        }
        if f_best < best - config.ftol.abs().max(1e-12) {
            best = f_best;
            stagnant = 0;
        } else {
            stagnant += 1;
        }
        if config.stagnation_limit > 0 && stagnant >= config.stagnation_limit {
            break OptimizerStopReason::Stagnation;
        }
        if iterations >= config.max_iterations {
            break OptimizerStopReason::MaxIterations;
        }
        if eval_count.get() >= config.max_evaluations {
            break OptimizerStopReason::MaxEvaluations;
        }
        iterations += 1;

        // Centroid of all but the worst.
        let mut centroid = vec![0.0; n];
        for v in simplex.iter().take(n) {
            for i in 0..n {
                centroid[i] += v[i];
            }
        }
        for c in centroid.iter_mut() {
            *c /= n as Scalar;
        }

        // Reflection.
        let reflected: Vec<Scalar> = (0..n)
            .map(|i| centroid[i] + alpha * (centroid[i] - simplex[n][i]))
            .collect();
        let reflected = project(&reflected, specs);
        let f_reflected = eval(&reflected);

        if f_reflected < f_best {
            // Expansion.
            let expanded: Vec<Scalar> = (0..n)
                .map(|i| centroid[i] + gamma * (reflected[i] - centroid[i]))
                .collect();
            let expanded = project(&expanded, specs);
            let f_expanded = eval(&expanded);
            if f_expanded < f_reflected {
                simplex[n] = expanded;
                values[n] = f_expanded;
            } else {
                simplex[n] = reflected;
                values[n] = f_reflected;
            }
        } else if f_reflected < f_second_worst {
            simplex[n] = reflected;
            values[n] = f_reflected;
        } else {
            // Contraction.
            let contract: Vec<Scalar> = if f_reflected < f_worst {
                (0..n)
                    .map(|i| centroid[i] + rho * (reflected[i] - centroid[i]))
                    .collect()
            } else {
                (0..n)
                    .map(|i| centroid[i] + rho * (simplex[n][i] - centroid[i]))
                    .collect()
            };
            let contract = project(&contract, specs);
            let f_contract = eval(&contract);
            let accepted = if f_reflected < f_worst {
                f_contract <= f_reflected
            } else {
                f_contract < f_worst
            };
            if accepted {
                simplex[n] = contract;
                values[n] = f_contract;
            } else {
                // Shrink toward the best vertex.
                for i in 1..=n {
                    let shrunk: Vec<Scalar> = (0..n)
                        .map(|d| simplex[0][d] + sigma * (simplex[i][d] - simplex[0][d]))
                        .collect();
                    let shrunk = project(&shrunk, specs);
                    simplex[i] = shrunk;
                    values[i] = eval(&simplex[i]);
                }
            }
        }
    };

    // Best vertex after the loop.
    let mut best_idx = 0;
    for i in 1..=n {
        if values[i] < values[best_idx] {
            best_idx = i;
        }
    }
    let best_params = simplex[best_idx].clone();
    let best_value = values[best_idx];
    let at_bound: Vec<bool> = best_params
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let s = &specs[i];
            let hit_lo = s.lower.map(|lo| (v - lo).abs() < 1e-12).unwrap_or(false);
            let hit_hi = s.upper.map(|hi| (hi - v).abs() < 1e-12).unwrap_or(false);
            hit_lo || hit_hi
        })
        .collect();

    let evaluations = eval_count.get();

    Ok(OptimizerResult {
        best_params,
        best_value,
        evaluations,
        iterations,
        reason,
        at_bound,
        start: initial.to_vec(),
    })
}

/// Multi-start optimization: run [`nelder_mead`] from several starting points and
/// return the best result. The starting points are tried in order and results are
/// reproducible for a fixed set of starts.
pub fn multi_start(
    objective: &mut dyn FnMut(&[Scalar]) -> Scalar,
    starts: &[Vec<Scalar>],
    specs: &[ParameterSpec],
    config: &OptimizerConfig,
) -> Result<OptimizerResult, String> {
    if starts.is_empty() {
        return Err("multi-start requires at least one starting point".to_string());
    }
    let mut best: Option<OptimizerResult> = None;
    for start in starts {
        let res = nelder_mead(objective, start, specs, config)?;
        best = match best {
            None => Some(res),
            Some(b) => {
                if res.best_value < b.best_value {
                    Some(res)
                } else {
                    Some(b)
                }
            }
        };
    }
    Ok(best.expect("at least one start produces a result"))
}

/// A zero-cost `BoundedOptimizer` implementation backed by [`nelder_mead`].
#[derive(Debug, Clone, Copy, Default)]
pub struct NelderMeadOptimizer;

impl BoundedOptimizer for NelderMeadOptimizer {
    fn minimize(
        &self,
        objective: &mut dyn FnMut(&[Scalar]) -> Scalar,
        initial: &[Scalar],
        specs: &[ParameterSpec],
        config: &OptimizerConfig,
    ) -> Result<OptimizerResult, String> {
        nelder_mead(objective, initial, specs, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quadratic_specs() -> Vec<ParameterSpec> {
        vec![
            ParameterSpec::free("x", 0.0).with_bounds(-5.0, 5.0),
            ParameterSpec::free("y", 0.0).with_bounds(-5.0, 5.0),
        ]
    }

    #[test]
    fn finds_unconstrained_minimum() {
        // f(x,y) = (x−1)² + (y+2)², min at (1, −2).
        let specs = quadratic_specs();
        let cfg = OptimizerConfig::default();
        let mut f = |p: &[Scalar]| (p[0] - 1.0).powi(2) + (p[1] + 2.0).powi(2);
        let res = nelder_mead(&mut f, &[0.0, 0.0], &specs, &cfg).unwrap();
        assert!(res.reason.is_converged(), "reason={:?}", res.reason);
        assert!(
            (res.best_params[0] - 1.0).abs() < 1e-4,
            "{:?}",
            res.best_params
        );
        assert!(
            (res.best_params[1] + 2.0).abs() < 1e-4,
            "{:?}",
            res.best_params
        );
        assert!(res.best_value < 1e-8);
    }

    #[test]
    fn minimum_outside_bounds_lands_on_bound() {
        // f(x) = (x−100)² decreases on [3,5], so the bounded minimum is at x=5.
        let specs = vec![ParameterSpec::free("x", 4.0).with_bounds(3.0, 5.0)];
        let cfg = OptimizerConfig::default();
        let mut f = |p: &[Scalar]| (p[0] - 100.0).powi(2);
        let res = nelder_mead(&mut f, &[4.0], &specs, &cfg).unwrap();
        assert!(
            (res.best_params[0] - 5.0).abs() < 1e-6,
            "{:?}",
            res.best_params
        );
        assert!(res.at_bound[0]);
    }

    #[test]
    fn minimum_below_bounds_lands_on_lower_bound() {
        // f(x) = (x+100)² increases on [3,5], so the bounded minimum is at x=3.
        let specs = vec![ParameterSpec::free("x", 4.0).with_bounds(3.0, 5.0)];
        let cfg = OptimizerConfig::default();
        let mut f = |p: &[Scalar]| (p[0] + 100.0).powi(2);
        let res = nelder_mead(&mut f, &[4.0], &specs, &cfg).unwrap();
        assert!(
            (res.best_params[0] - 3.0).abs() < 1e-6,
            "{:?}",
            res.best_params
        );
        assert!(res.at_bound[0]);
    }

    #[test]
    fn max_evaluations_is_reported() {
        // A flat objective never converges; the budget must be reported.
        let specs = quadratic_specs();
        let cfg = OptimizerConfig {
            max_evaluations: 7,
            max_iterations: 100000,
            ftol: 1e-300,
            xtol: 1e-300,
            stagnation_limit: 0,
            cancel: None,
        };
        let mut f = |_p: &[Scalar]| 1.0;
        let res = nelder_mead(&mut f, &[0.0, 0.0], &specs, &cfg).unwrap();
        assert!(matches!(
            res.reason,
            OptimizerStopReason::MaxEvaluations | OptimizerStopReason::MaxIterations
        ));
        assert!(!res.reason.is_converged());
    }

    #[test]
    fn cancellation_stops_early() {
        let specs = quadratic_specs();
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag2 = flag.clone();
        let cfg = OptimizerConfig {
            max_iterations: 100000,
            stagnation_limit: 0,
            cancel: Some(flag),
            ..Default::default()
        };
        let mut calls = 0;
        let mut f = |p: &[Scalar]| {
            calls += 1;
            if calls > 10 {
                flag2.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            (p[0] - 1.0).powi(2) + (p[1] + 2.0).powi(2)
        };
        let res = nelder_mead(&mut f, &[0.0, 0.0], &specs, &cfg).unwrap();
        assert_eq!(res.reason, OptimizerStopReason::Cancelled);
        assert!(res.evaluations < 100000);
    }

    #[test]
    fn multi_start_picks_best() {
        let specs = quadratic_specs();
        let cfg = OptimizerConfig::default();
        // A function with a global min at (2,2) and a local min at (−2,−2).
        let mut f = |p: &[Scalar]| {
            let a = (p[0] - 2.0).powi(2) + (p[1] - 2.0).powi(2);
            let b = 5.0 + (p[0] + 2.0).powi(2) + (p[1] + 2.0).powi(2);
            a.min(b)
        };
        let starts = vec![vec![-3.0, -3.0], vec![3.0, 3.0]];
        let res = multi_start(&mut f, &starts, &specs, &cfg).unwrap();
        assert!(
            (res.best_params[0] - 2.0).abs() < 1e-3,
            "{:?}",
            res.best_params
        );
    }

    #[test]
    fn invalid_config_rejected() {
        let specs = quadratic_specs();
        let cfg = OptimizerConfig {
            max_evaluations: 0,
            ..Default::default()
        };
        let mut f = |_p: &[Scalar]| 0.0;
        assert!(nelder_mead(&mut f, &[0.0, 0.0], &specs, &cfg).is_err());
    }
}
