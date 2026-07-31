//! Matrix operations — multiply, inverse, determinant, transpose, decompositions.
//!
//! All functions operate on `Vec<Vec<Scalar>>` (row-major dense storage).
//! This is the single source of truth for matrix computations in SCIcoRS;
//! domain modules (FEA, MNA, quantum, etc.) should use these rather than
//! re-implementing their own.

use crate::core::error::SimError;
use crate::core::types::Scalar;

/// Multiply two matrices: C = A * B.
///
/// A is m×n, B is n×p → result is m×p.
///
/// For non-trivial sizes this uses the pure-Rust SIMD `matrixmultiply` kernel
/// (real acceleration, no external BLAS required); tiny problems fall back to
/// the naive reference loop to avoid launch overhead. Results are identical.
pub fn mat_mul(a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    if a.is_empty() || b.is_empty() {
        return Ok(Vec::new());
    }
    let m = a.len();
    let k = a[0].len();
    if b.len() != k {
        return Err(SimError::numerical(format!(
            "mat_mul: inner dimensions don't match: A cols={}, B rows={}",
            k,
            b.len()
        )));
    }
    let n = b[0].len();
    let work = m.saturating_mul(k).saturating_mul(n);
    if work >= SIMD_MIN_WORK {
        Ok(mat_mul_simd(a, b, m, k, n))
    } else {
        Ok(mat_mul_naive(a, b, m, k, n))
    }
}

/// Work units at which the SIMD kernel beats the naive loop (its flatten copy
/// is O(n²), amortized by the O(n³) multiply).
const SIMD_MIN_WORK: usize = 4096;

/// Reference (naive, cache-friendly `ikj`) matrix multiply. Kept for
/// correctness tests and benchmarks; not used on the hot path.
pub fn mat_mul_naive(
    a: &[Vec<Scalar>],
    b: &[Vec<Scalar>],
    m: usize,
    k: usize,
    n: usize,
) -> Vec<Vec<Scalar>> {
    let mut c = vec![vec![0.0; n]; m];
    for i in 0..m {
        for kk in 0..k {
            let aik = a[i][kk];
            if aik == 0.0 {
                continue;
            }
            for j in 0..n {
                c[i][j] += aik * b[kk][j];
            }
        }
    }
    c
}

/// Pure-Rust SIMD matrix multiply via `matrixmultiply` (auto-vectorized,
/// runtime-detected x86-64 AVX2/FMA / aarch64 NEON kernels, zero C deps).
fn mat_mul_simd(
    a: &[Vec<Scalar>],
    b: &[Vec<Scalar>],
    m: usize,
    k: usize,
    n: usize,
) -> Vec<Vec<Scalar>> {
    // Flatten into contiguous row-major buffers (the O(n³) multiply dominates
    // the O(n²) copy).
    let mut a_flat = Vec::with_capacity(m * k);
    for row in a.iter().take(m) {
        a_flat.extend_from_slice(&row[..k]);
    }
    let mut b_flat = Vec::with_capacity(k * n);
    for row in b.iter().take(k) {
        b_flat.extend_from_slice(&row[..n]);
    }
    let mut c_flat = vec![0.0; m * n];
    super::simd::dgemm(m, k, n, &a_flat, &b_flat, &mut c_flat);
    let mut c = Vec::with_capacity(m);
    for i in 0..m {
        c.push(c_flat[i * n..(i + 1) * n].to_vec());
    }
    c
}

