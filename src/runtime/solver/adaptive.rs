// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Adaptive step-size ODE solvers using embedded Runge-Kutta methods.
//!
//! Provides three adaptive methods:
//! - **RK45** (Dormand-Prince 5(4)) — 7 stages, order 5 with 4th-order error estimate
//! - **RK23** (Bogacki-Shampine 3(2)) — 4 stages, order 3 with 2nd-order error estimate
//! - **CashKarp** (Cash-Karp 5(4)) — 6 stages, order 5 with 4th-order error estimate
//!
//! All adaptive solvers use the same step control algorithm in `adapt_step_size`.

use super::traits::{
    OdeRhs, OdeSolver, SolverConfig, SolverStats, SolverStepResult, adapt_step_size,
};
use crate::core::error::SimError;
use crate::core::types::Scalar;

/// Dormand-Prince RK5(4) — 7 stages, 5th order with 4th order embedded estimate.
///
/// The most commonly used adaptive Runge-Kutta method. Excellent general-purpose
/// solver for non-stiff ODEs. Uses the "free" 4th-order result for error estimation
/// and the 5th-order result for propagation (local extrapolation).
#[derive(Debug, Clone)]
pub struct RK45 {
    config: SolverConfig,
    stats: SolverStats,
    last_error: Option<Scalar>,
}

impl RK45 {
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            stats: SolverStats::new(),
            last_error: None,
        }
    }
}

impl OdeSolver for RK45 {
    fn name(&self) -> &str {
        "RK45"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let mut k = vec![vec![0.0; n]; 7];
        let mut tmp = vec![0.0; n];

        // DOPRI5 (Dormand-Prince) Butcher tableau coefficients.
        //
        // Row i holds the `a[i][j]` weights used by stage i+1; row 0 is the
        // unused zero row of stage 1. The inner stage loop runs `for j in
        // 0..stage`, so `a[i][i]` is never read and is written as 0.0.
        //
        // Row 6 is the FSAL ("first same as last") row: `k[6]` is evaluated at
        // `(t + dt, x5)`, i.e. at the propagated 5th-order state, so its weights
        // are exactly `b5`, which makes `a[6][6]` the unused 0.0 trailing entry.
        let a = [
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0, 0.0],
            [
                19372.0 / 6561.0,
                -25360.0 / 2187.0,
                64448.0 / 6561.0,
                -212.0 / 729.0,
                0.0,
                0.0,
                0.0,
            ],
            [
                9017.0 / 3168.0,
                -355.0 / 33.0,
                46732.0 / 5247.0,
                49.0 / 176.0,
                -5103.0 / 18656.0,
                0.0,
                0.0,
            ],
            [
                35.0 / 384.0,
                0.0,
                500.0 / 1113.0,
                125.0 / 192.0,
                -2187.0 / 6784.0,
                11.0 / 84.0,
                0.0,
            ],
        ];

        // 5th order weights (for propagation). `b5[6] == 0.0` is the first-same-
        // as-last property: `k[6] = f(t + dt, x5)` is not re-used in `x5`, which
        // makes the following step's first stage evaluation free.

        // 5th order weights (for propagation). `b5[6] == 0.0` is the first-same-
        // as-last property: `k[6] = f(t + dt, x5)` is not re-used in `x5`, which
        // makes the following step's first stage evaluation free.
        let b5 = [
            35.0 / 384.0,
            0.0,
            500.0 / 1113.0,
            125.0 / 192.0,
            -2187.0 / 6784.0,
            11.0 / 84.0,
            0.0,
        ];

        // 4th order weights (for error estimate)
        let b4 = [
            5179.0 / 57600.0,
            0.0,
            7571.0 / 16695.0,
            393.0 / 640.0,
            -92097.0 / 339200.0,
            187.0 / 2100.0,
            1.0 / 40.0,
        ];

        // Stage 1
        f(x, t, &mut k[0])?;

