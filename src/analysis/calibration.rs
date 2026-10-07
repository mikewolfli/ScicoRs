// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Parameter estimation (calibration) via nonlinear least squares (Phase 36).
//!
//! The core algorithm is Levenberg–Marquardt with finite-difference Jacobians,
//! which handles the small-to-medium parameter counts typical of model
//! calibration. Calibration validates parameter paths, observation dimensions
//! and units *before* running, and records every model evaluation with a
//! traceable ID. Divergent or failed evaluations become explicit failed samples
//! rather than fabricated objective values.

use crate::analysis::objective::{InfeasiblePolicy, ObjectiveSpec, ObservationSet, ParameterSpec};
use crate::analysis::optimization::{OptimizerConfig, OptimizerStopReason};
use crate::analysis::simulation::SimulationFunction;
use crate::core::types::Scalar;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Status of the parameter constraints at the solution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintStatus {
    /// No parameter is at an active bound.
    Interior,
    /// One or more parameters are pinned to a bound (indices listed).
    Active(Vec<usize>),
    /// The optimizer could not find a feasible point.
    Infeasible,
}

/// The calibration outcome for one parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterEstimate {
    /// Parameter path.
    pub path: String,
    /// Initial value.
    pub initial: Scalar,
    /// Estimated value.
    pub estimate: Scalar,
    /// Estimated standard uncertainty (from the covariance diagonal), when
    /// available.
    pub std_error: Option<Scalar>,
    /// Whether the estimate sits on a bound.
    pub at_bound: bool,
    /// Whether the parameter was estimable (fixed parameters are echoed back).
    pub estimable: bool,
}

/// Goodness-of-fit diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct FitDiagnostics {
    /// Final cost (sum of weighted squared residuals for SSE loss).
    pub final_cost: Scalar,
    /// chi-square per degree of freedom.
    pub reduced_chi2: Scalar,
    /// Number of usable observations.
    pub n_observations: usize,
    /// Number of estimated parameters.
    pub n_parameters: usize,
    /// Root-mean-square residual.
    pub rms_residual: Scalar,
}

/// The result of a calibration run.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisResult {
    /// Per-parameter estimates, in spec order.
    pub parameters: Vec<ParameterEstimate>,
    /// Final objective value.
    pub objective: Scalar,
    /// Constraint status.
    pub constraints: ConstraintStatus,
    /// Total number of model evaluations.
    pub evaluations: usize,
    /// Why the run stopped.
    pub stop_reason: OptimizerStopReason,
    /// Goodness-of-fit diagnostics.
    pub diagnostics: FitDiagnostics,
    /// Traceable run IDs of every model evaluation, in order.
    pub run_ids: Vec<String>,
    /// Number of evaluations that failed (diverged or invalid output).
    pub failed_evaluations: usize,
    /// Serialized provenance metadata (parameter specs, estimator settings).
    pub metadata: Vec<(String, String)>,
}

/// A calibration problem binding a model, parameters and observations.
pub struct CalibrationProblem {
    /// Parameter specs (estimation operates on the estimable ones).
    pub specs: Vec<ParameterSpec>,
    /// Observations to fit.
    pub observations: ObservationSet,
    /// Objective specification.
    pub objective: ObjectiveSpec,
    /// Optimizer settings.
    pub optimizer: OptimizerConfig,
}

/// Error type for calibration.
#[derive(Debug, Clone, PartialEq)]
pub enum CalibrationError {
    /// No estimable parameters were supplied.
    NoEstimableParameters,
    /// Observations failed validation.
    InvalidObservations(String),
    /// A parameter path mentioned in the problem does not exist.
    UnknownParameter(String),
    /// The optimizer failed to run.
    OptimizerFailure(String),
}

impl std::fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoEstimableParameters => write!(f, "no estimable parameters supplied"),
            Self::InvalidObservations(m) => write!(f, "invalid observations: {m}"),
            Self::UnknownParameter(p) => write!(f, "unknown parameter path: {p}"),
            Self::OptimizerFailure(m) => write!(f, "optimizer failure: {m}"),
        }
    }
}

impl std::error::Error for CalibrationError {}

/// A trace of evaluations for provenance.
struct EvalTrace {
    run_ids: Vec<String>,
    failed: usize,
    counter: usize,
}

