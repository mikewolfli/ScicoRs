// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Sparse matrix storage: COO, CSR and CSC formats with validated conversion.
//!
//! Three coordinate formats are provided so that assembly, row-oriented and
//! column-oriented algorithms can each use the layout they need without
//! re-parsing a dense matrix:
//!
//! * [`CooMatrix`] — triplets `(row, col, value)`, the natural output of finite
//!   element / finite volume assembly.
//! * [`CsrMatrix`] — compressed sparse row, the storage used by SpMV, Krylov
//!   solvers and row-based preconditioners.
//! * [`CscMatrix`] — compressed sparse column, convenient for column operations
//!   and transpose access.
//!
//! All constructors validate indices and numeric values and return a
//! [`SparseError`] instead of panicking or silently dropping entries. Explicit
//! zeros are preserved by assembly (a caller may legitimately need a stored
//! zero to keep a fixed sparsity pattern) and can be removed on demand with
//! [`CsrMatrix::eliminate_zeros`].

use crate::core::types::Scalar;
use std::collections::BTreeMap;

/// Error type for sparse matrix construction and operations.
///
/// Each variant carries the offending index/value so failures are locatable
/// rather than a bare "invalid input".
#[derive(Debug, Clone, PartialEq)]
pub enum SparseError {
    /// A row index was out of bounds.
    RowIndexOutOfBounds {
        /// The offending row index.
        row: usize,
        /// The number of rows in the matrix.
        nrows: usize,
    },
    /// A column index was out of bounds.
    ColIndexOutOfBounds {
        /// The offending column index.
        col: usize,
        /// The number of columns in the matrix.
        ncols: usize,
    },
    /// A value was not finite (NaN or infinity).
    NonFiniteValue {
        /// The offending row index.
        row: usize,
        /// The offending column index.
        col: usize,
        /// The offending value.
        value: Scalar,
    },
    /// The provided arrays had inconsistent lengths.
    ShapeMismatch {
        /// Description of what mismatched.
        detail: String,
    },
    /// A CSC/CSR structure was internally inconsistent.
    InvalidStructure {
        /// Description of the inconsistency.
        detail: String,
    },
    /// The requested operation is not defined for this matrix (e.g. a symmetric
    /// solver on a non-square system).
    Unsupported {
        /// Description of the unsupported condition.
        detail: String,
    },
}

impl std::fmt::Display for SparseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RowIndexOutOfBounds { row, nrows } => {
                write!(f, "row index {row} out of bounds (nrows={nrows})")
            }
            Self::ColIndexOutOfBounds { col, ncols } => {
                write!(f, "col index {col} out of bounds (ncols={ncols})")
            }
            Self::NonFiniteValue { row, col, value } => write!(
                f,
                "non-finite value {value} at ({row}, {col}); sparse matrices require finite entries"
            ),
            Self::ShapeMismatch { detail } => write!(f, "shape mismatch: {detail}"),
            Self::InvalidStructure { detail } => write!(f, "invalid sparse structure: {detail}"),
            Self::Unsupported { detail } => write!(f, "unsupported operation: {detail}"),
        }
    }
}

impl std::error::Error for SparseError {}

/// Coordinate-format (triplet) sparse matrix.
///
/// Duplicate `(row, col)` entries are **summed** on conversion to CSR/CSC, which
/// is the standard symmetric-assembly convention. Use
/// [`CooMatrix::push`] to append entries in any order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CooMatrix {
    nrows: usize,
    ncols: usize,
    rows: Vec<usize>,
    cols: Vec<usize>,
    values: Vec<Scalar>,
}

