// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Versioned benchmark problems with sourced reference values and justified
//! tolerances (Phase 40).
//!
//! # Why a benchmark is more than a test
//!
//! A unit test asserts that code behaves as the author expected. A *benchmark*
//! asserts that the numerical answer agrees with an external, citable reference
//! (an analytic solution, a published table, a conservation law) to within a
//! tolerance that the author had to *justify*. This module therefore makes the
//! provenance of every reference value — its source, unit, precision and domain
//! of applicability — part of the data, not a comment, and it refuses to build a
//! benchmark whose tolerance has no stated reason.
//!
//! # Categories
//!
//! [`BenchmarkCategory`] distinguishes the four kinds of check the roadmap asks
//! for: unit tests, algorithm regressions, physics benchmarks and performance
//! benchmarks. A performance benchmark carries no numerical reference value (its
//! "reference" is a trend, not a constant), so it is modelled by leaving the
//! reference values empty and marking the check as a trend check.

use crate::core::types::Scalar;
use std::fmt;

/// The kind of evidence a benchmark provides.
///
/// The four variants mirror the roadmap's requirement to "distinguish unit
/// tests, algorithm regressions, physics benchmarks and performance
/// benchmarks".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BenchmarkCategory {
    /// A unit test: verifies a small piece of behaviour in isolation.
    UnitTest,
    /// An algorithm regression: pins the numerical output of an algorithm so a
    /// future change that alters it is caught.
    AlgorithmRegression,
    /// A physics benchmark: compares against an external physical reference
    /// (analytic solution or published measurement).
    PhysicsBenchmark,
    /// A performance benchmark: tracks runtime or memory trends; it has no
    /// fixed numerical reference value.
    PerformanceBenchmark,
}

impl BenchmarkCategory {
    /// Short machine-readable label, stable across releases so reports can be
    /// filtered and diffed.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnitTest => "unit_test",
            Self::AlgorithmRegression => "algorithm_regression",
            Self::PhysicsBenchmark => "physics_benchmark",
            Self::PerformanceBenchmark => "performance_benchmark",
        }
    }

    /// Whether benchmarks of this category are expected to carry at least one
    /// numerical reference value with a tolerance.
    ///
    /// Performance benchmarks are the only category allowed to have no fixed
    /// reference; all others must supply one so that a pass/fail decision is
    /// meaningful.
    pub fn requires_reference_value(self) -> bool {
        !matches!(self, Self::PerformanceBenchmark)
    }
}

impl fmt::Display for BenchmarkCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Provenance of a reference value.
///
/// A reference value is only credible if we can say where it came from and how
/// precisely it is known. This is captured explicitly rather than in a comment.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReferenceSource {
    /// Human-readable citation (paper, standard, textbook section, or a note
    /// that the value is an analytic result).
    pub citation: String,
    /// Number of significant digits the reference is trusted to. Used to warn
    /// when a tolerance is tighter than the reference precision allows.
    pub significant_digits: u8,
    /// Whether the reference is an exact/analytic value (infinite precision)
    /// rather than a measured or tabulated one.
    pub analytic: bool,
}

impl ReferenceSource {
    /// Construct an analytic source with the given citation.
    pub fn analytic(citation: impl Into<String>) -> Self {
        Self {
            citation: citation.into(),
            // Analytic values are exact; a large digit count signals "do not
            // let a measured-precision warning fire spuriously".
            significant_digits: 15,
            analytic: true,
        }
    }

    /// Construct a measured/tabulated source with an explicit precision.
    pub fn measured(citation: impl Into<String>, significant_digits: u8) -> Self {
        Self {
            citation: citation.into(),
            significant_digits,
            analytic: false,
        }
    }

    /// Relative resolution implied by the stated precision.
    ///
    /// For an analytic source this is zero (exact); otherwise it is roughly
    /// `10^-digits`. This is used to detect a tolerance that is too tight to be
    /// justified by the reference's own precision.
    pub fn relative_resolution(&self) -> Scalar {
        if self.analytic {
            0.0
        } else {
            10.0_f64.powi(-(self.significant_digits as i32))
        }
    }
}