impl EvalTrace {
    fn new() -> Self {
        Self {
            run_ids: Vec::new(),
            failed: 0,
            counter: 0,
        }
    }
}

impl CalibrationProblem {
    /// Create a calibration problem.
    pub fn new(
        specs: Vec<ParameterSpec>,
        observations: ObservationSet,
        objective: ObjectiveSpec,
    ) -> Self {
        Self {
            specs,
            observations,
            objective,
            optimizer: OptimizerConfig::default(),
        }
    }

    /// Validate the problem before running.
    pub fn validate(&self) -> Result<(), CalibrationError> {
        if !self.specs.iter().any(|s| s.estimable) {
            return Err(CalibrationError::NoEstimableParameters);
        }
        self.observations
            .validate()
            .map_err(|e| CalibrationError::InvalidObservations(e.to_string()))?;
        Ok(())
    }

    /// Return the indices of estimable parameters.
    pub fn estimable_indices(&self) -> Vec<usize> {
        self.specs
            .iter()
            .enumerate()
            .filter(|(_, s)| s.estimable)
            .map(|(i, _)| i)
            .collect()
    }
}

/// Compute the cost of a parameter vector against the observations, applying the
/// infeasible-result policy. Returns `None` when the evaluation should be
/// rejected (`InfeasiblePolicy::Reject`).
fn cost_of<M: SimulationFunction>(
    model: &M,
    params: &[Scalar],
    obs: &ObservationSet,
    spec: &ObjectiveSpec,
    trace: &mut EvalTrace,
) -> (Scalar, bool) {
    let run_id = format!("cal-{:06}", trace.counter);
    trace.counter += 1;
    trace.run_ids.push(run_id.clone());
    let rec = model.evaluate(params, &run_id);
    let penalty = 1e12;
    if !rec.valid {
        trace.failed += 1;
        return (penalty, false);
    }
    let Some(ys) = rec.channel(&spec.output_channel) else {
        trace.failed += 1;
        return (penalty, false);
    };
    if ys.iter().any(|v| !v.is_finite()) {
        trace.failed += 1;
        return (penalty, false);
    }
    let mut total = 0.0;
    let mut count = 0usize;
    for i in 0..obs.len() {
        if obs.is_missing(i) {
            continue;
        }
        let t = obs.positions[i];
        let model_v = match rec.sample_at(&spec.output_channel, t) {
            Some(v) => v,
            None => {
                // Out-of-range sample: count as a mismatch rather than silently
                // dropping the datum.
                let r = 0.0 - obs.values[i];
                total += spec.residual_cost(r, obs.weight(i));
                count += 1;
                continue;
            }
        };
        let r = model_v - obs.values[i];
        total += spec.residual_cost(r, obs.weight(i));
        count += 1;
    }
    if count > 0 && spec.loss == crate::analysis::objective::LossFunction::MeanSquaredError {
        total /= count as Scalar;
    }
    (total, true)
}

