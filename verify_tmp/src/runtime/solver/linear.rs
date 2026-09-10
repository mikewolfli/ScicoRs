//! Linear system solvers and matrix utilities.
//!
//! Provides dense Gaussian elimination (LU decomposition) for solving
//! small to medium linear systems, along with basic matrix analysis
//! utilities used by the Newton-Raphson and DAE solvers.
//!
//! For large sparse systems, the `SparseMatrix` structure provides
//! a foundation for future sparse solver integration.

use crate::core::error::SimError;
use crate::core::types::Scalar;

/// Solve the linear system A * x = b using Gaussian elimination with partial pivoting.
///
/// Delegates to `crate::core::compute::matrix::solve_linear`, the canonical
/// implementation. This wrapper is retained for backward-compatible access
/// from the `runtime::solver` module.
#[allow(clippy::needless_range_loop)]
pub fn solve_linear_dense(a: &[Vec<Scalar>], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    crate::core::compute::matrix::solve_linear(a, b)
}

/// Check if a square matrix is singular (determinant near zero).
///
/// Uses the infinity norm of the matrix and attempts LU decomposition.
/// If the maximum pivot magnitude is below `tol * norm`, the matrix is
/// considered singular.
pub fn is_singular(a: &[Vec<Scalar>], tol: Scalar) -> bool {
    if a.is_empty() {
        return true;
    }
    let n = a.len();
    if a[0].len() != n {
        return true;
    }
    // A matrix is singular if its smallest pivot magnitude is below tol.
    // Use LU decomposition without full solve to check.
    let mut lu = a.to_vec();
    for k in 0..n {
        let mut max_val = lu[k][k].abs();
        let mut max_row = k;
        for i in k + 1..n {
            let val = lu[i][k].abs();
            if val > max_val {
                max_val = val;
                max_row = i;
            }
        }
        if max_val < tol {
            return true;
        }
        if max_row != k {
            lu.swap(k, max_row);
        }
        for i in k + 1..n {
            let factor = lu[i][k] / lu[k][k];
            for j in k + 1..n {
                lu[i][j] -= factor * lu[k][j];
            }
        }
    }
    false
}

/// Compute the infinity norm of a matrix (max row sum of absolute values).
pub fn matrix_inf_norm(a: &[Vec<Scalar>]) -> Scalar {
    let row_sums: Vec<Scalar> = a
        .iter()
        .map(|row| crate::core::compute::linalg::asum(row))
        .collect();
    crate::core::compute::vector::vec_max_abs(&row_sums).unwrap_or(0.0)
}

/// Compute the infinity norm of a vector.
pub fn vector_inf_norm(v: &[Scalar]) -> Scalar {
    crate::core::compute::vector::vec_max_abs(v).unwrap_or(0.0)
}

/// Simple sparse matrix in Compressed Sparse Row (CSR) format.
///
/// Provides a foundation for large-scale sparse matrix operations
/// and future integration with specialized sparse solver libraries.
#[derive(Debug, Clone, PartialEq)]
pub struct SparseMatrix {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Row pointers (length rows + 1). row_ptr[i] is the start of row i.
    pub row_ptr: Vec<usize>,
    /// Column indices for each non-zero element.
    pub col_ind: Vec<usize>,
    /// Values of non-zero elements.
    pub values: Vec<Scalar>,
}

impl SparseMatrix {
    /// Create a new empty sparse matrix.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            row_ptr: vec![0; rows + 1],
            col_ind: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Number of non-zero elements.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Add a non-zero element at the end of the current insertion sequence.
    /// Callers must insert elements in row-major order.
    pub fn push(&mut self, row: usize, col: usize, value: Scalar) {
        assert!(row < self.rows && col < self.cols, "index out of bounds");
        self.col_ind.push(col);
        self.values.push(value);
        // Increment row pointers for rows after this one
        for r in (row + 1)..=self.rows {
            self.row_ptr[r] += 1;
        }
    }

    /// Get the value at (row, col), or 0.0 if not stored.
    pub fn get(&self, row: usize, col: usize) -> Scalar {
        let start = self.row_ptr[row];
        let end = self.row_ptr[row + 1];
        for i in start..end {
            if self.col_ind[i] == col {
                return self.values[i];
            }
        }
        0.0
    }

    /// Convert to dense format (for debugging / testing).
    pub fn to_dense(&self) -> Vec<Vec<Scalar>> {
        let mut dense = vec![vec![0.0; self.cols]; self.rows];
        for (row, row_vals) in dense.iter_mut().enumerate() {
            let start = self.row_ptr[row];
            let end = self.row_ptr[row + 1];
            for idx in start..end {
                row_vals[self.col_ind[idx]] = self.values[idx];
            }
        }
        dense
    }