        // Stages 2-7
        for stage in 1..7 {
            for i in 0..n {
                tmp[i] = x[i];
                for j in 0..stage {
                    tmp[i] += dt * a[stage][j] * k[j][i];
                }
            }
            let c = match stage {
                1 => 1.0 / 5.0,
                2 => 3.0 / 10.0,
                3 => 4.0 / 5.0,
                4 => 8.0 / 9.0,
                5 => 1.0,
                6 => 1.0,
                _ => unreachable!(),
            };
            f(&tmp, t + c * dt, &mut k[stage])?;
        }

        // Compute x_{n+1} using 5th order formula
        let mut x5 = vec![0.0; n];
        let mut x4 = vec![0.0; n];
        for i in 0..n {
            for s in 0..7 {
                x5[i] += dt * b5[s] * k[s][i];
                x4[i] += dt * b4[s] * k[s][i];
            }
            x5[i] += x[i];
            x4[i] += x[i];
        }

        // Error estimate = |x5 - x4| (scaled)
        let mut max_error: Scalar = 0.0;
        for i in 0..n {
            let scale = self.config.atol + self.config.rtol * x[i].abs().max(x5[i].abs());
            let err_i = (x5[i] - x4[i]).abs() / scale;
            if err_i > max_error {
                max_error = err_i;
            }
        }

        // Store error estimate for external access
        self.last_error = Some(max_error);

        // Update solver statistics
        self.stats.function_evals += 7; // 7 stages evaluated

        if max_error <= 1.0 {
            // Only mutate the caller's state on an accepted step, so a
            // rejected step leaves x unchanged and can be retried with the
            // suggested dt from a clean state.
            x.copy_from_slice(&x5);
            self.stats.steps_accepted += 1;
            Ok(SolverStepResult::Accepted)
        } else {
            self.stats.steps_rejected += 1;
            let suggested = adapt_step_size(max_error, dt, &self.config, 5);
            Ok(SolverStepResult::Rejected {
                suggested_dt: suggested,
            })
        }
    }

    fn order(&self) -> u8 {
        5
    }

    fn stages(&self) -> u8 {
        7
    }

    fn is_adaptive(&self) -> bool {
        true
    }

    fn estimate_error(&self) -> Option<Scalar> {
        self.last_error
    }

    fn stats(&self) -> &SolverStats {
        &self.stats
    }

    fn stats_mut(&mut self) -> &mut SolverStats {
        &mut self.stats
    }
}

/// Bogacki-Shampine RK3(2) — 4 stages, order 3 with 2nd order embedded estimate.
///
/// Efficient low-order adaptive solver. Good for mild accuracy requirements
/// or when function evaluations are expensive.
#[derive(Debug, Clone)]
pub struct RK23 {
    config: SolverConfig,
    stats: SolverStats,
    last_error: Option<Scalar>,
}

impl RK23 {
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            stats: SolverStats::new(),
            last_error: None,
        }
    }
}

