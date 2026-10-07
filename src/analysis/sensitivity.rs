// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Local sensitivity analysis (Phase 36).
//!
//! Provides finite-difference derivatives of a model output with respect to its
//! parameters, plus the perturbation/normalization bookkeeping required to
//! interpret them. The first delivery is finite difference by design (no extra
//! runtime dependencies); adjoint or automatic differentiation can be added
//! later without changing this interface.
//!
//! Every derivative reported by this module is tagged with the step size and
//! scheme used, so results are auditable rather than a bare number.

use crate::analysis::objective::{ObjectiveSpec, ObservationSet};
use crate::analysis::simulation::SimulationFunction;
use crate::core::types::Scalar;

/// Finite-difference scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferenceScheme {
    /// `(f(x+h) − f(x)) / h` — one extra evaluation per parameter.
    Forward,
    /// `(f(x+h) − f(x−h)) / 2h` — two extra evaluations, second-order accurate.
    Central,
}

/// A single parameter's local derivative, with its perturbation context.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSensitivity {
    /// Parameter index.
    pub index: usize,
    /// Parameter path (from the spec, if available).
    pub path: String,
    /// The derivative `dy/dp` at the nominal point.
    pub derivative: Scalar,
    /// The (dimensionless) elasticity `(p/y)·(dy/dp)`, when `y ≠ 0` and `p ≠ 0`.
    pub elasticity: Option<Scalar>,
    /// The step size actually used.
    pub step: Scalar,
    /// The scheme used.
    pub scheme: DifferenceScheme,
}

/// The full local-sensitivity result for a model.
#[derive(Debug, Clone, PartialEq)]
pub struct SensitivityResult {
    /// Nominal output value at the base point.
    pub nominal_output: Scalar,
    /// Per-parameter sensitivities, in parameter order.
    pub sensitivities: Vec<ParameterSensitivity>,
    /// The number of model evaluations performed.
    pub evaluations: usize,
}

impl SensitivityResult {
    /// Look up a sensitivity by parameter path.
    pub fn by_path(&self, path: &str) -> Option<&ParameterSensitivity> {
        self.sensitivities.iter().find(|s| s.path == path)
    }
}

/// A compact summary ranking parameters by |elasticity|.
#[derive(Debug, Clone, PartialEq)]
pub struct SensitivitySummary {
    /// `(path, |elasticity|)` pairs, sorted descending.
    pub ranked: Vec<(String, Scalar)>,
    /// Parameters whose elasticity could not be computed (missing/zero).
    pub incomplete: Vec<String>,
}

impl SensitivityResult {
    /// Produce a ranked summary of relative influence.
    pub fn summary(&self) -> SensitivitySummary {
        let mut ranked: Vec<(String, Scalar)> = Vec::new();
        let mut incomplete = Vec::new();
        for s in &self.sensitivities {
            match s.elasticity {
                Some(e) => ranked.push((s.path.clone(), e.abs())),
                None => incomplete.push(s.path.clone()),
            }
        }
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        SensitivitySummary { ranked, incomplete }
    }
}

/// Error type for sensitivity evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum SensitivityError {
    /// A model evaluation failed (`valid == false`).
    EvaluationFailed(String),
    /// A base or perturbed output was missing the requested channel.
    MissingChannel(String),
    /// A step size was zero or non-finite.
    InvalidStep(usize),
}

impl std::fmt::Display for SensitivityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EvaluationFailed(m) => write!(f, "model evaluation failed: {m}"),
            Self::MissingChannel(c) => write!(f, "output channel '{c}' missing"),
            Self::InvalidStep(i) => write!(f, "invalid step size for parameter {i}"),
        }
    }
}

impl std::error::Error for SensitivityError {}

/// Evaluate the model's cost (from `spec`/`observations`) at a parameter point.
fn evaluate_cost<M: SimulationFunction>(
    model: &M,
    params: &[Scalar],
    observations: &ObservationSet,
    spec: &ObjectiveSpec,
    run_id: &str,
) -> Result<Scalar, SensitivityError> {
    let rec = model.evaluate(params, run_id);
    if !rec.valid {
        return Err(SensitivityError::EvaluationFailed(
            rec.message.unwrap_or_else(|| "unspecified".to_string()),
        ));
    }
    let ys = rec
        .channel(&spec.output_channel)
        .ok_or_else(|| SensitivityError::MissingChannel(spec.output_channel.clone()))?;
    let mut total = 0.0;
    let mut count = 0usize;
    for i in 0..observations.len() {
        if observations.is_missing(i) {
            continue;
        }
        let t = observations.positions[i];
        let model_v = match rec.sample_at(&spec.output_channel, t) {
            Some(v) => v,
            None => continue,
        };
        let _ = ys; // channel presence validated above
        let r = model_v - observations.values[i];
        total += spec.residual_cost(r, observations.weight(i));
        count += 1;
    }
    if count > 0 && spec.loss == crate::analysis::objective::LossFunction::MeanSquaredError {
        total /= count as Scalar;
    }
    Ok(total)
}

