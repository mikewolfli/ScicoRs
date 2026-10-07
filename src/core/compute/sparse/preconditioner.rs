// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Sparse preconditioners: Jacobi, diagonal scaling and ILU(0).
//!
//! A preconditioner applies an approximation `M⁻¹` to accelerate Krylov
//! convergence. Each implementation also reports whether construction succeeded
//! and why it failed (e.g. a zero diagonal entry for Jacobi/ILU), rather than
//! silently degrading to the identity.

use super::matrix::{CsrMatrix, SparseError};
use crate::core::types::Scalar;

/// A preconditioner that applies `z = M⁻¹ · r`.
pub trait Preconditioner {
    /// Apply the preconditioner to the residual vector `r`, writing into `z`.
    fn apply(&self, r: &[Scalar], z: &mut [Scalar]) -> Result<(), SparseError>;

    /// A short name for diagnostics.
    fn name(&self) -> &'static str;

    /// The dimension this preconditioner was built for.
    fn dim(&self) -> usize;
}

/// No preconditioning: `M = I`.
#[derive(Debug, Clone, Copy)]
pub struct IdentityPreconditioner {
    dim: usize,
}

impl IdentityPreconditioner {
    /// Create an identity preconditioner for a system of size `dim`.
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

impl Preconditioner for IdentityPreconditioner {
    fn apply(&self, r: &[Scalar], z: &mut [Scalar]) -> Result<(), SparseError> {
        if r.len() != self.dim || z.len() != self.dim {
            return Err(SparseError::ShapeMismatch {
                detail: format!(
                    "identity preconditioner: expected dim {}, got r={}, z={}",
                    self.dim,
                    r.len(),
                    z.len()
                ),
            });
        }
        z.copy_from_slice(r);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "identity"
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

/// Jacobi (diagonal) preconditioner: `M = diag(A)`.
#[derive(Debug, Clone)]
pub struct JacobiPreconditioner {
    inv_diag: Vec<Scalar>,
}

impl JacobiPreconditioner {
    /// Build a Jacobi preconditioner.
    ///
    /// Fails if the matrix is not square or a diagonal entry is zero, because
    /// inverting a zero diagonal would produce infinity rather than a usable
    /// preconditioner.
    pub fn new(a: &CsrMatrix) -> Result<Self, SparseError> {
        if !a.is_square() {
            return Err(SparseError::Unsupported {
                detail: format!(
                    "Jacobi requires a square matrix, got {}x{}",
                    a.nrows(),
                    a.ncols()
                ),
            });
        }
        let d = a.diagonal();
        let mut inv = Vec::with_capacity(d.len());
        for (i, &v) in d.iter().enumerate() {
            if v == 0.0 {
                return Err(SparseError::Unsupported {
                    detail: format!("Jacobi: zero diagonal entry at row {i}"),
                });
            }
            inv.push(1.0 / v);
        }
        Ok(Self { inv_diag: inv })
    }
}

impl Preconditioner for JacobiPreconditioner {
    fn apply(&self, r: &[Scalar], z: &mut [Scalar]) -> Result<(), SparseError> {
        if r.len() != self.inv_diag.len() || z.len() != self.inv_diag.len() {
            return Err(SparseError::ShapeMismatch {
                detail: format!(
                    "Jacobi: expected dim {}, got r={}, z={}",
                    self.inv_diag.len(),
                    r.len(),
                    z.len()
                ),
            });
        }
        for i in 0..r.len() {
            z[i] = self.inv_diag[i] * r[i];
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "jacobi"
    }

    fn dim(&self) -> usize {
        self.inv_diag.len()
    }
}

/// Diagonal (row/column) scaling of a linear system.
///
/// Storing the scaling factors separately from the matrix ensures the original
/// residual can always be recovered; the scaled residual must never be mistaken
/// for the residual of the original system.
#[derive(Debug, Clone)]
pub struct DiagonalScaling {
    /// Per-row scale factors.
    pub row_scale: Vec<Scalar>,
    /// The scaled matrix `D_r · A`.
    pub scaled: CsrMatrix,
}

impl DiagonalScaling {
    /// Build symmetric diagonal scaling `D_r·A` where `D_r[i] = 1/‖row_i‖∞`.
    ///
    /// Zero rows are left with a scale of 1 to avoid producing NaN; such rows
    /// are structurally singular and are reported by the solver anyway.
    pub fn row_equilibrate(a: &CsrMatrix) -> Self {
        let mut row_scale = vec![1.0 as Scalar; a.nrows()];
        for i in 0..a.nrows() {
            let mut mx = 0.0 as Scalar;
            for k in a.row_ptr()[i]..a.row_ptr()[i + 1] {
                mx = mx.max(a.values()[k].abs());
            }
            if mx > 0.0 {
                row_scale[i] = 1.0 / mx;
            }
        }
        let values: Vec<Scalar> = {
            let mut out = Vec::with_capacity(a.values().len());
            for i in 0..a.nrows() {
                for k in a.row_ptr()[i]..a.row_ptr()[i + 1] {
                    out.push(a.values()[k] * row_scale[i]);
                }
            }
            out
        };
        let scaled = CsrMatrix::from_parts(
            a.nrows(),
            a.ncols(),
            a.row_ptr().to_vec(),
            a.col_idx().to_vec(),
            values,
        )
        .expect("equilibration preserves a valid CSR structure");
        Self { row_scale, scaled }
    }

    /// Recover the solution of the original system from the scaled solution.
    ///
    /// Row scaling scales only the right-hand side (`b' = D_r·b`), not the
    /// unknowns, so the solution vector is unchanged. This method exists so a
    /// future column-scaling variant can override it, and so the residual of the
    /// original system can be reconstructed from a scaled solve.
    pub fn unscale_solution(&self, x: &[Scalar]) -> Vec<Scalar> {
        x.to_vec()
    }

    /// Build a *symmetric* scaling `D·A·D` where `D` is diagonal with
    /// `D[i] = 1/sqrt(‖row_i‖∞ · ‖col_i‖∞)`.
    ///
    /// For a symmetric matrix, `D·A·D` is again symmetric, so this is the
    /// equilibration to use before a symmetric solver such as CG. The solution
    /// transforms as `x = D·x'` and the right-hand side as `b' = D·b`.
    ///
    /// Zero rows/columns are left with a scale of 1 to avoid producing NaN.
    pub fn symmetric_equilibrate(a: &CsrMatrix) -> Self {
        // Row and column infinity norms.
        let mut row_norm = vec![1.0 as Scalar; a.nrows()];
        let mut col_norm = vec![1.0 as Scalar; a.ncols()];
        for i in 0..a.nrows() {
            let mut mx = 0.0 as Scalar;
            for k in a.row_ptr()[i]..a.row_ptr()[i + 1] {
                let av = a.values()[k].abs();
                mx = mx.max(av);
                if av > col_norm[a.col_idx()[k]] {
                    col_norm[a.col_idx()[k]] = av;
                }
            }
            row_norm[i] = mx;
        }
        // D[i] = 1/sqrt(row_norm[i] * col_norm[i]).
        let mut d = vec![1.0 as Scalar; a.nrows().min(a.ncols())];
        for i in 0..d.len() {
            let denom = (row_norm[i] * col_norm[i]).sqrt();
            d[i] = if denom > 0.0 { 1.0 / denom } else { 1.0 };
        }
        // Scaled values: D[i] * A[i][j] * D[j].
        let values: Vec<Scalar> = {
            let mut out = Vec::with_capacity(a.values().len());
            for i in 0..a.nrows() {
                for k in a.row_ptr()[i]..a.row_ptr()[i + 1] {
                    let j = a.col_idx()[k];
                    out.push(d[i] * a.values()[k] * d[j]);
                }
            }
            out
        };
        let scaled = CsrMatrix::from_parts(
            a.nrows(),
            a.ncols(),
            a.row_ptr().to_vec(),
            a.col_idx().to_vec(),
            values,
        )
        .expect("symmetric equilibration preserves a valid CSR structure");
        Self {
            row_scale: d,
            scaled,
        }
    }

    /// Scale a right-hand side by `D` for the symmetric scaling.
    pub fn scale_rhs_symmetric(&self, b: &[Scalar]) -> Vec<Scalar> {
        b.iter()
            .zip(self.row_scale.iter())
            .map(|(&bi, &d)| bi * d)
            .collect()
    }

    /// Map a scaled solution back to the original space: `x = D·x'`.
    pub fn unscale_solution_symmetric(&self, x_scaled: &[Scalar]) -> Vec<Scalar> {
        x_scaled
            .iter()
            .zip(self.row_scale.iter())
            .map(|(&xi, &d)| xi * d)
            .collect()
    }

    /// Scale a right-hand side by `D_r`.
    pub fn scale_rhs(&self, b: &[Scalar]) -> Vec<Scalar> {
        b.iter()
            .zip(self.row_scale.iter())
            .map(|(&bi, &s)| bi * s)
            .collect()
    }
}

/// Incomplete LU with zero fill-in, `A ≈ L·U` keeping the sparsity of `A`.
///
/// Only the strict lower triangle of `L` (unit diagonal implied) and the upper
/// triangle of `U` are stored, both in CSR. Fails on a zero or missing pivot.
#[derive(Debug, Clone)]
pub struct Ilu0Preconditioner {
    n: usize,
    /// Lower-triangular rows, unit diagonal implied, row-sorted by column.
    l_rows: Vec<Vec<(usize, Scalar)>>,
    /// Upper-triangular rows, row-sorted by column, diagonal included.
    u_rows: Vec<Vec<(usize, Scalar)>>,
}

impl Ilu0Preconditioner {
    /// Build an ILU(0) factorization of `a`.
    pub fn new(a: &CsrMatrix) -> Result<Self, SparseError> {
        if !a.is_square() {
            return Err(SparseError::Unsupported {
                detail: format!(
                    "ILU(0) requires a square matrix, got {}x{}",
                    a.nrows(),
                    a.ncols()
                ),
            });
        }
        let n = a.nrows();
        // Working copy of the CSR values (structure stays fixed = zero fill-in).
        let mut vals = a.values().to_vec();
        let row_ptr = a.row_ptr();
        let col_idx = a.col_idx();

        // Map (row, col) → position in the value array for O(1) lookup.
        // Because rows are column sorted we can binary search each row.
        let find = |row: usize, col: usize| -> Option<usize> {
            let start = row_ptr[row];
            let end = row_ptr[row + 1];
            // Binary search on col_idx[start..end].
            let slice = &col_idx[start..end];
            match slice.binary_search(&col) {
                Ok(pos) => Some(start + pos),
                Err(_) => None,
            }
        };

        for i in 0..n {
            for k in row_ptr[i]..row_ptr[i + 1] {
                let kcol = col_idx[k];
                if kcol >= i {
                    break;
                }
                let pivot = find(kcol, kcol).ok_or_else(|| SparseError::Unsupported {
                    detail: format!("ILU(0): missing diagonal row {kcol}"),
                })?;
                if vals[pivot] == 0.0 {
                    return Err(SparseError::Unsupported {
                        detail: format!("ILU(0): zero pivot at row {kcol}"),
                    });
                }
                vals[k] /= vals[pivot];
            }
            for k in row_ptr[i]..row_ptr[i + 1] {
                let kcol = col_idx[k];
                if kcol < i {
                    let krow_start = row_ptr[kcol];
                    let krow_end = row_ptr[kcol + 1];
                    let lik = vals[k];
                    let mut kk = krow_start;
                    let mut ki = k;
                    while kk < krow_end {
                        let kcol2 = col_idx[kk];
                        if kcol2 < kcol {
                            kk += 1;
                            continue;
                        }
                        if kcol2 == kcol {
                            kk += 1;
                            continue;
                        }
                        // Advance ki to the same column (or past it).
                        while ki < row_ptr[i + 1] && col_idx[ki] < kcol2 {
                            ki += 1;
                        }
                        if ki < row_ptr[i + 1] && col_idx[ki] == kcol2 {
                            vals[ki] -= lik * vals[kk];
                        }
                        // If column kcol2 is not in row i, ILU(0) simply drops it.
                        kk += 1;
                    }
                }
            }
        }

        let mut l_rows = Vec::with_capacity(n);
        let mut u_rows = Vec::with_capacity(n);
        for i in 0..n {
            let mut lrow = Vec::new();
            let mut urow = Vec::new();
            for k in row_ptr[i]..row_ptr[i + 1] {
                let c = col_idx[k];
                if c < i {
                    lrow.push((c, vals[k]));
                } else {
                    urow.push((c, vals[k]));
                }
            }
            l_rows.push(lrow);
            u_rows.push(urow);
        }
        Ok(Self { n, l_rows, u_rows })
    }

    /// Solve `M · z = r` where `M = L·U`, via forward then backward substitution.
    fn solve(&self, r: &[Scalar], z: &mut [Scalar]) {
        let n = self.n;
        // Forward solve L·y = r (unit diagonal).
        for i in 0..n {
            let mut s = r[i];
            for &(c, v) in &self.l_rows[i] {
                s -= v * z[c];
            }
            z[i] = s;
        }
        // Backward solve U·z = y.
        for i in (0..n).rev() {
            let mut s = z[i];
            let mut diag = 1.0;
            for &(c, v) in &self.u_rows[i] {
                if c == i {
                    diag = v;
                } else {
                    s -= v * z[c];
                }
            }
            z[i] = if diag != 0.0 { s / diag } else { 0.0 };
        }
    }
}

impl Preconditioner for Ilu0Preconditioner {
    fn apply(&self, r: &[Scalar], z: &mut [Scalar]) -> Result<(), SparseError> {
        if r.len() != self.n || z.len() != self.n {
            return Err(SparseError::ShapeMismatch {
                detail: format!(
                    "ILU(0): expected dim {}, got r={}, z={}",
                    self.n,
                    r.len(),
                    z.len()
                ),
            });
        }
        self.solve(r, z);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "ilu0"
    }

    fn dim(&self) -> usize {
        self.n
    }
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
    fn jacobi_inverts_diagonal() {
        let a = tridiag(4, 2.0, -1.0);
        let p = JacobiPreconditioner::new(&a).unwrap();
        let mut z = vec![0.0; 4];
        p.apply(&[2.0, 4.0, 6.0, 8.0], &mut z).unwrap();
        assert_eq!(z, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn jacobi_rejects_zero_diagonal() {
        let mut coo = CooMatrix::new(2, 2);
        coo.push(0, 0, 0.0).unwrap();
        coo.push(1, 1, 1.0).unwrap();
        assert!(JacobiPreconditioner::new(&coo.to_csr()).is_err());
    }

    #[test]
    fn diagonal_scaling_normalizes_rows() {
        let mut coo = CooMatrix::new(2, 2);
        coo.push(0, 0, 4.0).unwrap();
        coo.push(0, 1, 2.0).unwrap();
        coo.push(1, 1, 5.0).unwrap();
        let a = coo.to_csr();
        let s = DiagonalScaling::row_equilibrate(&a);
        // Row 0 had max 4 → scaled to [1, 0.5].
        let d = s.scaled.to_dense();
        assert!((d[0][0] - 1.0).abs() < 1e-12);
        assert!((d[0][1] - 0.5).abs() < 1e-12);
        // RHS scaling inverts in the solution only through b.
        let b = s.scale_rhs(&[4.0, 5.0]);
        assert!((b[0] - 1.0).abs() < 1e-12);
        // For row scaling the unknowns are unchanged, so recovery is the identity.
        let x = s.unscale_solution(&[1.5, -2.5]);
        assert_eq!(x, vec![1.5, -2.5]);
    }

    #[test]
    fn symmetric_equilibration_keeps_symmetry() {
        // A symmetric matrix must stay symmetric under D·A·D scaling.
        let a = tridiag(4, 4.0, -1.0);
        assert!(a.is_symmetric(1e-12));
        let s = DiagonalScaling::symmetric_equilibrate(&a);
        assert!(
            s.scaled.is_symmetric(1e-12),
            "symmetric equilibration broke symmetry"
        );
        // Diagonal entries become 1/sqrt(‖row‖∞·‖col‖∞) * a_ii * same = a_ii/‖row‖∞·‖col‖∞...
        // For a uniformly nonzero row, |scaled[i][i]| <= 1.
        for i in 0..4 {
            assert!(s.scaled.to_dense()[i][i].abs() <= 1.0 + 1e-12);
        }
        // Recovering the solution multiplies back by D.
        let xs = vec![2.0, 3.0, 4.0, 5.0];
        let recovered = s.unscale_solution_symmetric(&xs);
        for i in 0..4 {
            assert!((recovered[i] - xs[i] * s.row_scale[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn ilu0_factorization_solves_tridiagonal() {
        let a = tridiag(5, 4.0, -1.0);
        let ilu = Ilu0Preconditioner::new(&a).unwrap();
        // Apply to the exact solution of A·1 = [3,2,2,2,3] should be close since
        // ILU(0) is exact for tridiagonal matrices.
        let r = vec![3.0, 2.0, 2.0, 2.0, 3.0];
        let mut z = vec![0.0; 5];
        ilu.apply(&r, &mut z).unwrap();
        for &zi in &z {
            assert!((zi - 1.0).abs() < 1e-10, "got {zi}");
        }
    }

    #[test]
    fn ilu0_rejects_zero_pivot() {
        let mut coo = CooMatrix::new(2, 2);
        coo.push(0, 1, 1.0).unwrap();
        coo.push(1, 0, 1.0).unwrap();
        assert!(Ilu0Preconditioner::new(&coo.to_csr()).is_err());
    }

    #[test]
    fn identity_preconditioner_copies() {
        let p = IdentityPreconditioner::new(3);
        let mut z = vec![0.0; 3];
        p.apply(&[1.0, 2.0, 3.0], &mut z).unwrap();
        assert_eq!(z, vec![1.0, 2.0, 3.0]);
        assert_eq!(p.name(), "identity");
    }
}
