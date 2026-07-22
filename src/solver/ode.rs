//! Ordinary Differential Equation (ODE) solvers.
//!
//! Provides fixed-step and adaptive-step integrators for solving
//! initial value problems of the form dy/dt = f(t, y).

use crate::core::types::Scalar;

/// The derivative function type: f(t, y) -> dy/dt
pub type OdeFunction = fn(Scalar, &[Scalar], &mut [Scalar]);

/// Common settings for ODE solvers.
#[derive(Debug, Clone)]
pub struct OdeSolverConfig {
    /// Initial step size.
    pub dt: Scalar,
    /// Relative tolerance (adaptive solvers).
    pub rtol: Scalar,
    /// Absolute tolerance (adaptive solvers).
    pub atol: Scalar,
    /// Minimum step size.
    pub dt_min: Scalar,
    /// Maximum step size.
    pub dt_max: Scalar,
    /// Maximum number of steps.
    pub max_steps: usize,
}

impl Default for OdeSolverConfig {
    fn default() -> Self {
        Self {
            dt: 1e-3,
            rtol: 1e-6,
            atol: 1e-8,
            dt_min: 1e-12,
            dt_max: 1.0,
            max_steps: 100_000,
        }
    }
}

/// Result of a single solver step.
#[derive(Debug, Clone)]
pub struct OdeStepResult {
    /// New time after the step.
    pub t: Scalar,
    /// State vector after the step.
    pub y: Vec<Scalar>,
    /// Estimated error (adaptive solvers).
    pub error: Scalar,
    /// Whether the step was accepted.
    pub accepted: bool,
    /// Suggested step size for next step.
    pub dt_next: Scalar,
}

/// A trait for ODE solver implementations.
pub trait OdeSolver: Send {
    /// Get the solver name.
    fn name(&self) -> &str;

    /// Perform a single integration step.
    fn step(&self, f: OdeFunction, t: Scalar, y: &[Scalar], dt: Scalar) -> OdeStepResult;

    /// Solve from t0 to t_end, storing results at each step.
    fn solve(&self, f: OdeFunction, t0: Scalar, t_end: Scalar, y0: &[Scalar]) -> Vec<OdeStepResult> {
        let mut results = Vec::new();
        let mut t = t0;
        let mut y = y0.to_vec();
        let mut dt = self.config().dt;

        results.push(OdeStepResult {
            t,
            y: y.clone(),
            error: 0.0,
            accepted: true,
            dt_next: dt,
        });

        while t < t_end {
            dt = dt.min(t_end - t);
            let result = self.step(f, t, &y, dt);
            if result.accepted {
                t = result.t;
                y = result.y.clone();
                dt = result.dt_next;
                results.push(result);
            } else {
                dt *= 0.5;
                if dt < self.config().dt_min {
                    break;
                }
            }
        }

        results
    }

    /// Get the solver configuration.
    fn config(&self) -> &OdeSolverConfig;
}

/// Forward Euler solver (first-order explicit).
#[derive(Debug, Clone)]
pub struct EulerSolver {
    pub config: OdeSolverConfig,
}

impl EulerSolver {
    pub fn new(dt: Scalar) -> Self {
        Self {
            config: OdeSolverConfig { dt, ..Default::default() },
        }
    }
}

impl OdeSolver for EulerSolver {
    fn name(&self) -> &str {
        "Forward Euler"
    }