/// Parallel SIMD matrix multiply: tiles rows across the rayon pool; each tile
/// uses the SIMD `matrixmultiply` kernel (SIMD × multi-threading).
pub fn mat_mul_parallel(
    a: &[Vec<Scalar>],
    b: &[Vec<Scalar>],
) -> Result<Vec<Vec<Scalar>>, SimError> {
    if a.is_empty() || b.is_empty() {
        return Ok(Vec::new());
    }
    let m = a.len();
    let k = a[0].len();
    if b.len() != k {
        return Err(SimError::numerical(format!(
            "mat_mul_parallel: inner dimensions don't match: A cols={}, B rows={}",
            k,
            b.len()
        )));
    }
    let n = b[0].len();
    // Flatten B once (shared by all row tiles).
    let mut b_flat = Vec::with_capacity(k * n);
    for row in b.iter().take(k) {
        b_flat.extend_from_slice(&row[..n]);
    }
    let mut c_flat = vec![0.0; m * n];
    {
        use rayon::prelude::*;
        let tile = 64; // rows per SIMD tile
        c_flat
            .par_chunks_mut(tile * n)
            .enumerate()
            .for_each(|(t, c_tile)| {
                let i0 = t * tile;
                let rows = c_tile.len() / n;
                let i1 = (i0 + rows).min(m);
                // Flatten this tile's rows of A.
                let mut a_tile = Vec::with_capacity(rows * k);
                for row in a.iter().take(i1).skip(i0) {
                    a_tile.extend_from_slice(&row[..k]);
                }
                let m_tile = rows;
                super::simd::dgemm(m_tile, k, n, &a_tile, &b_flat, c_tile);
            });
    }
    let mut c = Vec::with_capacity(m);
    for i in 0..m {
        c.push(c_flat[i * n..(i + 1) * n].to_vec());
    }
    Ok(c)
}

/// Multiply matrix by vector: y = A * x.
///
/// Delegates to the adaptive BLAS-2 `gemv` so large products benefit from
/// rayon parallelism / a registered GPU backend.
pub fn mat_vec_mul(a: &[Vec<Scalar>], x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    super::linalg::gemv(a, x)
}

/// Transpose a matrix.
pub fn transpose(a: &[Vec<Scalar>]) -> Vec<Vec<Scalar>> {
    if a.is_empty() {
        return Vec::new();
    }
    let m = a.len();
    let n = a[0].len();
    let mut at = vec![vec![0.0; m]; n];
    for i in 0..m {
        for j in 0..n {
            at[j][i] = a[i][j];
        }
    }
    at
}

/// Compute the determinant of a square matrix using LU decomposition.
pub fn determinant(a: &[Vec<Scalar>]) -> Result<Scalar, SimError> {
    let n = a.len();
    if n == 0 {
        return Ok(1.0);
    }
    if a[0].len() != n {
        return Err(SimError::numerical("determinant: matrix is not square"));
    }
    if n == 1 {
        return Ok(a[0][0]);
    }
    if n == 2 {
        return Ok(a[0][0] * a[1][1] - a[0][1] * a[1][0]);
    }

    // LU decomposition with partial pivoting
    let mut lu = a.to_vec();
    let mut sign = 1.0;
    for k in 0..n - 1 {
        // Find pivot
        let mut max_val = lu[k][k].abs();
        let mut max_row = k;
        for i in k + 1..n {
            let val = lu[i][k].abs();
            if val > max_val {
                max_val = val;
                max_row = i;
            }
        }
        if max_val < 1e-15 {
            return Ok(0.0);
        }
        if max_row != k {
            lu.swap(k, max_row);
            sign = -sign;
        }
        let pivot = lu[k][k];
        let pivot_row: Vec<Scalar> = lu[k][k + 1..].to_vec();
        for i in k + 1..n {
            let factor = lu[i][k] / pivot;
            lu[i][k] = factor;
            for (a, &p) in lu[i][k + 1..].iter_mut().zip(pivot_row.iter()) {
                *a -= factor * p;
            }
        }
    }
    let mut det = sign;
    for i in 0..n {
        det *= lu[i][i];
    }
    Ok(det)
}

/// Compute the inverse of a square matrix using Gaussian elimination.
pub fn inverse(a: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    let n = a.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if a[0].len() != n {
        return Err(SimError::numerical("inverse: matrix is not square"));
    }
    // Large matrices use the LU-based path (≈1.33n³ with vectorized forward /
    // back substitution) instead of Gauss-Jordan (≈2n³). Results are
    // identical to machine precision.
    if n >= INVERSE_LU_MIN {
        return inverse_lu(a);
    }

    // Augmented matrix [A | I]
    let mut aug = vec![vec![0.0; 2 * n]; n];
    for i in 0..n {
        for j in 0..n {
            aug[i][j] = a[i][j];
        }
        aug[i][n + i] = 1.0;
    }

    // Forward elimination with partial pivoting
    for col in 0..n {
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for row in col + 1..n {
            let val = aug[row][col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }
        if max_val < 1e-15 {
            return Err(SimError::numerical("inverse: matrix is singular"));
        }
        if max_row != col {
            aug.swap(col, max_row);
        }

        let pivot = aug[col][col];
        for v in aug[col].iter_mut() {
            *v /= pivot;
        }
        // Snapshot the (now normalized) pivot row once; the row eliminations
        // below then vectorize (disjoint source/destination slices).
        let pivot_row: Vec<Scalar> = aug[col].clone();
        for row in 0..n {
            if row != col {
                let factor = aug[row][col];
                for (a, &p) in aug[row].iter_mut().zip(pivot_row.iter()) {
                    *a -= factor * p;
                }
            }
        }
    }

    // Extract inverse from augmented matrix
    let mut inv = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            inv[i][j] = aug[i][n + j];
        }
    }
    Ok(inv)
}

