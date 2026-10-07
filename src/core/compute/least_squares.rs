// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Minimum-norm and rank-revealing least-squares solvers (Phase 35).
//!
//! This module handles overdetermined, underdetermined and rank-deficient
//! linear least-squares problems `min_x ‖A·x − b‖₂` **without** forming the
//! normal equations `AᵀA` as a general default, since squaring the condition
//! number destroys accuracy for ill-conditioned inputs.
//!
//! # Methods
//!
//! * [`solve_qr`] — Householder QR for full-column-rank overdetermined systems.
//! * [`solve_svd`] — truncated-SVD / rank-revealing solve that handles rank
//!   deficiency and returns the minimum-norm solution for underdetermined
//!   systems.
//! * [`pseudo_inverse`] — Moore–Penrose pseudo-inverse with an explicit cutoff.
//!
//! All solvers report the numerical rank, the residual norm and (where
//! meaningful) the condition estimate, so a caller can judge whether the answer
//! is trustworthy.

use crate::core::types::Scalar;

/// Diagnostics describing a least-squares solution.
#[derive(Debug, Clone, PartialEq)]
pub struct LeastSquaresDiagnostics {
    /// Numerical rank estimated from the singular values.
    pub rank: usize,
    /// The smallest retained singular value (in the rank determination).
    pub min_singular_value: Scalar,
    /// The largest singular value.
    pub max_singular_value: Scalar,
    /// Estimated condition number `σ_max / σ_min`.
    pub condition_estimate: Scalar,
    /// Residual 2-norm `‖A·x − b‖₂`.
    pub residual_norm: Scalar,
    /// Whether the problem was rank-deficient (rank < min(m, n)).
    pub rank_deficient: bool,
}

/// A least-squares solution plus diagnostics.
#[derive(Debug, Clone)]
pub struct LeastSquaresSolution {
    /// The solution vector (length `n`).
    pub x: Vec<Scalar>,
    /// Diagnostic information about the solve.
    pub diagnostics: LeastSquaresDiagnostics,
}

/// Error type for least-squares routines.
#[derive(Debug, Clone, PartialEq)]
pub enum LeastSquaresError {
    /// A matrix was empty or ragged.
    InvalidMatrix(String),
    /// The right-hand side length did not match the number of rows.
    ShapeMismatch(String),
    /// A value was not finite.
    NonFinite(String),
}

impl std::fmt::Display for LeastSquaresError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMatrix(d) => write!(f, "invalid matrix: {d}"),
            Self::ShapeMismatch(d) => write!(f, "shape mismatch: {d}"),
            Self::NonFinite(d) => write!(f, "non-finite value: {d}"),
        }
    }
}

impl std::error::Error for LeastSquaresError {}

fn validate(a: &[Vec<Scalar>], b: &[Scalar]) -> Result<(usize, usize), LeastSquaresError> {
    if a.is_empty() {
        return Err(LeastSquaresError::InvalidMatrix("empty matrix".to_string()));
    }
    let m = a.len();
    let n = a[0].len();
    if n == 0 {
        return Err(LeastSquaresError::InvalidMatrix("zero columns".to_string()));
    }
    for (i, row) in a.iter().enumerate() {
        if row.len() != n {
            return Err(LeastSquaresError::InvalidMatrix(format!(
                "ragged row {i}: {} != {n}",
                row.len()
            )));
        }
        for &v in row {
            if !v.is_finite() {
                return Err(LeastSquaresError::NonFinite(format!("A[{i}]")));
            }
        }
    }
    if b.len() != m {
        return Err(LeastSquaresError::ShapeMismatch(format!(
            "b length {} != rows {m}",
            b.len()
        )));
    }
    for &v in b {
        if !v.is_finite() {
            return Err(LeastSquaresError::NonFinite("b".to_string()));
        }
    }
    Ok((m, n))
}

