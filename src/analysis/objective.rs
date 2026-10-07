// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Core data contracts for analysis: parameters, observations and objectives.
//!
//! These types are the shared vocabulary that calibration, sensitivity,
//! uncertainty and optimization all speak. Every value carries enough context
//! (units, bounds, weights) to be interpreted without out-of-band knowledge.

use crate::core::types::Scalar;

/// How a parameter is transformed before being handed to the model.
///
/// The transform is applied on the *outside* of the raw parameter; the raw value
/// is what appears in results and is what bounds apply to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterTransform {
    /// No transform.
    Identity,
    /// `exp(x)` — for strictly positive parameters modelled on a log scale.
    /// Estimates happen in log space so bounds are enforced naturally.
    Log,
    /// `logit(x)` mapping `(0,1)` to the real line; the raw value stays in
    /// `(0,1)`.
    Logit,
}

impl ParameterTransform {
    /// Apply the forward transform (raw → internal).
    pub fn forward(&self, raw: Scalar) -> Scalar {
        match self {
            Self::Identity => raw,
            Self::Log => raw.exp(),
            Self::Logit => {
                // Clamp to avoid infinities at the boundary.
                let c = raw.clamp(1e-12, 1.0 - 1e-12);
                (c / (1.0 - c)).ln()
            }
        }
    }

    /// Apply the inverse transform (internal → raw).
    pub fn inverse(&self, internal: Scalar) -> Scalar {
        match self {
            Self::Identity => internal,
            Self::Log => internal.ln(),
            Self::Logit => {
                let e = internal.exp();
                e / (1.0 + e)
            }
        }
    }
}

/// Specification of a single estimable parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSpec {
    /// Stable dotted path identifying the parameter (e.g. `reactor.k`).
    pub path: String,
    /// Current/initial raw value.
    pub value: Scalar,
    /// Optional physical unit label (informational; conversions are the caller's
    /// responsibility and are validated for consistency).
    pub unit: Option<String>,
    /// Optional lower bound (raw space).
    pub lower: Option<Scalar>,
    /// Optional upper bound (raw space).
    pub upper: Option<Scalar>,
    /// Whether this parameter participates in estimation. Non-estimable
    /// parameters are held fixed but still recorded.
    pub estimable: bool,
    /// Transform applied before the model consumes the value.
    pub transform: ParameterTransform,
}

impl ParameterSpec {
    /// Construct a free (estimable) identity-transformed parameter.
    pub fn free(path: &str, value: Scalar) -> Self {
        Self {
            path: path.to_string(),
            value,
            unit: None,
            lower: None,
            upper: None,
            estimable: true,
            transform: ParameterTransform::Identity,
        }
    }

    /// Construct a fixed (non-estimable) parameter.
    pub fn fixed(path: &str, value: Scalar) -> Self {
        Self {
            estimable: false,
            ..Self::free(path, value)
        }
    }

    /// Set bounds.
    pub fn with_bounds(mut self, lower: Scalar, upper: Scalar) -> Self {
        self.lower = Some(lower);
        self.upper = Some(upper);
        self
    }

    /// Set the transform.
    pub fn with_transform(mut self, transform: ParameterTransform) -> Self {
        self.transform = transform;
        self
    }

    /// Set the unit label.
    pub fn with_unit(mut self, unit: &str) -> Self {
        self.unit = Some(unit.to_string());
        self
    }

    /// Clamp a raw value to this parameter's bounds.
    pub fn clamp(&self, v: Scalar) -> Scalar {
        let mut r = v;
        if let Some(lo) = self.lower {
            r = r.max(lo);
        }
        if let Some(hi) = self.upper {
            r = r.min(hi);
        }
        r
    }
}

/// A set of observations used to fit or validate a model.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservationSet {
    /// Time (or 1-D location) of each observation. Must be strictly increasing
    /// unless [`Self::missing`] marks the corresponding sample missing.
    pub positions: Vec<Scalar>,
    /// Observed values, parallel to `positions`.
    pub values: Vec<Scalar>,
    /// Standard deviation (or weight inverse) per observation. Empty means "all
    /// unit weights".
    pub sigmas: Vec<Scalar>,
    /// Unit label of the observed quantity.
    pub unit: Option<String>,
    /// Indices flagged as missing; their values are ignored in the loss.
    pub missing: Vec<usize>,
}