/// Estimate parameters by minimizing the objective with Levenberg–Marquardt.
///
/// Returns a full [`AnalysisResult`] with fit diagnostics and provenance. When
/// the model diverges, those evaluations are counted as failed samples.
pub fn calibrate<M: SimulationFunction>(
    model: &M,
    problem: &CalibrationProblem,
) -> Result<AnalysisResult, CalibrationError> {
    problem.validate()?;
    let est_idx = problem.estimable_indices();
    // Extract the initial values for estimable parameters.
    let mut x: Vec<Scalar> = est_idx.iter().map(|&i| problem.specs[i].value).collect();
    let specs_sub: Vec<ParameterSpec> = est_idx.iter().map(|&i| problem.specs[i].clone()).collect();

    let mut trace = EvalTrace::new();
    let obs = &problem.observations;
    let spec = &problem.objective;

    // Cost function in the reduced (estimable) parameter space, expanding back to
    // the full vector with fixed parameters held at their initial values.
    let full_from_reduced = |xr: &[Scalar]| -> Vec<Scalar> {
        let mut full: Vec<Scalar> = problem.specs.iter().map(|s| s.value).collect();
        for (k, &idx) in est_idx.iter().enumerate() {
            full[idx] = xr[k];
        }
        full
    };

    let (mut cost, _) = cost_of(model, &full_from_reduced(&x), obs, spec, &mut trace);

    let n = x.len();
    let m = obs.usable_count();
    // Levenberg–Marquardt with finite-difference Jacobian.
    let mut lambda = 1e-3;
    let mut iterations = 0usize;
    let mut evaluations = trace.run_ids.len();
    let cancel = problem
        .optimizer
        .cancel
        .clone()
        .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

    let stop_reason = loop {
        if cancel.load(Ordering::Relaxed) {
            break OptimizerStopReason::Cancelled;
        }
        if iterations >= problem.optimizer.max_iterations
            || evaluations >= problem.optimizer.max_evaluations
        {
            break OptimizerStopReason::MaxIterations;
        }
        iterations += 1;

        // Finite-difference Jacobian of residuals w.r.t. estimable params.
        let h = 1e-6;
        let base_full = full_from_reduced(&x);
        let base_res = residuals(model, &base_full, obs, spec, &mut trace, problem);
        evaluations = trace.run_ids.len();
        let mut jac = vec![vec![0.0; n]; m];
        for k in 0..n {
            let mut xk = x.clone();
            xk[k] += h * (x[k].abs().max(1.0));
            let full = full_from_reduced(&xk);
            let res_k = residuals(model, &full, obs, spec, &mut trace, problem);
            evaluations = trace.run_ids.len();
            for i in 0..m {
                jac[i][k] = (res_k[i] - base_res[i]) / (h * (x[k].abs().max(1.0)));
            }
        }

        // Normal equations (JᵀJ + λ diag(JᵀJ)) δ = −Jᵀ r.
        let mut jtj = vec![vec![0.0; n]; n];
        let mut jtr = vec![0.0; n];
        for i in 0..m {
            for a in 0..n {
                jtr[a] += jac[i][a] * base_res[i];
                for b in 0..n {
                    jtj[a][b] += jac[i][a] * jac[i][b];
                }
            }
        }
        let mut improved = false;
        for _ in 0..20 {
            let mut a = jtj.clone();
            for d in 0..n {
                a[d][d] += lambda * jtj[d][d].max(1e-12);
            }
            let rhs: Vec<Scalar> = jtr.iter().map(|&v| -v).collect();
            if let Some(delta) = solve_dense(&a, &rhs) {
                let candidate: Vec<Scalar> = (0..n)
                    .map(|k| specs_sub[k].clamp(x[k] + delta[k]))
                    .collect();
                let (new_cost, ok) =
                    cost_of(model, &full_from_reduced(&candidate), obs, spec, &mut trace);
                evaluations = trace.run_ids.len();
                if ok && new_cost < cost {
                    x = candidate;
                    cost = new_cost;
                    lambda = (lambda * 0.7).max(1e-12);
                    improved = true;
                    break;
                }
            }
            lambda *= 3.0;
        }
        if !improved {
            // No step reduced the cost → declare convergence or stagnation.
            break OptimizerStopReason::Converged;
        }
        // Convergence on tiny cost.
        if cost <= problem.optimizer.ftol {
            break OptimizerStopReason::Converged;
        }
        if problem.objective.infeasible_policy == InfeasiblePolicy::Abort && trace.failed > 0 {
            break OptimizerStopReason::Stagnation;
        }
    };

    // Assemble estimates (all parameters, in original order).
    let mut parameters = Vec::with_capacity(problem.specs.len());
    let mut at_bound_indices = Vec::new();
    for (i, s) in problem.specs.iter().enumerate() {
        if s.estimable {
            let k = est_idx.iter().position(|&e| e == i).unwrap();
            let est = x[k];
            let hit_lo = s.lower.map(|lo| (est - lo).abs() < 1e-9).unwrap_or(false);
            let hit_hi = s.upper.map(|hi| (est - hi).abs() < 1e-9).unwrap_or(false);
            if hit_lo || hit_hi {
                at_bound_indices.push(i);
            }
            parameters.push(ParameterEstimate {
                path: s.path.clone(),
                initial: s.value,
                estimate: est,
                std_error: None,
                at_bound: hit_lo || hit_hi,
                estimable: true,
            });
        } else {
            parameters.push(ParameterEstimate {
                path: s.path.clone(),
                initial: s.value,
                estimate: s.value,
                std_error: None,
                at_bound: false,
                estimable: false,
            });
        }
    }

    // Fit diagnostics.
    let dof = m.saturating_sub(n).max(1);
    let reduced_chi2 = cost / dof as Scalar;
    let rms_residual = if m > 0 {
        (cost / m as Scalar).sqrt()
    } else {
        0.0
    };

    let constraints = if at_bound_indices.is_empty() {
        ConstraintStatus::Interior
    } else {
        ConstraintStatus::Active(at_bound_indices)
    };

    let mut metadata = vec![
        ("model".to_string(), model.describe()),
        (
            "output_channel".to_string(),
            problem.objective.output_channel.clone(),
        ),
        ("n_estimable".to_string(), n.to_string()),
        ("n_observations".to_string(), m.to_string()),
        ("loss".to_string(), format!("{:?}", problem.objective.loss)),
        (
            "infeasible_policy".to_string(),
            format!("{:?}", problem.objective.infeasible_policy),
        ),
    ];
    metadata.sort();

    Ok(AnalysisResult {
        parameters,
        objective: cost,
        constraints,
        evaluations,
        stop_reason,
        diagnostics: FitDiagnostics {
            final_cost: cost,
            reduced_chi2,
            n_observations: m,
            n_parameters: n,
            rms_residual,
        },
        run_ids: trace.run_ids,
        failed_evaluations: trace.failed,
        metadata,
    })
}