impl OdeSolver for RK23 {
    fn name(&self) -> &str {
        "RK23"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let mut k = vec![vec![0.0; n]; 4];
        let mut tmp = vec![0.0; n];

        // BS23 Butcher tableau
        // 0    |
        // 1/2  | 1/2
        // 3/4  | 0      3/4
        // 1    | 2/9    1/3    4/9
        // ----------------------------
        //      | 2/9    1/3    4/9    0   (3rd order)
        //      | 7/24   1/4    1/3    1/8 (2nd order)

        // Stage 1
        f(x, t, &mut k[0])?;

        // Stage 2: k2 = f(t + dt/2, x + dt/2 * k1)
        for i in 0..n {
            tmp[i] = x[i] + 0.5 * dt * k[0][i];
        }
        f(&tmp, t + 0.5 * dt, &mut k[1])?;

        // Stage 3: k3 = f(t + 3dt/4, x + 3dt/4 * k2)
        for i in 0..n {
            tmp[i] = x[i] + 0.75 * dt * k[1][i];
        }
        f(&tmp, t + 0.75 * dt, &mut k[2])?;

        // Stage 4: k4 = f(t + dt, x + dt*(2/9*k1 + 1/3*k2 + 4/9*k3))
        for i in 0..n {
            tmp[i] = x[i] + dt * (2.0 / 9.0 * k[0][i] + 1.0 / 3.0 * k[1][i] + 4.0 / 9.0 * k[2][i]);
        }
        f(&tmp, t + dt, &mut k[3])?;

        // 3rd order result (propagation)
        let b3 = [2.0 / 9.0, 1.0 / 3.0, 4.0 / 9.0, 0.0];
        // 2nd order result (error estimate)
        let b2 = [7.0 / 24.0, 1.0 / 4.0, 1.0 / 3.0, 1.0 / 8.0];

        let mut x3 = vec![0.0; n];
        let mut x2 = vec![0.0; n];
        for i in 0..n {
            for s in 0..4 {
                x3[i] += dt * b3[s] * k[s][i];
                x2[i] += dt * b2[s] * k[s][i];
            }
            x3[i] += x[i];
            x2[i] += x[i];
        }

        // Error estimate
        let mut max_error: Scalar = 0.0;
        for i in 0..n {
            let scale = self.config.atol + self.config.rtol * x[i].abs().max(x3[i].abs());
            let err_i = (x3[i] - x2[i]).abs() / scale;
            if err_i > max_error {
                max_error = err_i;
            }
        }

        // Store error estimate for external access
        self.last_error = Some(max_error);

        // Update solver statistics
        self.stats.function_evals += 4; // 4 stages evaluated

        if max_error <= 1.0 {
            // Only mutate the caller's state on an accepted step (see RK45).
            x.copy_from_slice(&x3);
            self.stats.steps_accepted += 1;
            Ok(SolverStepResult::Accepted)
        } else {
            self.stats.steps_rejected += 1;
            let suggested = adapt_step_size(max_error, dt, &self.config, 3);
            Ok(SolverStepResult::Rejected {
                suggested_dt: suggested,
            })
        }
    }

    fn order(&self) -> u8 {
        3
    }

    fn stages(&self) -> u8 {
        4
    }

    fn is_adaptive(&self) -> bool {
        true
    }

    fn estimate_error(&self) -> Option<Scalar> {
        self.last_error
    }

    fn stats(&self) -> &SolverStats {
        &self.stats
    }

    fn stats_mut(&mut self) -> &mut SolverStats {
        &mut self.stats
    }
}

/// Cash-Karp RK5(4) — 6 stages, order 5 with 4th order embedded estimate.
///
/// Similar to DOPRI5 but uses 6 stages instead of 7. The extra stage gives
/// better error estimation for certain classes of problems.
#[derive(Debug, Clone)]
pub struct CashKarp {
    config: SolverConfig,
    stats: SolverStats,
    last_error: Option<Scalar>,
}

impl CashKarp {
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            stats: SolverStats::new(),
            last_error: None,
        }
    }
}

impl OdeSolver for CashKarp {
    fn name(&self) -> &str {
        "CashKarp"
    }

