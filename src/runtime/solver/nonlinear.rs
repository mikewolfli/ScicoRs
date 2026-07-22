//! Nonlinear equation solvers.
//!
//! Provides the Newton-Raphson method for solving systems of nonlinear
//! equations F(x) = 0. Supports both analytical Jacobian and automatic
//! finite-difference approximation.

use crate::core::error::SimError;
use crate::core::types::Scalar;
use super::linear::solve_linear_dense;
use super::traits::{finite_diff_jacobian, JacobianFunc, NlsFunc, SolverConfig, SolverStats, SolverStepResult};

/// Newton-Raphson solver for nonlinear systems F(x) = 0.
///
/// Iteratively solves J(x_k) * dx = -F(x_k) and updates x_{k+1} = x_k + dx.
/// If no Jacobian function is provided, uses finite-difference approximation.
///
/// Convergence is quadratic near the root for smooth problems.
#[derive(Debug, Clone)]
pub struct NewtonRaphson {
    config: SolverConfig,
    stats: SolverStats,
}

impl NewtonRaphson {
    /// Create a new Newton-Raphson solver with the given configuration.
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            stats: SolverStats::new(),
        }
    }

    /// Solve F(x) = 0 starting from initial guess `x`.
    ///
    /// * `f` — evaluates F(x), writes result to the output slice
    /// * `jacobian` — optional analytical Jacobian evaluator. If `None`,
    ///   uses finite-difference approximation.
    /// * `x` — initial guess (modified in-place to the solution)
    ///
    /// Returns `Converged` on success, `NotConverged` if max iterations reached,
    /// or `Singular` if the Jacobian is singular.
    pub fn solve(
        &mut self,
        f: &mut NlsFunc,
        mut jacobian: Option<&mut JacobianFunc>,
        x: &mut [Scalar],
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let mut fx = vec![0.0; n];

        for _iter in 0..self.config.max_iter {
            self.stats.function_evals += 1;

            // Evaluate F(x)
            f(x, &mut fx)?;

            // Check for convergence: |F(x)| < atol
            let norm = fx.iter().map(|v| v.abs()).fold(0.0_f64, |a, b| a.max(b));
            if norm < self.config.atol {
                self.stats.steps_accepted += 1;
                return Ok(SolverStepResult::Converged);
            }

            // Compute Jacobian J = dF/dx
            let jac = if let Some(jac_fn) = jacobian.as_mut() {
                self.stats.jacobian_evals += 1;
                let mut jac_mat = vec![vec![0.0; n]; n];
                jac_fn(x, &mut jac_mat)?;
                jac_mat
            } else {
                self.stats.jacobian_evals += 1;
                finite_diff_jacobian(f, x, &fx)?
            };

            // Solve J * dx = -F(x)
            let neg_fx: Vec<Scalar> = fx.iter().map(|v| -v).collect();
            match solve_linear_dense(&jac, &neg_fx) {
                Ok(dx) => {
                    // Update: x_{k+1} = x_k + dx
                    for i in 0..n {
                        x[i] += dx[i];
                    }
                }
                Err(_) => {
                    return Ok(SolverStepResult::Singular);
                }
            }
        }

        // Max iterations reached without convergence
        self.stats.steps_rejected += 1;
        Ok(SolverStepResult::NotConverged)
    }

    /// Get a reference to the solver statistics.
    pub fn stats(&self) -> &SolverStats {
        &self.stats
    }

    /// Reset statistics.
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_newton_solve_quadratic() {
        // F(x) = x^2 - 4 = 0  → root at x = 2
        let mut f = |x: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            result[0] = x[0] * x[0] - 4.0;
            Ok(())
        };

        let config = SolverConfig::new(1e-10, 1e-14);
        let mut solver = NewtonRaphson::new(config);
        let mut x = vec![1.0]; // initial guess

        let result = solver.solve(&mut f, None, &mut x).unwrap();
        assert_eq!(result, SolverStepResult::Converged);
        assert!((x[0] - 2.0).abs() < 1e-8, "Newton did not converge to root: {}", x[0]);
    }

    #[test]
    fn test_newton_two_variables() {
        // F(x,y) = [x^2 + y^2 - 1, x - y]  → intersection of circle and line
        // Solutions: x = y = ±1/√2
        let mut f = |x: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            result[0] = x[0] * x[0] + x[1] * x[1] - 1.0;
            result[1] = x[0] - x[1];
            Ok(())
        };

        let config = SolverConfig::new(1e-10, 1e-14);
        let mut solver = NewtonRaphson::new(config);
        let mut x = vec![0.8, 0.5]; // initial guess near (1/√2, 1/√2)

        let result = solver.solve(&mut f, None, &mut x).unwrap();
        assert_eq!(result, SolverStepResult::Converged);

        let expected = 1.0 / (2.0_f64).sqrt();
        assert!((x[0] - expected).abs() < 1e-6, "x error: {}", x[0] - expected);
        assert!((x[1] - expected).abs() < 1e-6, "y error: {}", x[1] - expected);
    }

    #[test]
    fn test_newton_not_converged() {
        // F(x) = x^2 + 1 = 0  → no real root
        let mut f = |x: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            result[0] = x[0] * x[0] + 1.0;
            Ok(())
        };

        let config = SolverConfig {
            max_iter: 5,
            ..Default::default()
        };
        let mut solver = NewtonRaphson::new(config);
        let mut x = vec![0.0];

        let result = solver.solve(&mut f, None, &mut x).unwrap();
        // For x^2 + 1 = 0 (no real root), Newton either fails to converge
        // or hits a near-singular Jacobian when x approaches 0. Accept both.
        assert!(
            result == SolverStepResult::NotConverged || result == SolverStepResult::Singular,
            "expected NotConverged or Singular, got {:?}",
            result
        );
    }

    #[test]
    fn test_newton_linear_system() {
        // F(x) = [2x + 3y - 8, x - y + 1] = 0
        // Solution: x = 1, y = 2
        let mut f = |x: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            result[0] = 2.0 * x[0] + 3.0 * x[1] - 8.0;
            result[1] = x[0] - x[1] + 1.0;
            Ok(())
        };

        let config = SolverConfig::new(1e-10, 1e-14);
        let mut solver = NewtonRaphson::new(config);
        let mut x = vec![0.0, 0.0]; // initial guess

        let result = solver.solve(&mut f, None, &mut x).unwrap();
        assert_eq!(result, SolverStepResult::Converged);
        assert!((x[0] - 1.0).abs() < 1e-8);
        assert!((x[1] - 2.0).abs() < 1e-8);
    }

    #[test]
    fn test_newton_with_analytical_jacobian() {
        // F(x) = x^2 - 4, J = 2x
        let mut f = |x: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            result[0] = x[0] * x[0] - 4.0;
            Ok(())
        };

        let mut jac = |x: &[Scalar], j: &mut [Vec<Scalar>]| -> Result<(), SimError> {
            j[0][0] = 2.0 * x[0];
            Ok(())
        };

        let config = SolverConfig::new(1e-10, 1e-14);
        let mut solver = NewtonRaphson::new(config);
        let mut x = vec![1.0];

        let result = solver.solve(&mut f, Some(&mut jac), &mut x).unwrap();
        assert_eq!(result, SolverStepResult::Converged);
        assert!((x[0] - 2.0).abs() < 1e-8);
    }
}
