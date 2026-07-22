//! Core solver trait, configuration, statistics, and result types.
//!
//! Defines the `OdeSolver` trait that all numerical ODE solvers implement,
//! along with shared configuration and statistics structures used throughout
//! the solver module.

use crate::core::error::SimError;
use crate::core::types::Scalar;

/// Result of a single solver step.
#[derive(Debug, Clone, PartialEq)]
pub enum SolverStepResult {
    /// Step was accepted (state advanced by dt).
    Accepted,
    /// Step was rejected with a suggested new step size.
    Rejected { suggested_dt: Scalar },
    /// Iterative solver converged (Newton, DAE).
    Converged,
    /// Iterative solver did not converge.
    NotConverged,
    /// Matrix is singular (cannot proceed).
    Singular,
}

impl SolverStepResult {
    /// Returns `true` if the step was successfully accepted or converged.
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Accepted | Self::Converged)
    }
}

/// Configuration for adaptive and iterative solvers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolverConfig {
    /// Relative tolerance for error control.
    pub rtol: Scalar,
    /// Absolute tolerance for error control.
    pub atol: Scalar,
    /// Maximum step size.
    pub max_step: Scalar,
    /// Minimum step size.
    pub min_step: Scalar,
    /// Maximum number of iterations (for Newton, DAE).
    pub max_iter: usize,
    /// Step size safety factor (default 0.9).
    pub safety_factor: Scalar,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            rtol: 1e-6,
            atol: 1e-12,
            max_step: 1.0,
            min_step: 1e-12,
            max_iter: 50,
            safety_factor: 0.9,
        }
    }
}

impl SolverConfig {
    /// Create a new solver configuration with the given tolerances.
    pub fn new(rtol: Scalar, atol: Scalar) -> Self {
        Self {
            rtol,
            atol,
            ..Default::default()
        }
    }

    /// Create a configuration suitable for stiff solvers (tighter tolerances).
    pub fn stiff() -> Self {
        Self {
            rtol: 1e-8,
            atol: 1e-14,
            max_iter: 100,
            ..Default::default()
        }
    }