/// Matrices at least this large use the LU-based inverse path.
const INVERSE_LU_MIN: usize = 96;

/// LU-based inverse: `A⁻¹ = U⁻¹·L⁻¹·P`.
///
/// Factors once (`A = P⁻¹·L·U`) then performs matrix forward/back
/// substitution on all `n` columns at once. The substitution inner loops are
/// row-wise axpys over disjoint slices, so they auto-vectorize; total work is
/// ≈1.33n³ vs ≈2n³ for the Gauss-Jordan path. Singularity detection uses the
/// LU factor's tolerance (pivot < 1e-300).
fn inverse_lu(a: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    let n = a.len();
    let (lu, piv) = crate::core::compute::linalg::lu_decompose(a)?;
    // X (flat row-major) starts as the permutation P: X[i][piv[i]] = 1, so
    // column c of X is the permuted RHS e_c (matching `lu_solve`).
    let mut xf = vec![0.0; n * n];
    for i in 0..n {
        xf[i * n + piv[i]] = 1.0;
    }
    // Forward substitution: L·X = P (L is unit lower triangular). Row i uses
    // rows j < i (already final); `split_at_mut` gives disjoint borrows of the
    // two rows so the axpy auto-vectorizes.
    for i in 0..n {
        for j in 0..i {
            let l = lu[i][j];
            let (low, high) = xf.split_at_mut(i * n);
            let xj = &low[j * n..(j + 1) * n];
            let xi = &mut high[..n];
            for (a, &p) in xi.iter_mut().zip(xj.iter()) {
                *a -= l * p;
            }
        }
    }
    // Back substitution: U·X = L⁻¹·P → X = A⁻¹. Row i uses rows j > i.
    for i in (0..n).rev() {
        for j in i + 1..n {
            let u = lu[i][j];
            let (low, high) = xf.split_at_mut(j * n);
            let xi = &mut low[i * n..(i + 1) * n];
            let xj = &high[..n];
            for (a, &p) in xi.iter_mut().zip(xj.iter()) {
                *a -= u * p;
            }
        }
        let d = lu[i][i];
        for v in xf[i * n..(i + 1) * n].iter_mut() {
            *v /= d;
        }
    }
    // Rebuild Vec<Vec>.
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        x.push(xf[i * n..(i + 1) * n].to_vec());
    }
    Ok(x)
}

/// Solve A * x = b using Gaussian elimination with partial pivoting.
pub fn solve_linear(a: &[Vec<Scalar>], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    let n = a.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if a[0].len() != n {
        return Err(SimError::numerical("solve_linear: matrix is not square"));
    }
    if b.len() != n {
        return Err(SimError::numerical("solve_linear: RHS length mismatch"));
    }

    let mut aug: Vec<Vec<Scalar>> = a.to_vec();
    for i in 0..n {
        aug[i].push(b[i]);
    }

    for col in 0..n {
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for row in col + 1..n {
            let val = aug[row][col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }
        if max_val < 1e-15 {
            return Err(SimError::numerical("solve_linear: singular matrix"));
        }
        if max_row != col {
            aug.swap(col, max_row);
        }

        let pivot = aug[col][col];
        for v in aug[col].iter_mut().skip(col) {
            *v /= pivot;
        }
        let pivot_row: Vec<Scalar> = aug[col][col..=n].to_vec();
        for row in col + 1..n {
            let factor = aug[row][col];
            for (a, &p) in aug[row][col..=n].iter_mut().zip(pivot_row.iter()) {
                *a -= factor * p;
            }
        }
    }

    // Back substitution
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        x[i] = aug[i][n];
        for j in i + 1..n {
            x[i] -= aug[i][j] * x[j];
        }
    }
    Ok(x)
}