impl CooMatrix {
    /// Create an empty COO matrix with the given logical dimensions.
    pub fn new(nrows: usize, ncols: usize) -> Self {
        Self {
            nrows,
            ncols,
            rows: Vec::new(),
            cols: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Create a COO matrix from raw triplet arrays, validating every index and
    /// value.
    pub fn from_triplets(
        nrows: usize,
        ncols: usize,
        rows: Vec<usize>,
        cols: Vec<usize>,
        values: Vec<Scalar>,
    ) -> Result<Self, SparseError> {
        if rows.len() != cols.len() || rows.len() != values.len() {
            return Err(SparseError::ShapeMismatch {
                detail: format!(
                    "triplet arrays have lengths rows={}, cols={}, values={}",
                    rows.len(),
                    cols.len(),
                    values.len()
                ),
            });
        }
        let m = Self {
            nrows,
            ncols,
            rows,
            cols,
            values,
        };
        m.validate()?;
        Ok(m)
    }

    /// Append a single `(row, col, value)` entry.
    pub fn push(&mut self, row: usize, col: usize, value: Scalar) -> Result<(), SparseError> {
        self.check_index(row, col)?;
        if !value.is_finite() {
            return Err(SparseError::NonFiniteValue { row, col, value });
        }
        self.rows.push(row);
        self.cols.push(col);
        self.values.push(value);
        Ok(())
    }

    /// Number of stored (non-zero-structure) entries, including explicit zeros.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Logical row count.
    pub fn nrows(&self) -> usize {
        self.nrows
    }

    /// Logical column count.
    pub fn ncols(&self) -> usize {
        self.ncols
    }

    /// Whether the matrix has no stored entries.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Read-only access to the triplet arrays.
    pub fn triplets(&self) -> (&[usize], &[usize], &[Scalar]) {
        (&self.rows, &self.cols, &self.values)
    }

    fn check_index(&self, row: usize, col: usize) -> Result<(), SparseError> {
        if row >= self.nrows {
            return Err(SparseError::RowIndexOutOfBounds {
                row,
                nrows: self.nrows,
            });
        }
        if col >= self.ncols {
            return Err(SparseError::ColIndexOutOfBounds {
                col,
                ncols: self.ncols,
            });
        }
        Ok(())
    }

    /// Validate all stored entries against the logical dimensions and finiteness.
    pub fn validate(&self) -> Result<(), SparseError> {
        for i in 0..self.values.len() {
            self.check_index(self.rows[i], self.cols[i])?;
            if !self.values[i].is_finite() {
                return Err(SparseError::NonFiniteValue {
                    row: self.rows[i],
                    col: self.cols[i],
                    value: self.values[i],
                });
            }
        }
        Ok(())
    }

    /// Convert to compressed sparse row, summing duplicate coordinates.
    pub fn to_csr(&self) -> CsrMatrix {
        // Bucket entries per row, accumulating duplicates; BTreeMap keeps the
        // column index sorted within each row, so CSR column indices are ordered
        // without a separate sort pass.
        let mut per_row: Vec<BTreeMap<usize, Scalar>> = vec![BTreeMap::new(); self.nrows];
        for i in 0..self.values.len() {
            let r = self.rows[i];
            let c = self.cols[i];
            *per_row[r].entry(c).or_insert(0.0) += self.values[i];
        }
        let mut row_ptr = Vec::with_capacity(self.nrows + 1);
        let mut col_idx = Vec::with_capacity(self.values.len());
        let mut vals = Vec::with_capacity(self.values.len());
        row_ptr.push(0);
        for row in &per_row {
            for (&c, &v) in row {
                col_idx.push(c);
                vals.push(v);
            }
            row_ptr.push(vals.len());
        }
        CsrMatrix {
            nrows: self.nrows,
            ncols: self.ncols,
            row_ptr,
            col_idx,
            values: vals,
        }
    }

    /// Convert to compressed sparse column, summing duplicate coordinates.
    pub fn to_csc(&self) -> CscMatrix {
        let mut per_col: Vec<BTreeMap<usize, Scalar>> = vec![BTreeMap::new(); self.ncols];
        for i in 0..self.values.len() {
            let r = self.rows[i];
            let c = self.cols[i];
            *per_col[c].entry(r).or_insert(0.0) += self.values[i];
        }
        let mut col_ptr = Vec::with_capacity(self.ncols + 1);
        let mut row_idx = Vec::with_capacity(self.values.len());
        let mut vals = Vec::with_capacity(self.values.len());
        col_ptr.push(0);
        for col in &per_col {
            for (&r, &v) in col {
                row_idx.push(r);
                vals.push(v);
            }
            col_ptr.push(vals.len());
        }
        CscMatrix {
            nrows: self.nrows,
            ncols: self.ncols,
            col_ptr,
            row_idx,
            values: vals,
        }
    }

    /// Materialize a dense row-major matrix. Intended for small matrices and
    /// tests; large systems should stay sparse.
    pub fn to_dense(&self) -> Vec<Vec<Scalar>> {
        let mut dense = vec![vec![0.0; self.ncols]; self.nrows];
        for i in 0..self.values.len() {
            dense[self.rows[i]][self.cols[i]] += self.values[i];
        }
        dense
    }
}

/// Compressed sparse row matrix.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CsrMatrix {
    nrows: usize,
    ncols: usize,
    row_ptr: Vec<usize>,
    col_idx: Vec<usize>,
    values: Vec<Scalar>,
}

impl CsrMatrix {
    /// Build a CSR matrix from its three component arrays, validating structure.
    pub fn from_parts(
        nrows: usize,
        ncols: usize,
        row_ptr: Vec<usize>,
        col_idx: Vec<usize>,
        values: Vec<Scalar>,
    ) -> Result<Self, SparseError> {
        if row_ptr.len() != nrows + 1 {
            return Err(SparseError::InvalidStructure {
                detail: format!(
                    "row_ptr length {} != nrows+1 = {}",
                    row_ptr.len(),
                    nrows + 1
                ),
            });
        }
        if col_idx.len() != values.len() {
            return Err(SparseError::ShapeMismatch {
                detail: format!(
                    "col_idx length {} != values length {}",
                    col_idx.len(),
                    values.len()
                ),
            });
        }
        if row_ptr[0] != 0 {
            return Err(SparseError::InvalidStructure {
                detail: format!("row_ptr[0] = {} (must be 0)", row_ptr[0]),
            });
        }
        if *row_ptr.last().unwrap() != values.len() {
            return Err(SparseError::InvalidStructure {
                detail: format!(
                    "row_ptr[last] = {} != nnz = {}",
                    row_ptr.last().unwrap(),
                    values.len()
                ),
            });
        }
        for i in 0..nrows {
            if row_ptr[i] > row_ptr[i + 1] {
                return Err(SparseError::InvalidStructure {
                    detail: format!("row_ptr not non-decreasing at row {i}"),
                });
            }
        }
        for &c in &col_idx {
            if c >= ncols {
                return Err(SparseError::ColIndexOutOfBounds { col: c, ncols });
            }
        }
        for (k, &v) in values.iter().enumerate() {
            if !v.is_finite() {
                return Err(SparseError::NonFiniteValue {
                    row: 0,
                    col: k,
                    value: v,
                });
            }
        }
        Ok(Self {
            nrows,
            ncols,
            row_ptr,
            col_idx,
            values,
        })
    }

