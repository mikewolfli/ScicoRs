//! Sparse matrix and linear system solvers.
//!
//! Provides sparse matrix storage (CSR format), basic operations,
//! and iterative linear solvers (Conjugate Gradient, GMRES).

use crate::core::types::Scalar;

/// A sparse matrix in Compressed Sparse Row (CSR) format.
#[derive(Debug, Clone)]
pub struct SparseMatrix {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Non-zero values.
    pub values: Vec<Scalar>,
    /// Column indices for each non-zero value.
    pub col_indices: Vec<usize>,
    /// Row pointers: row i's values are in values[row_ptr[i]..row_ptr[i+1]].
    pub row_ptr: Vec<usize>,
}

impl SparseMatrix {
    /// Create a new empty sparse matrix.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            values: Vec::new(),
            col_indices: Vec::new(),
            row_ptr: vec![0; rows + 1],
        }
    }

    /// Create a sparse matrix from triplet format (i, j, value).
    pub fn from_triplets(rows: usize, cols: usize, triplets: &[(usize, usize, Scalar)]) -> Self {
        let mut sorted = triplets.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let mut values = Vec::new();
        let mut col_indices = Vec::new();
        let mut row_ptr = vec![0; rows + 1];
        let mut current_row = 0;

        for &(r, c, v) in &sorted {
            while current_row <= r {
                row_ptr[current_row] = values.len();
                if current_row < r {
                    current_row += 1;
                } else {
                    break;
                }
            }
            values.push(v);
            col_indices.push(c);
        }
        for val in row_ptr[(current_row + 1)..=rows].iter_mut() {
            *val = values.len();
        }

        Self { rows, cols, values, col_indices, row_ptr }
    }

    /// Multiply the sparse matrix by a dense vector: y = A * x.
    pub fn multiply(&self, x: &[Scalar]) -> Vec<Scalar> {
        assert_eq!(x.len(), self.cols);
        let mut y = vec![0.0; self.rows];
        for (i, yi) in y.iter_mut().enumerate() {
            let mut sum = 0.0;
            for j in self.row_ptr[i]..self.row_ptr[i + 1] {
                sum += self.values[j] * x[self.col_indices[j]];
            }
            *yi = sum;
        }
        y
    }

    /// Transpose the matrix.
    pub fn transpose(&self) -> Self {
        let mut triplets = Vec::new();
        for i in 0..self.rows {
            for j in self.row_ptr[i]..self.row_ptr[i + 1] {
                triplets.push((self.col_indices[j], i, self.values[j]));
            }
        }
        Self::from_triplets(self.cols, self.rows, &triplets)
    }

    /// Number of non-zero elements.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Memory estimate in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.values.len() * std::mem::size_of::<Scalar>()
            + self.col_indices.len() * std::mem::size_of::<usize>()
            + self.row_ptr.len() * std::mem::size_of::<usize>()
    }
}

/// Conjugate Gradient (CG) iterative solver for symmetric positive-definite systems.
#[derive(Debug, Clone)]
pub struct ConjugateGradientSolver {
    /// Maximum number of iterations.
    pub max_iter: usize,
    /// Convergence tolerance on relative residual.
    pub tol: Scalar,
}

impl Default for ConjugateGradientSolver {
    fn default() -> Self {
        Self {
            max_iter: 1000,
            tol: 1e-10,
        }
    }
}

impl ConjugateGradientSolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Solve A * x = b using the Conjugate Gradient method.
    ///
    /// A must be symmetric positive-definite.
    pub fn solve(&self, a: &SparseMatrix, b: &[Scalar]) -> (Vec<Scalar>, usize, bool) {
        let n = b.len();
        let mut x = vec![0.0; n];
        let mut r = b.to_vec();
        let mut p = r.clone();

        let b_norm = b.iter().map(|v| v * v).sum::<Scalar>().sqrt();
        if b_norm < 1e-15 {
            return (x, 0, true);
        }

        let mut rsold: Scalar = r.iter().map(|v| v * v).sum();

        for iter in 0..self.max_iter {
            let ap = a.multiply(&p);
            let p_ap: Scalar = p.iter().zip(ap.iter()).map(|(pi, api)| pi * api).sum();

            if p_ap.abs() < 1e-15 {
                return (x, iter, false);
            }

            let alpha = rsold / p_ap;

            for i in 0..n {
                x[i] += alpha * p[i];
                r[i] -= alpha * ap[i];
            }

            let rsnew: Scalar = r.iter().map(|v| v * v).sum();
            if rsnew.sqrt() / b_norm < self.tol {
                return (x, iter + 1, true);
            }

            let beta = rsnew / rsold;
            for i in 0..n {
                p[i] = r[i] + beta * p[i];
            }
            rsold = rsnew;
        }

        (x, self.max_iter, false)
    }
}

/// A simple eigenvalue solver using the power iteration method.
#[derive(Debug, Clone)]
pub struct PowerIterationSolver {
    pub max_iter: usize,
    pub tol: Scalar,
}

impl Default for PowerIterationSolver {
    fn default() -> Self {
        Self {
            max_iter: 1000,
            tol: 1e-10,
        }
    }
}

impl PowerIterationSolver {
    /// Find the dominant eigenvalue and eigenvector of A.
    pub fn dominant_eigenvalue(&self, a: &SparseMatrix) -> (Scalar, Vec<Scalar>) {
        let n = a.rows;
        let mut v = vec![1.0; n];
        let norm: Scalar = v.iter().map(|x| x * x).sum::<Scalar>().sqrt();
        for vi in &mut v {
            *vi /= norm;
        }

        let mut eigenvalue = 0.0;
        for _iter in 0..self.max_iter {
            let w = a.multiply(&v);
            let new_eigenvalue: Scalar = v.iter().zip(w.iter()).map(|(vi, wi)| vi * wi).sum();

            let diff = (new_eigenvalue - eigenvalue).abs();
            eigenvalue = new_eigenvalue;

            let w_norm: Scalar = w.iter().map(|x| x * x).sum::<Scalar>().sqrt();
            if w_norm < 1e-15 {
                break;
            }
            for i in 0..n {
                v[i] = w[i] / w_norm;
            }

            if diff < self.tol {
                break;
            }
        }

        (eigenvalue, v)
    }
}