/// A single reference value to compare an observed quantity against.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReferenceValue {
    /// Name of the observed quantity this reference applies to.
    pub quantity: String,
    /// The reference number.
    pub value: Scalar,
    /// SI (or otherwise stated) unit of the quantity, e.g. `"m/s"`, `"J"`.
    pub unit: String,
    /// Provenance of the value.
    pub source: ReferenceSource,
    /// Domain of applicability, e.g. `"Re in [1e4, 1e6]"`. Empty means the
    /// reference is intended to hold wherever the benchmark is defined.
    pub applicability: String,
}

impl ReferenceValue {
    /// Construct a reference value, capturing source, unit and applicability.
    pub fn new(
        quantity: impl Into<String>,
        value: Scalar,
        unit: impl Into<String>,
        source: ReferenceSource,
        applicability: impl Into<String>,
    ) -> Self {
        Self {
            quantity: quantity.into(),
            value,
            unit: unit.into(),
            source,
            applicability: applicability.into(),
        }
    }
}

/// The norm used when comparing a field or series of values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ErrorNorm {
    /// Largest absolute difference.
    MaxAbsolute,
    /// Root-mean-square difference.
    Rms,
    /// Largest value after normalising by the reference magnitude.
    RelativeMax,
    /// Root-mean-square difference normalised by the RMS reference magnitude.
    RelativeRms,
}

impl ErrorNorm {
    /// Short machine-readable label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaxAbsolute => "max_abs",
            Self::Rms => "rms",
            Self::RelativeMax => "rel_max",
            Self::RelativeRms => "rel_rms",
        }
    }
}

/// A tolerance together with the reason it is acceptable.
///
/// The reason is mandatory: the roadmap forbids silently relaxing tolerances to
/// turn a failing check green, so a tolerance without a stated rationale cannot
/// be constructed through the normal constructor.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tolerance {
    /// The allowed error magnitude (absolute, or dimensionless for the
    /// relative norms).
    pub value: Scalar,
    /// Which norm the tolerance is expressed in.
    pub norm: ErrorNorm,
    /// Why this tolerance is acceptable — e.g. "second-order spatial scheme
    /// with 64 cells", or "reference only known to 3 significant digits".
    pub reason: String,
}

impl Tolerance {
    /// Construct a tolerance, rejecting an empty reason.
    ///
    /// Returns `Err` when `reason` is blank, which enforces the roadmap rule
    /// that every tolerance must be justified.
    pub fn with_reason(
        value: Scalar,
        norm: ErrorNorm,
        reason: impl Into<String>,
    ) -> Result<Self, ValidationError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(ValidationError::ToleranceReasonRequired { value });
        }
        if !value.is_finite() || value < 0.0 {
            return Err(ValidationError::InvalidTolerance { value });
        }
        Ok(Self {
            value,
            norm,
            reason,
        })
    }

    /// Whether this tolerance is tighter than the reference's own precision can
    /// justify. Callers use this to warn rather than fail, since an analytic
    /// reference has infinite precision.
    pub fn tighter_than_reference(&self, source: &ReferenceSource) -> bool {
        if source.analytic {
            return false;
        }
        self.value < source.relative_resolution()
    }
}

/// A versioned benchmark problem.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Benchmark {
    /// Stable identifier, e.g. `"heat1d/steady_sine"`.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Category of evidence.
    pub category: BenchmarkCategory,
    /// Semver-style version of the *benchmark definition* (not of the code).
    pub version: String,
    /// Reference values the observed quantities are checked against.
    pub references: Vec<ReferenceValue>,
    /// Tolerance applied to all reference values.
    pub tolerance: Tolerance,
}

