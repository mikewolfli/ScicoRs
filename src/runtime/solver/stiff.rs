//! Stiff ODE solvers using implicit methods.
//!
//! Provides three stiff solvers:
//! - **BackwardEuler** (BDF1) — 1st order implicit, A-stable
//! - **Trapezoidal** — 2nd order implicit, A-stable
//! - **BDF2** — 2nd order implicit backward differentiation formula
//!
//! All stiff solvers use Newton iteration to solve the implicit system
//! at each step, with finite-difference Jacobian approximation.

use super::nonlinear::NewtonRaphson;
use super::traits::{OdeRhs, OdeSolver, SolverConfig, SolverStats, SolverStepResult};
use crate::core::error::SimError;
use crate::core::types::Scalar;
use std::sync::Mutex;

/// Backward Euler method (BDF1): 1st order, A-stable.
///
/// x_{n+1} = x_n + dt * f(t_{n+1}, x_{n+1})
///
/// Solved via Newton iteration at each step. Suitable for stiff systems
/// where explicit methods would require extremely small step sizes.
#[derive(Debug, Clone)]
pub struct BackwardEuler {
    config: SolverConfig,
    stats: SolverStats,
}

impl BackwardEuler {
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            stats: SolverStats::new(),
        }
    }
}

impl OdeSolver for BackwardEuler {
    fn name(&self) -> &str {
        "BackwardEuler"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let x_n = x.to_vec();
        let t_next = t + dt;

        // Track function evaluations from f(t_n, x_n) for the step start
        let mut temp_fx = vec![0.0; n];
        f(x, t, &mut temp_fx)?;
        self.stats.function_evals += 1;

        // Define the implicit residual: G(x) = x - x_n - dt * f(t_next, x) = 0
        let mut newton = NewtonRaphson::new(self.config);
        let stats_ptr = &mut self.stats as *mut SolverStats;

        // Use Newton to solve G(x) = 0
        let mut solve_f = |x_curr: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            let mut fx = vec![0.0; n];
            f(x_curr, t_next, &mut fx)?;
            unsafe {
                (*stats_ptr).function_evals += 1;
            }
            for i in 0..n {
                result[i] = x_curr[i] - x_n[i] - dt * fx[i];
            }
            Ok(())
        };

        let result = newton.solve(&mut solve_f, None, x);
        // Accumulate Newton's internal stats
        self.stats.jacobian_evals += newton.stats().jacobian_evals;
        self.stats.function_evals += newton.stats().function_evals - 1; // subtract counted calls
        result
    }

    fn order(&self) -> u8 {
        1
    }

    fn stages(&self) -> u8 {
        1
    }

    fn stats(&self) -> &SolverStats {
        &self.stats
    }

    fn stats_mut(&mut self) -> &mut SolverStats {
        &mut self.stats
    }
}

/// Trapezoidal rule: 2nd order, A-stable.
///
/// x_{n+1} = x_n + dt/2 * (f(t_n, x_n) + f(t_{n+1}, x_{n+1}))
///
/// Implicit second-order method. Good accuracy-to-cost ratio for stiff problems.
#[derive(Debug, Clone)]
pub struct Trapezoidal {
    config: SolverConfig,
    stats: SolverStats,
}

impl Trapezoidal {
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            stats: SolverStats::new(),
        }
    }
}

impl OdeSolver for Trapezoidal {
    fn name(&self) -> &str {
        "Trapezoidal"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let x_n = x.to_vec();
        let t_next = t + dt;

        // Compute f(t_n, x_n) — explicit part
        let mut f_n = vec![0.0; n];
        f(&x_n, t, &mut f_n)?;

        // Define the implicit residual:
        // G(x) = x - x_n - dt/2 * (f_n + f(t_next, x)) = 0
        let mut newton = NewtonRaphson::new(self.config);
        let stats_ptr = &mut self.stats as *mut SolverStats;

        self.stats.function_evals += 1; // f_n already computed

        let mut solve_f = |x_curr: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            let mut fx = vec![0.0; n];
            f(x_curr, t_next, &mut fx)?;
            unsafe {
                (*stats_ptr).function_evals += 1;
            }
            for i in 0..n {
                result[i] = x_curr[i] - x_n[i] - 0.5 * dt * (f_n[i] + fx[i]);
            }
            Ok(())
        };

        let result = newton.solve(&mut solve_f, None, x);
        self.stats.jacobian_evals += newton.stats().jacobian_evals;
        self.stats.function_evals += newton.stats().function_evals - 1; // avoid double count
        result
    }

    fn order(&self) -> u8 {
        2
    }

    fn stages(&self) -> u8 {
        2
    }

    fn stats(&self) -> &SolverStats {
        &self.stats
    }

    fn stats_mut(&mut self) -> &mut SolverStats {
        &mut self.stats
    }
}

/// BDF2 — Second-order backward differentiation formula.
///
/// x_{n+1} = 4/3*x_n - 1/3*x_{n-1} + 2/3*dt*f(t_{n+1}, x_{n+1})
///
/// Requires storing the previous state x_{n-1} for the two-step startup.
/// For the first step, falls back to Backward Euler (BDF1).
/// Uses `Mutex` for interior mutability to track x_prev through `&mut self`.
#[derive(Debug)]
pub struct BDF2 {
    config: SolverConfig,
    stats: SolverStats,
    x_prev: Mutex<Option<Vec<Scalar>>>,
}

impl BDF2 {
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            stats: SolverStats::new(),
            x_prev: Mutex::new(None),
        }
    }
}

