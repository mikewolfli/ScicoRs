// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Sparse matrix arithmetic: products, combinations and sparse vector helpers.
//!
//! These routines operate directly on CSR storage so that no dense matrix is
//! materialized. The sparse-matrix-times-sparse-matrix product uses a
//! row-by-row symbolic pass with a dense accumulator per row, giving `O(nnz(A)
//! · avg_row_nnz(B))` work without a global dense intermediate.

use super::matrix::{CsrMatrix, SparseError};
use crate::core::types::Scalar;

/// Add two sparse matrices with identical logical shape: `C = A + B`.
///
/// The result keeps the union of the two sparsity patterns; entries that cancel
/// to exactly zero remain structural (call [`CsrMatrix::eliminate_zeros`] to
/// drop them).
pub fn add(a: &CsrMatrix, b: &CsrMatrix) -> Result<CsrMatrix, SparseError> {
    if a.nrows() != b.nrows() || a.ncols() != b.ncols() {
        return Err(SparseError::ShapeMismatch {
            detail: format!(
                "add: A is {}x{}, B is {}x{}",
                a.nrows(),
                a.ncols(),
                b.nrows(),
                b.ncols()
            ),
        });
    }
    merge_by_row(a, b, |x, y| x + y)
}

/// Subtract two sparse matrices with identical logical shape: `C = A - B`.
pub fn sub(a: &CsrMatrix, b: &CsrMatrix) -> Result<CsrMatrix, SparseError> {
    if a.nrows() != b.nrows() || a.ncols() != b.ncols() {
        return Err(SparseError::ShapeMismatch {
            detail: format!(
                "sub: A is {}x{}, B is {}x{}",
                a.nrows(),
                a.ncols(),
                b.nrows(),
                b.ncols()
            ),
        });
    }
    merge_by_row(a, b, |x, y| x - y)
}

/// Shared row-merge used by [`add`] and [`sub`].
///
/// Rows are already column-sorted in CSR, so a two-pointer merge yields the
/// union with ordered column indices in a single pass.
fn merge_by_row(
    a: &CsrMatrix,
    b: &CsrMatrix,
    op: impl Fn(Scalar, Scalar) -> Scalar,
) -> Result<CsrMatrix, SparseError> {
    let nrows = a.nrows();
    let ncols = a.ncols();
    let mut row_ptr = Vec::with_capacity(nrows + 1);
    let mut col_idx = Vec::new();
    let mut values = Vec::new();
    row_ptr.push(0);
    for i in 0..nrows {
        let mut ka = a.row_ptr()[i];
        let mut kb = b.row_ptr()[i];
        let ea = a.row_ptr()[i + 1];
        let eb = b.row_ptr()[i + 1];
        while ka < ea || kb < eb {
            let ca = if ka < ea { a.col_idx()[ka] } else { usize::MAX };
            let cb = if kb < eb { b.col_idx()[kb] } else { usize::MAX };
            if ca == cb {
                col_idx.push(ca);
                values.push(op(a.values()[ka], b.values()[kb]));
                ka += 1;
                kb += 1;
            } else if ca < cb {
                col_idx.push(ca);
                values.push(op(a.values()[ka], 0.0));
                ka += 1;
            } else {
                col_idx.push(cb);
                values.push(op(0.0, b.values()[kb]));
                kb += 1;
            }
        }
        row_ptr.push(values.len());
    }
    CsrMatrix::from_parts(nrows, ncols, row_ptr, col_idx, values)
}

/// Scalar-multiply a sparse matrix: `B = alpha · A`.
pub fn scale(a: &CsrMatrix, alpha: Scalar) -> CsrMatrix {
    let values: Vec<Scalar> = a.values().iter().map(|&v| v * alpha).collect();
    CsrMatrix::from_parts(
        a.nrows(),
        a.ncols(),
        a.row_ptr().to_vec(),
        a.col_idx().to_vec(),
        values,
    )
    .expect("scaling preserves a valid CSR structure")
}