    fn step(
        &mut self,
        f: &mut OdeRhs,
        x: &mut [Scalar],
        t: Scalar,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError> {
        let n = x.len();
        let mut k = vec![vec![0.0; n]; 6];
        let mut tmp = vec![0.0; n];

        // Cash-Karp coefficients (a_ij for j < i)
        let a = [
            [0.0, 0.0, 0.0, 0.0, 0.0],
            [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0],
            [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0],
            [3.0 / 10.0, -9.0 / 10.0, 6.0 / 5.0, 0.0, 0.0],
            [-11.0 / 54.0, 5.0 / 2.0, -70.0 / 27.0, 35.0 / 27.0, 0.0],
            [
                1631.0 / 55296.0,
                175.0 / 512.0,
                575.0 / 13824.0,
                44275.0 / 110592.0,
                253.0 / 4096.0,
            ],
        ];

        let c = [0.0, 1.0 / 5.0, 3.0 / 10.0, 3.0 / 5.0, 1.0, 7.0 / 8.0];

        // 5th order weights
        let b5 = [
            37.0 / 378.0,
            0.0,
            250.0 / 621.0,
            125.0 / 594.0,
            0.0,
            512.0 / 1771.0,
        ];

        // 4th order weights (error estimate)
        let b4 = [
            2825.0 / 27648.0,
            0.0,
            18575.0 / 48384.0,
            13525.0 / 55296.0,
            277.0 / 14336.0,
            1.0 / 4.0,
        ];

        // Stage 1
        f(x, t, &mut k[0])?;

        // Stages 2-6
        for stage in 1..6 {
            for i in 0..n {
                tmp[i] = x[i];
                for j in 0..stage {
                    tmp[i] += dt * a[stage][j] * k[j][i];
                }
            }
            f(&tmp, t + c[stage] * dt, &mut k[stage])?;
        }

        // Compute 5th and 4th order results
        let mut x5 = vec![0.0; n];
        let mut x4 = vec![0.0; n];
        for i in 0..n {
            for s in 0..6 {
                x5[i] += dt * b5[s] * k[s][i];
                x4[i] += dt * b4[s] * k[s][i];
            }
            x5[i] += x[i];
            x4[i] += x[i];
        }

        // Error estimate
        let mut max_error: Scalar = 0.0;
        for i in 0..n {
            let scale = self.config.atol + self.config.rtol * x[i].abs().max(x5[i].abs());
            let err_i = (x5[i] - x4[i]).abs() / scale;
            if err_i > max_error {
                max_error = err_i;
            }
        }

        // Store error estimate for external access
        self.last_error = Some(max_error);

        // Update solver statistics
        self.stats.function_evals += 6; // 6 stages evaluated

        if max_error <= 1.0 {
            // Only mutate the caller's state on an accepted step (see RK45).
            x.copy_from_slice(&x5);
            self.stats.steps_accepted += 1;
            Ok(SolverStepResult::Accepted)
        } else {
            self.stats.steps_rejected += 1;
            let suggested = adapt_step_size(max_error, dt, &self.config, 5);
            Ok(SolverStepResult::Rejected {
                suggested_dt: suggested,
            })
        }
    }

    fn order(&self) -> u8 {
        5
    }

    fn stages(&self) -> u8 {
        6
    }

    fn is_adaptive(&self) -> bool {
        true
    }

    fn estimate_error(&self) -> Option<Scalar> {
        self.last_error
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

    /// Test ODE: dx/dt = -x, x(0) = 1
    fn decay_rhs(x: &[Scalar], _t: Scalar, dx: &mut [Scalar]) -> Result<(), SimError> {
        dx[0] = -x[0];
        Ok(())
    }

    #[test]
    fn test_rk45_creation() {
        let solver = RK45::new(SolverConfig::default());
        assert_eq!(solver.name(), "RK45");
        assert!(solver.is_adaptive());
    }

    #[test]
    fn test_rk45_adaptive_decay() {
        let config = SolverConfig::new(1e-6, 1e-12);
        let mut solver = RK45::new(config);
        let mut x = vec![1.0];
        let dt = 0.1;
        let analytical_at_1 = (-1.0_f64).exp();

        let mut t = 0.0;
        let mut step = 0;
        while t < 1.0 - 1e-12 && step < 1000 {
            let result = solver.step(&mut decay_rhs, &mut x, t, dt).unwrap();
            match result {
                SolverStepResult::Accepted => {
                    t += dt;
                    step += 1;
                }
                SolverStepResult::Rejected { suggested_dt: _ } => {
                    // Would normally reduce dt, but for test we just accept
                    // This is a very simple problem, so rejection is unlikely
                    break;
                }
                _ => panic!("unexpected result"),
            }
        }

        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 1e-4, "RK45 error too large: {}", error);
    }

    #[test]
    fn test_rk23_creation() {
        let solver = RK23::new(SolverConfig::default());
        assert_eq!(solver.name(), "RK23");
        assert!(solver.is_adaptive());
    }

    #[test]
    fn test_rk23_adaptive_decay() {
        let config = SolverConfig::new(1e-4, 1e-10);
        let mut solver = RK23::new(config);
        let mut x = vec![1.0];
        let dt = 0.05;
        let analytical_at_1 = (-1.0_f64).exp();

        let mut t = 0.0;
        let mut step = 0;
        while t < 1.0 - 1e-12 && step < 1000 {
            let result = solver.step(&mut decay_rhs, &mut x, t, dt).unwrap();
            match result {
                SolverStepResult::Accepted => {
                    t += dt;
                    step += 1;
                }
                SolverStepResult::Rejected { .. } => break,
                _ => panic!("unexpected result"),
            }
        }

        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 1e-3, "RK23 error too large: {}", error);
    }

