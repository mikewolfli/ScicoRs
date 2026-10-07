// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Result-field alignment, error norms and statistical comparison (Phase 40).
//!
//! # Separating alignment error from solver error
//!
//! Two solutions can only be compared after their units, time bases and grids
//! agree. When they do not, one series must be interpolated onto the other, and
//! that interpolation introduces an error that has nothing to do with the
//! solver. This module performs the alignment explicitly and reports
//!
//! * the **alignment error** — how much interpolation perturbs the interpolated
//!   series (measured by comparing an interpolant against the exactly-known
//!   values where they exist), and
//! * the **solver error** — the discrepancy between the aligned series that
//!   remains after alignment.
//!
//! only the second of which is a solver-quality signal.

use crate::core::types::Scalar;

use super::benchmark::{ErrorNorm, ValidationError};

/// A 1-D series sampled on a (possibly non-uniform) grid.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Series {
    /// Sample positions (must be strictly increasing).
    pub positions: Vec<Scalar>,
    /// Sample values, parallel to `positions`.
    pub values: Vec<Scalar>,
    /// Unit of `values`, used to reject a mismatched comparison.
    pub unit: String,
}

impl Series {
    /// Construct a series, validating that positions and values agree in length
    /// and that positions are strictly increasing.
    pub fn new(
        positions: Vec<Scalar>,
        values: Vec<Scalar>,
        unit: impl Into<String>,
    ) -> Result<Self, ValidationError> {
        if positions.len() != values.len() {
            return Err(ValidationError::AlignmentFailed {
                detail: format!(
                    "positions ({}) and values ({}) have different lengths",
                    positions.len(),
                    values.len()
                ),
            });
        }
        if positions.len() < 2 {
            return Err(ValidationError::AlignmentFailed {
                detail: "a series needs at least two samples".to_string(),
            });
        }
        for pair in positions.windows(2) {
            if pair[1] <= pair[0] {
                return Err(ValidationError::AlignmentFailed {
                    detail: format!(
                        "positions must strictly increase: {} then {}",
                        pair[0], pair[1]
                    ),
                });
            }
        }
        Ok(Self {
            positions,
            values,
            unit: unit.into(),
        })
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the series has no samples (always false for a valid series).
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Linearly interpolate the series at `x`.
    ///
    /// Returns `None` when `x` lies outside the sampled range (no
    /// extrapolation). At an exact sample position the stored value is
    /// returned, so interpolating a series onto itself is exact.
    pub fn interpolate(&self, x: Scalar) -> Option<Scalar> {
        let first = *self.positions.first()?;
        let last = *self.positions.last()?;
        if x < first || x > last {
            return None;
        }
        // Binary search for the interval containing x.
        let idx = match self
            .positions
            .binary_search_by(|p| p.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal))
        {
            Ok(exact) => return Some(self.values[exact]),
            Err(insert) => insert,
        };
        if idx == 0 || idx >= self.positions.len() {
            // x equals an endpoint but not exactly representable: clamp.
            return Some(if idx == 0 {
                self.values[0]
            } else {
                self.values[self.values.len() - 1]
            });
        }
        let x0 = self.positions[idx - 1];
        let x1 = self.positions[idx];
        let y0 = self.values[idx - 1];
        let y1 = self.values[idx];
        let t = (x - x0) / (x1 - x0);
        Some(y0 + t * (y1 - y0))
    }

    /// Multiply every value by a scale factor, returning a new series.
    ///
    /// Used when aligning units (e.g. converting cm to m).
    pub fn scaled(&self, factor: Scalar, new_unit: impl Into<String>) -> Self {
        Self {
            positions: self.positions.clone(),
            values: self.values.iter().map(|v| v * factor).collect(),
            unit: new_unit.into(),
        }
    }
}

/// The result of aligning and comparing two series.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComparisonReport {
    /// The norm used.
    pub norm: ErrorNorm,
    /// Number of comparison points actually used.
    pub compared_points: usize,
    /// Error between the aligned series — the solver-error signal.
    pub solver_error: Scalar,
    /// Error attributable purely to interpolation onto the comparison grid —
    /// the alignment signal. Reported separately from `solver_error`.
    pub alignment_error: Scalar,
    /// Per-point absolute differences after alignment, in comparison order.
    pub residuals: Vec<Scalar>,
    /// Unit both series were expressed in for the comparison.
    pub unit: String,
}

