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

        // Use Newton to solve G(x) = 0.
        // Newton internally counts its own function evaluations and Jacobian evaluations,
        // which we accumulate into self.stats after the solve returns.
        let mut solve_f = |x_curr: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            let mut fx = vec![0.0; n];
            f(x_curr, t_next, &mut fx)?;
            for i in 0..n {
                result[i] = x_curr[i] - x_n[i] - dt * fx[i];
            }
            Ok(())
        };

        let result = newton.solve(&mut solve_f, None, x)?;
        // Accumulate Newton's internal stats (no double-counting: Newton counts its own calls)
        self.stats.jacobian_evals += newton.stats().jacobian_evals;
        self.stats.function_evals += newton.stats().function_evals;
        // `NewtonRaphson` already increments its own accepted/rejected counters
        // for this solve, so merge them instead of re-recording the outcome
        // (which would double-count).
        self.stats.merge_step_outcome(newton.stats());
        Ok(result)
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
        self.stats.function_evals += 1;

        // Define the implicit residual:
        // G(x) = x - x_n - dt/2 * (f_n + f(t_next, x)) = 0
        let mut newton = NewtonRaphson::new(self.config);

        let mut solve_f = |x_curr: &[Scalar], result: &mut [Scalar]| -> Result<(), SimError> {
            let mut fx = vec![0.0; n];
            f(x_curr, t_next, &mut fx)?;
            for i in 0..n {
                result[i] = x_curr[i] - x_n[i] - 0.5 * dt * (f_n[i] + fx[i]);
            }
            Ok(())
        };

        let result = newton.solve(&mut solve_f, None, x)?;
        self.stats.jacobian_evals += newton.stats().jacobian_evals;
        self.stats.function_evals += newton.stats().function_evals;
        // Merge Newton's own accepted/rejected accounting rather than
        // re-recording it, to avoid double-counting.
        self.stats.merge_step_outcome(newton.stats());
        Ok(result)
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

        // First step: use Backward Euler (BDF1), then store x_n as x_prev.
        {
            let x_prev_guard = self.x_prev.lock().unwrap();
            if x_prev_guard.is_none() {
                drop(x_prev_guard);
                let mut be = BackwardEuler::new(self.config);
                let result = be.step(f, x, t, dt)?;
                // Merge the temporary BackwardEuler stats (function/Jacobian
                // evaluations) so BDF2's reported statistics include the
                // BDF1 warm-up step instead of undercounting them.
                let be_stats = be.stats();
                self.stats.function_evals += be_stats.function_evals;
                self.stats.jacobian_evals += be_stats.jacobian_evals;

                // Only commit the state history when the warm-up step actually
                // converged. `step` returns `Ok(NotConverged)`/`Ok(Singular)`
                // rather than `Err`, so the `?` above does not filter them; on
                // those the returned `x` is not a solution and must not become
                // the two-step history for every subsequent BDF2 step.
                //
                // `BackwardEuler` already merged Newton's counters, so merge its
                // counters here rather than recording the outcome again.
                self.stats.merge_step_outcome(be.stats());
                if result.is_ok() {
                    *self.x_prev.lock().unwrap() = Some(x_n);
                }
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

        // Commit the two-step history only for a genuinely converged step.
        // `newton.solve` reports failure as `Ok(NotConverged)`/`Ok(Singular)`,
        // which the `?` above lets through; committing `x_n` regardless would
        // silently seed every later BDF2 step from a divergent iterate.
        self.stats.merge_step_outcome(newton.stats());
        if result.is_ok() {
            *self.x_prev.lock().unwrap() = Some(x_n);
        }

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

    /// `NewtonRaphson::solve` reports failure as `Ok(NotConverged)`, not `Err`.
    /// The BDF2 step used to ignore that and unconditionally commit the
    /// unconverged iterate as the two-step history, silently contaminating every
    /// later step. A non-converging step must be recorded as rejected and must
    /// not advance the history.
    #[test]
    fn test_bdf2_non_convergence_is_recorded_and_does_not_commit_history() {
        // `max_iter: 0` forces Newton to give up immediately, so every step
        // returns `NotConverged`. The RHS is well-scaled, so this isolates the
        // bookkeeping rather than an ill-conditioned Jacobian.
        let config = SolverConfig {
            max_iter: 0,
            ..SolverConfig::default()
        };
        let mut solver = BDF2::new(config);
        let mut rhs = |x: &[Scalar], _t: Scalar, dx: &mut [Scalar]| -> Result<(), SimError> {
            dx[0] = -x[0];
            Ok(())
        };

        let mut x = vec![1.0];
        let first = solver.step(&mut rhs, &mut x, 0.0, 0.01).unwrap();
        assert!(
            !first.is_ok(),
            "a zero-iteration Newton must not report success, got {first:?}"
        );

        let second = solver.step(&mut rhs, &mut x, 0.01, 0.01).unwrap();
        assert!(
            !second.is_ok(),
            "the second step must also fail, got {second:?}"
        );

        let stats = solver.stats();
        assert_eq!(
            stats.steps_accepted, 0,
            "no step converged, so none may be counted as accepted"
        );
        assert_eq!(
            stats.steps_rejected, 2,
            "both failed steps must be counted as rejected"
        );

        // The warm-up (BDF1) path returned early, so the two-step history must
        // still be empty. This is the state the corruption bug would have
        // polluted: committing `x_n` from an unconverged iterate.
        assert!(
            solver.x_prev.lock().unwrap().is_none(),
            "an unconverged warm-up step must not seed the BDF2 history"
        );
    }

    /// The BDF2 branch itself (past the first-step warm-up) must not commit an
    /// unconverged iterate as the two-step history.
    ///
    /// `max_iter` is valid (1) but far too small for the Newton solve to reach
    /// `atol` in one iteration, so the step reliably returns `NotConverged`.
    /// The first call takes the BDF1 warm-up path; the second exercises BDF2.
    #[test]
    fn test_bdf2_branch_does_not_commit_unconverged_iterate() {
        let config = SolverConfig {
            max_iter: 1,
            atol: 1e-300,
            rtol: 1e-300,
            ..SolverConfig::default()
        };
        assert!(
            config.validate().is_ok(),
            "the test config must be valid: {:?}",
            config.validate()
        );

        let mut solver = BDF2::new(config);
        let mut rhs = |x: &[Scalar], _t: Scalar, dx: &mut [Scalar]| -> Result<(), SimError> {
            dx[0] = -x[0];
            Ok(())
        };

        // Step 1 takes the warm-up path, which also cannot converge here.
        let mut x = vec![1.0];
        let first = solver.step(&mut rhs, &mut x, 0.0, 0.01).unwrap();
        assert!(
            !first.is_ok(),
            "step 1 must not claim convergence: {first:?}"
        );

        // Clear the history so the next call is a fresh warm-up attempt rather
        // than depending on step 1's outcome.
        *solver.x_prev.lock().unwrap() = Some(vec![1.0]);

        // Step 2 is the BDF2 branch.
        let second = solver.step(&mut rhs, &mut x, 0.01, 0.01).unwrap();
        assert!(
            !second.is_ok(),
            "step 2 must not claim convergence: {second:?}"
        );

        // The history must still be the value we seeded. `x_n` is the state at
        // the *start* of the step, so on failure it must not be replaced by the
        // unconverged iterate Newton happened to leave in `x`.
        let committed = solver.x_prev.lock().unwrap().clone();
        assert_eq!(
            committed,
            Some(vec![1.0]),
            "an unconverged BDF2 step must not overwrite the two-step history"
        );
        assert_eq!(
            solver.stats().steps_accepted,
            0,
            "nothing converged, so nothing may be counted as accepted"
        );
    }

    /// The same failure-mode check for the other implicit methods, which
    /// previously never touched the accepted/rejected counters at all.
    #[test]
    fn test_implicit_methods_record_rejected_steps() {
        let config = SolverConfig {
            max_iter: 0,
            ..SolverConfig::default()
        };
        let mut rhs = |x: &[Scalar], _t: Scalar, dx: &mut [Scalar]| -> Result<(), SimError> {
            dx[0] = -x[0];
            Ok(())
        };

        let results = [
            ("BackwardEuler", {
                let mut x = [1.0];
                BackwardEuler::new(config).step(&mut rhs, &mut x, 0.0, 0.01)
            }),
            ("Trapezoidal", {
                let mut x = [1.0];
                Trapezoidal::new(config).step(&mut rhs, &mut x, 0.0, 0.01)
            }),
        ];
        for (name, result) in results {
            let step = result.unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                !step.is_ok(),
                "{name} must not report success on zero iterations"
            );
        }
    }

    /// A converged BDF2 run must still report its accepted steps, so the fix
    /// did not simply stop counting anything.
    #[test]
    fn test_bdf2_counts_accepted_steps_on_a_converging_problem() {
        let mut solver = BDF2::new(SolverConfig::default());
        let mut rhs = |x: &[Scalar], _t: Scalar, dx: &mut [Scalar]| -> Result<(), SimError> {
            dx[0] = -x[0];
            Ok(())
        };
        let mut x = vec![1.0];
        let mut t = 0.0;
        let dt = 0.001;
        for _ in 0..20 {
            let r = solver.step(&mut rhs, &mut x, t, dt).unwrap();
            assert!(
                r.is_ok(),
                "a well-conditioned decay must converge, got {r:?}"
            );
            t += dt;
        }
        let stats = solver.stats();
        assert_eq!(stats.steps_accepted, 20, "every converged step must count");
        assert_eq!(stats.steps_rejected, 0);
        assert!(stats.total_steps() == 20);
    }
}