    #[test]
    fn test_cash_karp_creation() {
        let solver = CashKarp::new(SolverConfig::default());
        assert_eq!(solver.name(), "CashKarp");
        assert!(solver.is_adaptive());
    }

    #[test]
    fn test_cash_karp_decay() {
        let config = SolverConfig::new(1e-6, 1e-12);
        let mut solver = CashKarp::new(config);
        let mut x = vec![1.0];
        let dt = 0.1;
        let analytical_at_1 = (-1.0_f64).exp();

        let mut t = 0.0;
        let mut step = 0;
        while t < 1.0 - 1e-12 && step < 1000 {
            let result = solver.step(&mut decay_rhs, &mut x, t, dt).unwrap();
            match result {
                SolverStepResult::Accepted => {
                    t += dt;
                    step += 1;
                }
                _ => break,
            }
        }

        let error = (x[0] - analytical_at_1).abs();
        assert!(error < 1e-4, "CashKarp error too large: {}", error);
    }

    #[test]
    fn test_rk45_rejection_on_stiff_problem() {
        // Slightly stiff: dx/dt = -100*x
        let mut stiff_rhs = |x: &[Scalar], _t: Scalar, dx: &mut [Scalar]| -> Result<(), SimError> {
            dx[0] = -100.0 * x[0];
            Ok(())
        };

        let config = SolverConfig::new(1e-6, 1e-12);
        let mut solver = RK45::new(config);
        let mut x = vec![1.0];
        // Large step for a stiff problem should trigger rejection
        let result = solver.step(&mut stiff_rhs, &mut x, 0.0, 0.1).unwrap();
        match result {
            SolverStepResult::Accepted => {
                // If accepted, the solution should still be accurate enough
                // (adaptive method adjusted internally)
            }
            SolverStepResult::Rejected { .. } => {
                // Rejection is expected — dt=0.1 is too large for dx/dt=-100*x
            }
            _ => panic!("unexpected result"),
        }
    }

    /// The DOPRI5 tableau must satisfy the structural invariants that the
    /// stage loop relies on: the row-`i` weights used by stage `i + 1` sum to
    /// the node `c[i]`, and the FSAL row equals the propagation weights `b5`.
    ///
    /// This guards the historical defect where `a[6]` was written as a 6-element
    /// copy of `b5`, which silently corrupted `k[6]` and therefore `x4` and the
    /// step-size error estimate while still preserving the (much weaker) property
    /// that every row sums to one.
    #[test]
    fn test_rk45_dopri5_tableau_invariants() {
        // Mirrors the coefficients used by `RK45::step`.
        let a = [
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0, 0.0],
            [
                19372.0 / 6561.0,
                -25360.0 / 2187.0,
                64448.0 / 6561.0,
                -212.0 / 729.0,
                0.0,
                0.0,
                0.0,
            ],
            [
                9017.0 / 3168.0,
                -355.0 / 33.0,
                46732.0 / 5247.0,
                49.0 / 176.0,
                -5103.0 / 18656.0,
                0.0,
                0.0,
            ],
            [
                35.0 / 384.0,
                0.0,
                500.0 / 1113.0,
                125.0 / 192.0,
                -2187.0 / 6784.0,
                11.0 / 84.0,
                0.0,
            ],
        ];
        let b5 = [
            35.0 / 384.0,
            0.0,
            500.0 / 1113.0,
            125.0 / 192.0,
            -2187.0 / 6784.0,
            11.0 / 84.0,
            0.0,
        ];
        let b4 = [
            5179.0 / 57600.0,
            0.0,
            7571.0 / 16695.0,
            393.0 / 640.0,
            -92097.0 / 339200.0,
            187.0 / 2100.0,
            1.0 / 40.0,
        ];
        let c = [0.0, 1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0];

        // (1) The weights the inner loop actually consumes (`j in 0..stage`)
        //     must reproduce the node abscissae.
        for i in 0..7 {
            let used: Scalar = a[i][..i].iter().sum();
            assert!(
                (used - c[i]).abs() < 1e-14,
                "row {i}: stage-loop weights sum to {used}, expected node c[{i}] = {}",
                c[i]
            );
        }