/// Compute the finite-difference derivative of the model cost with respect to
/// each parameter, using the supplied per-parameter step sizes.
///
/// This is the "scalar derivative of the scalar objective" sensitivity. It is
/// what calibration and uncertainty propagation consume.
pub fn finite_difference_gradient<M: SimulationFunction>(
    model: &M,
    base: &[Scalar],
    observations: &ObservationSet,
    spec: &ObjectiveSpec,
    steps: &[Scalar],
    scheme: DifferenceScheme,
) -> Result<SensitivityResult, SensitivityError> {
    if steps.len() != base.len() {
        return Err(SensitivityError::InvalidStep(base.len()));
    }
    let mut evaluations = 1usize;
    let nominal = evaluate_cost(model, base, observations, spec, "sens-base")?;
    let mut sensitivities = Vec::with_capacity(base.len());
    for (i, (&p, &h)) in base.iter().zip(steps.iter()).enumerate() {
        if h == 0.0 || !h.is_finite() {
            return Err(SensitivityError::InvalidStep(i));
        }
        let derivative = match scheme {
            DifferenceScheme::Forward => {
                let mut xp = base.to_vec();
                xp[i] = p + h;
                let fp = evaluate_cost(model, &xp, observations, spec, &format!("sens-fwd-{i}"))?;
                evaluations += 1;
                (fp - nominal) / h
            }
            DifferenceScheme::Central => {
                let mut xp = base.to_vec();
                let mut xm = base.to_vec();
                xp[i] = p + h;
                xm[i] = p - h;
                let fp = evaluate_cost(model, &xp, observations, spec, &format!("sens-cp-{i}"))?;
                let fm = evaluate_cost(model, &xm, observations, spec, &format!("sens-cm-{i}"))?;
                evaluations += 2;
                (fp - fm) / (2.0 * h)
            }
        };
        let elasticity = if nominal != 0.0 && p != 0.0 {
            Some((p / nominal) * derivative)
        } else {
            None
        };
        sensitivities.push(ParameterSensitivity {
            index: i,
            path: format!("p{i}"),
            derivative,
            elasticity,
            step: h,
            scheme,
        });
    }
    Ok(SensitivityResult {
        nominal_output: nominal,
        sensitivities,
        evaluations,
    })
}

/// Compute local sensitivities with named parameter paths, using relative step
/// sizes (a fraction of each parameter's magnitude, with an absolute floor).
pub fn local_sensitivity<M: SimulationFunction>(
    model: &M,
    base: &[Scalar],
    observations: &ObservationSet,
    spec: &ObjectiveSpec,
    parameter_paths: &[String],
    relative_step: Scalar,
    scheme: DifferenceScheme,
) -> Result<SensitivityResult, SensitivityError> {
    let steps: Vec<Scalar> = base
        .iter()
        .map(|&p| {
            let rel = relative_step * p.abs();
            rel.max(relative_step * 1.0)
        })
        .collect();
    let mut result = finite_difference_gradient(model, base, observations, spec, &steps, scheme)?;
    for (i, s) in result.sensitivities.iter_mut().enumerate() {
        if let Some(path) = parameter_paths.get(i) {
            s.path = path.clone();
        }
    }
    Ok(result)
}

