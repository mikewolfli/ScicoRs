// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Sparse solver diagnostics: convergence statistics, stop reasons and
//! condition-number estimation.
//!
//! These types are shared by every Krylov solver in [`super::iterative`] so that
//! a caller can inspect *why* a solve stopped and how trustworthy the result is,
//! instead of only receiving a solution vector.

use super::matrix::CsrMatrix;
use crate::core::types::Scalar;

/// The reason a Krylov iteration stopped.
///
/// The distinction between [`Self::Converged`] and every other variant is
/// load-bearing: an iteration that hits the limit or stagnates must never be
/// reported as a successful solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The residual tolerance was met.
    Converged,
    /// The iteration limit was reached without meeting the tolerance.
    MaxIterationsReached,
    /// The residual norm stagnated (no meaningful reduction over a window).
    Stagnation,
    /// A breakdown occurred (e.g. a division by ~zero pivot in BiCGSTAB/sqrt of
    /// a negative inner product in CG).
    Breakdown,
    /// An input was invalid (shape mismatch, wrong symmetry, non-finite data).
    InvalidInput,
}

impl StopReason {
    /// Whether this stop reason represents a successful solve.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Converged)
    }

    /// A short stable identifier for reporting.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Converged => "converged",
            Self::MaxIterationsReached => "max-iterations",
            Self::Stagnation => "stagnation",
            Self::Breakdown => "breakdown",
            Self::InvalidInput => "invalid-input",
        }
    }
}

/// Convergence statistics for a single Krylov solve.
#[derive(Debug, Clone, PartialEq)]
pub struct IterationStats {
    /// Number of iterations actually performed.
    pub iterations: usize,
    /// The initial residual norm (`‖b − A·x₀‖` in the solver's norm).
    pub initial_residual: Scalar,
    /// The final residual norm.
    pub final_residual: Scalar,
    /// `final_residual / initial_residual` (0 when the initial residual is 0 and
    /// the answer was already exact).
    pub relative_residual: Scalar,
    /// The tolerance the solve was targeting.
    pub tolerance: Scalar,
    /// Why the solve stopped.
    pub reason: StopReason,
    /// Residual history (one entry per iteration, including the initial one).
    pub residual_history: Vec<Scalar>,
}

impl IterationStats {
    /// Whether the solve converged within tolerance.
    pub fn converged(&self) -> bool {
        self.reason.is_success()
    }
}

/// Build an [`IterationStats`] value from a residual history and stop reason.
pub(crate) fn summarize(
    history: Vec<Scalar>,
    tolerance: Scalar,
    reason: StopReason,
) -> IterationStats {
    let initial = history.first().copied().unwrap_or(0.0);
    let final_r = history.last().copied().unwrap_or(initial);
    let relative = if initial > 0.0 {
        final_r / initial
    } else {
        0.0
    };
    IterationStats {
        iterations: history.len().saturating_sub(1),
        initial_residual: initial,
        final_residual: final_r,
        relative_residual: relative,
        tolerance,
        reason,
        residual_history: history,
    }
}

/// Normalized residual `‖b − A·x‖₂ / ‖b‖₂`, the scale-free convergence measure.
///
/// Falls back to `‖b − A·x‖₂` when `b` is the zero vector (in which case the
/// absolute residual is the only meaningful measure).
pub fn relative_residual_norm(a: &CsrMatrix, x: &[Scalar], b: &[Scalar]) -> Scalar {
    let r = match super::operations::residual(a, x, b) {
        Ok(r) => r,
        Err(_) => return Scalar::INFINITY,
    };
    let rnorm = super::operations::norm2(&r);
    let bnorm = super::operations::norm2(b);
    if bnorm > 0.0 { rnorm / bnorm } else { rnorm }
}

/// Estimate the 1-norm condition number `κ₁(A) = ‖A‖₁ · ‖A⁻¹‖₁`.
///
/// The inverse is computed with dense Gauss–Jordan elimination, so this is
/// intended for small and medium matrices only; it is a diagnostic, not a
/// production path. Returns [`Scalar::INFINITY`] if the matrix is singular.
pub fn condition_number_1norm(a: &CsrMatrix) -> Scalar {
    if !a.is_square() || a.nrows() == 0 {
        return Scalar::INFINITY;
    }
    let norm_a = infinity_induced_1norm(a);
    let dense = a.to_dense();
    match invert_dense(&dense) {
        Some(inv) => {
            let norm_inv = dense_1norm(&inv);
            norm_a * norm_inv
        }
        None => Scalar::INFINITY,
    }
}