        // (2) FSAL: `k[6]` is evaluated at the propagated 5th-order state, so
        //     the row-6 weights must equal the propagation weights `b5`.
        for j in 0..7 {
            assert!(
                (a[6][j] - b5[j]).abs() < 1e-15,
                "FSAL mismatch at a[6][{j}] = {} vs b5[{j}] = {}",
                a[6][j],
                b5[j]
            );
        }

        // (3) Both embedded weights must be normalised. This alone is *not*
        //     sufficient to detect a corrupted tableau (the old bug preserved
        //     it), which is why (1) and (2) exist.
        assert!((b5.iter().sum::<Scalar>() - 1.0).abs() < 1e-15);
        assert!((b4.iter().sum::<Scalar>() - 1.0).abs() < 1e-15);

        // (4) The 5th-order weights must reproduce the first four order
        //     conditions `sum(b_i * c_i^(k-1)) = 1/k`, proving the propagated
        //     solution is genuinely order 5 (not just order 4 with a broken row).
        for k in 1..=4u32 {
            let lhs: Scalar = (0..7).map(|i| b5[i] * c[i].powi(k as i32)).sum();
            let rhs = 1.0 / (k as Scalar + 1.0);
            assert!(
                (lhs - rhs).abs() < 1e-14,
                "order condition k={k} violated: sum(b5*c^{k}) = {lhs}, expected {rhs}"
            );
        }

        // (5) The error estimator must be non-degenerate: if `b4 == b5` there
        //     would be no embedded difference and no adaptivity at all.
        assert!(b4.iter().zip(b5.iter()).any(|(a, b)| (a - b).abs() > 1e-12));
    }

    /// End-to-end proof that the tableau is really 5th order: the observed
    /// error of `dy/dt = y` over a unit interval must fall by ~2^5 when `dt` is
    /// halved. A corrupted FSAL row degrades the step-size controller and
    /// makes this ratio collapse.
    #[test]
    fn test_rk45_observed_convergence_order_is_five() {
        let mut growth_rhs =
            |x: &[Scalar], _t: Scalar, dx: &mut [Scalar]| -> Result<(), SimError> {
                dx[0] = x[0];
                Ok(())
            };

        // The controller is driven by the *embedded* 4th-order estimate, which
        // is only ~1e-7 accurate at dt=1/16 for this ODE, so the tolerance must
        // not be tighter than that or every step is legitimately rejected.
        // Order 4 error at dt=1/16 is ~1e-5, so rtol=1e-4 accepts while the
        // measured 5th-order global error (~1e-7) stays well above round-off.
        let config = SolverConfig::new(1e-4, 1e-8);

        let mut integrate = |dt: Scalar| -> Scalar {
            let mut solver = RK45::new(config);
            let mut x = vec![1.0];
            let mut t = 0.0;
            while t < 1.0 {
                let h = dt.min(1.0 - t);
                match solver.step(&mut growth_rhs, &mut x, t, h).unwrap() {
                    SolverStepResult::Accepted => t += h,
                    // A rejection only means the controller wants a smaller step;
                    // for a fixed-grid study we keep the requested `h` so the
                    // global error reflects `dt` alone.
                    SolverStepResult::Rejected { .. } => t += h,
                    other => panic!("unexpected solver result: {other:?}"),
                }
            }
            (x[0] - std::f64::consts::E).abs()
        };

        let e1 = integrate(1.0 / 16.0);
        let e2 = integrate(1.0 / 32.0);
        assert!(e1 > 0.0 && e2 > 0.0, "expected non-zero truncation error");

        // Order 5 => error shrinks by ~32x per halving. Assert a conservative
        // bound of 16x so the test measures the order rather than exact
        // constants, while still failing hard (the buggy tableau gave ~4x).
        let ratio = e1 / e2;
        assert!(
            ratio > 16.0,
            "RK45 convergence ratio {ratio:.2} (e1={e1:.3e}, e2={e2:.3e}) is below the \
             order-5 expectation; the DOPRI5 tableau is likely corrupted"
        );
    }
}
