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
pub fn mat_mul(a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    if a.is_empty() || b.is_empty() {
        return Ok(Vec::new());
    }
    let m = a.len();
    let n = a[0].len();
    if b.len() != n {
        return Err(SimError::numerical(format!(
            "mat_mul: inner dimensions don't match: A cols={}, B rows={}",
            n,
            b.len()
        )));
    }
    let p = b[0].len();
    let mut c = vec![vec![0.0; p]; m];
    for i in 0..m {
        for k in 0..n {
            let aik = a[i][k];
            if aik == 0.0 {
                continue;
            }
            for j in 0..p {
                c[i][j] += aik * b[k][j];
            }
        }
    }
    Ok(c)
}

/// Multiply matrix by vector: y = A * x.
pub fn mat_vec_mul(a: &[Vec<Scalar>], x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    if a.is_empty() {
        return Ok(Vec::new());
    }
    let m = a.len();
    let n = a[0].len();
    if x.len() != n {
        return Err(SimError::numerical(format!(
            "mat_vec_mul: matrix cols={}, vector len={}",
            n,
            x.len()
        )));
    }
    let mut y = vec![0.0; m];
    for i in 0..m {
        let row = &a[i];
        let mut s = 0.0;
        for j in 0..n {
            s += row[j] * x[j];
        }
        y[i] = s;
    }
    Ok(y)
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
        for i in k + 1..n {
            let factor = lu[i][k] / lu[k][k];
            lu[i][k] = factor;
            for j in k + 1..n {
                lu[i][j] -= factor * lu[k][j];
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

    // Extract inverse from augmented matrix
    let mut inv = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            inv[i][j] = aug[i][n + j];
        }
    }
    Ok(inv)
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
        for j in col..=n {
            aug[col][j] /= pivot;
        }
        for row in col + 1..n {
            let factor = aug[row][col];
            for j in col..=n {
                aug[row][j] -= factor * aug[col][j];
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
pub fn par_mat_mul(a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
    if a.is_empty() || b.is_empty() {
        return Ok(Vec::new());
    }
    let m = a.len();
    let n = a[0].len();
    if b.len() != n {
        return Err(SimError::numerical(format!(
            "par_mat_mul: inner dimensions don't match: A cols={}, B rows={}",
            n,
            b.len()
        )));
    }
    let p = b[0].len();

    // For small matrices, serial is faster
    if m * n * p < 1000 {
        return mat_mul(a, b);
    }

    use rayon::prelude::*;
    let mut c = vec![vec![0.0; p]; m];
    let a_ref = a; // borrow for closure
    c.par_iter_mut().enumerate().for_each(|(i, row)| {
        for k in 0..n {
            let aik = a_ref[i][k];
            if aik == 0.0 {
                continue;
            }
            let bk = &b[k];
            for j in 0..p {
                row[j] += aik * bk[j];
            }
        }
    });
    Ok(c)
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
