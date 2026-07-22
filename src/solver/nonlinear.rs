//! Nonlinear equation solvers.
//!
//! Provides Newton-Raphson and related methods for solving
//! systems of nonlinear equations F(x) = 0.

use crate::core::types::Scalar;

/// The nonlinear function type: F(x) -> residual.
pub type NonlinearFunction = fn(&[Scalar], &mut [Scalar]);

/// The Jacobian function type: J(x) -> Jacobian matrix (row-major).
pub type JacobianFunction = fn(&[Scalar], &mut [Scalar]);

/// Configuration for nonlinear solvers.
#[derive(Debug, Clone)]
pub struct NonlinearSolverConfig {
    /// Convergence tolerance on |F(x)|.
    pub tol: Scalar,
    /// Tolerance on step size |dx|.
    pub step_tol: Scalar,
    /// Maximum number of iterations.
    pub max_iter: usize,
    /// Damping factor for Newton step (0.0 < damping <= 1.0).
    pub damping: Scalar,
}

impl Default for NonlinearSolverConfig {
    fn default() -> Self {
        Self {
            tol: 1e-10,
            step_tol: 1e-12,
            max_iter: 100,
            damping: 1.0,
        }
    }
}

/// Result of a nonlinear solve.
#[derive(Debug, Clone)]
pub struct NonlinearSolveResult {
    /// Solution vector.
    pub x: Vec<Scalar>,
    /// Number of iterations used.
    pub iterations: usize,
    /// Final residual norm.
    pub residual_norm: Scalar,
    /// Whether the solver converged.
    pub converged: bool,
}

/// Newton-Raphson solver for systems of nonlinear equations.
#[derive(Debug, Clone)]
pub struct NewtonRaphsonSolver {
    pub config: NonlinearSolverConfig,
    /// Number of unknowns.
    n: usize,
}

impl NewtonRaphsonSolver {
    pub fn new(n: usize) -> Self {
        Self {
            config: NonlinearSolverConfig::default(),
            n,
        }
    }

    pub fn with_config(n: usize, config: NonlinearSolverConfig) -> Self {
        Self { config, n }
    }

    /// Solve F(x) = 0 using the Newton-Raphson method.
    ///
    /// When `jacobian` is None, uses a finite-difference approximation.
    pub fn solve(
        &self,
        f: NonlinearFunction,
        jacobian: Option<JacobianFunction>,
        x0: &[Scalar],
    ) -> NonlinearSolveResult {
        let n = self.n;
        let mut x = x0.to_vec();
        let mut f_val = vec![0.0; n];

        for iter in 0..self.config.max_iter {
            f(&x, &mut f_val);

            // Check convergence
            let norm: Scalar = f_val.iter().map(|v| v * v).sum::<Scalar>().sqrt();
            if norm < self.config.tol {
                return NonlinearSolveResult {
                    x,
                    iterations: iter,
                    residual_norm: norm,
                    converged: true,
                };
            }

            // Compute Jacobian
            let mut jac = vec![0.0; n * n];
            if let Some(j) = jacobian {
                j(&x, &mut jac);
            } else {
                // Finite-difference approximation
                let eps = 1e-8;
                let mut f_plus = vec![0.0; n];
                for j_col in 0..n {
                    let saved = x[j_col];
                    x[j_col] += eps;
                    f(&x, &mut f_plus);
                    x[j_col] = saved;
                    for i in 0..n {
                        jac[i * n + j_col] = (f_plus[i] - f_val[i]) / eps;
                    }
                }
            }

            // Solve J * dx = -F using Gaussian elimination
            let rhs: Vec<Scalar> = f_val.iter().map(|v| -v).collect();

            // Simple Gaussian elimination (for small systems)
            let mut dx = rhs;
            for col in 0..n {
                // Find pivot
                let mut max_val = jac[col * n + col].abs();
                let mut max_row = col;
                for row in (col + 1)..n {
                    let val = jac[row * n + col].abs();
                    if val > max_val {
                        max_val = val;
                        max_row = row;
                    }
                }

                if max_val < 1e-15 {
                    continue;
                }

                // Swap rows
                if max_row != col {
                    for c in col..n {
                        jac.swap(col * n + c, max_row * n + c);
                    }
                    dx.swap(col, max_row);
                }

                // Eliminate
                let pivot = jac[col * n + col];
                for row in (col + 1)..n {
                    let factor = jac[row * n + col] / pivot;
                    for c in col..n {
                        let idx = row * n + c;
                        jac[idx] -= factor * jac[col * n + c];
                    }
                    dx[row] -= factor * dx[col];
                }
            }

            // Back substitution
            for i in (0..n).rev() {
                let mut sum = dx[i];
                for j in (i + 1)..n {
                    sum -= jac[i * n + j] * dx[j];
                }
                if jac[i * n + i].abs() > 1e-15 {
                    dx[i] = sum / jac[i * n + i];
                } else {
                    dx[i] = 0.0;
                }
            }

            // Apply damped step
            for i in 0..n {
                x[i] += self.config.damping * dx[i];
            }

            // Check step convergence
            let step_norm: Scalar = dx.iter().map(|v| v * v).sum::<Scalar>().sqrt();
            if step_norm < self.config.step_tol {
                f(&x, &mut f_val);
                let final_norm: Scalar = f_val.iter().map(|v| v * v).sum::<Scalar>().sqrt();
                return NonlinearSolveResult {
                    x,
                    iterations: iter + 1,
                    residual_norm: final_norm,
                    converged: final_norm < self.config.tol,
                };
            }
        }

        // Did not converge
        f(&x, &mut f_val);
        let norm: Scalar = f_val.iter().map(|v| v * v).sum::<Scalar>().sqrt();
        NonlinearSolveResult {
            x,
            iterations: self.config.max_iter,
            residual_norm: norm,
            converged: false,
        }
    }
}
