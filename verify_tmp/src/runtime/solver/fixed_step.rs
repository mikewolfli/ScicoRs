//! Fixed-step ODE solvers.
//!
//! Provides four fixed-step integration methods:
//! - **Euler** (1st order) — simplest, lowest accuracy
//! - **RK4** (4th order) — classical Runge-Kutta, good balance
//! - **Heun** (2nd order) — improved Euler / predictor-corrector
//! - **Midpoint** (2nd order) — explicit midpoint rule
//!
//! # Butcher tableau coefficients
//!
//! The classical RK4 coefficients are exported as constants so domain modules
//! (aerospace, quantum/lindblad) can reference them for their own domain-aware
//! RK4 wrappers without duplicating the numerical values.

use super::traits::{OdeRhs, OdeSolver, SolverStats, SolverStepResult};
use crate::core::error::SimError;
use crate::core::types::Scalar;

/// Butcher tableau `c` vector (nodes) for classical RK4: `[0, ½, ½, 1]`.
pub const RK4_C: [Scalar; 4] = [0.0, 0.5, 0.5, 1.0];

/// Butcher tableau `b` vector (weights) for classical RK4: `[¹/₆, ¹/₃, ¹/₃, ¹/₆]`.
pub const RK4_B: [Scalar; 4] = [1.0 / 6.0, 1.0 / 3.0, 1.0 / 3.0, 1.0 / 6.0];