/// Sparse matrix product: `C = A · B`.
///
/// `A` is `m × k`, `B` is `k × n`. The implementation accumulates each output
/// row into a dense workspace of length `n`, so peak extra memory is `O(n)`
/// regardless of `nnz`.
pub fn mat_mul(a: &CsrMatrix, b: &CsrMatrix) -> Result<CsrMatrix, SparseError> {
    if a.ncols() != b.nrows() {
        return Err(SparseError::ShapeMismatch {
            detail: format!("mat_mul: A cols {} != B rows {}", a.ncols(), b.nrows()),
        });
    }
    let m = a.nrows();
    let n = b.ncols();
    let mut row_ptr = Vec::with_capacity(m + 1);
    let mut col_idx = Vec::new();
    let mut values = Vec::new();
    row_ptr.push(0);
    let mut acc = vec![0.0; n];
    let mut touched: Vec<usize> = Vec::new();
    for i in 0..m {
        touched.clear();
        for ka in a.row_ptr()[i]..a.row_ptr()[i + 1] {
            let k = a.col_idx()[ka];
            let av = a.values()[ka];
            if av == 0.0 {
                continue;
            }
            for kb in b.row_ptr()[k]..b.row_ptr()[k + 1] {
                let j = b.col_idx()[kb];
                if acc[j] == 0.0 {
                    touched.push(j);
                }
                acc[j] += av * b.values()[kb];
            }
        }
        // Emit in ascending column order without re-sorting: collect indices
        // then sort, since `touched` order follows B's row traversal.
        touched.sort_unstable();
        for &j in &touched {
            if acc[j] != 0.0 {
                col_idx.push(j);
                values.push(acc[j]);
            }
            acc[j] = 0.0;
        }
        row_ptr.push(values.len());
    }
    CsrMatrix::from_parts(m, n, row_ptr, col_idx, values)
}

/// Transpose a sparse matrix (CSR → CSR of the transpose).
pub fn transpose(a: &CsrMatrix) -> CsrMatrix {
    // Counting-sort transpose: count entries per old column (= new row), prefix
    // sum into the new row pointers, then scatter. O(nnz) with no dense buffer.
    let rows = a.nrows();
    let cols = a.ncols();
    let mut row_ptr = vec![0usize; cols + 1];
    for &c in a.col_idx() {
        row_ptr[c + 1] += 1;
    }
    for i in 0..cols {
        row_ptr[i + 1] += row_ptr[i];
    }
    let mut next = row_ptr.clone();
    let mut col_idx = vec![0usize; a.nnz()];
    let mut values = vec![0.0 as Scalar; a.nnz()];
    for i in 0..rows {
        for k in a.row_ptr()[i]..a.row_ptr()[i + 1] {
            let c = a.col_idx()[k];
            let pos = next[c];
            col_idx[pos] = i;
            values[pos] = a.values()[k];
            next[c] += 1;
        }
    }
    CsrMatrix::from_parts(cols, rows, row_ptr, col_idx, values)
        .expect("transpose preserves a valid CSR structure")
}