/// Solve a complex linear system A * x = b by embedding into a real 2n×2n system.
///
/// A is n×n complex, b is length-n complex. Returns x of length-n complex.
/// Uses `solve_linear` internally (Gaussian elimination on the 2n×2n real embedding).
pub fn solve_complex(
    a: &[Vec<num_complex::Complex64>],
    b: &[num_complex::Complex64],
) -> Result<Vec<num_complex::Complex64>, SimError> {
    let n = a.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if a[0].len() != n {
        return Err(SimError::numerical("solve_complex: matrix is not square"));
    }
    if b.len() != n {
        return Err(SimError::numerical("solve_complex: RHS length mismatch"));
    }

    // Embed complex n×n system into real 2n×2n:
    // [ Re(A)  -Im(A) ] [ Re(x) ] = [ Re(b) ]
    // [ Im(A)   Re(A) ] [ Im(x) ]   [ Im(b) ]
    let mut real_a = vec![vec![0.0; 2 * n]; 2 * n];
    for i in 0..n {
        for j in 0..n {
            real_a[i][j] = a[i][j].re;
            real_a[i][n + j] = -a[i][j].im;
            real_a[n + i][j] = a[i][j].im;
            real_a[n + i][n + j] = a[i][j].re;
        }
    }

    let mut real_b = vec![0.0; 2 * n];
    for i in 0..n {
        real_b[i] = b[i].re;
        real_b[n + i] = b[i].im;
    }

    let real_x = solve_linear(&real_a, &real_b)?;

    Ok((0..n)
        .map(|i| num_complex::Complex64::new(real_x[i], real_x[n + i]))
        .collect())
}

/// Check if a matrix is symmetric (within EPSILON).
pub fn is_symmetric(a: &[Vec<Scalar>]) -> bool {
    if a.is_empty() {
        return true;
    }
    let n = a.len();
    for i in 0..n {
        for j in 0..n {
            if (a[i][j] - a[j][i]).abs() > crate::core::types::EPSILON {
                return false;
            }
        }
    }
    true
}

/// Frobenius norm of a matrix.
pub fn frobenius_norm(a: &[Vec<Scalar>]) -> Scalar {
    let mut s = 0.0;
    for row in a {
        for &v in row {
            s += v * v;
        }
    }
    s.sqrt()
}

/// Identity matrix of size n×n.
pub fn identity(n: usize) -> Vec<Vec<Scalar>> {
    let mut ident = vec![vec![0.0; n]; n];
    for i in 0..n {
        ident[i][i] = 1.0;
    }
    ident
}

/// Create a zero matrix of size m×n.
pub fn zeros(m: usize, n: usize) -> Vec<Vec<Scalar>> {
    vec![vec![0.0; n]; m]
}

/// Create a diagonal matrix from a vector.
pub fn diag(d: &[Scalar]) -> Vec<Vec<Scalar>> {
    let n = d.len();
    let mut diag_mat = vec![vec![0.0; n]; n];
    for i in 0..n {
        diag_mat[i][i] = d[i];
    }
    diag_mat
}

/// Trace of a square matrix.
pub fn trace(a: &[Vec<Scalar>]) -> Result<Scalar, SimError> {
    let n = a.len();
    if n == 0 {
        return Ok(0.0);
    }
    if a[0].len() != n {
        return Err(SimError::numerical("trace: matrix is not square"));
    }
    let mut t = 0.0;
    for i in 0..n {
        t += a[i][i];
    }
    Ok(t)
}