    /// Validate the configuration; returns errors if invalid.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.rtol <= 0.0 {
            errors.push("rtol must be positive".to_string());
        }
        if self.atol <= 0.0 {
            errors.push("atol must be positive".to_string());
        }
        if self.max_step <= 0.0 {
            errors.push("max_step must be positive".to_string());
        }
        if self.min_step <= 0.0 {
            errors.push("min_step must be positive".to_string());
        }
        if self.min_step > self.max_step {
            errors.push("min_step must not exceed max_step".to_string());
        }
        if self.max_iter == 0 {
            errors.push("max_iter must be positive".to_string());
        }
        if self.safety_factor <= 0.0 || self.safety_factor > 1.0 {
            errors.push("safety_factor must be in (0, 1]".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Statistics collected during solver execution.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SolverStats {
    /// Number of accepted steps.
    pub steps_accepted: u64,
    /// Number of rejected steps (adaptive methods only).
    pub steps_rejected: u64,
    /// Number of function (RHS) evaluations.
    pub function_evals: u64,
    /// Number of Jacobian evaluations (implicit methods).
    pub jacobian_evals: u64,
}

impl SolverStats {
    /// Create new empty statistics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total number of steps attempted.
    pub fn total_steps(&self) -> u64 {
        self.steps_accepted + self.steps_rejected
    }

    /// Reset all statistics to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Type alias for the ODE right-hand side function.
///
/// `f(t, x, dx)` — computes dx/dt given current time and state.
pub type OdeRhs<'a> = dyn FnMut(&[Scalar], Scalar, &mut [Scalar]) -> Result<(), SimError> + 'a;

/// Type alias for the nonlinear system function.
///
/// `F(x, result)` — evaluates the system at x, writes to result.
pub type NlsFunc<'a> = dyn FnMut(&[Scalar], &mut [Scalar]) -> Result<(), SimError> + 'a;

/// Type alias for a Jacobian evaluation function.
///
/// `J(x, J_out)` — evaluates J = dF/dx at x, writes to J_out as row-major matrix.
pub type JacobianFunc<'a> = dyn FnMut(&[Scalar], &mut [Vec<Scalar>]) -> Result<(), SimError> + 'a;

/// The core trait that all ODE solver methods must implement.
pub trait OdeSolver: Send + Sync {
    /// Human-readable name of this solver method.
    fn name(&self) -> &str;

    /// Advance the system state `x` by one step of size `dt`.
    ///
    /// `f` computes the ODE right-hand side: `dx/dt = f(t, x)`.
    /// On success, `x` is updated to the new state at time `t + dt`.
    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError>;

    /// Return the order of accuracy of this method.
    fn order(&self) -> u8;

    /// Return the number of internal stages per step.
    fn stages(&self) -> u8;

    /// Whether this solver supports adaptive step size control.
    fn is_adaptive(&self) -> bool {
        false
    }

    /// Estimate the error for the current step (only for adaptive methods).
    fn estimate_error(&self) -> Option<Scalar> {
        None
    }

    /// Get a reference to the solver statistics.
    fn stats(&self) -> &SolverStats;

    /// Get a mutable reference to the solver statistics.
    fn stats_mut(&mut self) -> &mut SolverStats;
}

/// Adaptive step size controller.
///
/// Given the estimated error, computes the recommended next step size.
/// Used by all adaptive Runge-Kutta methods.
pub fn adapt_step_size(
    error: Scalar,
    dt: Scalar,
    config: &SolverConfig,
    order: u8,
) -> Scalar {
    // Avoid division by zero / log of zero
    let scale = (error / (config.rtol + config.atol)).max(1e-14);
    let exponent = -(1.0 / (order as Scalar + 1.0));
    let factor = config.safety_factor * scale.powf(exponent);
    let new_dt = dt * factor;
    new_dt.clamp(config.min_step, config.max_step)
}

/// Compute a finite-difference Jacobian for a nonlinear function.
///
/// Returns J[i][j] = dF_i/dx_j ≈ (F(x + h*e_j) - F(x)) / h
pub fn finite_diff_jacobian(
    f: &mut NlsFunc,
    x: &[Scalar],
    fx: &[Scalar],
) -> Result<Vec<Vec<Scalar>>, SimError> {
    let n = x.len();
    let eps = 1e-8;
    let mut jac = vec![vec![0.0; n]; n];
    let mut x_perturbed = x.to_vec();
    let mut f_perturbed = vec![0.0; n];

    for j in 0..n {
        let h = eps * x[j].abs().max(1.0);
        x_perturbed[j] += h;
        f(&x_perturbed, &mut f_perturbed)?;
        x_perturbed[j] = x[j]; // restore

        for i in 0..n {
            jac[i][j] = (f_perturbed[i] - fx[i]) / h;
        }
    }
    Ok(jac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solver_config_default() {
        let cfg = SolverConfig::default();
        assert!((cfg.rtol - 1e-6).abs() < 1e-12);
        assert!((cfg.atol - 1e-12).abs() < 1e-12);
        assert_eq!(cfg.max_iter, 50);
    }

    #[test]
    fn test_solver_config_validation() {
        assert!(SolverConfig::default().validate().is_ok());

        let bad = SolverConfig {
            rtol: -1.0,
            ..Default::default()
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn test_solver_config_new() {
        let cfg = SolverConfig::new(1e-4, 1e-8);
        assert!((cfg.rtol - 1e-4).abs() < 1e-12);
        assert!((cfg.atol - 1e-8).abs() < 1e-12);
    }

    #[test]
    fn test_solver_config_stiff() {
        let cfg = SolverConfig::stiff();
        assert_eq!(cfg.max_iter, 100);
    }

    #[test]
    fn test_solver_stats() {
        let mut stats = SolverStats::new();
        assert_eq!(stats.total_steps(), 0);
        stats.steps_accepted = 10;
        stats.steps_rejected = 2;
        assert_eq!(stats.total_steps(), 12);
        stats.reset();
        assert_eq!(stats.total_steps(), 0);
    }

    #[test]
    fn test_step_result_is_ok() {
        assert!(SolverStepResult::Accepted.is_ok());
        assert!(SolverStepResult::Converged.is_ok());
        assert!(!SolverStepResult::Rejected { suggested_dt: 0.0 }.is_ok());
        assert!(!SolverStepResult::NotConverged.is_ok());
        assert!(!SolverStepResult::Singular.is_ok());
    }

    #[test]
    fn test_adapt_step_size_basic() {
        let cfg = SolverConfig::default();
        // Very small error → step size should increase
        let new_dt = adapt_step_size(1e-14, 0.01, &cfg, 4);
        assert!(new_dt > 0.01);

        // Large error → step size should decrease
        let new_dt2 = adapt_step_size(1.0, 0.01, &cfg, 4);
        assert!(new_dt2 < 0.01);
    }

    #[test]
    fn test_adapt_step_size_clamping() {
        let cfg = SolverConfig {
            min_step: 1e-8,
            max_step: 1.0,
            ..Default::default()
        };
        // Very large error should clamp to min_step
        let dt = adapt_step_size(1e10, 0.01, &cfg, 4);
        assert!(dt >= 1e-8 - 1e-15);

        // Very small error should clamp to max_step
        let dt2 = adapt_step_size(1e-30, 0.01, &cfg, 4);
        assert!(dt2 <= 1.0 + 1e-15);
    }

    #[test]
    fn test_finite_diff_jacobian() {
        // F(x) = [x0^2 + x1, x0 * x1]
        // Jacobian: [[2*x0, 1], [x1, x0]]
        let mut f = |x: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            result[0] = x[0] * x[0] + x[1];
            result[1] = x[0] * x[1];
            Ok(())
        };

        let x = vec![2.0, 3.0];
        let mut fx = vec![0.0; 2];
        f(&x, &mut fx).unwrap();

        let jac = finite_diff_jacobian(&mut f, &x, &fx).unwrap();
        // J[0][0] ≈ 2*x0 = 4
        assert!((jac[0][0] - 4.0).abs() < 1e-6);
        // J[0][1] ≈ 1
        assert!((jac[0][1] - 1.0).abs() < 1e-6);
        // J[1][0] ≈ x1 = 3
        assert!((jac[1][0] - 3.0).abs() < 1e-6);
        // J[1][1] ≈ x0 = 2
        assert!((jac[1][1] - 2.0).abs() < 1e-6);
    }
}