// Manual Clone impl for BDF2 (Mutex requires it)
impl Clone for BDF2 {
    fn clone(&self) -> Self {
        Self {
            config: self.config,
            stats: self.stats,
            x_prev: Mutex::new(self.x_prev.lock().unwrap().clone()),
        }
    }
}

impl OdeSolver for BDF2 {
    fn name(&self) -> &str {
        "BDF2"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let x_n = x.to_vec();
        let t_next = t + dt;

        // First step: use Backward Euler (BDF1), then store x_n as x_prev
        {
            let x_prev_guard = self.x_prev.lock().unwrap();
            if x_prev_guard.is_none() {
                drop(x_prev_guard);
                let mut be = BackwardEuler::new(self.config);
                let result = be.step(f, x, t, dt)?;
                // Store x_n as x_prev for next call
                *self.x_prev.lock().unwrap() = Some(x_n);
                self.stats.steps_accepted += 1;
                return Ok(result);
            }
        }

        let x_nm1 = {
            let guard = self.x_prev.lock().unwrap();
            guard.as_ref().unwrap().clone()
        };

        // Define the BDF2 residual:
        // G(x) = x - 4/3*x_n + 1/3*x_{n-1} - 2/3*dt*f(t_next, x) = 0
        let mut newton = NewtonRaphson::new(self.config);

        let dt_val = dt;

        let mut solve_f = |x_curr: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            let mut fx = vec![0.0; n];
            f(x_curr, t_next, &mut fx)?;
            for i in 0..n {
                result[i] = x_curr[i] - 4.0 / 3.0 * x_n[i] + 1.0 / 3.0 * x_nm1[i]
                    - 2.0 / 3.0 * dt_val * fx[i];
            }
            Ok(())
        };

        let result = newton.solve(&mut solve_f, None, x)?;

        // Accumulate Newton's internal stats
        self.stats.jacobian_evals += newton.stats().jacobian_evals;
        self.stats.function_evals += newton.stats().function_evals;

        // Store x_n as x_prev for the next step
        *self.x_prev.lock().unwrap() = Some(x_n);
        self.stats.steps_accepted += 1;

        Ok(result)
    }

    fn order(&self) -> u8 {
        2
    }

    fn stages(&self) -> u8 {
        1
    }

    fn stats(&self) -> &SolverStats {
        &self.stats
    }

    fn stats_mut(&mut self) -> &mut SolverStats {
        &mut self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Non-stiff test: dx/dt = -x
    fn decay_rhs(x: &[Scalar], _t: Scalar, dx: &mut [Scalar]) -> Result<(), SimError> {
        dx[0] = -x[0];
        Ok(())
    }

    /// Stiff test: dx/dt = -1000*x
    fn stiff_rhs(x: &[Scalar], _t: Scalar, dx: &mut [Scalar]) -> Result<(), SimError> {
        dx[0] = -1000.0 * x[0];
        Ok(())
    }

    #[test]
    fn test_backward_euler_creation() {
        let solver = BackwardEuler::new(SolverConfig::default());
        assert_eq!(solver.name(), "BackwardEuler");
        assert_eq!(solver.order(), 1);
    }

    #[test]
    fn test_backward_euler_decay() {
        let mut solver = BackwardEuler::new(SolverConfig::default());
        let mut x = vec![1.0];
        let dt = 0.01;
        let analytical_at_1 = (-1.0_f64).exp();

        for step in 0..100 {
            solver
                .step(&mut decay_rhs, &mut x, step as Scalar * dt, dt)
                .unwrap();
        }

        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 0.02, "BackwardEuler error too large: {}", error);
    }

    #[test]
    fn test_backward_euler_stiff() {
        // Backward Euler should handle stiff problems with large step sizes
        let mut solver = BackwardEuler::new(SolverConfig::stiff());
        let mut x = vec![1.0];
        let dt = 0.1; // This is too large for explicit methods on dx/dt=-1000*x

        for step in 0..10 {
            solver
                .step(&mut stiff_rhs, &mut x, step as Scalar * dt, dt)
                .unwrap();
        }

        // At t=1.0, the solution should be approximately exp(-1000) ≈ 0
        // But with large dt, backward Euler gives a qualitatively correct answer
        assert!(x[0] >= 0.0 && x[0] < 0.5);
    }

    #[test]
    fn test_trapezoidal_creation() {
        let solver = Trapezoidal::new(SolverConfig::default());
        assert_eq!(solver.name(), "Trapezoidal");
        assert_eq!(solver.order(), 2);
    }

    #[test]
    fn test_trapezoidal_decay() {
        let mut solver = Trapezoidal::new(SolverConfig::default());
        let mut x = vec![1.0];
        let dt = 0.01;
        let analytical_at_1 = (-1.0_f64).exp();

        for step in 0..100 {
            solver
                .step(&mut decay_rhs, &mut x, step as Scalar * dt, dt)
                .unwrap();
        }

        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 0.0002, "Trapezoidal error too large: {}", error);
    }

    #[test]
    fn test_bdf2_creation() {
        let solver = BDF2::new(SolverConfig::default());
        assert_eq!(solver.name(), "BDF2");
        assert_eq!(solver.order(), 2);
    }

    #[test]
    fn test_bdf2_decay() {
        let mut solver = BDF2::new(SolverConfig::default());
        let mut x = vec![1.0];
        let dt = 0.01;
        let analytical_at_1 = (-1.0_f64).exp();

        for step in 0..100 {
            solver
                .step(&mut decay_rhs, &mut x, step as Scalar * dt, dt)
                .unwrap();
        }

        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 0.02, "BDF2 error too large: {}", error);
    }
}