/// Outcome of checking observed values against a [`Benchmark`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BenchmarkOutcome {
    /// Benchmark id, copied for traceability.
    pub id: String,
    /// Benchmark version, copied for traceability.
    pub version: String,
    /// Whether every reference passed within tolerance.
    pub passed: bool,
    /// The largest observed error over all references, in the tolerance's norm.
    pub max_observed_error: Scalar,
    /// The applied tolerance value, echoed back.
    pub tolerance: Scalar,
    /// Per-quantity observed errors, in insertion order.
    pub per_quantity: Vec<(String, Scalar)>,
    /// A human-readable explanation of the verdict.
    pub detail: String,
}

impl Benchmark {
    /// Build a benchmark, validating its invariants.
    ///
    /// Fails when a category that requires reference values has none, or when a
    /// reference value is not finite.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        category: BenchmarkCategory,
        version: impl Into<String>,
        references: Vec<ReferenceValue>,
        tolerance: Tolerance,
    ) -> Result<Self, ValidationError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ValidationError::EmptyBenchmarkId);
        }
        if category.requires_reference_value() && references.is_empty() {
            return Err(ValidationError::MissingReferenceValues { id: id.clone() });
        }
        for r in &references {
            if !r.value.is_finite() {
                return Err(ValidationError::NonFiniteReference {
                    quantity: r.quantity.clone(),
                    value: r.value,
                });
            }
        }
        Ok(Self {
            id,
            name: name.into(),
            category,
            version: version.into(),
            references,
            tolerance,
        })
    }

    /// Check a set of observed values against the benchmark's references.
    ///
    /// `observed` maps a quantity name to the solver's computed value. A
    /// reference whose quantity is absent from `observed` is reported as an
    /// error for that quantity but does not, on its own, silently pass.
    ///
    /// Returns `Err` only for a structural misuse (the caller supplied no
    /// observations at all); a numerical mismatch is reported through the
    /// returned [`BenchmarkOutcome`] with `passed == false`.
    pub fn check(
        &self,
        observed: &[(String, Scalar)],
    ) -> Result<BenchmarkOutcome, ValidationError> {
        if self.category.requires_reference_value() && self.references.is_empty() {
            // Defensive: `new` prevents this, but deserialization could bypass it.
            return Err(ValidationError::MissingReferenceValues {
                id: self.id.clone(),
            });
        }

        let mut per_quantity = Vec::with_capacity(self.references.len());
        let mut max_err = 0.0 as Scalar;
        let mut missing = Vec::new();

        for reference in &self.references {
            match observed.iter().find(|(k, _)| k == &reference.quantity) {
                Some((_, value)) => {
                    if !value.is_finite() {
                        return Err(ValidationError::NonFiniteObservation {
                            quantity: reference.quantity.clone(),
                            value: *value,
                        });
                    }
                    let err = self.reference_error(reference.value, *value);
                    if err > max_err {
                        max_err = err;
                    }
                    per_quantity.push((reference.quantity.clone(), err));
                }
                None => missing.push(reference.quantity.clone()),
            }
        }

        let passed = missing.is_empty() && max_err <= self.tolerance.value;
        let detail = if !missing.is_empty() {
            format!("missing observed quantities: {}", missing.join(", "))
        } else if passed {
            format!(
                "max {} error {:.3e} <= tolerance {:.3e} ({})",
                self.tolerance.norm.as_str(),
                max_err,
                self.tolerance.value,
                self.tolerance.reason
            )
        } else {
            format!(
                "max {} error {:.3e} > tolerance {:.3e} ({})",
                self.tolerance.norm.as_str(),
                max_err,
                self.tolerance.value,
                self.tolerance.reason
            )
        };

        Ok(BenchmarkOutcome {
            id: self.id.clone(),
            version: self.version.clone(),
            passed,
            max_observed_error: max_err,
            tolerance: self.tolerance.value,
            per_quantity,
            detail,
        })
    }

    /// Error between one reference and one observed value, in the tolerance's
    /// norm.
    ///
    /// For the max norms this is an absolute (or relative) difference; for RMS
    /// norms a single pair degenerates to the same magnitude, which keeps the
    /// scalar-benchmark path identical to the field path.
    pub fn reference_error(&self, reference: Scalar, observed: Scalar) -> Scalar {
        let diff = (observed - reference).abs();
        match self.tolerance.norm {
            ErrorNorm::MaxAbsolute => diff,
            ErrorNorm::Rms => diff,
            ErrorNorm::RelativeMax | ErrorNorm::RelativeRms => {
                let scale = reference.abs();
                if scale > 0.0 { diff / scale } else { diff }
            }
        }
    }
}