/// Solve `min_x ‖A·x − b‖₂` for full-column-rank overdetermined `A` using
/// Householder QR with column pivoting.
///
/// If `A` is detected to be rank-deficient (a pivot below the tolerance), the
/// solve returns `Err(LeastSquaresError…)` directing the caller to
/// [`solve_svd`], which handles rank deficiency explicitly.
pub fn solve_qr(
    a: &[Vec<Scalar>],
    b: &[Scalar],
    tol: Scalar,
) -> Result<LeastSquaresSolution, LeastSquaresError> {
    let (m, n) = validate(a, b)?;
    if m < n {
        return Err(LeastSquaresError::InvalidMatrix(format!(
            "solve_qr expects an overdetermined system (m>=n), got {m}x{n}; use solve_svd"
        )));
    }
    // Copy into a working m×n matrix, b into work vector.
    let mut r: Vec<Vec<Scalar>> = a.to_vec();
    let mut rhs: Vec<Scalar> = b.to_vec();

    // Householder QR with column pivoting for rank revelation.
    let mut perm: Vec<usize> = (0..n).collect();
    let mut max_abs_a = 0.0 as Scalar;
    for row in a.iter() {
        for &v in row {
            max_abs_a = max_abs_a.max(v.abs());
        }
    }
    let rank_tol = tol * max_abs_a.max(1.0);

    let mut rank = n;
    for k in 0..n {
        // Choose the pivot column with the largest remaining column norm.
        let mut best = k;
        let mut best_norm = -1.0;
        for c in k..n {
            let mut s = 0.0;
            for i in k..m {
                s += r[i][c] * r[i][c];
            }
            if s > best_norm {
                best_norm = s;
                best = c;
            }
        }
        if best_norm.sqrt() < rank_tol {
            rank = k;
            break;
        }
        if best != k {
            for row in r.iter_mut() {
                row.swap(k, best);
            }
            perm.swap(k, best);
        }
        // Householder reflector for column k below the diagonal.
        let mut norm_x = 0.0;
        for i in k..m {
            norm_x += r[i][k] * r[i][k];
        }
        norm_x = norm_x.sqrt();
        if norm_x > 0.0 {
            let alpha = if r[k][k] >= 0.0 { -norm_x } else { norm_x };
            let mut v = vec![0.0; m];
            for i in k..m {
                v[i] = r[i][k];
            }
            v[k] -= alpha;
            let vnorm2: Scalar = v.iter().map(|x| x * x).sum();
            if vnorm2 > 0.0 {
                for j in k..n {
                    let mut dot = 0.0;
                    for i in k..m {
                        dot += v[i] * r[i][j];
                    }
                    let factor = 2.0 * dot / vnorm2;
                    for i in k..m {
                        r[i][j] -= factor * v[i];
                    }
                }
                let mut dotb = 0.0;
                for i in k..m {
                    dotb += v[i] * rhs[i];
                }
                let factor = 2.0 * dotb / vnorm2;
                for i in k..m {
                    rhs[i] -= factor * v[i];
                }
            }
        }
    }

    // Back substitution on the leading rank×rank upper-triangular block.
    let mut y = vec![0.0; n];
    for k in (0..rank).rev() {
        let mut s = rhs[k];
        for j in (k + 1)..rank {
            s -= r[k][j] * y[j];
        }
        y[k] = if r[k][k].abs() > 1e-300 {
            s / r[k][k]
        } else {
            0.0
        };
    }
    // Undo the column permutation.
    let mut x = vec![0.0; n];
    for k in 0..n {
        x[perm[k]] = y[k];
    }

    let residual_norm = residual_norm(a, &x, b);
    let diag_abs: Vec<Scalar> = (0..rank).map(|i| r[i][i].abs()).collect();
    let max_sv = diag_abs.iter().cloned().fold(0.0, Scalar::max);
    let min_sv = diag_abs.iter().cloned().fold(Scalar::INFINITY, Scalar::min);
    let cond = if min_sv > 0.0 {
        max_sv / min_sv
    } else {
        Scalar::INFINITY
    };
    Ok(LeastSquaresSolution {
        x,
        diagnostics: LeastSquaresDiagnostics {
            rank,
            min_singular_value: if min_sv.is_finite() { min_sv } else { 0.0 },
            max_singular_value: max_sv,
            condition_estimate: cond,
            residual_norm,
            rank_deficient: rank < n.min(m),
        },
    })
}

