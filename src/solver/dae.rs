//! Differential-Algebraic Equation (DAE) solvers.
//!
//! Provides solvers for systems of the form F(t, y, y') = 0,
//! including index-1 Hessenberg form and general DAE systems.

use crate::core::types::Scalar;

/// The DAE residual function: F(t, y, yp) -> residual
pub type DaeFunction = fn(Scalar, &[Scalar], &[Scalar], &mut [Scalar]);

/// Configuration for DAE solvers.
#[derive(Debug, Clone)]
pub struct DaeSolverConfig {
    /// Initial step size.
    pub dt: Scalar,
    /// Relative tolerance.
    pub rtol: Scalar,
    /// Absolute tolerance.
    pub atol: Scalar,
    /// Maximum number of iterations.
    pub max_iter: usize,
}

impl Default for DaeSolverConfig {
    fn default() -> Self {
        Self {
            dt: 1e-3,
            rtol: 1e-6,
            atol: 1e-8,
            max_iter: 100,
        }
    }
}

/// Result of a DAE solver step.
#[derive(Debug, Clone)]
pub struct DaeStepResult {
    pub t: Scalar,
    pub y: Vec<Scalar>,
    pub yp: Vec<Scalar>,
    pub accepted: bool,
}

/// A trait for DAE solver implementations.
pub trait DaeSolver: Send {
    fn name(&self) -> &str;
    fn step(&self, f: DaeFunction, t: Scalar, y: &[Scalar], _yp: &[Scalar], dt: Scalar) -> DaeStepResult;
    fn config(&self) -> &DaeSolverConfig;
}

/// Backward Euler-based DAE solver (index-1).
///
/// Discretizes with BDF-1: y' ≈ (y_{n+1} - y_n) / dt
/// and solves F(t_{n+1}, y_{n+1}, (y_{n+1} - y_n)/dt) = 0 via Newton iteration.
#[derive(Debug, Clone)]
pub struct DaeBdf1Solver {
    pub config: DaeSolverConfig,
    pub newton_tol: Scalar,
}

impl DaeBdf1Solver {
    pub fn new(dt: Scalar) -> Self {
        Self {
            config: DaeSolverConfig { dt, ..Default::default() },
            newton_tol: 1e-8,
        }
    }
}

impl DaeSolver for DaeBdf1Solver {
    fn name(&self) -> &str {
        "DAE BDF-1"
    }

    fn step(&self, f: DaeFunction, t: Scalar, y: &[Scalar], _yp: &[Scalar], dt: Scalar) -> DaeStepResult {
        let n = y.len();
        let t_new = t + dt;
        let mut y_new = y.to_vec();
        let mut yp_new = vec![0.0; n];

        // Simplified Newton iteration
        for _iter in 0..self.config.max_iter {
            // Compute yp from BDF-1 discretization
            for i in 0..n {
                yp_new[i] = (y_new[i] - y[i]) / dt;
            }

            let mut residual = vec![0.0; n];
            f(t_new, &y_new, &yp_new, &mut residual);

            let max_res = residual.iter().map(|r| r.abs()).fold(0.0, f64::max);
            if max_res < self.newton_tol {
                break;
            }

            // Simplified Newton update (approximate)
            for i in 0..n {
                y_new[i] -= residual[i] * dt / (1.0 + dt);
            }
        }

        DaeStepResult {
            t: t_new,
            y: y_new,
            yp: yp_new,
            accepted: true,
        }
    }

    fn config(&self) -> &DaeSolverConfig {
        &self.config
    }
}