/// Error type for benchmark construction and checking.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// A tolerance was requested without a reason.
    ToleranceReasonRequired {
        /// The tolerance value that lacked a reason.
        value: Scalar,
    },
    /// A tolerance value was negative or non-finite.
    InvalidTolerance {
        /// The offending value.
        value: Scalar,
    },
    /// A benchmark was created with an empty id.
    EmptyBenchmarkId,
    /// A category that requires references was created with none.
    MissingReferenceValues {
        /// The benchmark id.
        id: String,
    },
    /// A reference value was not finite.
    NonFiniteReference {
        /// Quantity name.
        quantity: String,
        /// Offending value.
        value: Scalar,
    },
    /// An observed value was not finite.
    NonFiniteObservation {
        /// Quantity name.
        quantity: String,
        /// Offending value.
        value: Scalar,
    },
    /// A convergence study was given insufficient or malformed data.
    ConvergenceData {
        /// Explanation.
        detail: String,
    },
    /// An invariant was violated beyond its tolerance.
    InvariantViolated {
        /// Invariant name.
        name: String,
        /// Observed drift relative to the invariant's scale.
        drift: Scalar,
        /// The invariant's tolerance.
        tolerance: Scalar,
    },
    /// An operation was requested for a model the invariant does not apply to.
    InvariantNotApplicable {
        /// Invariant name.
        name: String,
        /// The model/component it was requested for.
        model: String,
    },
    /// Two series could not be aligned (length, monotonicity or unit mismatch).
    AlignmentFailed {
        /// Explanation.
        detail: String,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ToleranceReasonRequired { value } => {
                write!(f, "tolerance {value:.3e} has no stated reason")
            }
            Self::InvalidTolerance { value } => {
                write!(f, "invalid tolerance value {value}")
            }
            Self::EmptyBenchmarkId => write!(f, "benchmark id must not be empty"),
            Self::MissingReferenceValues { id } => {
                write!(f, "benchmark '{id}' requires at least one reference value")
            }
            Self::NonFiniteReference { quantity, value } => {
                write!(f, "reference value for '{quantity}' is not finite: {value}")
            }
            Self::NonFiniteObservation { quantity, value } => {
                write!(f, "observed value for '{quantity}' is not finite: {value}")
            }
            Self::ConvergenceData { detail } => {
                write!(f, "convergence study data invalid: {detail}")
            }
            Self::InvariantViolated {
                name,
                drift,
                tolerance,
            } => {
                write!(
                    f,
                    "invariant '{name}' violated: drift {drift:.3e} > {tolerance:.3e}"
                )
            }
            Self::InvariantNotApplicable { name, model } => {
                write!(f, "invariant '{name}' is not applicable to '{model}'")
            }
            Self::AlignmentFailed { detail } => {
                write!(f, "series alignment failed: {detail}")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn analytic_ref(q: &str, v: Scalar) -> ReferenceValue {
        ReferenceValue::new(
            q,
            v,
            "1",
            ReferenceSource::analytic("analytic benchmark"),
            "all",
        )
    }

    #[test]
    fn tolerance_requires_a_reason() {
        let err = Tolerance::with_reason(1e-6, ErrorNorm::MaxAbsolute, "   ");
        assert!(matches!(
            err,
            Err(ValidationError::ToleranceReasonRequired { .. })
        ));
        // A non-blank reason is accepted.
        let ok = Tolerance::with_reason(1e-6, ErrorNorm::MaxAbsolute, "second order scheme");
        assert!(ok.is_ok());
    }

    #[test]
    fn benchmark_check_pass_and_fail() {
        let bench = Benchmark::new(
            "analytic/quadratic",
            "Quadratic decay",
            BenchmarkCategory::PhysicsBenchmark,
            "1.0.0",
            vec![analytic_ref("u", 0.25)],
            Tolerance::with_reason(1e-9, ErrorNorm::MaxAbsolute, "exact analytic value").unwrap(),
        )
        .unwrap();

        let pass = bench.check(&[("u".to_string(), 0.25 + 1e-12)]).unwrap();
        assert!(pass.passed, "detail: {}", pass.detail);
        assert!(pass.max_observed_error <= 1e-9);

        let fail = bench.check(&[("u".to_string(), 0.30)]).unwrap();
        assert!(!fail.passed);
        assert!(fail.max_observed_error > 1e-9);
    }

    #[test]
    fn benchmark_missing_observation_does_not_pass() {
        let bench = Benchmark::new(
            "analytic/two",
            "Two quantities",
            BenchmarkCategory::PhysicsBenchmark,
            "1.0.0",
            vec![analytic_ref("u", 1.0), analytic_ref("v", 2.0)],
            Tolerance::with_reason(1e-6, ErrorNorm::MaxAbsolute, "loose").unwrap(),
        )
        .unwrap();
        // Only 'u' supplied: 'v' is missing, so the check must not pass.
        let out = bench.check(&[("u".to_string(), 1.0)]).unwrap();
        assert!(!out.passed);
        assert!(out.detail.contains('v'));
    }

    #[test]
    fn benchmark_requires_reference_values_for_numeric_categories() {
        let err = Benchmark::new(
            "x",
            "no refs",
            BenchmarkCategory::AlgorithmRegression,
            "1.0.0",
            vec![],
            Tolerance::with_reason(1e-6, ErrorNorm::MaxAbsolute, "reason").unwrap(),
        );
        assert!(matches!(
            err,
            Err(ValidationError::MissingReferenceValues { .. })
        ));
    }

    #[test]
    fn performance_benchmark_may_have_no_reference_values() {
        let bench = Benchmark::new(
            "perf/fft",
            "FFT throughput trend",
            BenchmarkCategory::PerformanceBenchmark,
            "1.0.0",
            vec![],
            Tolerance::with_reason(0.2, ErrorNorm::RelativeMax, "trend tolerance").unwrap(),
        );
        assert!(bench.is_ok());
    }

    #[test]
    fn relative_norm_normalises_by_reference() {
        let bench = Benchmark::new(
            "rel",
            "relative",
            BenchmarkCategory::AlgorithmRegression,
            "1.0.0",
            vec![analytic_ref("u", 100.0)],
            Tolerance::with_reason(0.01, ErrorNorm::RelativeMax, "1%").unwrap(),
        )
        .unwrap();
        let out = bench.check(&[("u".to_string(), 101.0)]).unwrap();
        assert!((out.max_observed_error - 0.01).abs() < 1e-12);
        assert!(
            out.passed,
            "exactly at tolerance should pass: {}",
            out.detail
        );
    }

    #[test]
    fn tolerance_tighter_than_reference_is_flagged() {
        let measured = ReferenceSource::measured("table 3.2", 3);
        // 1e-6 is far tighter than a 3-digit reference (resolution 1e-3).
        let tol = Tolerance::with_reason(1e-6, ErrorNorm::MaxAbsolute, "tight").unwrap();
        assert!(tol.tighter_than_reference(&measured));
        let analytic = ReferenceSource::analytic("exact");
        assert!(!tol.tighter_than_reference(&analytic));
    }
}