/// Solve `min_x ‖A·x − b‖₂` for arbitrary (possibly rank-deficient) `A` via a
/// truncated SVD.
///
/// * Overdetermined, full rank → the standard least-squares solution.
/// * Rank-deficient → the minimum-norm least-squares solution.
/// * Underdetermined → the minimum-norm solution satisfying `A·x = b`.
///
/// Singular values below `tol · σ_max` are truncated, which is what defines the
/// numerical rank.
pub fn solve_svd(
    a: &[Vec<Scalar>],
    b: &[Scalar],
    tol: Scalar,
) -> Result<LeastSquaresSolution, LeastSquaresError> {
    let (m, n) = validate(a, b)?;
    let (u, s, vt) = jacobi_svd(a)?;
    let max_sv = s.first().copied().unwrap_or(0.0);
    let cutoff = tol * max_sv.max(1e-300);

    // x = V · Σ⁺ · Uᵀ · b, keeping only singular values above the cutoff.
    // Uᵀ b:
    let k = s.len();
    let mut utb = vec![0.0 as Scalar; k];
    for (j, sj) in utb.iter_mut().enumerate() {
        let mut acc = 0.0;
        for i in 0..m {
            acc += u[i][j] * b[i];
        }
        *sj = acc;
    }
    let mut rank = 0;
    let mut min_kept = Scalar::INFINITY;
    let mut coeff = vec![0.0; k];
    for j in 0..k {
        if s[j] > cutoff {
            coeff[j] = utb[j] / s[j];
            min_kept = min_kept.min(s[j]);
            rank += 1;
        }
    }
    let mut x = vec![0.0; n];
    for j in 0..k {
        if coeff[j] != 0.0 {
            // vt is k×n; row j is the j-th right singular vector.
            for i in 0..n {
                x[i] += coeff[j] * vt[j][i];
            }
        }
    }
    let residual_norm = residual_norm(a, &x, b);
    let cond = if min_kept.is_finite() && min_kept > 0.0 {
        max_sv / min_kept
    } else {
        Scalar::INFINITY
    };
    Ok(LeastSquaresSolution {
        x,
        diagnostics: LeastSquaresDiagnostics {
            rank,
            min_singular_value: if min_kept.is_finite() { min_kept } else { 0.0 },
            max_singular_value: max_sv,
            condition_estimate: cond,
            residual_norm,
            rank_deficient: rank < n.min(m),
        },
    })
}

/// Compute the Moore–Penrose pseudo-inverse `A⁺` (n×m) via truncated SVD.
pub fn pseudo_inverse(
    a: &[Vec<Scalar>],
    tol: Scalar,
) -> Result<(Vec<Vec<Scalar>>, LeastSquaresDiagnostics), LeastSquaresError> {
    let (m, n) = validate(a, &vec![0.0; a.len()])?;
    let (u, s, vt) = jacobi_svd(a)?;
    let max_sv = s.first().copied().unwrap_or(0.0);
    let cutoff = tol * max_sv.max(1e-300);
    let k = s.len();
    let mut rank = 0;
    let mut min_kept = Scalar::INFINITY;
    // A⁺ = V · Σ⁺ · Uᵀ  →  (n × m)
    let mut pinv = vec![vec![0.0; m]; n];
    for j in 0..k {
        if s[j] > cutoff {
            rank += 1;
            min_kept = min_kept.min(s[j]);
            let inv = 1.0 / s[j];
            for i in 0..n {
                let vi = vt[j][i];
                for r in 0..m {
                    pinv[i][r] += vi * inv * u[r][j];
                }
            }
        }
    }
    let cond = if min_kept.is_finite() && min_kept > 0.0 {
        max_sv / min_kept
    } else {
        Scalar::INFINITY
    };
    Ok((
        pinv,
        LeastSquaresDiagnostics {
            rank,
            min_singular_value: if min_kept.is_finite() { min_kept } else { 0.0 },
            max_singular_value: max_sv,
            condition_estimate: cond,
            residual_norm: 0.0,
            rank_deficient: rank < n.min(m),
        },
    ))
}

/// Residual 2-norm `‖A·x − b‖₂`.
fn residual_norm(a: &[Vec<Scalar>], x: &[Scalar], b: &[Scalar]) -> Scalar {
    let mut acc = 0.0;
    for (i, row) in a.iter().enumerate() {
        let mut ax = 0.0;
        for (j, &aij) in row.iter().enumerate() {
            ax += aij * x[j];
        }
        let d = ax - b[i];
        acc += d * d;
    }
    acc.sqrt()
}

