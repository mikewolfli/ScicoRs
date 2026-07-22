//! Differential-Algebraic Equation (DAE) solver.
//!
//! Provides a basic index-1 DAE solver using backward Euler discretization.
//! For a semi-explicit DAE: dx/dt = f(t, x, z), 0 = g(t, x, z)
//! the solver discretizes the differential part with backward Euler and
//! solves the coupled system via Newton iteration.

use crate::core::error::SimError;
use crate::core::types::Scalar;
use super::nonlinear::NewtonRaphson;
use super::traits::{SolverConfig, SolverStepResult};

/// A function that evaluates the differential part of a DAE.
///
/// `f(t, x, z, dx)` — computes dx/dt = f(t, x, z)
pub type DaeDiffFn<'a> = dyn FnMut(Scalar, &[Scalar], &[Scalar], &mut [Scalar]) -> Result<(), SimError> + 'a;

/// A function that evaluates the algebraic constraints of a DAE.
///
/// `g(t, x, z, result)` — computes 0 = g(t, x, z)
pub type DaeAlgFn<'a> = dyn FnMut(Scalar, &[Scalar], &[Scalar], &mut [Scalar]) -> Result<(), SimError> + 'a;

/// Index-1 DAE solver using backward Euler discretization.
///
/// For the semi-explicit DAE system:
///   dx/dt = f(t, x, z)
///   0     = g(t, x, z)
///
/// The solver applies backward Euler to the differential part:
///   (x_{n+1} - x_n)/dt = f(t_{n+1}, x_{n+1}, z_{n+1})
///   0 = g(t_{n+1}, x_{n+1}, z_{n+1})
///
/// This coupled system is solved simultaneously via Newton-Raphson.
#[derive(Debug, Clone)]
pub struct DaeSolver {
    config: SolverConfig,
}

impl DaeSolver {
    /// Create a new DAE solver with the given configuration.
    pub fn new(config: SolverConfig) -> Self {
        Self { config }
    }

    /// Perform one step of DAE integration.
    ///
    /// * `f` — differential function: dx/dt = f(t, x, z)
    /// * `g` — algebraic constraint: 0 = g(t, x, z)
    /// * `x` — differential state variables (updated in place)
    /// * `z` — algebraic state variables (updated in place)
    /// * `t` — current time
    /// * `dt` — step size
    pub fn step(
        &self,
        f: &mut DaeDiffFn,
        g: &mut DaeAlgFn,
        x: &mut [Scalar],
        z: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let nx = x.len();
        let nz = z.len();
        let n = nx + nz;
        let x_n = x.to_vec();
        let t_next = t + dt;

        // Build the combined residual function for Newton:
        // G(x_new, z_new) = [
        //   (x_new - x_n)/dt - f(t_next, x_new, z_new),
        //   g(t_next, x_new, z_new)
        // ] = 0
        let x_n_clone = x_n.clone();
        let mut newton = NewtonRaphson::new(self.config);

        let mut residual = |u: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            let x_part = &u[0..nx];
            let z_part = &u[nx..n];

            // Differential residual
            let mut fx = vec![0.0; nx];
            f(t_next, x_part, z_part, &mut fx)?;
            for i in 0..nx {
                result[i] = (u[i] - x_n_clone[i]) / dt - fx[i];
            }

            // Algebraic residual
            let mut gx = vec![0.0; nz];
            g(t_next, x_part, z_part, &mut gx)?;
            result[nx..n].copy_from_slice(&gx[..nz]);

            Ok(())
        };

        // Initial guess: combine x and z
        let mut u = vec![0.0; n];
        u[0..nx].copy_from_slice(x);
        u[nx..n].copy_from_slice(z);

        let result = newton.solve(&mut residual, None, &mut u)?;

        if result == SolverStepResult::Converged {
            x.copy_from_slice(&u[0..nx]);
            z.copy_from_slice(&u[nx..n]);
            Ok(SolverStepResult::Accepted)
        } else {
            Ok(result)
        }
    }

    /// Get the solver configuration.
    pub fn config(&self) -> &SolverConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::Scalar;

    /// Simple index-1 DAE:
    /// dx/dt = z  (differential)
    /// 0 = x - sin(t)  (algebraic constraint)
    /// Analytical solution: x = sin(t), z = cos(t)
    fn test_f(_t: Scalar, _x: &[Scalar], z: &[Scalar], dx: &mut [Scalar]) -> Result<(), SimError> {
        dx[0] = z[0];
        Ok(())
    }

    fn test_g(t: Scalar, x: &[Scalar], _z: &[Scalar], res: &mut [Scalar]) -> Result<(), SimError> {
        res[0] = x[0] - t.sin();
        Ok(())
    }

    #[test]
    fn test_dae_solver_creation() {
        let solver = DaeSolver::new(SolverConfig::default());
        assert!(solver.config().max_iter > 0);
    }

    #[test]
    fn test_dae_simple_constraint() {
        let config = SolverConfig::new(1e-8, 1e-12);
        let solver = DaeSolver::new(config);

        let mut x = vec![0.0]; // x(0) = sin(0) = 0
        let mut z = vec![1.0]; // z(0) = cos(0) = 1
        let dt = 0.01;
        let mut t = 0.0;

        for _step in 0..100 {
            solver
                .step(&mut test_f, &mut test_g, &mut x, &mut z, t, dt)
                .unwrap();
            t += dt;

            // Verify constraint holds: x ≈ sin(t) (Backward Euler, O(dt) accuracy)
            assert!(
                (x[0] - t.sin()).abs() < 0.01,
                "Constraint violation at t={}: x={}, sin(t)={}",
                t,
                x[0],
                t.sin()
            );
        }

        // At t=1.0, verify final state
        let t_expected: Scalar = 1.0;
        assert!((x[0] - t_expected.sin()).abs() < 0.01, "x final error too large");
        assert!((z[0] - t_expected.cos()).abs() < 0.05, "z final error too large at t=1.0");
    }

    #[test]
    fn test_dae_consistent_initial_condition() {
        // Test that the DAE solver maintains consistent ICs at the first step
        let config = SolverConfig::new(1e-8, 1e-12);
        let solver = DaeSolver::new(config);

        let mut x = vec![0.0];
        let mut z = vec![1.0];

        solver
            .step(&mut test_f, &mut test_g, &mut x, &mut z, 0.0, 1e-6)
            .unwrap();

        // After a tiny step, x should still be near 0 and z near 1
        assert!((x[0] - 0.0).abs() < 1e-4);
        assert!((z[0] - 1.0).abs() < 1e-2);
    }
}