/// Compute the dense residual vector `r = b − A · x` without forming `A·x`
/// separately.
pub fn residual(a: &CsrMatrix, x: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SparseError> {
    if x.len() != a.ncols() {
        return Err(SparseError::ShapeMismatch {
            detail: format!("residual: x length {} != ncols {}", x.len(), a.ncols()),
        });
    }
    if b.len() != a.nrows() {
        return Err(SparseError::ShapeMismatch {
            detail: format!("residual: b length {} != nrows {}", b.len(), a.nrows()),
        });
    }
    let mut r = vec![0.0; a.nrows()];
    for i in 0..a.nrows() {
        let mut acc = 0.0;
        for k in a.row_ptr()[i]..a.row_ptr()[i + 1] {
            acc += a.values()[k] * x[a.col_idx()[k]];
        }
        r[i] = b[i] - acc;
    }
    Ok(r)
}

/// Euclidean (2-)norm of a dense vector.
pub fn norm2(x: &[Scalar]) -> Scalar {
    x.iter().map(|&v| v * v).sum::<Scalar>().sqrt()
}

/// Dot product of two dense vectors.
pub fn dot(a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SparseError> {
    if a.len() != b.len() {
        return Err(SparseError::ShapeMismatch {
            detail: format!("dot: lengths {} and {}", a.len(), b.len()),
        });
    }
    Ok(a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum())
}

/// AXPY: `y += alpha · x`, returning the updated vector.
pub fn axpy(alpha: Scalar, x: &[Scalar], y: &mut [Scalar]) -> Result<(), SparseError> {
    if x.len() != y.len() {
        return Err(SparseError::ShapeMismatch {
            detail: format!("axpy: x length {} != y length {}", x.len(), y.len()),
        });
    }
    for (yi, &xi) in y.iter_mut().zip(x.iter()) {
        *yi += alpha * xi;
    }
    Ok(())
}

/// Extract a sparse submatrix by selecting rows and columns.
///
/// Indices must be strictly increasing; the result keeps only entries whose row
/// and column are both selected.
pub fn select(a: &CsrMatrix, rows: &[usize], cols: &[usize]) -> Result<CsrMatrix, SparseError> {
    // Map old column index → new position, or usize::MAX if not selected.
    let mut col_map = vec![usize::MAX; a.ncols()];
    for (new_c, &old_c) in cols.iter().enumerate() {
        if old_c >= a.ncols() {
            return Err(SparseError::ColIndexOutOfBounds {
                col: old_c,
                ncols: a.ncols(),
            });
        }
        col_map[old_c] = new_c;
    }
    let mut row_ptr = Vec::with_capacity(rows.len() + 1);
    let mut col_idx = Vec::new();
    let mut values = Vec::new();
    row_ptr.push(0);
    for &old_r in rows {
        if old_r >= a.nrows() {
            return Err(SparseError::RowIndexOutOfBounds {
                row: old_r,
                nrows: a.nrows(),
            });
        }
        for k in a.row_ptr()[old_r]..a.row_ptr()[old_r + 1] {
            let mapped = col_map[a.col_idx()[k]];
            if mapped != usize::MAX {
                col_idx.push(mapped);
                values.push(a.values()[k]);
            }
        }
        row_ptr.push(values.len());
    }
    CsrMatrix::from_parts(rows.len(), cols.len(), row_ptr, col_idx, values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compute::sparse::matrix::CooMatrix;

    fn tridiag(n: usize, diag: Scalar, off: Scalar) -> CsrMatrix {
        let mut coo = CooMatrix::new(n, n);
        for i in 0..n {
            coo.push(i, i, diag).unwrap();
            if i + 1 < n {
                coo.push(i, i + 1, off).unwrap();
                coo.push(i + 1, i, off).unwrap();
            }
        }
        coo.to_csr()
    }

    #[test]
    fn add_and_sub_are_inverse() {
        let a = tridiag(4, 2.0, -1.0);
        let b = tridiag(4, 1.0, 0.5);
        let sum = add(&a, &b).unwrap();
        let back = sub(&sum, &b).unwrap();
        for i in 0..4 {
            for j in 0..4 {
                assert!((back.to_dense()[i][j] - a.to_dense()[i][j]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn mat_mul_matches_dense() {
        let a = tridiag(3, 2.0, -1.0);
        let b = tridiag(3, 1.0, 1.0);
        let c = mat_mul(&a, &b).unwrap();
        // Reference dense product.
        let da = a.to_dense();
        let db = b.to_dense();
        for i in 0..3 {
            for j in 0..3 {
                let mut expect = 0.0;
                for k in 0..3 {
                    expect += da[i][k] * db[k][j];
                }
                assert!((c.to_dense()[i][j] - expect).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn mat_mul_shape_mismatch() {
        let a = tridiag(3, 2.0, -1.0);
        let b = tridiag(4, 1.0, 0.0);
        assert!(matches!(
            mat_mul(&a, &b),
            Err(SparseError::ShapeMismatch { .. })
        ));
    }

    #[test]
    fn transpose_matches_dense() {
        let mut coo = CooMatrix::new(2, 3);
        coo.push(0, 0, 1.0).unwrap();
        coo.push(0, 2, 2.0).unwrap();
        coo.push(1, 1, 3.0).unwrap();
        let a = coo.to_csr();
        let t = transpose(&a);
        assert_eq!(t.nrows(), 3);
        assert_eq!(t.ncols(), 2);
        // (Aᵀ)ᵀ == A
        assert_eq!(transpose(&t).to_dense(), a.to_dense());
    }

    #[test]
    fn residual_is_b_minus_ax() {
        let a = tridiag(3, 2.0, -1.0);
        let x = vec![1.0, 1.0, 1.0];
        let b = vec![5.0, 5.0, 5.0];
        let r = residual(&a, &x, &b).unwrap();
        // A·1 = [1, 0, 1] for this tridiagonal (2-1=1, -1+2-1=0, -1+2=1)
        assert_eq!(r, vec![4.0, 5.0, 4.0]);
    }

    #[test]
    fn axpy_and_dot() {
        let mut y = vec![1.0, 2.0, 3.0];
        axpy(2.0, &[1.0, 1.0, 1.0], &mut y).unwrap();
        assert_eq!(y, vec![3.0, 4.0, 5.0]);
        assert_eq!(dot(&[1.0, 2.0], &[3.0, 4.0]).unwrap(), 11.0);
        assert_eq!(norm2(&[3.0, 4.0]), 5.0);
    }

    #[test]
    fn select_submatrix() {
        let mut coo = CooMatrix::new(3, 3);
        for i in 0..3 {
            coo.push(i, i, (i + 1) as Scalar).unwrap();
        }
        let a = coo.to_csr();
        let s = select(&a, &[0, 2], &[0, 2]).unwrap();
        assert_eq!(s.nrows(), 2);
        assert_eq!(s.to_dense(), vec![vec![1.0, 0.0], vec![0.0, 3.0]]);
    }

    #[test]
    fn scale_multiplies_values() {
        let a = tridiag(2, 2.0, -1.0);
        let s = scale(&a, 0.5);
        assert_eq!(s.to_dense(), vec![vec![1.0, -0.5], vec![-0.5, 1.0]]);
    }
}