/// Butcher tableau `a` matrix (stage coefficients) for classical RK4.
///
/// ```text
/// 0   | 0  0  0 0
/// ½   | ½  0  0 0
/// ½   | 0  ½  0 0
/// 1   | 0  0  1 0
///     | ¹/₆ ¹/₃ ¹/₃ ¹/₆
/// ```
pub const RK4_A: [[Scalar; 4]; 4] = [
    [0.0, 0.0, 0.0, 0.0],
    [0.5, 0.0, 0.0, 0.0],
    [0.0, 0.5, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

/// Forward Euler method: 1st order, 1 stage.
///
/// x_{n+1} = x_n + dt * f(t_n, x_n)
///
/// Simplest solver. Suitable for non-stiff problems with small step sizes.
#[derive(Debug, Clone)]
pub struct Euler {
    stats: SolverStats,
}

impl Euler {
    pub fn new() -> Self {
        Self {
            stats: SolverStats::new(),
        }
    }
}

impl Default for Euler {
    fn default() -> Self {
        Self::new()
    }
}

impl OdeSolver for Euler {
    fn name(&self) -> &str {
        "Euler"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let mut k1 = vec![0.0; n];
        f(x, t, &mut k1)?;

        for i in 0..n {
            x[i] += dt * k1[i];
        }

        self.stats.function_evals += 1;
        self.stats.steps_accepted += 1;
        Ok(SolverStepResult::Accepted)
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

/// Classical fourth-order Runge-Kutta method: 4th order, 4 stages.
///
/// The most widely used fixed-step ODE solver. Excellent accuracy-to-cost ratio.
#[derive(Debug, Clone)]
pub struct RK4 {
    stats: SolverStats,
}

impl RK4 {
    pub fn new() -> Self {
        Self {
            stats: SolverStats::new(),
        }
    }
}

impl Default for RK4 {
    fn default() -> Self {
        Self::new()
    }
}

impl OdeSolver for RK4 {
    fn name(&self) -> &str {
        "RK4"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let half = dt / 2.0;
        let mut k1 = vec![0.0; n];
        let mut k2 = vec![0.0; n];
        let mut k3 = vec![0.0; n];
        let mut k4 = vec![0.0; n];
        let mut tmp = vec![0.0; n];

        // k1 = f(t, x)
        f(x, t, &mut k1)?;
        self.stats.function_evals += 1;

        // k2 = f(t + dt/2, x + dt/2 * k1)
        for i in 0..n {
            tmp[i] = x[i] + half * k1[i];
        }
        f(&tmp, t + half, &mut k2)?;
        self.stats.function_evals += 1;

        // k3 = f(t + dt/2, x + dt/2 * k2)
        for i in 0..n {
            tmp[i] = x[i] + half * k2[i];
        }
        f(&tmp, t + half, &mut k3)?;
        self.stats.function_evals += 1;

        // k4 = f(t + dt, x + dt * k3)
        for i in 0..n {
            tmp[i] = x[i] + dt * k3[i];
        }
        f(&tmp, t + dt, &mut k4)?;
        self.stats.function_evals += 1;

        // x_{n+1} = x_n + dt/6 * (k1 + 2*k2 + 2*k3 + k4)
        let sixth = dt / 6.0;
        for i in 0..n {
            x[i] += sixth * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }

        self.stats.steps_accepted += 1;
        Ok(SolverStepResult::Accepted)
    }

    fn order(&self) -> u8 {
        4
    }

    fn stages(&self) -> u8 {
        4
    }

    fn stats(&self) -> &SolverStats {
        &self.stats
    }

    fn stats_mut(&mut self) -> &mut SolverStats {
        &mut self.stats
    }
}

/// Heun's method (improved Euler): 2nd order, 2 stages.
///
/// Predictor-corrector: Euler prediction, trapezoidal correction.
#[derive(Debug, Clone)]
pub struct Heun {
    stats: SolverStats,
}

impl Heun {
    pub fn new() -> Self {
        Self {
            stats: SolverStats::new(),
        }
    }
}

impl Default for Heun {
    fn default() -> Self {
        Self::new()
    }
}

impl OdeSolver for Heun {
    fn name(&self) -> &str {
        "Heun"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let mut k1 = vec![0.0; n];
        let mut k2 = vec![0.0; n];
        let mut tmp = vec![0.0; n];

        // k1 = f(t, x)
        f(x, t, &mut k1)?;
        self.stats.function_evals += 1;

        // k2 = f(t + dt, x + dt * k1)
        for i in 0..n {
            tmp[i] = x[i] + dt * k1[i];
        }
        f(&tmp, t + dt, &mut k2)?;
        self.stats.function_evals += 1;

        // x_{n+1} = x_n + dt/2 * (k1 + k2)
        for i in 0..n {
            x[i] += 0.5 * dt * (k1[i] + k2[i]);
        }

        self.stats.steps_accepted += 1;
        Ok(SolverStepResult::Accepted)
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

/// Explicit Midpoint method: 2nd order, 2 stages.
#[derive(Debug, Clone)]
pub struct Midpoint {
    stats: SolverStats,
}

impl Midpoint {
    pub fn new() -> Self {
        Self {
            stats: SolverStats::new(),
        }
    }
}

impl Default for Midpoint {
    fn default() -> Self {
        Self::new()
    }
}

impl OdeSolver for Midpoint {
    fn name(&self) -> &str {
        "Midpoint"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let half = dt / 2.0;
        let mut k1 = vec![0.0; n];
        let mut k2 = vec![0.0; n];
        let mut tmp = vec![0.0; n];

        // k1 = f(t, x)
        f(x, t, &mut k1)?;
        self.stats.function_evals += 1;

        // k2 = f(t + dt/2, x + dt/2 * k1)
        for i in 0..n {
            tmp[i] = x[i] + half * k1[i];
        }
        f(&tmp, t + half, &mut k2)?;
        self.stats.function_evals += 1;

        // x_{n+1} = x_n + dt * k2
        for i in 0..n {
            x[i] += dt * k2[i];
        }

        self.stats.steps_accepted += 1;
        Ok(SolverStepResult::Accepted)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Test ODE: dx/dt = -x, x(0) = 1. Analytical: x(t) = exp(-t)
    fn decay_rhs(x: &[Scalar], _t: Scalar, dx: &mut [Scalar]) -> Result<(), SimError> {
        dx[0] = -x[0];
        Ok(())
    }

    #[test]
    fn test_euler_creation() {
        let solver = Euler::new();
        assert_eq!(solver.name(), "Euler");
        assert_eq!(solver.order(), 1);
        assert_eq!(solver.stages(), 1);
        assert!(!solver.is_adaptive());
    }

    #[test]
    fn test_euler_decay() {
        let mut solver = Euler::new();
        let mut x = vec![1.0];
        let dt = 0.01;
        let analytical_at_1 = (-1.0_f64).exp();

        for step in 0..100 {
            solver
                .step(&mut decay_rhs, &mut x, step as Scalar * dt, dt)
                .unwrap();
        }

        // Euler is 1st order: with dt=0.01, error ~ O(dt) ≈ 0.01
        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 0.02, "Euler error too large: {}", error);
    }

    #[test]
    fn test_rk4_creation() {
        let solver = RK4::new();
        assert_eq!(solver.name(), "RK4");
        assert_eq!(solver.order(), 4);
    }

    #[test]
    fn test_rk4_decay() {
        let mut solver = RK4::new();
        let mut x = vec![1.0];
        let dt = 0.01;
        let analytical_at_1 = (-1.0_f64).exp();

        for step in 0..100 {
            solver
                .step(&mut decay_rhs, &mut x, step as Scalar * dt, dt)
                .unwrap();
        }

        // RK4 is 4th order: with dt=0.01, error ≈ O(dt^4) ≈ 1e-8
        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 1e-6, "RK4 error too large: {}", error);
    }

    #[test]
    fn test_heun_creation() {
        let solver = Heun::new();
        assert_eq!(solver.name(), "Heun");
        assert_eq!(solver.order(), 2);
    }

    #[test]
    fn test_heun_decay() {
        let mut solver = Heun::new();
        let mut x = vec![1.0];
        let dt = 0.01;
        let analytical_at_1 = (-1.0_f64).exp();

        for step in 0..100 {
            solver
                .step(&mut decay_rhs, &mut x, step as Scalar * dt, dt)
                .unwrap();
        }

        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 0.0002, "Heun error too large: {}", error);
    }

    #[test]
    fn test_midpoint_creation() {
        let solver = Midpoint::new();
        assert_eq!(solver.name(), "Midpoint");
        assert_eq!(solver.order(), 2);
    }

    #[test]
    fn test_midpoint_decay() {
        let mut solver = Midpoint::new();
        let mut x = vec![1.0];
        let dt = 0.01;
        let analytical_at_1 = (-1.0_f64).exp();

        for step in 0..100 {
            solver
                .step(&mut decay_rhs, &mut x, step as Scalar * dt, dt)
                .unwrap();
        }

        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 0.0002, "Midpoint error too large: {}", error);
    }

    #[test]
    fn test_multi_step_stability() {
        // Solve dx/dt = -x for 1000 steps with RK4 — should remain stable
        let mut solver = RK4::new();
        let mut x = vec![1.0];
        let dt = 0.1;

        for step in 0..1000 {
            solver
                .step(&mut decay_rhs, &mut x, step as Scalar * dt, dt)
                .unwrap();
        }

        // At t=100, exp(-100) ≈ 3.7e-44 — should be very small but positive
        assert!(x[0] > 0.0 && x[0] < 1e-10);
    }

    #[test]
    fn test_rk4_two_state_system() {
        // Simple harmonic oscillator: dx/dt = y, dy/dt = -x
        // E = x^2 + y^2 should be conserved
        let mut f = |x: &[Scalar], _t: Scalar, dx: &mut [Scalar]| -> Result<(), SimError> {
            dx[0] = x[1];
            dx[1] = -x[0];
            Ok(())
        };

        let mut solver = RK4::new();
        let mut state = vec![1.0, 0.0]; // x(0)=1, y(0)=0
        let dt = 0.01;
        let initial_energy = 1.0;

        for step in 0..628 {
            // ~one period: 2*pi/0.01 ≈ 628
            solver
                .step(&mut f, &mut state, step as Scalar * dt, dt)
                .unwrap();
        }

        let energy = state[0] * state[0] + state[1] * state[1];
        let error = (energy - initial_energy).abs();
        assert!(error < 0.01, "Energy drift too large: {}", error);
    }
}