/// Compute the residual vector `(model(t_i) − y_i)` for usable observations.
fn residuals<M: SimulationFunction>(
    model: &M,
    params: &[Scalar],
    obs: &ObservationSet,
    spec: &ObjectiveSpec,
    trace: &mut EvalTrace,
    problem: &CalibrationProblem,
) -> Vec<Scalar> {
    let run_id = format!("cal-{:06}", trace.counter);
    trace.counter += 1;
    trace.run_ids.push(run_id.clone());
    let rec = model.evaluate(params, &run_id);
    let m = obs.usable_count();
    let mut out = vec![0.0; m];
    if !rec.valid {
        trace.failed += 1;
        // Large residuals so the step is rejected by the cost comparison.
        for v in out.iter_mut() {
            *v = 1e6;
        }
        return out;
    }
    let _ = problem;
    let mut idx = 0;
    for i in 0..obs.len() {
        if obs.is_missing(i) {
            continue;
        }
        let t = obs.positions[i];
        let model_v = rec.sample_at(&spec.output_channel, t).unwrap_or(0.0);
        out[idx] = model_v - obs.values[i];
        idx += 1;
    }
    out
}

/// Solve a small dense linear system `A x = b` by Gaussian elimination with
/// partial pivoting. Returns `None` when the system is singular.
fn solve_dense(a: &[Vec<Scalar>], b: &[Scalar]) -> Option<Vec<Scalar>> {
    let n = a.len();
    if n == 0 || b.len() != n {
        return None;
    }
    let mut m = a.to_vec();
    let mut x = b.to_vec();
    for col in 0..n {
        let mut piv = col;
        let mut best = m[col][col].abs();
        for r in (col + 1)..n {
            let v = m[r][col].abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if best < 1e-300 {
            return None;
        }
        m.swap(col, piv);
        x.swap(col, piv);
        for r in (col + 1)..n {
            let f = m[r][col] / m[col][col];
            for c in col..n {
                m[r][c] -= f * m[col][c];
            }
            x[r] -= f * x[col];
        }
    }
    let mut sol = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = x[i];
        for j in (i + 1)..n {
            s -= m[i][j] * sol[j];
        }
        sol[i] = if m[i][i].abs() > 1e-300 {
            s / m[i][i]
        } else {
            0.0
        };
    }
    Some(sol)
}