/// Error constructing or using an [`ObservationSet`].
#[derive(Debug, Clone, PartialEq)]
pub enum ObservationError {
    /// Parallel arrays had different lengths.
    LengthMismatch(String),
    /// Positions were not strictly increasing.
    NonMonotonicPositions(usize),
    /// A sigma was non-positive.
    InvalidSigma(usize),
    /// Missing indices out of range.
    MissingIndexOutOfRange(usize),
}

impl std::fmt::Display for ObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LengthMismatch(d) => write!(f, "observation length mismatch: {d}"),
            Self::NonMonotonicPositions(i) => {
                write!(f, "observation positions not increasing at index {i}")
            }
            Self::InvalidSigma(i) => write!(f, "observation sigma must be > 0 (index {i})"),
            Self::MissingIndexOutOfRange(i) => write!(f, "missing index {i} out of range"),
        }
    }
}

impl std::error::Error for ObservationError {}

impl ObservationSet {
    /// Build and validate an observation set.
    pub fn new(
        positions: Vec<Scalar>,
        values: Vec<Scalar>,
        sigmas: Vec<Scalar>,
    ) -> Result<Self, ObservationError> {
        let set = Self {
            positions,
            values,
            sigmas,
            unit: None,
            missing: Vec::new(),
        };
        set.validate()?;
        Ok(set)
    }

    /// Validate lengths, monotonicity, sigma positivity and missing indices.
    pub fn validate(&self) -> Result<(), ObservationError> {
        if self.positions.len() != self.values.len() {
            return Err(ObservationError::LengthMismatch(format!(
                "positions {} vs values {}",
                self.positions.len(),
                self.values.len()
            )));
        }
        if !self.sigmas.is_empty() && self.sigmas.len() != self.values.len() {
            return Err(ObservationError::LengthMismatch(format!(
                "sigmas {} vs values {}",
                self.sigmas.len(),
                self.values.len()
            )));
        }
        let mut prev = Scalar::NEG_INFINITY;
        for (i, &p) in self.positions.iter().enumerate() {
            if p <= prev {
                return Err(ObservationError::NonMonotonicPositions(i));
            }
            prev = p;
        }
        for (i, &s) in self.sigmas.iter().enumerate() {
            if s <= 0.0 || !s.is_finite() {
                return Err(ObservationError::InvalidSigma(i));
            }
        }
        for &m in &self.missing {
            if m >= self.values.len() {
                return Err(ObservationError::MissingIndexOutOfRange(m));
            }
        }
        Ok(())
    }

    /// Number of observations (including missing ones).
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Whether index `i` is flagged missing.
    pub fn is_missing(&self, i: usize) -> bool {
        self.missing.contains(&i)
    }

    /// Weight for observation `i` (1/sigma, or 1 when no sigmas supplied).
    pub fn weight(&self, i: usize) -> Scalar {
        if self.sigmas.is_empty() {
            1.0
        } else {
            1.0 / self.sigmas[i]
        }
    }

    /// Number of usable (non-missing) observations.
    pub fn usable_count(&self) -> usize {
        self.values.len() - self.missing.len()
    }
}

/// The loss function used to compare model output with observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossFunction {
    /// Ordinary sum of squared (weighted) residuals.
    SumSquaredError,
    /// Mean of squared (weighted) residuals.
    MeanSquaredError,
    /// Sum of absolute (weighted) residuals — robust to outliers.
    AbsoluteError,
    /// Huber loss with the given transition parameter (set via
    /// [`ObjectiveSpec::huber_delta`]).
    Huber,
}

/// Specification of what the optimizer minimizes.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectiveSpec {
    /// The comparison loss.
    pub loss: LossFunction,
    /// Name of the model output channel used for comparison.
    pub output_channel: String,
    /// Expected noise scale (also the observation default sigma when unset).
    pub noise_scale: Scalar,
    /// Huber transition parameter, used when `loss == Huber`.
    pub huber_delta: Scalar,
    /// L2 regularization weight applied to parameter deviations from the initial
    /// value (0 disables regularization).
    pub regularization: Scalar,
    /// What to do when the model reports an infeasible / failed evaluation.
    pub infeasible_policy: InfeasiblePolicy,
}

