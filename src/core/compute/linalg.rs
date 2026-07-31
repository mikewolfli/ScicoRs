//! BLAS / LAPACK-style dense linear algebra with adaptive CPU/GPU dispatch.
//!
//! This module mirrors the numpy/MKL `linalg` API surface: BLAS level-1/2
//! primitives (`scal`, `nrm2`, `asum`, `iamax`, `gemv`) and LAPACK-style dense
//! decompositions / solves (`lu_decompose`/`lu_solve` ≈ dgetrf/dgesv,
//! `cholesky` ≈ dpotrf, `qr_decompose` ≈ dgeqrf).
//!
//! Every routine routes through the adaptive backend
//! ([`crate::core::compute::backend::AdaptiveCompute`]) so that large
//! workloads automatically use the rayon parallel pool (and a registered GPU
//! backend where supported). All paths return numerically identical results.

use crate::core::compute::backend::{self, BackendKind};
use crate::core::error::SimError;
use crate::core::types::Scalar;

/// LU factorisation result: `(lu, piv)`.
pub type LuResult = (Vec<Vec<Scalar>>, Vec<usize>);

/// QR factorisation result: `(q, r)`.
pub type QrResult = (Vec<Vec<Scalar>>, Vec<Vec<Scalar>>);

// ──────────────────────────────────────────────
// BLAS level-1
// ──────────────────────────────────────────────

/// BLAS-1 `scal`: `y = α·x` with adaptive dispatch.
pub fn scal(alpha: Scalar, x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    let a = backend::global();
    match a.kind_for(x.len()) {
        BackendKind::VendorCpu => a.vendor_or_cpu(
            |v| v.scal(alpha, x),
            || Ok(x.iter().map(|&v| alpha * v).collect()),
        ),
        BackendKind::CpuParallel => {
            use rayon::prelude::*;
            Ok(x.par_iter().map(|&v| alpha * v).collect())
        }
        _ => Ok(x.iter().map(|&v| alpha * v).collect()),
    }
}

/// BLAS-1 `nrm2`: Euclidean norm ‖x‖₂ with adaptive dispatch.
pub fn nrm2(x: &[Scalar]) -> Scalar {
    let a = backend::global();
    match a.kind_for(x.len()) {
        BackendKind::VendorCpu => a
            .vendor_or_cpu(
                |v| v.nrm2(x),
                || Ok(x.iter().map(|&v| v * v).sum::<Scalar>().sqrt()),
            )
            .unwrap_or(0.0),
        BackendKind::CpuParallel => {
            use rayon::prelude::*;
            x.par_iter().map(|&v| v * v).sum::<Scalar>().sqrt()
        }
        _ => x.iter().map(|&v| v * v).sum::<Scalar>().sqrt(),
    }
}

/// BLAS-1 `asum`: sum of absolute values Σ|xᵢ| with adaptive dispatch.
pub fn asum(x: &[Scalar]) -> Scalar {
    let a = backend::global();
    match a.kind_for(x.len()) {
        BackendKind::VendorCpu => a
            .vendor_or_cpu(|v| v.asum(x), || Ok(x.iter().map(|&v| v.abs()).sum()))
            .unwrap_or(0.0),
        BackendKind::CpuParallel => {
            use rayon::prelude::*;
            x.par_iter().map(|&v| v.abs()).sum()
        }
        _ => x.iter().map(|&v| v.abs()).sum(),
    }
}