/// Convenience: compute sensitivities and immediately rank them.
pub fn normalized_sensitivity<M: SimulationFunction>(
    model: &M,
    base: &[Scalar],
    observations: &ObservationSet,
    spec: &ObjectiveSpec,
    parameter_paths: &[String],
    relative_step: Scalar,
) -> Result<SensitivitySummary, SensitivityError> {
    let result = local_sensitivity(
        model,
        base,
        observations,
        spec,
        parameter_paths,
        relative_step,
        DifferenceScheme::Central,
    )?;
    Ok(result.summary())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::simulation::{FnSimulationFunction, SimulationRecord};

    /// Model: y(t) = a·t + b, evaluated on t ∈ {0,1,2,3}.
    fn linear_model() -> FnSimulationFunction<impl Fn(&[Scalar], &str) -> SimulationRecord> {
        FnSimulationFunction::new(2, "linear", |p, id| {
            let (a, b) = (p[0], p[1]);
            let ts = vec![0.0, 1.0, 2.0, 3.0];
            let ys: Vec<Scalar> = ts.iter().map(|&t| a * t + b).collect();
            SimulationRecord::single("y", ts, ys, id)
        })
    }

    #[test]
    fn gradient_recovers_known_derivatives() {
        // Observations from y = 2t + 1 → cost minimal at (2,1); dCost/da and
        // dCost/db have analytic signs, but we validate against central FD of an
        // exactly-known function: ∂/∂a of Σ(a·t+b − y)² at exactly-fit point = 0.
        let model = linear_model();
        let obs = ObservationSet::new(vec![0.0, 1.0, 2.0, 3.0], vec![1.0, 3.0, 5.0, 7.0], vec![])
            .unwrap();
        let spec = ObjectiveSpec::for_output("y");
        let res = finite_difference_gradient(
            &model,
            &[2.0, 1.0],
            &obs,
            &spec,
            &[1e-5, 1e-5],
            DifferenceScheme::Central,
        )
        .unwrap();
        // At the exact fit both derivatives vanish.
        assert!(res.sensitivities[0].derivative.abs() < 1e-4);
        assert!(res.sensitivities[1].derivative.abs() < 1e-4);
    }

    #[test]
    fn gradient_sign_when_offset() {
        // If b is off by one, increasing b increases the cost → positive slope.
        let model = linear_model();
        let obs = ObservationSet::new(vec![0.0, 1.0, 2.0, 3.0], vec![1.0, 3.0, 5.0, 7.0], vec![])
            .unwrap();
        let spec = ObjectiveSpec::for_output("y");
        let res = finite_difference_gradient(
            &model,
            &[2.0, 2.0], // b too large by 1
            &obs,
            &spec,
            &[1e-5, 1e-5],
            DifferenceScheme::Central,
        )
        .unwrap();
        assert!(res.sensitivities[1].derivative > 0.0);
        // Four points each off by +1 → dCost/db = 2·Σ(r) = 2·4 = 8.
        assert!((res.sensitivities[1].derivative - 8.0).abs() < 1e-3);
    }

    #[test]
    fn central_more_accurate_than_forward() {
        // y = p², cost = y². dCost/dp = 4p³. At p=1.5 → 13.5.
        let model = FnSimulationFunction::new(1, "square", |p, id| {
            let v = p[0] * p[0];
            SimulationRecord::single("y", vec![0.0], vec![v], id)
        });
        let obs = ObservationSet::new(vec![0.0], vec![0.0], vec![]).unwrap();
        let spec = ObjectiveSpec::for_output("y");
        let fwd = finite_difference_gradient(
            &model,
            &[1.5],
            &obs,
            &spec,
            &[1e-3],
            DifferenceScheme::Forward,
        )
        .unwrap();
        let cen = finite_difference_gradient(
            &model,
            &[1.5],
            &obs,
            &spec,
            &[1e-3],
            DifferenceScheme::Central,
        )
        .unwrap();
        let expected = 4.0 * 1.5f64.powi(3); // 13.5
        let fwd_err = (fwd.sensitivities[0].derivative - expected).abs();
        let cen_err = (cen.sensitivities[0].derivative - expected).abs();
        assert!(
            cen_err < fwd_err,
            "central {cen_err} should beat forward {fwd_err}"
        );
    }

    #[test]
    fn named_paths_and_summary() {
        let model = linear_model();
        let obs = ObservationSet::new(vec![0.0, 1.0, 2.0, 3.0], vec![1.0, 3.0, 5.0, 7.0], vec![])
            .unwrap();
        let spec = ObjectiveSpec::for_output("y");
        let paths = vec!["slope".to_string(), "intercept".to_string()];
        let res = local_sensitivity(
            &model,
            &[2.5, 1.0],
            &obs,
            &spec,
            &paths,
            1e-4,
            DifferenceScheme::Central,
        )
        .unwrap();
        assert!(res.by_path("slope").is_some());
        assert!(res.by_path("intercept").is_some());
        let summary = res.summary();
        assert_eq!(summary.ranked.len(), 2);
    }

    #[test]
    fn failed_evaluation_is_reported() {
        let model = FnSimulationFunction::new(1, "fail", |_p, id| {
            SimulationRecord::failed(id, "model diverged")
        });
        let obs = ObservationSet::new(vec![0.0], vec![1.0], vec![]).unwrap();
        let spec = ObjectiveSpec::for_output("y");
        let err = finite_difference_gradient(
            &model,
            &[1.0],
            &obs,
            &spec,
            &[1e-5],
            DifferenceScheme::Forward,
        )
        .unwrap_err();
        assert!(matches!(err, SensitivityError::EvaluationFailed(_)));
    }

    #[test]
    fn invalid_step_is_rejected() {
        let model = linear_model();
        let obs = ObservationSet::new(vec![0.0], vec![1.0], vec![]).unwrap();
        let spec = ObjectiveSpec::for_output("y");
        assert!(matches!(
            finite_difference_gradient(
                &model,
                &[1.0, 1.0],
                &obs,
                &spec,
                &[0.0, 1e-5],
                DifferenceScheme::Forward
            ),
            Err(SensitivityError::InvalidStep(_))
        ));
    }
}
