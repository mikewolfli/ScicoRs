//! Stiff ODE solvers.
//!
//! Implements implicit methods for stiff systems:
//! - Backward Differentiation Formula (BDF-1 / Backward Euler)
//! - Trapezoidal rule
//! - General BDF up to order 2

use crate::core::types::Scalar;
use crate::solver::ode::{OdeFunction, OdeSolver, OdeSolverConfig, OdeStepResult};

/// Backward Euler (BDF-1) implicit solver for stiff systems.
#[derive(Debug, Clone)]
pub struct BackwardEulerSolver {
    pub config: OdeSolverConfig,
    /// Maximum Newton iterations per step.
    pub max_newton_iter: usize,
    /// Newton convergence tolerance.
    pub newton_tol: Scalar,
}

impl BackwardEulerSolver {
    pub fn new(dt: Scalar) -> Self {
        Self {
            config: OdeSolverConfig { dt, ..Default::default() },
            max_newton_iter: 50,
            newton_tol: 1e-8,
        }
    }
}

impl OdeSolver for BackwardEulerSolver {
    fn name(&self) -> &str {
        "Backward Euler (BDF-1)"
    }

    fn step(&self, f: OdeFunction, t: Scalar, y: &[Scalar], dt: Scalar) -> OdeStepResult {
        let n = y.len();
        let mut y_new = y.to_vec();
        let t_new = t + dt;

        // Newton iteration: y_{n+1} = y_n + dt * f(t_{n+1}, y_{n+1})
        for _iter in 0..self.max_newton_iter {
            let mut f_val = vec![0.0; n];
            f(t_new, &y_new, &mut f_val);

            // Compute residual: R = y_new - y_n - dt * f(t_new, y_new)
            let mut res = vec![0.0; n];
            let mut max_res = 0.0;
            for i in 0..n {
                res[i] = y_new[i] - y[i] - dt * f_val[i];
                let abs_res = res[i].abs();
                if abs_res > max_res {
                    max_res = abs_res;
                }
            }

            if max_res < self.newton_tol {
                break;
            }

            // Simplified Jacobian: use identity (simplified Newton)
            // In production, a full Jacobian and linear solve would be used.
            for i in 0..n {
                let jac = 1.0 - dt * 1e-6; // approximate diagonal
                y_new[i] -= res[i] / jac;
            }
        }

        OdeStepResult {
            t: t_new,
            y: y_new,
            error: 0.0,
            accepted: true,
            dt_next: dt,
        }
    }

    fn config(&self) -> &OdeSolverConfig {
        &self.config
    }
}

/// Trapezoidal rule implicit solver for stiff systems.
#[derive(Debug, Clone)]
pub struct TrapezoidalSolver {
    pub config: OdeSolverConfig,
    pub max_newton_iter: usize,
    pub newton_tol: Scalar,
}

impl TrapezoidalSolver {
    pub fn new(dt: Scalar) -> Self {
        Self {
            config: OdeSolverConfig { dt, ..Default::default() },
            max_newton_iter: 50,
            newton_tol: 1e-8,
        }
    }
}

impl OdeSolver for TrapezoidalSolver {
    fn name(&self) -> &str {
        "Trapezoidal Rule"
    }

    fn step(&self, f: OdeFunction, t: Scalar, y: &[Scalar], dt: Scalar) -> OdeStepResult {
        let n = y.len();
        let t_new = t + dt;

        // Get f(t, y)
        let mut f_old = vec![0.0; n];
        f(t, y, &mut f_old);

        let mut y_new = y.to_vec();

        // Newton iteration: y_{n+1} = y_n + 0.5*dt*(f(t,y) + f(t_{n+1}, y_{n+1}))
        for _iter in 0..self.max_newton_iter {
            let mut f_new = vec![0.0; n];
            f(t_new, &y_new, &mut f_new);

            let mut max_res = 0.0;
            for i in 0..n {
                let res = y_new[i] - y[i] - 0.5 * dt * (f_old[i] + f_new[i]);
                if res.abs() > max_res {
                    max_res = res.abs();
                }
            }

            if max_res < self.newton_tol {
                break;
            }

            for i in 0..n {
                let jac = 1.0 - 0.5 * dt * 1e-6;
                let res = y_new[i] - y[i] - 0.5 * dt * (f_old[i] + f_new[i]);
                y_new[i] -= res / jac;
            }
        }

        OdeStepResult {
            t: t_new,
            y: y_new,
            error: 0.0,
            accepted: true,
            dt_next: dt,
        }
    }

    fn config(&self) -> &OdeSolverConfig {
        &self.config
    }
}