    fn step(&self, f: OdeFunction, t: Scalar, y: &[Scalar], dt: Scalar) -> OdeStepResult {
        let mut k1 = vec![0.0; y.len()];
        f(t, y, &mut k1);

        let y_new: Vec<Scalar> = y.iter().zip(k1.iter()).map(|(yi, ki)| yi + dt * ki).collect();

        OdeStepResult {
            t: t + dt,
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

/// Classical Runge-Kutta 4th order solver (RK4).
#[derive(Debug, Clone)]
pub struct RK4Solver {
    pub config: OdeSolverConfig,
}

impl RK4Solver {
    pub fn new(dt: Scalar) -> Self {
        Self {
            config: OdeSolverConfig { dt, ..Default::default() },
        }
    }
}

impl OdeSolver for RK4Solver {
    fn name(&self) -> &str {
        "RK4"
    }

    fn step(&self, f: OdeFunction, t: Scalar, y: &[Scalar], dt: Scalar) -> OdeStepResult {
        let n = y.len();
        let mut k1 = vec![0.0; n];
        let mut k2 = vec![0.0; n];
        let mut k3 = vec![0.0; n];
        let mut k4 = vec![0.0; n];
        let mut tmp = vec![0.0; n];

        f(t, y, &mut k1);

        for i in 0..n {
            tmp[i] = y[i] + 0.5 * dt * k1[i];
        }
        f(t + 0.5 * dt, &tmp, &mut k2);

        for i in 0..n {
            tmp[i] = y[i] + 0.5 * dt * k2[i];
        }
        f(t + 0.5 * dt, &tmp, &mut k3);

        for i in 0..n {
            tmp[i] = y[i] + dt * k3[i];
        }
        f(t + dt, &tmp, &mut k4);

        let y_new: Vec<Scalar> = y
            .iter()
            .zip(k1.iter())
            .zip(k2.iter())
            .zip(k3.iter())
            .zip(k4.iter())
            .map(|((((&yi, &k1i), &k2i), &k3i), &k4i)| {
                yi + (dt / 6.0) * (k1i + 2.0 * k2i + 2.0 * k3i + k4i)
            })
            .collect();

        OdeStepResult {
            t: t + dt,
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

/// Adaptive RK45 (Runge-Kutta-Fehlberg / Dormand-Prince) solver.
#[derive(Debug, Clone)]
pub struct RK45Solver {
    pub config: OdeSolverConfig,
}

impl RK45Solver {
    pub fn new(config: OdeSolverConfig) -> Self {
        Self { config }
    }
}

impl OdeSolver for RK45Solver {
    fn name(&self) -> &str {
        "RK45 (Dormand-Prince)"
    }

    fn step(&self, f: OdeFunction, t: Scalar, y: &[Scalar], dt: Scalar) -> OdeStepResult {
        // Dormand-Prince coefficients (RK5(4)7M)
        const A: &[Scalar] = &[
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0,
            44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0,
            19372.0 / 6561.0, -25360.0 / 2187.0, 64448.0 / 6561.0, -212.0 / 729.0, 0.0, 0.0,
            9017.0 / 3168.0, -355.0 / 33.0, 46732.0 / 5247.0, 49.0 / 176.0, -5103.0 / 18656.0, 0.0,
            35.0 / 384.0, 0.0, 500.0 / 1113.0, 125.0 / 192.0, -2187.0 / 6784.0, 11.0 / 84.0,
        ];
        // 4th-order error coefficients
        const E: &[Scalar] = &[
            71.0 / 57600.0, 0.0, -71.0 / 16695.0, 71.0 / 1920.0, -17253.0 / 339200.0, 22.0 / 525.0, -1.0 / 40.0,
        ];

        let n = y.len();
        let stages = 7;
        let mut k = vec![vec![0.0; n]; stages];
        let mut tmp = vec![0.0; n];

        for s in 0..stages {
            for i in 0..n {
                tmp[i] = y[i];
                for j in 0..s {
                    tmp[i] += dt * A[s * 6 + j] * k[j][i];
                }
            }
            let t_s = t + [0.0, 1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0][s];
            f(t_s, &tmp, &mut k[s]);
        }

        // 5th-order solution
        let c5: &[Scalar] = &[35.0 / 384.0, 0.0, 500.0 / 1113.0, 125.0 / 192.0, -2187.0 / 6784.0, 11.0 / 84.0, 0.0];
        let mut y5 = vec![0.0; n];
        for i in 0..n {
            y5[i] = y[i];
            for s in 0..stages {
                y5[i] += dt * c5[s] * k[s][i];
            }
        }

        // Error estimate
        let mut max_error = 0.0;
        for i in 0..n {
            let mut err = 0.0;
            for s in 0..stages {
                err += dt * E[s] * k[s][i];
            }
            let scale = self.config.atol + self.config.rtol * y5[i].abs();
            let err_rel = err.abs() / scale;
            if err_rel > max_error {
                max_error = err_rel;
            }
        }

        let accepted = max_error <= 1.0;
        let dt_factor = if max_error < 1e-12 {
            2.0
        } else {
            0.9 * (1.0 / max_error).powf(1.0 / 5.0)
        };
        let dt_next = (dt * dt_factor).clamp(self.config.dt_min, self.config.dt_max);

        OdeStepResult {
            t: t + dt,
            y: y5,
            error: max_error,
            accepted,
            dt_next,
        }
    }

    fn config(&self) -> &OdeSolverConfig {
        &self.config
    }
}