/// Parallel matrix multiply using rayon.
///
/// Same semantics as `mat_mul` but the outer row loop is dispatched
/// across available threads. Best for large matrices (m × n × p > ~1000).
///
/// Falls back to serial `mat_mul` if rayon is not available or the
/// matrix is too small to benefit from parallelism.
///
/// This is now a thin wrapper over the adaptive dispatcher
/// ([`crate::core::compute::backend`]) which also supports GPU dispatch
/// once a GPU backend is registered.
pub fn par_mat_mul(a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    crate::core::compute::backend::adaptive_mat_mul(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mat_mul_basic() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let b = vec![vec![5.0, 6.0], vec![7.0, 8.0]];
        let c = mat_mul(&a, &b).unwrap();
        assert!((c[0][0] - 19.0).abs() < 1e-10);
        assert!((c[0][1] - 22.0).abs() < 1e-10);
        assert!((c[1][0] - 43.0).abs() < 1e-10);
        assert!((c[1][1] - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_mat_mul_simd_matches_naive_reference() {
        // 32×32×32 = 32768 work units, well above the SIMD threshold (4096),
        // so `mat_mul` exercises the SIMD kernel; must equal the naive
        // reference within float tolerance.
        let a: Vec<Vec<Scalar>> = (0..32)
            .map(|i| {
                (0..32)
                    .map(|j| ((i * 13 + j * 5) as Scalar) * 0.75)
                    .collect()
            })
            .collect();
        let b: Vec<Vec<Scalar>> = (0..32)
            .map(|i| {
                (0..32)
                    .map(|j| ((i * 3 - j * 7) as Scalar) * -0.4)
                    .collect()
            })
            .collect();
        let want = mat_mul_naive(&a, &b, 32, 32, 32);
        let got = mat_mul(&a, &b).unwrap();
        for i in 0..32 {
            for j in 0..32 {
                assert!(
                    (got[i][j] - want[i][j]).abs() < 1e-9,
                    "SIMD mismatch at ({i},{j}): {} vs {}",
                    got[i][j],
                    want[i][j]
                );
            }
        }
    }

    #[test]
    fn test_mat_mul_parallel_matches_naive_reference() {
        let a: Vec<Vec<Scalar>> = (0..40)
            .map(|i| {
                (0..24)
                    .map(|j| ((i * 11 - j * 3) as Scalar) * 0.5)
                    .collect()
            })
            .collect();
        let b: Vec<Vec<Scalar>> = (0..24)
            .map(|i| {
                (0..36)
                    .map(|j| ((i * 7 + j * 2) as Scalar) * 0.25)
                    .collect()
            })
            .collect();
        let want = mat_mul_naive(&a, &b, 40, 24, 36);
        let got = mat_mul_parallel(&a, &b).unwrap();
        for i in 0..40 {
            for j in 0..36 {
                assert!(
                    (got[i][j] - want[i][j]).abs() < 1e-9,
                    "parallel mismatch at ({i},{j}): {} vs {}",
                    got[i][j],
                    want[i][j]
                );
            }
        }
    }

    #[test]
    fn test_mat_vec_mul() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let x = vec![5.0, 6.0];
        let y = mat_vec_mul(&a, &x).unwrap();
        assert!((y[0] - 17.0).abs() < 1e-10);
        assert!((y[1] - 39.0).abs() < 1e-10);
    }

    #[test]
    fn test_determinant_2x2() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        assert!((determinant(&a).unwrap() - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn test_determinant_3x3() {
        let a = vec![
            vec![6.0, 1.0, 1.0],
            vec![4.0, -2.0, 5.0],
            vec![2.0, 8.0, 7.0],
        ];
        assert!((determinant(&a).unwrap() - (-306.0)).abs() < 1e-10);
    }

    #[test]
    fn test_inverse_2x2() {
        let a = vec![vec![4.0, 7.0], vec![2.0, 6.0]];
        let inv = inverse(&a).unwrap();
        let i = mat_mul(&a, &inv).unwrap();
        assert!((i[0][0] - 1.0).abs() < 1e-10);
        assert!((i[0][1]).abs() < 1e-10);
        assert!((i[1][0]).abs() < 1e-10);
        assert!((i[1][1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_inverse_lu_path_matches_identity() {
        // 120 ≥ INVERSE_LU_MIN → exercises the LU-based fast path; A·A⁻¹ = I.
        let n = 120;
        let a: Vec<Vec<Scalar>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        let v = ((i * 7 + j * 3) % 97) as Scalar * 0.05
                            + if i == j { 3.0 } else { 0.0 };
                        v
                    })
                    .collect()
            })
            .collect();
        let inv = inverse(&a).unwrap();
        let prod = mat_mul(&a, &inv).unwrap();
        for i in 0..n {
            for j in 0..n {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (prod[i][j] - want).abs() < 1e-6,
                    "A·A⁻¹ mismatch at ({i},{j}): {} vs {want}",
                    prod[i][j]
                );
            }
        }
    }

    #[test]
    fn test_inverse_lu_matches_gauss_jordan() {
        // The LU path (n ≥ 96) must equal Gauss-Jordan on the same matrix.
        // Implement a small Gauss-Jordan reference inline for comparison.
        fn gauss_jordan_inverse(a: &[Vec<Scalar>]) -> Vec<Vec<Scalar>> {
            let n = a.len();
            let mut aug = vec![vec![0.0; 2 * n]; n];
            for i in 0..n {
                for j in 0..n {
                    aug[i][j] = a[i][j];
                }
                aug[i][n + i] = 1.0;
            }
            for col in 0..n {
                let pivot = aug[col][col];
                for j in 0..2 * n {
                    aug[col][j] /= pivot;
                }
                for row in 0..n {
                    if row != col {
                        let factor = aug[row][col];
                        for j in 0..2 * n {
                            aug[row][j] -= factor * aug[col][j];
                        }
                    }
                }
            }
            (0..n).map(|i| aug[i][n..].to_vec()).collect()
        }
        let n = 100;
        let a: Vec<Vec<Scalar>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        ((i * 11 + j * 5) % 89) as Scalar * 0.1 + if i == j { 5.0 } else { 0.0 }
                    })
                    .collect()
            })
            .collect();
        let lu_inv = inverse(&a).unwrap();
        let gj_inv = gauss_jordan_inverse(&a);
        for i in 0..n {
            for j in 0..n {
                assert!(
                    (lu_inv[i][j] - gj_inv[i][j]).abs() < 1e-6,
                    "LU vs Gauss-Jordan inverse mismatch at ({i},{j}): {} vs {}",
                    lu_inv[i][j],
                    gj_inv[i][j]
                );
            }
        }
    }

    #[test]
    fn test_solve_linear() {
        let a = vec![vec![3.0, 2.0], vec![1.0, 2.0]];
        let b = vec![7.0, 5.0];
        let x = solve_linear(&a, &b).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-10);
        assert!((x[1] - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_transpose() {
        let a = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let at = transpose(&a);
        assert_eq!(at.len(), 3);
        assert_eq!(at[0].len(), 2);
        assert!((at[0][0] - 1.0).abs() < 1e-10);
        assert!((at[2][1] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_identity() {
        let i = identity(3);
        assert!((i[0][0] - 1.0).abs() < 1e-10);
        assert!((i[0][1]).abs() < 1e-10);
        assert!((i[1][1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_is_symmetric() {
        let a = vec![vec![1.0, 2.0], vec![2.0, 3.0]];
        assert!(is_symmetric(&a));
        let b = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        assert!(!is_symmetric(&b));
    }

    #[test]
    fn test_frobenius_norm() {
        let a = vec![vec![3.0, 4.0]];
        assert!((frobenius_norm(&a) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_empty_matrix() {
        let a: Vec<Vec<Scalar>> = vec![];
        let b: Vec<Vec<Scalar>> = vec![];
        let c = mat_mul(&a, &b).unwrap();
        assert!(c.is_empty());
        assert!(determinant(&a).unwrap() - 1.0 < 1e-10);
    }

    #[test]
    fn test_trace() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        assert!((trace(&a).unwrap() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_zeros_and_diag() {
        let z = zeros(2, 3);
        assert_eq!(z.len(), 2);
        assert_eq!(z[0].len(), 3);
        let d = diag(&[1.0, 2.0, 3.0]);
        assert!((d[0][0] - 1.0).abs() < 1e-10);
        assert!((d[1][1] - 2.0).abs() < 1e-10);
        assert!((d[0][1]).abs() < 1e-10);
    }

    #[test]
    fn test_singular_matrix_error() {
        let a = vec![vec![1.0, 2.0], vec![1.0, 2.0]];
        let b = vec![3.0, 4.0];
        assert!(solve_linear(&a, &b).is_err());
        assert!(inverse(&a).is_err());
    }
}