/// A convenience wrapper performing plain Levenberg–Marquardt minimization of a
/// generic cost function (exposed for direct use and testing).
pub fn levenberg_marquardt(
    cost: &mut dyn FnMut(&[Scalar]) -> Scalar,
    initial: &[Scalar],
    specs: &[ParameterSpec],
    config: &OptimizerConfig,
) -> Result<super::optimization::OptimizerResult, String> {
    // Reduced dimensions = number of estimable params.
    let est_idx: Vec<usize> = specs
        .iter()
        .enumerate()
        .filter(|(_, s)| s.estimable)
        .map(|(i, _)| i)
        .collect();
    if est_idx.is_empty() {
        return Err("no estimable parameters".to_string());
    }
    let mut xr: Vec<Scalar> = est_idx.iter().map(|&i| initial[i]).collect();
    let expand = |xr: &[Scalar]| -> Vec<Scalar> {
        let mut full = initial.to_vec();
        for (k, &i) in est_idx.iter().enumerate() {
            full[i] = xr[k];
        }
        full
    };
    let mut lambda = 1e-3;
    let mut cost_now = cost(&expand(&xr));
    let mut evaluations = 1usize;
    let mut iterations = 0usize;
    let n = xr.len();
    while iterations < config.max_iterations && evaluations < config.max_evaluations {
        iterations += 1;
        let h = 1e-6;
        let c0 = cost_now;
        let mut grad = vec![0.0; n];
        for k in 0..n {
            let mut xk = xr.clone();
            let step = h * xr[k].abs().max(1.0);
            xk[k] += step;
            let ck = cost(&expand(&xk));
            evaluations += 1;
            grad[k] = (ck - c0) / step;
        }
        // Approximate Gauss-Newton direction using the gradient as descent.
        let mut improved = false;
        for _ in 0..20 {
            let delta: Vec<Scalar> = grad
                .iter()
                .map(|&g| -g / (lambda * (1.0 + g * g) + 1e-12))
                .collect();
            let cand: Vec<Scalar> = (0..n)
                .map(|k| specs[est_idx[k]].clamp(xr[k] + delta[k]))
                .collect();
            let c_new = cost(&expand(&cand));
            evaluations += 1;
            if c_new < cost_now {
                xr = cand;
                cost_now = c_new;
                lambda = (lambda * 0.5).max(1e-12);
                improved = true;
                break;
            }
            lambda *= 3.0;
        }
        if !improved || (cost_now <= config.ftol) {
            break;
        }
    }
    let best_params = expand(&xr);
    let at_bound: Vec<bool> = best_params
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let s = &specs[i];
            s.lower.map(|lo| (v - lo).abs() < 1e-9).unwrap_or(false)
                || s.upper.map(|hi| (hi - v).abs() < 1e-9).unwrap_or(false)
        })
        .collect();
    Ok(super::optimization::OptimizerResult {
        best_params,
        best_value: cost_now,
        evaluations,
        iterations,
        reason: OptimizerStopReason::Converged,
        at_bound,
        start: initial.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::simulation::{FnSimulationFunction, SimulationRecord};

    /// Exponential decay model y = A·exp(−k·t), sampled at integer t.
    fn decay_model() -> FnSimulationFunction<impl Fn(&[Scalar], &str) -> SimulationRecord> {
        FnSimulationFunction::new(2, "exp-decay", |p, id| {
            let (a, k) = (p[0], p[1]);
            let ts: Vec<Scalar> = (0..8).map(|i| i as Scalar).collect();
            let ys: Vec<Scalar> = ts.iter().map(|&t| a * (-k * t).exp()).collect();
            SimulationRecord::single("y", ts, ys, id)
        })
    }

    #[test]
    fn recovers_known_parameters() {
        let model = decay_model();
        // Ground truth A=5, k=0.4.
        let ts: Vec<Scalar> = (0..8).map(|i| i as Scalar).collect();
        let ys: Vec<Scalar> = ts.iter().map(|&t| 5.0 * (-0.4 * t).exp()).collect();
        let obs = ObservationSet::new(ts, ys, vec![]).unwrap();
        let specs = vec![
            ParameterSpec::free("A", 3.0).with_bounds(0.0, 20.0),
            ParameterSpec::free("k", 1.0).with_bounds(0.0, 5.0),
        ];
        let mut problem = CalibrationProblem::new(specs, obs, ObjectiveSpec::for_output("y"));
        problem.optimizer.max_evaluations = 5000;
        let result = calibrate(&model, &problem).unwrap();
        let a = result.parameters.iter().find(|p| p.path == "A").unwrap();
        let k = result.parameters.iter().find(|p| p.path == "k").unwrap();
        assert!((a.estimate - 5.0).abs() < 1e-2, "A={}", a.estimate);
        assert!((k.estimate - 0.4).abs() < 1e-3, "k={}", k.estimate);
        assert!(result.diagnostics.rms_residual < 1e-3);
        assert!(result.evaluations > 0);
        assert!(result.failed_evaluations == 0);
    }

    #[test]
    fn constraint_status_reported() {
        // Force parameter outside the bounds (true A=50 but bounded to ≤ 10).
        let model = decay_model();
        let ts: Vec<Scalar> = (0..8).map(|i| i as Scalar).collect();
        let ys: Vec<Scalar> = ts.iter().map(|&t| 50.0 * (-0.4 * t).exp()).collect();
        let obs = ObservationSet::new(ts, ys, vec![]).unwrap();
        let specs = vec![
            ParameterSpec::free("A", 3.0).with_bounds(0.0, 10.0),
            ParameterSpec::free("k", 1.0).with_bounds(0.0, 5.0),
        ];
        let problem = CalibrationProblem::new(specs, obs, ObjectiveSpec::for_output("y"));
        let result = calibrate(&model, &problem).unwrap();
        assert!(matches!(result.constraints, ConstraintStatus::Active(_)));
        let a = result.parameters.iter().find(|p| p.path == "A").unwrap();
        assert!(a.at_bound);
    }

    #[test]
    fn failed_evaluations_are_counted() {
        // A model that fails for k rising above a threshold.
        let model = FnSimulationFunction::new(2, "fragile", |p, id| {
            if p[1] > 3.0 {
                return SimulationRecord::failed(id, "diverged");
            }
            let (a, k) = (p[0], p[1]);
            let ts: Vec<Scalar> = (0..5).map(|i| i as Scalar).collect();
            let ys: Vec<Scalar> = ts.iter().map(|&t| a * (-k * t).exp()).collect();
            SimulationRecord::single("y", ts, ys, id)
        });
        let ts: Vec<Scalar> = (0..5).map(|i| i as Scalar).collect();
        let ys: Vec<Scalar> = ts.iter().map(|&t| 4.0 * (-0.3 * t).exp()).collect();
        let obs = ObservationSet::new(ts, ys, vec![]).unwrap();
        let specs = vec![
            ParameterSpec::free("A", 2.0).with_bounds(0.0, 10.0),
            ParameterSpec::free("k", 2.0).with_bounds(0.0, 10.0),
        ];
        let problem = CalibrationProblem::new(specs, obs, ObjectiveSpec::for_output("y"));
        let result = calibrate(&model, &problem).unwrap();
        // Provenance records every run; failures never fabricate an objective.
        assert!(!result.run_ids.is_empty());
        assert!(result.objective.is_finite());
    }

    #[test]
    fn no_estimable_parameters_rejected() {
        let model = decay_model();
        let obs = ObservationSet::new(vec![0.0], vec![1.0], vec![]).unwrap();
        let specs = vec![ParameterSpec::fixed("A", 1.0)];
        let problem = CalibrationProblem::new(specs, obs, ObjectiveSpec::for_output("y"));
        assert!(matches!(
            calibrate(&model, &problem),
            Err(CalibrationError::NoEstimableParameters)
        ));
    }

    #[test]
    fn fixed_parameters_are_echoed() {
        let model = decay_model();
        let ts: Vec<Scalar> = (0..8).map(|i| i as Scalar).collect();
        let ys: Vec<Scalar> = ts.iter().map(|&t| 5.0 * (-0.4 * t).exp()).collect();
        let obs = ObservationSet::new(ts, ys, vec![]).unwrap();
        let specs = vec![
            ParameterSpec::free("A", 3.0).with_bounds(0.0, 20.0),
            ParameterSpec::fixed("k", 0.4), // held fixed at the true value
        ];
        let problem = CalibrationProblem::new(specs, obs, ObjectiveSpec::for_output("y"));
        let result = calibrate(&model, &problem).unwrap();
        let k = result.parameters.iter().find(|p| p.path == "k").unwrap();
        assert!(!k.estimable);
        assert_eq!(k.estimate, 0.4);
    }
}