/// BLAS-1 `iamax`: index of the element with maximum absolute value.
pub fn iamax(x: &[Scalar]) -> usize {
    let serial = |x: &[Scalar]| {
        x.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.abs()
                    .partial_cmp(&b.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    let a = backend::global();
    match a.kind_for(x.len()) {
        BackendKind::VendorCpu => a
            .vendor_or_cpu(|v| v.iamax(x), || Ok(serial(x)))
            .unwrap_or_else(|_| serial(x)),
        _ => serial(x),
    }
}

// ──────────────────────────────────────────────
// BLAS level-2
// ──────────────────────────────────────────────

/// BLAS-2 `gemv`: `y = A·x` (m×n matrix × n-vector) with adaptive dispatch.
pub fn gemv(a: &[Vec<Scalar>], x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    if a.is_empty() {
        return Ok(Vec::new());
    }
    let m = a.len();
    let n = a[0].len();
    if x.len() != n {
        return Err(SimError::numerical(format!(
            "gemv: matrix cols={}, vector len={}",
            n,
            x.len()
        )));
    }
    let serial = || {
        // Treat `x` as a k×1 column and evaluate `A·x` as a BLAS-3 gemm of
        // m×k × k×1 so the pure-Rust SIMD kernel accelerates it too.
        if m.saturating_mul(n) >= 4096 {
            let mut a_flat = Vec::with_capacity(m * n);
            for row in a.iter().take(m) {
                a_flat.extend_from_slice(&row[..n]);
            }
            let mut y = vec![0.0; m];
            super::simd::dgemv(m, n, &a_flat, x, &mut y);
            Ok(y)
        } else {
            let mut y = vec![0.0; m];
            for (i, row) in a.iter().enumerate() {
                let mut s = 0.0;
                for j in 0..n {
                    s += row[j] * x[j];
                }
                y[i] = s;
            }
            Ok(y)
        }
    };
    let dispatcher = backend::global();
    match dispatcher.kind_for(m.saturating_mul(n)) {
        BackendKind::VendorCpu => dispatcher.vendor_or_cpu(|v| v.gemv(a, x), serial),
        BackendKind::CpuParallel => {
            use rayon::prelude::*;
            Ok(a.par_iter()
                .map(|row| row.iter().zip(x.iter()).map(|(r, xi)| r * xi).sum())
                .collect())
        }
        _ => serial(),
    }
}

// ──────────────────────────────────────────────
// LAPACK-style decompositions
// ──────────────────────────────────────────────

/// LAPACK `dgetrf`: LU decomposition with partial pivoting.
///
/// Returns `(lu, piv)` where `lu` is the in-place LU factor (unit lower
/// triangular L stored below the diagonal, upper triangular U on/above the
/// diagonal) and `piv` is the row permutation such that `P·A = L·U`.
pub fn lu_decompose(a: &[Vec<Scalar>]) -> Result<LuResult, SimError> {
    let n = a.len();
    if n == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    if a[0].len() != n {
        return Err(SimError::numerical("lu_decompose: matrix is not square"));
    }
    let mut lu = a.to_vec();
    let mut piv = (0..n).collect::<Vec<usize>>();

    for k in 0..n {
        // Partial pivoting.
        let mut max_i = k;
        let mut max_abs = lu[k][k].abs();
        for i in (k + 1)..n {
            let v = lu[i][k].abs();
            if v > max_abs {
                max_abs = v;
                max_i = i;
            }
        }
        if max_abs < 1e-300 {
            return Err(SimError::numerical("lu_decompose: singular matrix"));
        }
        if max_i != k {
            lu.swap(k, max_i);
            piv.swap(k, max_i);
        }
        let pivot = lu[k][k];
        // Snapshot the pivot row into a temporary Vec (O(n) per pivot, O(n²)
        // total — negligible vs the O(n³) update). The trailing update then
        // reads a disjoint buffer, so LLVM can auto-vectorize the axpy.
        let pivot_row: Vec<Scalar> = lu[k][k + 1..].to_vec();
        for i in (k + 1)..n {
            let factor = lu[i][k] / pivot;
            lu[i][k] = factor;
            for (a, &p) in lu[i][k + 1..].iter_mut().zip(pivot_row.iter()) {
                *a -= factor * p;
            }
        }
    }
    Ok((lu, piv))
}

/// LAPACK `dgesv`: solve `A·x = b` via LU decomposition with partial pivoting
/// (adaptive dispatch, CPU fallback).
pub fn lu_solve(a: &[Vec<Scalar>], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    let work = a.len().saturating_mul(a.len());
    let dispatcher = backend::global();
    match dispatcher.kind_for(work) {
        BackendKind::VendorCpu => {
            dispatcher.vendor_or_cpu(|v| v.lu_solve(a, b), || lu_solve_cpu(a, b))
        }
        _ => lu_solve_cpu(a, b),
    }
}

/// CPU LU solve (reference implementation, used directly by vendor mocks).
pub(crate) fn lu_solve_cpu(a: &[Vec<Scalar>], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    let n = a.len();
    if b.len() != n {
        return Err(SimError::numerical(format!(
            "lu_solve: A is {}×{}, b has length {}",
            n,
            if n > 0 { a[0].len() } else { 0 },
            b.len()
        )));
    }
    if n == 0 {
        return Ok(Vec::new());
    }
    let (lu, piv) = lu_decompose(a)?;
    // Apply the row permutation to b.
    let mut x = vec![0.0; n];
    for i in 0..n {
        x[i] = b[piv[i]];
    }
    // Forward substitution: L·y = P·b (L is unit lower triangular).
    for i in 0..n {
        for j in 0..i {
            x[i] -= lu[i][j] * x[j];
        }
    }
    // Back substitution: U·x = y.
    for i in (0..n).rev() {
        for j in (i + 1)..n {
            x[i] -= lu[i][j] * x[j];
        }
        x[i] /= lu[i][i];
    }
    Ok(x)
}

/// LAPACK `dpotrf`: Cholesky decomposition `A = L·Lᵀ` for a symmetric
/// positive-definite matrix (adaptive dispatch, CPU fallback).
pub fn cholesky(a: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    let dispatcher = backend::global();
    let work = a.len().saturating_mul(a.len());
    // Large SPD systems get the blocked (BLAS-3) kernel with SIMD syrk updates;
    // everything else uses the reference. Both return identical factors.
    let large = a.len() >= CHOL_BLOCK_MIN;
    match dispatcher.kind_for(work) {
        BackendKind::VendorCpu => dispatcher.vendor_or_cpu(
            |v| v.cholesky(a),
            || {
                if large {
                    cholesky_blocked(a)
                } else {
                    cholesky_cpu(a)
                }
            },
        ),
        _ => {
            if large {
                cholesky_blocked(a)
            } else {
                cholesky_cpu(a)
            }
        }
    }
}

/// Matrices at least this large use the blocked kernel.
const CHOL_BLOCK_MIN: usize = 96;

/// Block size for the blocked Cholesky (columns per block).
const CHOL_BLOCK: usize = 64;

/// Blocked Cholesky (`dpotrf` style) on a flat row-major buffer.
///
/// Factorizes `A = L·Lᵀ` in column blocks:
///   1. scalar Cholesky on the diagonal block (small, vectorized),
///   2. `trsm`: `L21 = A21·L11⁻ᵀ` (forward substitution on the panel rows),
///   3. `syrk`: `A22 ← A22 − L21·L21ᵀ` as an **in-place strided SIMD gemm**
///      (`C ← −A·B + C`) — no extract/transpose/write-back copies, so the
///      O(n³) trailing update runs at SIMD matrix-multiply speed.
///
/// Returns the same lower-triangular factor as [`cholesky_cpu`]; verified
/// against it in tests.
pub(crate) fn cholesky_blocked(a: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    let n = a.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if a[0].len() != n {
        return Err(SimError::numerical("cholesky: matrix is not square"));
    }
    // Flat row-major working copy.
    let mut lf = Vec::with_capacity(n * n);
    for row in a {
        lf.extend_from_slice(row);
    }

    let mut s = 0usize;
    while s < n {
        let bs = (s + CHOL_BLOCK).min(n);
        // 1. Scalar Cholesky on the diagonal block [s, bs)×[s, bs).
        for i in s..bs {
            for j in s..=i {
                let mut sum = lf[i * n + j];
                for t in s..j {
                    sum -= lf[i * n + t] * lf[j * n + t];
                }
                if i == j {
                    if sum <= 1e-300 {
                        return Err(SimError::numerical(
                            "cholesky: matrix is not positive definite",
                        ));
                    }
                    lf[i * n + j] = sum.sqrt();
                } else {
                    lf[i * n + j] = sum / lf[j * n + j];
                }
            }
        }
        // 2. trsm: L21 = A21·L11⁻ᵀ (row-wise forward substitution, in place).
        for i in bs..n {
            for j in s..bs {
                let mut sum = lf[i * n + j];
                for t in s..j {
                    sum -= lf[i * n + t] * lf[j * n + t];
                }
                lf[i * n + j] = sum / lf[j * n + j];
            }
        }
        // 3. syrk: A22 ← A22 − L21·L21ᵀ. L21 is gathered into a separate
        //    buffer (its region interleaves with C per row, so it cannot share
        //    the mutable borrow), then C is updated in place via strided SIMD
        //    gemm — no extract/write-back of the trailing block.
        if bs < n {
            let m2 = n - bs;
            let k2 = bs - s;
            let mut l21 = Vec::with_capacity(m2 * k2);
            for rr in 0..m2 {
                l21.extend_from_slice(&lf[(bs + rr) * n + s..(bs + rr) * n + bs]);
            }
            // B = L21ᵀ (k2×m2) from the contiguous L21.
            let mut l21t = vec![0.0; k2 * m2];
            for (c, v) in l21t.iter_mut().enumerate() {
                let (cc, rr) = (c / m2, c % m2);
                *v = l21[rr * k2 + cc];
            }
            // C region [bs..n, bs..n] of `lf` (single mutable borrow).
            super::simd::dgemm_strided(
                m2,
                k2,
                m2,
                -1.0,
                &l21,
                k2 as isize,
                1,
                &l21t,
                m2 as isize,
                1,
                1.0,
                &mut lf[bs * n + bs..],
                n as isize,
                1,
            );
        }
        s = bs;
    }

    // Rebuild the Vec<Vec> lower-triangular result.
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        l[i][..=i].copy_from_slice(&lf[i * n..i * n + i + 1]);
    }
    Ok(l)
}

/// CPU Cholesky (reference implementation, used directly by vendor mocks).
pub(crate) fn cholesky_cpu(a: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    let n = a.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if a[0].len() != n {
        return Err(SimError::numerical("cholesky: matrix is not square"));
    }
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            // Reduction over disjoint slices — zipped iterators let LLVM
            // auto-vectorize the dot product.
            let mut sum = a[i][j];
            sum -= l[i][..j]
                .iter()
                .zip(l[j][..j].iter())
                .map(|(&x, &y)| x * y)
                .sum::<Scalar>();
            if i == j {
                if sum <= 1e-300 {
                    return Err(SimError::numerical(
                        "cholesky: matrix is not positive definite",
                    ));
                }
                l[i][j] = sum.sqrt();
            } else {
                l[i][j] = sum / l[j][j];
            }
        }
    }
    Ok(l)
}