/// Policy for handling model evaluations that fail to produce a finite output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfeasiblePolicy {
    /// Treat the evaluation as a large but finite penalty (keeps optimizers
    /// moving) and record it as a failed sample.
    Penalty,
    /// Reject the step (the optimizer must stay at the last feasible point).
    Reject,
    /// Stop the whole analysis and report the failure.
    Abort,
}

impl Default for ObjectiveSpec {
    fn default() -> Self {
        Self {
            loss: LossFunction::SumSquaredError,
            output_channel: "output".to_string(),
            noise_scale: 1.0,
            huber_delta: 1.0,
            regularization: 0.0,
            infeasible_policy: InfeasiblePolicy::Penalty,
        }
    }
}

impl ObjectiveSpec {
    /// Construct a spec for a named output channel.
    pub fn for_output(channel: &str) -> Self {
        Self {
            output_channel: channel.to_string(),
            ..Default::default()
        }
    }

    /// Compute the loss contribution of a single residual `r = model − observed`
    /// with the given weight.
    pub fn residual_cost(&self, r: Scalar, weight: Scalar) -> Scalar {
        let w = weight * weight;
        match self.loss {
            LossFunction::SumSquaredError | LossFunction::MeanSquaredError => w * r * r,
            LossFunction::AbsoluteError => w * r.abs(),
            LossFunction::Huber => {
                let a = r.abs();
                if a <= self.huber_delta {
                    0.5 * w * r * r
                } else {
                    w * self.huber_delta * (a - 0.5 * self.huber_delta)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_roundtrip() {
        let log = ParameterTransform::Log;
        let raw = 3.5;
        assert!((log.inverse(log.forward(raw)) - raw).abs() < 1e-12);
        let logit = ParameterTransform::Logit;
        let p = 0.3;
        assert!((logit.inverse(logit.forward(p)) - p).abs() < 1e-9);
    }

    #[test]
    fn parameter_bounds_clamp() {
        let p = ParameterSpec::free("k", 1.0).with_bounds(0.0, 2.0);
        assert_eq!(p.clamp(5.0), 2.0);
        assert_eq!(p.clamp(-5.0), 0.0);
        assert_eq!(p.clamp(1.5), 1.5);
    }

    #[test]
    fn observation_set_validates() {
        let ok = ObservationSet::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0], vec![]).unwrap();
        assert_eq!(ok.len(), 3);
        assert_eq!(ok.usable_count(), 3);
        // Non-monotonic rejected.
        assert!(ObservationSet::new(vec![1.0, 0.0], vec![1.0, 2.0], vec![]).is_err());
        // Bad sigma rejected.
        assert!(ObservationSet::new(vec![0.0, 1.0], vec![1.0, 2.0], vec![1.0, -1.0]).is_err());
        // Length mismatch.
        assert!(ObservationSet::new(vec![0.0, 1.0], vec![1.0], vec![]).is_err());
    }

    #[test]
    fn missing_points_reduce_usable_count() {
        let mut obs =
            ObservationSet::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0], vec![]).unwrap();
        obs.missing.push(1);
        assert_eq!(obs.usable_count(), 2);
        assert!(obs.is_missing(1));
        assert!(!obs.is_missing(0));
    }

    #[test]
    fn loss_functions() {
        let spec = ObjectiveSpec::default();
        // SSE with unit weight: r=2 → 4.
        assert!((spec.residual_cost(2.0, 1.0) - 4.0).abs() < 1e-12);
        let abs = ObjectiveSpec {
            loss: LossFunction::AbsoluteError,
            ..Default::default()
        };
        assert!((abs.residual_cost(-3.0, 1.0) - 3.0).abs() < 1e-12);
        let huber = ObjectiveSpec {
            loss: LossFunction::Huber,
            huber_delta: 1.0,
            ..Default::default()
        };
        // Within delta: quadratic.
        assert!((huber.residual_cost(0.5, 1.0) - 0.125).abs() < 1e-12);
        // Beyond delta: linear.
        assert!((huber.residual_cost(2.0, 1.0) - 1.5).abs() < 1e-12);
    }
}