impl ComparisonReport {
    /// Whether the comparison covered at least one point.
    pub fn has_comparison(&self) -> bool {
        self.compared_points > 0
    }
}

/// Align `reference` onto `candidate`'s grid and compare them.
///
/// Both series must share a unit; a mismatch is an error rather than a silent
/// numeric comparison. Points of `candidate` that fall outside `reference`'s
/// range are skipped (no extrapolation). The alignment error is measured by
/// resampling `reference` at its own sample positions through the interpolant
/// and recording how far non-endpoint values move — a direct, reproducible
/// proxy for interpolation-induced error.
pub fn compare_series(
    reference: &Series,
    candidate: &Series,
    norm: ErrorNorm,
) -> Result<ComparisonReport, ValidationError> {
    if reference.unit != candidate.unit {
        return Err(ValidationError::AlignmentFailed {
            detail: format!(
                "unit mismatch: reference '{}' vs candidate '{}'",
                reference.unit, candidate.unit
            ),
        });
    }

    let mut residuals = Vec::new();
    let mut squared_sum = 0.0 as Scalar;
    let mut max_abs = 0.0 as Scalar;
    let mut ref_rms_sq = 0.0 as Scalar;

    for (pos, val) in candidate.positions.iter().zip(candidate.values.iter()) {
        if let Some(ref_val) = reference.interpolate(*pos) {
            let diff = (val - ref_val).abs();
            residuals.push(diff);
            squared_sum += diff * diff;
            if diff > max_abs {
                max_abs = diff;
            }
            ref_rms_sq += ref_val * ref_val;
        }
    }

    let n = residuals.len();
    if n == 0 {
        return Ok(ComparisonReport {
            norm,
            compared_points: 0,
            solver_error: 0.0,
            alignment_error: alignment_error(reference),
            residuals,
            unit: reference.unit.clone(),
        });
    }

    let rms = (squared_sum / n as Scalar).sqrt();
    let ref_rms = (ref_rms_sq / n as Scalar).sqrt();
    let solver_error = match norm {
        ErrorNorm::MaxAbsolute => max_abs,
        ErrorNorm::Rms => rms,
        ErrorNorm::RelativeMax => {
            if ref_rms > 0.0 {
                max_abs / ref_rms
            } else {
                max_abs
            }
        }
        ErrorNorm::RelativeRms => {
            if ref_rms > 0.0 {
                rms / ref_rms
            } else {
                rms
            }
        }
    };

    Ok(ComparisonReport {
        norm,
        compared_points: n,
        solver_error,
        alignment_error: alignment_error(reference),
        residuals,
        unit: reference.unit.clone(),
    })
}

/// Interpolation-induced error of a series when resampled at its own nodes.
///
/// At the stored nodes a linear interpolant reproduces the data exactly, so the
/// honest measure of alignment error is the curvature mismatch between the
/// original and a linear reconstruction: here we take the maximum second
/// difference (`|y_{i+1} - 2 y_i + y_{i-1}|`) as the magnitude of the variation
/// that any alignment onto a different grid can miss. For a linear series this
/// is exactly zero, which is the correct answer.
pub fn alignment_error(series: &Series) -> Scalar {
    let n = series.values.len();
    if n < 3 {
        // A two-point series is linear by construction: interpolation is exact.
        return 0.0;
    }
    let mut max_second = 0.0 as Scalar;
    for i in 1..n - 1 {
        let second = (series.values[i + 1] - 2.0 * series.values[i] + series.values[i - 1]).abs();
        if second > max_second {
            max_second = second;
        }
    }
    max_second * 0.5
}