/// LAPACK `dgeqrf`-style QR decomposition via modified Gram-Schmidt
/// (adaptive dispatch, CPU fallback).
///
/// Returns `(q, r)` with `q` m×n (orthonormal columns) and `r` n×n upper
/// triangular such that `A = Q·R`.
pub fn qr_decompose(a: &[Vec<Scalar>]) -> Result<QrResult, SimError> {
    let dispatcher = backend::global();
    let m = a.len();
    let n = if m > 0 { a[0].len() } else { 0 };
    let work = m.saturating_mul(n);
    match dispatcher.kind_for(work) {
        BackendKind::VendorCpu => dispatcher.vendor_or_cpu(|v| v.qr(a), || qr_cpu(a)),
        _ => qr_cpu(a),
    }
}

/// CPU QR (reference implementation, used directly by vendor mocks).
pub(crate) fn qr_cpu(a: &[Vec<Scalar>]) -> Result<QrResult, SimError> {
    let m = a.len();
    let n = if m > 0 { a[0].len() } else { 0 };
    if m == 0 || n == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut q = vec![vec![0.0; n]; m];
    let mut r = vec![vec![0.0; n]; n];

    for j in 0..n {
        // v_j = a_j (column j).
        let mut col: Vec<Scalar> = (0..m).map(|i| a[i][j]).collect();
        for k in 0..j {
            let dot: Scalar = (0..m).map(|i| q[i][k] * col[i]).sum();
            r[k][j] = dot;
            for i in 0..m {
                col[i] -= dot * q[i][k];
            }
        }
        let norm: Scalar = col.iter().map(|c| c * c).sum::<Scalar>().sqrt();
        if norm < 1e-300 {
            return Err(SimError::numerical(
                "qr_decompose: linearly dependent columns",
            ));
        }
        r[j][j] = norm;
        for i in 0..m {
            q[i][j] = col[i] / norm;
        }
    }
    Ok((q, r))
}