    /// Build an all-empty CSR matrix with `nrows × ncols` logical shape.
    pub fn zeros(nrows: usize, ncols: usize) -> Self {
        Self {
            nrows,
            ncols,
            row_ptr: vec![0; nrows + 1],
            col_idx: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Build the `n × n` identity matrix in CSR form (one entry per row).
    pub fn identity(n: usize) -> Self {
        let row_ptr: Vec<usize> = (0..=n).collect();
        let col_idx: Vec<usize> = (0..n).collect();
        let values = vec![1.0; n];
        Self {
            nrows: n,
            ncols: n,
            row_ptr,
            col_idx,
            values,
        }
    }

    /// Number of stored entries.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Logical row count.
    pub fn nrows(&self) -> usize {
        self.nrows
    }

    /// Logical column count.
    pub fn ncols(&self) -> usize {
        self.ncols
    }

    /// Whether the matrix is square.
    pub fn is_square(&self) -> bool {
        self.nrows == self.ncols
    }

    /// Whether the matrix has no stored entries.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Access the raw CSR arrays `(row_ptr, col_idx, values)`.
    pub fn parts(&self) -> (&[usize], &[usize], &[Scalar]) {
        (&self.row_ptr, &self.col_idx, &self.values)
    }

    /// Iterate over the `(col, value)` pairs of row `i`.
    pub fn row(&self, i: usize) -> impl Iterator<Item = (usize, Scalar)> + '_ {
        let start = self.row_ptr[i];
        let end = self.row_ptr[i + 1];
        (start..end).map(move |k| (self.col_idx[k], self.values[k]))
    }

    /// Structural row pointer slice.
    pub fn row_ptr(&self) -> &[usize] {
        &self.row_ptr
    }

    /// Structural column index slice.
    pub fn col_idx(&self) -> &[usize] {
        &self.col_idx
    }

    /// Numerical value slice (parallel to [`Self::col_idx`]).
    pub fn values(&self) -> &[Scalar] {
        &self.values
    }

    /// Explicitly drop stored zeros, keeping the sparsity pattern otherwise
    /// intact. Returns the number of entries removed.
    pub fn eliminate_zeros(&mut self) -> usize {
        let mut new_col = Vec::with_capacity(self.col_idx.len());
        let mut new_val = Vec::with_capacity(self.values.len());
        let mut new_ptr = Vec::with_capacity(self.nrows + 1);
        new_ptr.push(0);
        let mut removed = 0;
        for i in 0..self.nrows {
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                if self.values[k] != 0.0 {
                    new_col.push(self.col_idx[k]);
                    new_val.push(self.values[k]);
                } else {
                    removed += 1;
                }
            }
            new_ptr.push(new_val.len());
        }
        self.col_idx = new_col;
        self.values = new_val;
        self.row_ptr = new_ptr;
        removed
    }