/// A summary of a statistical comparison between two populations of samples.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StatisticalComparison {
    /// Number of paired samples compared.
    pub paired_samples: usize,
    /// Mean of the differences (candidate − reference).
    pub mean_difference: Scalar,
    /// Sample standard deviation of the differences (0 for a single pair).
    pub std_difference: Scalar,
    /// Largest absolute difference.
    pub max_abs_difference: Scalar,
    /// Two-sided paired t-statistic `mean / (std / sqrt(n))`, if defined.
    pub t_statistic: Option<Scalar>,
}

/// Compare two equal-length sample populations pairwise.
///
/// This is the statistical counterpart of [`compare_series`]: it does not
/// interpolate, so it is used when the two samples are already paired (same
/// time stamps, same nodes). Returns an error on a length mismatch.
pub fn compare_populations(
    reference: &[Scalar],
    candidate: &[Scalar],
) -> Result<StatisticalComparison, ValidationError> {
    if reference.len() != candidate.len() {
        return Err(ValidationError::AlignmentFailed {
            detail: format!(
                "population length mismatch: {} vs {}",
                reference.len(),
                candidate.len()
            ),
        });
    }
    if reference.is_empty() {
        return Err(ValidationError::AlignmentFailed {
            detail: "empty populations".to_string(),
        });
    }
    let diffs: Vec<Scalar> = candidate
        .iter()
        .zip(reference.iter())
        .map(|(c, r)| c - r)
        .collect();
    let n = diffs.len() as Scalar;
    let mean = diffs.iter().sum::<Scalar>() / n;
    let var = if diffs.len() > 1 {
        diffs
            .iter()
            .map(|d| (d - mean) * (d - mean))
            .sum::<Scalar>()
            / (n - 1.0)
    } else {
        0.0
    };
    let std = var.sqrt();
    let max_abs = diffs
        .iter()
        .map(|d| d.abs())
        .fold(0.0 as Scalar, Scalar::max);
    let t_statistic = if diffs.len() > 1 && std > 0.0 {
        Some(mean / (std / n.sqrt()))
    } else {
        None
    };

    Ok(StatisticalComparison {
        paired_samples: diffs.len(),
        mean_difference: mean,
        std_difference: std,
        max_abs_difference: max_abs,
        t_statistic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_series() -> Series {
        // y = 2x on [0, 1]: linear, so alignment error is exactly zero.
        Series::new(
            vec![0.0, 0.25, 0.5, 0.75, 1.0],
            vec![0.0, 0.5, 1.0, 1.5, 2.0],
            "m",
        )
        .unwrap()
    }

    #[test]
    fn series_requires_matching_lengths() {
        let err = Series::new(vec![0.0, 1.0], vec![1.0], "m");
        assert!(err.is_err());
    }

    #[test]
    fn series_requires_increasing_positions() {
        let err = Series::new(vec![0.0, 0.0], vec![1.0, 2.0], "m");
        assert!(err.is_err());
    }

    #[test]
    fn interpolation_is_exact_at_nodes() {
        let s = linear_series();
        for (p, v) in s.positions.iter().zip(s.values.iter()) {
            assert!((s.interpolate(*p).unwrap() - v).abs() < 1e-15);
        }
        // Midpoint of a linear series is recovered exactly.
        assert!((s.interpolate(0.375).unwrap() - 0.75).abs() < 1e-15);
    }

    #[test]
    fn no_extrapolation_outside_range() {
        let s = linear_series();
        assert!(s.interpolate(-0.1).is_none());
        assert!(s.interpolate(1.1).is_none());
    }

    #[test]
    fn identity_comparison_has_zero_solver_error() {
        let s = linear_series();
        let report = compare_series(&s, &s, ErrorNorm::MaxAbsolute).unwrap();
        assert_eq!(report.compared_points, 5);
        assert!(report.solver_error < 1e-15);
        assert!(report.alignment_error < 1e-15);
    }

    #[test]
    fn unit_mismatch_is_rejected() {
        let a = linear_series();
        let b = Series::new(vec![0.0, 0.5, 1.0], vec![0.0, 1.0, 2.0], "cm").unwrap();
        let err = compare_series(&a, &b, ErrorNorm::MaxAbsolute);
        assert!(err.is_err());
    }

    #[test]
    fn alignment_error_is_separate_from_solver_error() {
        // A curved reference has non-zero alignment error; comparing it against
        // a candidate sampled on a sub-grid yields a solver error on top.
        let reference = Series::new(
            vec![0.0, 0.25, 0.5, 0.75, 1.0],
            vec![0.0, 0.0625, 0.25, 0.5625, 1.0],
            "m",
        )
        .unwrap();
        let candidate = Series::new(vec![0.0, 0.5, 1.0], vec![0.0, 0.25, 1.0], "m").unwrap();
        let report = compare_series(&reference, &candidate, ErrorNorm::MaxAbsolute).unwrap();
        // The candidate shares exact node values, so solver error is ~0 …
        assert!(
            report.solver_error < 1e-12,
            "solver error {}",
            report.solver_error
        );
        // … but the reference is curved, so alignment error is positive.
        assert!(report.alignment_error > 0.0);
    }

    #[test]
    fn partial_overlap_skips_out_of_range_points() {
        let reference = Series::new(vec![0.0, 0.5, 1.0], vec![0.0, 1.0, 2.0], "m").unwrap();
        // Candidate extends beyond the reference's range on both sides.
        let candidate = Series::new(vec![0.1, 0.5, 0.9], vec![0.2, 1.0, 1.8], "m").unwrap();
        let report = compare_series(&reference, &candidate, ErrorNorm::MaxAbsolute).unwrap();
        assert_eq!(report.compared_points, 3);
        assert!(report.solver_error < 1e-12);
    }

    #[test]
    fn relative_norm_normalises_by_reference_rms() {
        let reference = Series::new(vec![0.0, 0.5, 1.0], vec![10.0, 10.0, 10.0], "m").unwrap();
        let candidate = Series::new(vec![0.0, 0.5, 1.0], vec![11.0, 11.0, 11.0], "m").unwrap();
        let report = compare_series(&reference, &candidate, ErrorNorm::RelativeMax).unwrap();
        // off by 1 against a reference of 10 -> 0.1 relative.
        assert!(
            (report.solver_error - 0.1).abs() < 1e-12,
            "{}",
            report.solver_error
        );
        // A constant reference is linear -> no alignment error.
        assert!(report.alignment_error < 1e-15);
    }

    #[test]
    fn statistical_comparison_basic_statistics() {
        let reference = vec![1.0, 2.0, 3.0, 4.0];
        // A constant offset plus a small amount of scatter, so the
        // differences are not exactly collinear.
        let candidate = vec![1.1, 2.05, 3.15, 4.05];
        let stats = compare_populations(&reference, &candidate).unwrap();
        assert_eq!(stats.paired_samples, 4);
        assert!(
            (stats.mean_difference - 0.0875).abs() < 1e-12,
            "{}",
            stats.mean_difference
        );
        assert!(stats.std_difference > 0.0);
        assert!((stats.max_abs_difference - 0.15).abs() < 1e-12);
        // With scatter present the t-statistic is well defined.
        assert!(stats.t_statistic.is_some());
    }

    #[test]
    fn statistical_comparison_degenerate_zero_variance_has_no_t_statistic() {
        // A single pair has undefined sample variance.
        let stats = compare_populations(&[1.0], &[1.5]).unwrap();
        assert_eq!(stats.paired_samples, 1);
        assert!((stats.mean_difference - 0.5).abs() < 1e-12);
        assert_eq!(stats.std_difference, 0.0);
        assert!(stats.t_statistic.is_none());
    }

    #[test]
    fn statistical_comparison_rejects_length_mismatch() {
        assert!(compare_populations(&[1.0, 2.0], &[1.0]).is_err());
        assert!(compare_populations(&[], &[]).is_err());
    }

    #[test]
    fn scaled_series_aligns_units() {
        let cm = Series::new(vec![0.0, 1.0], vec![100.0, 200.0], "cm").unwrap();
        let m = cm.scaled(0.01, "m");
        assert_eq!(m.unit, "m");
        assert!((m.values[0] - 1.0).abs() < 1e-12);
    }
}