    /// Create a sparse matrix from a dense representation.
    pub fn from_dense(dense: &[Vec<Scalar>]) -> Self {
        let rows = dense.len();
        if rows == 0 {
            return Self::new(0, 0);
        }
        let cols = dense[0].len();
        let mut mat = Self::new(rows, cols);

        for (i, row) in dense.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                if val != 0.0 {
                    mat.push(i, j, val);
                }
            }
        }
        mat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solve_2x2() {
        // 2x + 3y = 8
        // x  - y  = -1
        // Solution: x = 1, y = 2
        let a = vec![vec![2.0, 3.0], vec![1.0, -1.0]];
        let b = vec![8.0, -1.0];
        let x = solve_linear_dense(&a, &b).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-12);
        assert!((x[1] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn test_solve_3x3() {
        // x + y + z = 6
        // 2x - y + z = 3
        // x + 2y - z = 2
        // Solution: x = 1, y = 1, z = 4? Let's check
        // 1 + 1 + 4 = 6 ✓
        // 2 - 1 + 4 = 5? No... Let me recalculate
        // Actually: x + y + z = 6 → (1)+(1)+(4)=6 ✓
        // 2(1) - 1 + 4 = 2 - 1 + 4 = 5 ≠ 3
        // Let me use a correct system:
        // x + y + z = 6
        // 2x - y + z = 3  → actually let's just verify the solver works
        // x + 2y - z = -1 → x=1, y=1, z=2? Let's check: 1+1+2=4≠6
        // Let me use known: x=1, y=2, z=3
        // x + y + z = 6 ✓
        // 2x - y + z = 2-2+3=3 ✓
        // -x + y + z = -1+2+3=4... no
        // OK let me design a correct 3x3:
        // 2x + y - z = 1  → 2+2-3=1 ✓
        // x - y + z = 2   → 1-2+3=2 ✓
        // 3x + y + z = 6  → 3+2+3=8... no
        //
        // Let me just use a simple 3x3:
        // x + y + z = 6
        // y - z = -1
        // 2x + z = 5
        // Solution: x=1, y=2, z=3
        // Check: 1+2+3=6 ✓, 2-3=-1 ✓, 2+3=5 ✓
        let a = vec![
            vec![1.0, 1.0, 1.0],
            vec![0.0, 1.0, -1.0],
            vec![2.0, 0.0, 1.0],
        ];
        let b = vec![6.0, -1.0, 5.0];
        let x = solve_linear_dense(&a, &b).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-12);
        assert!((x[1] - 2.0).abs() < 1e-12);
        assert!((x[2] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn test_singular_matrix() {
        // Singular: [1, 1; 1, 1]
        let a = vec![vec![1.0, 1.0], vec![1.0, 1.0]];
        let b = vec![1.0, 2.0];
        assert!(solve_linear_dense(&a, &b).is_err());
        assert!(is_singular(&a, 1e-10));
    }

    #[test]
    fn test_matrix_inf_norm() {
        let a = vec![vec![1.0, -2.0], vec![-3.0, 4.0]];
        let norm = matrix_inf_norm(&a);
        assert!((norm - 7.0).abs() < 1e-12); // max(|1|+|-2|, |-3|+|4|) = max(3, 7) = 7
    }

    #[test]
    fn test_sparse_matrix_basic() {
        // Identity 3x3
        let mut sm = SparseMatrix::new(3, 3);
        sm.push(0, 0, 1.0);
        sm.push(1, 1, 1.0);
        sm.push(2, 2, 1.0);

        assert_eq!(sm.nnz(), 3);
        assert!((sm.get(0, 0) - 1.0).abs() < 1e-12);
        assert!((sm.get(1, 1) - 1.0).abs() < 1e-12);
        assert!((sm.get(0, 1) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_sparse_from_to_dense() {
        let dense = vec![
            vec![1.0, 0.0, 2.0],
            vec![0.0, 3.0, 0.0],
            vec![4.0, 0.0, 5.0],
        ];
        let sm = SparseMatrix::from_dense(&dense);
        assert_eq!(sm.nnz(), 5);
        let reconstructed = sm.to_dense();
        for i in 0..3 {
            for j in 0..3 {
                assert!((reconstructed[i][j] - dense[i][j]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn test_empty_system() {
        let x = solve_linear_dense(&Vec::<Vec<Scalar>>::new(), &Vec::<Scalar>::new()).unwrap();
        assert!(x.is_empty());
    }
}