    /// Compute `y = A · x` (SpMV) without allocating a dense matrix.
    pub fn matvec(&self, x: &[Scalar]) -> Result<Vec<Scalar>, SparseError> {
        if x.len() != self.ncols {
            return Err(SparseError::ShapeMismatch {
                detail: format!("matvec: x length {} != ncols {}", x.len(), self.ncols),
            });
        }
        let mut y = vec![0.0; self.nrows];
        for i in 0..self.nrows {
            let mut acc = 0.0;
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                acc += self.values[k] * x[self.col_idx[k]];
            }
            y[i] = acc;
        }
        Ok(y)
    }

    /// Compute `y = Aᵀ · x` (transpose SpMV).
    pub fn matvec_transpose(&self, x: &[Scalar]) -> Result<Vec<Scalar>, SparseError> {
        if x.len() != self.nrows {
            return Err(SparseError::ShapeMismatch {
                detail: format!(
                    "matvec_transpose: x length {} != nrows {}",
                    x.len(),
                    self.nrows
                ),
            });
        }
        let mut y = vec![0.0; self.ncols];
        for i in 0..self.nrows {
            let xi = x[i];
            if xi == 0.0 {
                continue;
            }
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                y[self.col_idx[k]] += self.values[k] * xi;
            }
        }
        Ok(y)
    }

    /// Return the main-diagonal entries as a dense vector of length `min(nrows,
    /// ncols)`. Missing diagonal entries are zero.
    pub fn diagonal(&self) -> Vec<Scalar> {
        let n = self.nrows.min(self.ncols);
        let mut d = vec![0.0; n];
        for i in 0..n {
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                if self.col_idx[k] == i {
                    d[i] = self.values[k];
                    break;
                }
            }
        }
        d
    }

    /// Materialize a dense row-major matrix. For diagnostics and tests only.
    pub fn to_dense(&self) -> Vec<Vec<Scalar>> {
        let mut dense = vec![vec![0.0; self.ncols]; self.nrows];
        for i in 0..self.nrows {
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                dense[i][self.col_idx[k]] = self.values[k];
            }
        }
        dense
    }

    /// Build a CSR matrix from a dense row-major matrix, dropping (structural)
    /// zeros. A dense source is assumed rectangular; ragged input is rejected.
    pub fn from_dense(dense: &[Vec<Scalar>]) -> Result<Self, SparseError> {
        let nrows = dense.len();
        let ncols = if nrows == 0 { 0 } else { dense[0].len() };
        for (i, row) in dense.iter().enumerate() {
            if row.len() != ncols {
                return Err(SparseError::ShapeMismatch {
                    detail: format!("dense row {i} length {} != ncols {}", row.len(), ncols),
                });
            }
        }
        let mut row_ptr = Vec::with_capacity(nrows + 1);
        let mut col_idx = Vec::new();
        let mut values = Vec::new();
        row_ptr.push(0);
        for row in dense.iter() {
            for (c, &v) in row.iter().enumerate() {
                if !v.is_finite() {
                    return Err(SparseError::NonFiniteValue {
                        row: row_ptr.len() - 1,
                        col: c,
                        value: v,
                    });
                }
                if v != 0.0 {
                    col_idx.push(c);
                    values.push(v);
                }
            }
            row_ptr.push(values.len());
        }
        Ok(Self {
            nrows,
            ncols,
            row_ptr,
            col_idx,
            values,
        })
    }

    /// Whether the stored values are symmetric as a matrix (`A = Aᵀ`).
    ///
    /// Structural asymmetry counts as asymmetry even if the missing entry is
    /// zero-valued; call [`Self::eliminate_zeros`] first to test the logical
    /// matrix instead of the stored pattern.
    pub fn is_symmetric(&self, tol: Scalar) -> bool {
        if !self.is_square() {
            return false;
        }
        let n = self.nrows;
        // Index the transpose for O(nnz log nnz) comparison.
        let mut trans: Vec<Vec<(usize, Scalar)>> = vec![Vec::new(); n];
        for i in 0..n {
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                trans[self.col_idx[k]].push((i, self.values[k]));
            }
        }
        for i in 0..n {
            let mut a: Vec<(usize, Scalar)> = self.row(i).collect();
            let mut b: Vec<(usize, Scalar)> = trans[i].clone();
            a.sort_by_key(|e| e.0);
            b.sort_by_key(|e| e.0);
            if a.len() != b.len() {
                return false;
            }
            for (l, r) in a.iter().zip(b.iter()) {
                if l.0 != r.0 || (l.1 - r.1).abs() > tol {
                    return false;
                }
            }
        }
        true
    }
}