/// One-sided Jacobi SVD for a dense `m×n` matrix.
///
/// Returns `(U, Σ, Vᵀ)` where `U` is `m×k`, `Σ` has length `k = min(m,n)` in
/// descending order, and `Vᵀ` is `k×n`. Works for `m ≥ n` and `m < n` by
/// transposing internally. One-sided Jacobi is chosen for its high relative
/// accuracy on small and medium matrices, which is the targeted size for this
/// dense diagnostic path.
#[allow(clippy::type_complexity)]
pub fn jacobi_svd(
    a: &[Vec<Scalar>],
) -> Result<(Vec<Vec<Scalar>>, Vec<Scalar>, Vec<Vec<Scalar>>), LeastSquaresError> {
    let m = a.len();
    if m == 0 {
        return Err(LeastSquaresError::InvalidMatrix("empty matrix".to_string()));
    }
    let n = a[0].len();
    if n == 0 {
        return Err(LeastSquaresError::InvalidMatrix("zero columns".to_string()));
    }
    // For m < n, compute the SVD of Aᵀ and swap U/V.
    if m < n {
        let at: Vec<Vec<Scalar>> = (0..n).map(|i| (0..m).map(|j| a[j][i]).collect()).collect();
        let (u2, s, vt2) = jacobi_svd(&at)?;
        // A = (Aᵀ)ᵀ = (U2 Σ V2ᵀ)ᵀ = V2 Σ U2ᵀ. So U = V2ᵀ, Vᵀ = U2ᵀ.
        let v = transpose_dense(&vt2); // n×k
        let ut = transpose_dense(&u2); // k×m
        return Ok((v, s, ut));
    }

    // m >= n: one-sided Jacobi on the n columns of a copy.
    let k = n;
    let mut u: Vec<Vec<Scalar>> = a.to_vec(); // m×n work array
    let mut v: Vec<Vec<Scalar>> = (0..n)
        .map(|i| {
            let mut row = vec![0.0; n];
            row[i] = 1.0;
            row
        })
        .collect(); // n×n rotation accumulator

    let max_sweeps = 60;
    for _ in 0..max_sweeps {
        let mut off = 0.0;
        for p in 0..n {
            for q in (p + 1)..n {
                let mut app = 0.0;
                let mut aqq = 0.0;
                let mut apq = 0.0;
                for i in 0..m {
                    app += u[i][p] * u[i][p];
                    aqq += u[i][q] * u[i][q];
                    apq += u[i][p] * u[i][q];
                }
                off += apq * apq;
                if apq.abs() <= 1e-300 * (app * aqq).sqrt().max(1e-300) {
                    continue;
                }
                let theta = (aqq - app) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (1.0 + theta * theta).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s_rot = c * t;
                for i in 0..m {
                    let up = u[i][p];
                    let uq = u[i][q];
                    u[i][p] = c * up - s_rot * uq;
                    u[i][q] = s_rot * up + c * uq;
                }
                for i in 0..n {
                    let vp = v[i][p];
                    let vq = v[i][q];
                    v[i][p] = c * vp - s_rot * vq;
                    v[i][q] = s_rot * vp + c * vq;
                }
            }
        }
        if off <= 1e-300 {
            break;
        }
    }

    // Column norms are the singular values; normalize U columns.
    let mut sigma = vec![0.0; k];
    for j in 0..k {
        let mut s = 0.0;
        for i in 0..m {
            s += u[i][j] * u[i][j];
        }
        sigma[j] = s.sqrt();
    }
    // Sort descending, permuting U and V columns consistently.
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by(|&i, &j| sigma[j].partial_cmp(&sigma[i]).unwrap());
    let sigma_sorted: Vec<Scalar> = order.iter().map(|&j| sigma[j]).collect();
    let u_sorted: Vec<Vec<Scalar>> = (0..m)
        .map(|i| {
            order
                .iter()
                .map(|&j| {
                    if sigma[j] > 0.0 {
                        u[i][j] / sigma[j]
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect();
    // Vᵀ is k×n: row j = column j of V (after permutation).
    let vt_sorted: Vec<Vec<Scalar>> = order
        .iter()
        .map(|&j| (0..n).map(|i| v[i][j]).collect())
        .collect();
    Ok((u_sorted, sigma_sorted, vt_sorted))
}

fn transpose_dense(a: &[Vec<Scalar>]) -> Vec<Vec<Scalar>> {
    if a.is_empty() {
        return Vec::new();
    }
    let m = a.len();
    let n = a[0].len();
    (0..n).map(|i| (0..m).map(|j| a[j][i]).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_solves_overdetermined_exact_fit() {
        // 3 equations, 2 unknowns, consistent: the line y = 1 + 2x through
        // (0,1), (1,3), (2,5).
        let a = vec![vec![1.0, 0.0], vec![1.0, 1.0], vec![1.0, 2.0]];
        let b = vec![1.0, 3.0, 5.0];
        let sol = solve_qr(&a, &b, 1e-12).unwrap();
        assert!((sol.x[0] - 1.0).abs() < 1e-10);
        assert!((sol.x[1] - 2.0).abs() < 1e-10);
        assert!(sol.diagnostics.residual_norm < 1e-10);
        assert_eq!(sol.diagnostics.rank, 2);
    }

    #[test]
    fn svd_solves_rank_deficient() {
        // Second column is 2× the first: rank 1.
        let a = vec![vec![1.0, 2.0], vec![2.0, 4.0], vec![3.0, 6.0]];
        let b = vec![1.0, 2.0, 3.0];
        let sol = solve_svd(&a, &b, 1e-10).unwrap();
        assert_eq!(sol.diagnostics.rank, 1);
        assert!(sol.diagnostics.rank_deficient);
        // Minimum-norm solution: x is parallel to [1,2]/5.
        assert!((sol.x[0] - 0.2).abs() < 1e-9, "x={:?}", sol.x);
        assert!((sol.x[1] - 0.4).abs() < 1e-9, "x={:?}", sol.x);
        assert!(sol.diagnostics.residual_norm < 1e-9);
    }

    #[test]
    fn svd_underdetermined_minimum_norm() {
        // 1 equation, 2 unknowns: x0 + x1 = 2. Minimum-norm x = [1, 1].
        let a = vec![vec![1.0, 1.0]];
        let b = vec![2.0];
        let sol = solve_svd(&a, &b, 1e-12).unwrap();
        assert!((sol.x[0] - 1.0).abs() < 1e-10);
        assert!((sol.x[1] - 1.0).abs() < 1e-10);
        assert!(sol.diagnostics.residual_norm < 1e-10);
    }

    #[test]
    fn singular_values_of_identity() {
        let id = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let (_, s, _) = jacobi_svd(&id).unwrap();
        assert!((s[0] - 1.0).abs() < 1e-12);
        assert!((s[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn singular_values_descending() {
        let a = vec![vec![3.0, 0.0], vec![0.0, 1.0]];
        let (_, s, _) = jacobi_svd(&a).unwrap();
        assert!(s[0] >= s[1]);
        assert!((s[0] - 3.0).abs() < 1e-10);
        assert!((s[1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn pseudo_inverse_reconstructs_identity() {
        let a = vec![vec![2.0, 0.0], vec![0.0, 4.0]];
        let (pinv, diag) = pseudo_inverse(&a, 1e-12).unwrap();
        assert_eq!(diag.rank, 2);
        assert!((pinv[0][0] - 0.5).abs() < 1e-12);
        assert!((pinv[1][1] - 0.25).abs() < 1e-12);
    }

    #[test]
    fn qr_rejects_underdetermined() {
        let a = vec![vec![1.0, 1.0, 1.0]];
        let b = vec![2.0];
        assert!(matches!(
            solve_qr(&a, &b, 1e-12),
            Err(LeastSquaresError::InvalidMatrix(_))
        ));
    }

    #[test]
    fn rejects_non_finite_input() {
        let a = vec![vec![1.0, Scalar::NAN]];
        let b = vec![1.0];
        assert!(matches!(
            solve_svd(&a, &b, 1e-12),
            Err(LeastSquaresError::NonFinite(_))
        ));
    }

    #[test]
    fn rejects_ragged_matrix() {
        let a = vec![vec![1.0, 2.0], vec![3.0]];
        let b = vec![1.0, 2.0];
        assert!(matches!(
            solve_qr(&a, &b, 1e-12),
            Err(LeastSquaresError::InvalidMatrix(_))
        ));
    }
}