// ──────────────────────────────────────────────
// Global convenience API (numpy.linalg-style)
// ──────────────────────────────────────────────

/// Convenience: adaptive `scal` via the global dispatcher.
pub fn adaptive_scal(alpha: Scalar, x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    scal(alpha, x)
}

/// Convenience: adaptive `nrm2` via the global dispatcher.
pub fn adaptive_nrm2(x: &[Scalar]) -> Scalar {
    nrm2(x)
}

/// Convenience: adaptive `asum` via the global dispatcher.
pub fn adaptive_asum(x: &[Scalar]) -> Scalar {
    asum(x)
}

/// Convenience: adaptive `iamax` via the global dispatcher.
pub fn adaptive_iamax(x: &[Scalar]) -> usize {
    iamax(x)
}

/// Convenience: adaptive `gemv` via the global dispatcher.
pub fn adaptive_gemv(a: &[Vec<Scalar>], x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    gemv(a, x)
}

/// Convenience: adaptive LU solve via the global dispatcher.
pub fn adaptive_lu_solve(a: &[Vec<Scalar>], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    lu_solve(a, b)
}

/// Convenience: adaptive Cholesky decomposition via the global dispatcher.
pub fn adaptive_cholesky(a: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    cholesky(a)
}

/// Convenience: adaptive QR decomposition via the global dispatcher.
pub fn adaptive_qr(a: &[Vec<Scalar>]) -> Result<QrResult, SimError> {
    qr_decompose(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scal() {
        let x = vec![1.0, 2.0, 3.0];
        let y = scal(2.0, &x).unwrap();
        assert_eq!(y, vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_nrm2_asum_iamax() {
        let x = vec![3.0, -4.0, 0.0];
        assert!((nrm2(&x) - 5.0).abs() < 1e-12);
        assert!((asum(&x) - 7.0).abs() < 1e-12);
        assert_eq!(iamax(&x), 1);
    }

    #[test]
    fn test_gemv() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let x = vec![5.0, 6.0];
        let y = gemv(&a, &x).unwrap();
        assert!((y[0] - 17.0).abs() < 1e-12);
        assert!((y[1] - 39.0).abs() < 1e-12);
    }

    #[test]
    fn test_gemv_large_simd_matches_reference() {
        // 128×96 = 12288 work units → exercises the SIMD (BLAS-3) fast path;
        // must equal the scalar reference within float tolerance.
        let a: Vec<Vec<Scalar>> = (0..128)
            .map(|i| {
                (0..96)
                    .map(|j| ((i * 17 - j * 5) as Scalar) * 0.25)
                    .collect()
            })
            .collect();
        let x: Vec<Scalar> = (0..96).map(|j| ((j * 3 + 1) as Scalar) * -0.5).collect();
        let y = gemv(&a, &x).unwrap();
        for i in 0..128 {
            let mut want = 0.0;
            for j in 0..96 {
                want += a[i][j] * x[j];
            }
            assert!((y[i] - want).abs() < 1e-9, "gemv SIMD mismatch at {i}");
        }
    }

    #[test]
    fn test_lu_solve_matches_inverse() {
        // A·x = b with known solution.
        let a = vec![vec![4.0, 3.0], vec![6.0, 3.0]];
        let b = vec![10.0, 12.0];
        let x = lu_solve(&a, &b).unwrap();
        // Verify A·x = b.
        let ax0 = 4.0 * x[0] + 3.0 * x[1];
        let ax1 = 6.0 * x[0] + 3.0 * x[1];
        assert!((ax0 - 10.0).abs() < 1e-10);
        assert!((ax1 - 12.0).abs() < 1e-10);
    }

    #[test]
    fn test_lu_solve_3x3() {
        let a = vec![
            vec![2.0, 1.0, 1.0],
            vec![4.0, -6.0, 0.0],
            vec![-2.0, 7.0, 2.0],
        ];
        let b = vec![5.0, -2.0, 9.0];
        let x = lu_solve(&a, &b).unwrap();
        // Verify A·x = b.
        for i in 0..3 {
            let mut s = 0.0;
            for j in 0..3 {
                s += a[i][j] * x[j];
            }
            assert!((s - b[i]).abs() < 1e-10, "row {} residual", i);
        }
    }

    #[test]
    fn test_lu_singular() {
        let a = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        assert!(lu_decompose(&a).is_err());
    }

    #[test]
    fn test_cholesky_roundtrip() {
        // Symmetric positive-definite matrix.
        let a = vec![vec![4.0, 2.0], vec![2.0, 3.0]];
        let l = cholesky(&a).unwrap();
        // Reconstruct L·Lᵀ and compare.
        let n = 2;
        let mut rec = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    rec[i][j] += l[i][k] * l[j][k];
                }
            }
        }
        for i in 0..n {
            for j in 0..n {
                assert!((rec[i][j] - a[i][j]).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_cholesky_not_pd() {
        let a = vec![vec![1.0, 2.0], vec![2.0, 1.0]];
        assert!(cholesky(&a).is_err());
    }

    fn rand_spd(n: usize) -> Vec<Vec<Scalar>> {
        // B·Bᵀ + n·I is symmetric positive definite.
        let mut x: u64 = 0x9E3779B97F4A7C15;
        let b: Vec<Vec<Scalar>> = (0..n)
            .map(|_| {
                (0..n)
                    .map(|_| {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        (x as f64 / u64::MAX as f64) * 2.0 - 1.0
                    })
                    .collect()
            })
            .collect();
        let mut m = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                let mut s = 0.0;
                for (bik, bjk) in b[i].iter().zip(b[j].iter()) {
                    s += bik * bjk;
                }
                m[i][j] = s + if i == j { n as Scalar } else { 0.0 };
            }
        }
        m
    }

    #[test]
    fn test_cholesky_blocked_matches_reference() {
        // Sizes that straddle the blocked threshold (96) and block size (32),
        // including one with a partial trailing block (110 = 3×32 + 14).
        for n in [96, 110, 128, 160, 200] {
            let a = rand_spd(n);
            let want = cholesky_cpu(&a).unwrap();
            let got = cholesky_blocked(&a).unwrap();
            for i in 0..n {
                for j in 0..=i {
                    assert!(
                        (got[i][j] - want[i][j]).abs() < 1e-9,
                        "blocked cholesky mismatch at n={n}, ({i},{j}): {} vs {}",
                        got[i][j],
                        want[i][j]
                    );
                }
            }
        }
    }

    #[test]
    fn test_cholesky_blocked_roundtrip() {
        let n = 128;
        let a = rand_spd(n);
        let l = cholesky_blocked(&a).unwrap();
        for i in 0..n {
            for j in 0..n {
                let mut s = 0.0;
                for (a_, b_) in l[i][..n].iter().zip(l[j][..n].iter()) {
                    s += a_ * b_;
                }
                assert!((s - a[i][j]).abs() < 1e-8, "L·Lᵀ mismatch at ({i},{j})");
            }
        }
    }

    #[test]
    fn test_qr_roundtrip() {
        let a = vec![
            vec![12.0, -51.0, 4.0],
            vec![6.0, 167.0, -68.0],
            vec![-4.0, 24.0, -41.0],
        ];
        let (q, r) = qr_decompose(&a).unwrap();
        let m = a.len();
        let n = a[0].len();
        // Reconstruct Q·R and compare to A.
        let mut rec = vec![vec![0.0; n]; m];
        for i in 0..m {
            for j in 0..n {
                for k in 0..n {
                    rec[i][j] += q[i][k] * r[k][j];
                }
            }
        }
        for i in 0..m {
            for j in 0..n {
                assert!((rec[i][j] - a[i][j]).abs() < 1e-8);
            }
        }
        // Q should have orthonormal columns: Qᵀ·Q = I.
        for i in 0..n {
            for j in 0..n {
                let dot: Scalar = (0..m).map(|k| q[k][i] * q[k][j]).sum();
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((dot - expected).abs() < 1e-8);
            }
        }
    }
}