/// Compressed sparse column matrix.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CscMatrix {
    nrows: usize,
    ncols: usize,
    col_ptr: Vec<usize>,
    row_idx: Vec<usize>,
    values: Vec<Scalar>,
}

impl CscMatrix {
    /// Build a CSC matrix from its three component arrays, validating structure.
    pub fn from_parts(
        nrows: usize,
        ncols: usize,
        col_ptr: Vec<usize>,
        row_idx: Vec<usize>,
        values: Vec<Scalar>,
    ) -> Result<Self, SparseError> {
        if col_ptr.len() != ncols + 1 {
            return Err(SparseError::InvalidStructure {
                detail: format!(
                    "col_ptr length {} != ncols+1 = {}",
                    col_ptr.len(),
                    ncols + 1
                ),
            });
        }
        if row_idx.len() != values.len() {
            return Err(SparseError::ShapeMismatch {
                detail: format!(
                    "row_idx length {} != values length {}",
                    row_idx.len(),
                    values.len()
                ),
            });
        }
        for &r in &row_idx {
            if r >= nrows {
                return Err(SparseError::RowIndexOutOfBounds { row: r, nrows });
            }
        }
        Ok(Self {
            nrows,
            ncols,
            col_ptr,
            row_idx,
            values,
        })
    }

    /// Number of stored entries.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Logical row count.
    pub fn nrows(&self) -> usize {
        self.nrows
    }

    /// Logical column count.
    pub fn ncols(&self) -> usize {
        self.ncols
    }

    /// Access the raw CSC arrays `(col_ptr, row_idx, values)`.
    pub fn parts(&self) -> (&[usize], &[usize], &[Scalar]) {
        (&self.col_ptr, &self.row_idx, &self.values)
    }