/// The 1-norm `max_j Σ_i |aᵢⱼ|` of a CSR matrix.
pub fn infinity_induced_1norm(a: &CsrMatrix) -> Scalar {
    let mut col_sum = vec![0.0; a.ncols()];
    for i in 0..a.nrows() {
        for k in a.row_ptr()[i]..a.row_ptr()[i + 1] {
            col_sum[a.col_idx()[k]] += a.values()[k].abs();
        }
    }
    col_sum.iter().cloned().fold(0.0, Scalar::max)
}

fn dense_1norm(m: &[Vec<Scalar>]) -> Scalar {
    let n = if m.is_empty() { 0 } else { m[0].len() };
    let mut best = 0.0 as Scalar;
    for j in 0..n {
        let mut s = 0.0 as Scalar;
        for row in m {
            s += row[j].abs();
        }
        best = best.max(s);
    }
    best
}

/// Invert a dense square matrix via Gauss–Jordan with partial pivoting.
/// Returns `None` if the matrix is singular to working precision.
fn invert_dense(m: &[Vec<Scalar>]) -> Option<Vec<Vec<Scalar>>> {
    let n = m.len();
    if n == 0 {
        return Some(Vec::new());
    }
    if m[0].len() != n {
        return None;
    }
    let mut a = m.to_vec();
    let mut inv = vec![vec![0.0; n]; n];
    for (i, row) in inv.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for col in 0..n {
        // Partial pivot.
        let mut piv = col;
        let mut best = a[col][col].abs();
        for r in (col + 1)..n {
            let v = a[r][col].abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if best < 1e-300 {
            return None;
        }
        a.swap(col, piv);
        inv.swap(col, piv);
        let d = a[col][col];
        for j in 0..n {
            a[col][j] /= d;
            inv[col][j] /= d;
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = a[r][col];
            if f == 0.0 {
                continue;
            }
            for j in 0..n {
                a[r][j] -= f * a[col][j];
                inv[r][j] -= f * inv[col][j];
            }
        }
    }
    Some(inv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compute::sparse::matrix::CooMatrix;

    fn diag(vals: &[Scalar]) -> CsrMatrix {
        let n = vals.len();
        let mut coo = CooMatrix::new(n, n);
        for (i, &v) in vals.iter().enumerate() {
            coo.push(i, i, v).unwrap();
        }
        coo.to_csr()
    }

    #[test]
    fn condition_number_of_identity_is_one() {
        let id = CsrMatrix::identity(4);
        let k = condition_number_1norm(&id);
        assert!((k - 1.0).abs() < 1e-12);
    }

    #[test]
    fn condition_number_of_diagonal() {
        let a = diag(&[1.0, 100.0, 1e4]);
        let k = condition_number_1norm(&a);
        // κ = max/min = 1e4
        assert!((k - 1e4).abs() / 1e4 < 1e-9);
    }

    #[test]
    fn singular_matrix_has_infinite_condition() {
        // [[1,1],[1,1]] is singular.
        let mut coo = CooMatrix::new(2, 2);
        coo.push(0, 0, 1.0).unwrap();
        coo.push(0, 1, 1.0).unwrap();
        coo.push(1, 0, 1.0).unwrap();
        coo.push(1, 1, 1.0).unwrap();
        assert!(condition_number_1norm(&coo.to_csr()).is_infinite());
    }

    #[test]
    fn relative_residual_is_scale_free() {
        let a = CsrMatrix::identity(3);
        let x = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!(relative_residual_norm(&a, &x, &b) < 1e-14);
        // A large b with a proportional error still reports the same relative error.
        let b2 = vec![1e6, 0.0, 0.0];
        let x2 = vec![1e6 - 1.0, 0.0, 0.0];
        let rr = relative_residual_norm(&a, &x2, &b2);
        assert!((rr - 1e-6).abs() < 1e-12);
    }

    #[test]
    fn summarize_reports_convergence_and_iterations() {
        let stats = summarize(vec![1.0, 0.5, 1e-10], 1e-9, StopReason::Converged);
        assert_eq!(stats.iterations, 2);
        assert!(stats.converged());
        assert!(stats.relative_residual < 1e-9);
    }

    #[test]
    fn stop_reason_strings_are_stable() {
        assert!(StopReason::MaxIterationsReached.as_str() == "max-iterations");
        assert!(!StopReason::Stagnation.is_success());
    }
}