    /// Convert to CSR by transposing the index space.
    pub fn to_csr(&self) -> CsrMatrix {
        let mut row_ptr = vec![0usize; self.nrows + 1];
        for &r in &self.row_idx {
            row_ptr[r + 1] += 1;
        }
        for i in 0..self.nrows {
            row_ptr[i + 1] += row_ptr[i];
        }
        let mut next = row_ptr.clone();
        let mut col_idx = vec![0usize; self.values.len()];
        let mut values = vec![0.0; self.values.len()];
        for c in 0..self.ncols {
            for k in self.col_ptr[c]..self.col_ptr[c + 1] {
                let r = self.row_idx[k];
                let pos = next[r];
                col_idx[pos] = c;
                values[pos] = self.values[k];
                next[r] += 1;
            }
        }
        CsrMatrix {
            nrows: self.nrows,
            ncols: self.ncols,
            row_ptr,
            col_idx,
            values,
        }
    }

    /// Materialize a dense row-major matrix.
    pub fn to_dense(&self) -> Vec<Vec<Scalar>> {
        let mut dense = vec![vec![0.0; self.ncols]; self.nrows];
        for c in 0..self.ncols {
            for k in self.col_ptr[c]..self.col_ptr[c + 1] {
                dense[self.row_idx[k]][c] = self.values[k];
            }
        }
        dense
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coo_rejects_out_of_bounds() {
        let mut m = CooMatrix::new(2, 2);
        assert!(matches!(
            m.push(2, 0, 1.0),
            Err(SparseError::RowIndexOutOfBounds { .. })
        ));
        assert!(matches!(
            m.push(0, 2, 1.0),
            Err(SparseError::ColIndexOutOfBounds { .. })
        ));
    }

    #[test]
    fn coo_rejects_non_finite() {
        let mut m = CooMatrix::new(2, 2);
        assert!(matches!(
            m.push(0, 0, Scalar::NAN),
            Err(SparseError::NonFiniteValue { .. })
        ));
        assert!(matches!(
            m.push(0, 0, Scalar::INFINITY),
            Err(SparseError::NonFiniteValue { .. })
        ));
    }

    #[test]
    fn coo_sums_duplicates_on_conversion() {
        let mut m = CooMatrix::new(2, 2);
        m.push(0, 0, 1.0).unwrap();
        m.push(0, 0, 2.5).unwrap();
        m.push(1, 1, -1.0).unwrap();
        let csr = m.to_csr();
        assert_eq!(csr.nnz(), 2);
        assert_eq!(csr.to_dense(), vec![vec![3.5, 0.0], vec![0.0, -1.0]]);
    }

    #[test]
    fn csr_matvec_matches_dense() {
        // 3x3 tridiagonal.
        let mut coo = CooMatrix::new(3, 3);
        for i in 0..3 {
            coo.push(i, i, 2.0).unwrap();
        }
        coo.push(0, 1, -1.0).unwrap();
        coo.push(1, 2, -1.0).unwrap();
        let csr = coo.to_csr();
        let x = vec![1.0, 2.0, 3.0];
        let y = csr.matvec(&x).unwrap();
        // row0: 2·1 − 1·2 = 0; row1: 2·2 − 1·3 = 1; row2: 2·3 = 6.
        assert_eq!(y, vec![0.0, 1.0, 6.0]);
    }

    #[test]
    fn csr_transpose_matvec() {
        let mut coo = CooMatrix::new(2, 3);
        coo.push(0, 0, 1.0).unwrap();
        coo.push(0, 2, 2.0).unwrap();
        coo.push(1, 1, 3.0).unwrap();
        let csr = coo.to_csr();
        // Aᵀ is 3x2; Aᵀ · [1, 1] = [1, 3, 2]
        let y = csr.matvec_transpose(&[1.0, 1.0]).unwrap();
        assert_eq!(y, vec![1.0, 3.0, 2.0]);
    }

    #[test]
    fn csr_csc_roundtrip() {
        let mut coo = CooMatrix::new(3, 4);
        coo.push(0, 1, 5.0).unwrap();
        coo.push(2, 3, -2.0).unwrap();
        coo.push(1, 0, 7.0).unwrap();
        let csr = coo.to_csr();
        let csc = coo.to_csc();
        assert_eq!(csc.to_csr().to_dense(), csr.to_dense());
        assert_eq!(
            csr.to_dense(),
            vec![
                vec![0.0, 5.0, 0.0, 0.0],
                vec![7.0, 0.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0, -2.0],
            ]
        );
    }

    #[test]
    fn empty_and_zero_row_matrices() {
        let empty = CooMatrix::new(3, 3).to_csr();
        assert_eq!(empty.nnz(), 0);
        assert_eq!(empty.row(1).count(), 0);
        let y = empty.matvec(&[1.0, 2.0, 3.0]).unwrap();
        assert_eq!(y, vec![0.0, 0.0, 0.0]);

        // Matrix with one populated row among empty rows.
        let mut coo = CooMatrix::new(2, 2);
        coo.push(1, 1, 4.0).unwrap();
        let csr = coo.to_csr();
        assert_eq!(csr.nnz(), 1);
        assert_eq!(csr.matvec(&[1.0, 1.0]).unwrap(), vec![0.0, 4.0]);
    }

    #[test]
    fn eliminate_zeros_rebuilds_structure() {
        let mut coo = CooMatrix::new(2, 2);
        coo.push(0, 0, 1.0).unwrap();
        coo.push(0, 1, 0.0).unwrap();
        coo.push(1, 1, 2.0).unwrap();
        let mut csr = coo.to_csr();
        assert_eq!(csr.nnz(), 3);
        let removed = csr.eliminate_zeros();
        assert_eq!(removed, 1);
        assert_eq!(csr.nnz(), 2);
        assert_eq!(csr.to_dense(), vec![vec![1.0, 0.0], vec![0.0, 2.0]]);
    }

    #[test]
    fn from_dense_rejects_ragged() {
        let dense = vec![vec![1.0, 2.0], vec![3.0]];
        assert!(matches!(
            CsrMatrix::from_dense(&dense),
            Err(SparseError::ShapeMismatch { .. })
        ));
    }

    #[test]
    fn symmetry_detection() {
        let mut coo = CooMatrix::new(2, 2);
        coo.push(0, 0, 1.0).unwrap();
        coo.push(0, 1, 2.0).unwrap();
        coo.push(1, 0, 2.0).unwrap();
        coo.push(1, 1, 3.0).unwrap();
        let csr = coo.to_csr();
        assert!(csr.is_symmetric(1e-12));

        let mut asym = CooMatrix::new(2, 2);
        asym.push(0, 1, 2.0).unwrap();
        asym.push(1, 0, 2.5).unwrap();
        assert!(!asym.to_csr().is_symmetric(1e-12));
    }

    #[test]
    fn identity_and_diagonal() {
        let id = CsrMatrix::identity(3);
        assert_eq!(id.nnz(), 3);
        assert_eq!(id.diagonal(), vec![1.0, 1.0, 1.0]);
        assert!(id.is_symmetric(1e-12));
    }

    #[test]
    fn csc_from_parts_validates() {
        // row_idx referencing a nonexistent row is rejected.
        let res = CscMatrix::from_parts(2, 1, vec![0, 1], vec![5], vec![1.0]);
        assert!(matches!(res, Err(SparseError::RowIndexOutOfBounds { .. })));
    }

    #[test]
    fn csr_from_parts_validates_structure() {
        // row_ptr[last] must equal nnz.
        let res = CsrMatrix::from_parts(1, 1, vec![0, 5], vec![0], vec![1.0]);
        assert!(matches!(res, Err(SparseError::InvalidStructure { .. })));
        // col index out of range.
        let res = CsrMatrix::from_parts(1, 1, vec![0, 1], vec![3], vec![1.0]);
        assert!(matches!(res, Err(SparseError::ColIndexOutOfBounds { .. })));
    }
}
