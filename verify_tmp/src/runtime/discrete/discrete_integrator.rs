//! Discrete-time integrators for fixed-step simulation.
//!
//! Provides a `DiscreteIntegrator` with three integration methods:
//! Forward Euler, Backward Euler, and Trapezoidal (Tustin's method).

use crate::core::types::Scalar;

/// Numerical integration method for the discrete-time integrator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntegrationMethod {
    /// Forward (explicit) Euler: `y[n+1] = y[n] + dt * u[n]`
    ForwardEuler,
    /// Backward (implicit) Euler: `y[n+1] = y[n] + dt * u[n+1]`
    BackwardEuler,
    /// Trapezoidal (Tustin): `y[n+1] = y[n] + dt/2 * (u[n] + u[n+1])`
    Trapezoidal,
}

/// A discrete-time integrator block with configurable method and clamping.
///
/// # State update formulas
///
/// | Method          | Formula                                              |
/// |-----------------|------------------------------------------------------|
/// | ForwardEuler    | `x' = x + dt * u`                                    |
/// | BackwardEuler   | `x' = x + dt * u_next`                               |
/// | Trapezoidal     | `x' = x + dt/2 * (u + u_next)`                       |
///
/// The integrator supports optional output limits, and a reset-on-zero
/// mode that resets the state to the initial value when `input ≈ 0`.
#[derive(Debug, Clone)]
pub struct DiscreteIntegrator {
    /// Integration method to use.
    pub method: IntegrationMethod,
    /// Current integrator state (output).
    pub state: Scalar,
    /// Step size in seconds.
    pub dt: Scalar,
    /// Initial state value at reset / simulation start.
    pub initial: Scalar,
    /// Lower output limit (`None` = no limit).
    pub limit_min: Option<Scalar>,
    /// Upper output limit (`None` = no limit).
    pub limit_max: Option<Scalar>,
    /// When `true`, the state is reset to `initial` whenever `input ≈ 0`.
    reset_on_zero: bool,
}

impl DiscreteIntegrator {
    /// Create a new discrete integrator.
    ///
    /// # Panics
    /// Panics if `dt` is not positive.
    pub fn new(method: IntegrationMethod, dt: Scalar, initial: Scalar) -> Self {
        assert!(dt > 0.0, "DiscreteIntegrator: dt must be positive, got {dt}");
        Self {
            method,
            state: initial,
            dt,
            initial,
            limit_min: None,
            limit_max: None,
            reset_on_zero: false,
        }
    }

    /// Advance the integrator by one time step.
    ///
    /// `input` is `u[n]` (current input).  `input_next` is `u[n+1]` and is
    /// **required** for BackwardEuler and Trapezoidal; it may be `None`
    /// for ForwardEuler (in which case only the current input is used).
    ///
    /// Returns the new integrator output `x[n+1]`.
    pub fn step(&mut self, input: Scalar, input_next: Option<Scalar>) -> Scalar {
        // Check zero-crossing reset.
        if self.reset_on_zero && input.abs() < 1e-12 {
            self.state = self.initial;
            return self.state;
        }

        let dt = self.dt;
        let next = match self.method {
            IntegrationMethod::ForwardEuler => {
                self.state + dt * input
            }
            IntegrationMethod::BackwardEuler => {
                let u_next = input_next.unwrap_or(input);
                self.state + dt * u_next
            }
            IntegrationMethod::Trapezoidal => {
                let u_next = input_next.unwrap_or(input);
                self.state + 0.5 * dt * (input + u_next)
            }
        };

        self.state = self.clamp(next);
        self.state
    }

    /// Return the current output (state) without advancing.
    pub fn output(&self) -> Scalar {
        self.state
    }

    /// Reset the integrator state to its initial value.
    pub fn reset(&mut self) {
        self.state = self.initial;
    }

    /// Enable or disable output clamping.
    pub fn set_limits(&mut self, min: Scalar, max: Scalar) {
        self.limit_min = Some(min);
        self.limit_max = Some(max);
    }

    /// Enable or disable the zero-input reset feature.
    pub fn set_reset_on_zero(&mut self, enabled: bool) {
        self.reset_on_zero = enabled;
    }

    /// Builder-pattern method to set output limits.
    pub fn with_limits(mut self, min: Scalar, max: Scalar) -> Self {
        self.limit_min = Some(min);
        self.limit_max = Some(max);
        self
    }

    /// Clamp a value to the configured limits.
    fn clamp(&self, value: Scalar) -> Scalar {
        let mut v = value;
        if let Some(min) = self.limit_min && v < min {
            v = min;
        }
        if let Some(max) = self.limit_max && v > max {
            v = max;
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Scalar, b: Scalar) {
        assert!(
            (a - b).abs() < 1e-10,
            "expected {b}, got {a}"
        );
    }

    #[test]
    fn forward_euler_step() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::ForwardEuler, 0.1, 0.0);
        // x' = 0 + 0.1 * 2.0 = 0.2
        let y = int.step(2.0, None);
        approx_eq(y, 0.2);
        // x' = 0.2 + 0.1 * 3.0 = 0.5
        let y = int.step(3.0, None);
        approx_eq(y, 0.5);
    }

    #[test]
    fn backward_euler_step() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::BackwardEuler, 0.1, 0.0);
        // x' = 0 + 0.1 * 5.0 = 0.5
        let y = int.step(1.0, Some(5.0));
        approx_eq(y, 0.5);
    }

    #[test]
    fn backward_euler_falls_back_to_current() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::BackwardEuler, 0.1, 0.0);
        // When input_next is None, uses current input.
        let y = int.step(3.0, None);
        approx_eq(y, 0.3);
    }

    #[test]
    fn trapezoidal_step() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::Trapezoidal, 0.1, 0.0);
        // x' = 0 + 0.05 * (1.0 + 3.0) = 0.2
        let y = int.step(1.0, Some(3.0));
        approx_eq(y, 0.2);
    }

    #[test]
    fn trapezoidal_falls_back_to_current() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::Trapezoidal, 0.1, 0.0);
        let y = int.step(2.0, None);
        approx_eq(y, 0.2); // 0 + 0.05 * (2.0 + 2.0)
    }

    #[test]
    fn reset() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::ForwardEuler, 0.1, 0.0);
        int.step(5.0, None);
        assert!(int.output() > 0.0);
        int.reset();
        approx_eq(int.output(), 0.0);
    }

    #[test]
    fn limit_clamping() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::ForwardEuler, 0.1, 0.0)
            .with_limits(-1.0, 1.0);
        // Large input would push state past 1.0; clamped.
        let y = int.step(100.0, None);
        approx_eq(y, 1.0);
        // Negative large input clamped to -1.0.
        let y = int.step(-200.0, None);
        approx_eq(y, -1.0);
    }

    #[test]
    fn zero_reset() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::ForwardEuler, 0.1, 5.0);
        int.set_reset_on_zero(true);
        int.step(3.0, None);
        assert!(int.output() > 5.0);
        // Input ≈ 0 triggers reset.
        int.step(0.0, None);
        approx_eq(int.output(), 5.0);
    }

    #[test]
    fn step_without_input_next() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::ForwardEuler, 0.5, 10.0);
        let y = int.step(2.0, None);
        approx_eq(y, 11.0);
    }

    #[test]
    fn output_method() {
        let int = DiscreteIntegrator::new(IntegrationMethod::Trapezoidal, 0.01, std::f64::consts::PI);
        approx_eq(int.output(), std::f64::consts::PI);
    }

    #[test]
    fn set_limits_api() {
        let mut int = DiscreteIntegrator::new(IntegrationMethod::ForwardEuler, 0.1, 0.0);
        int.set_limits(-5.0, 5.0);
        assert_eq!(int.limit_min, Some(-5.0));
        assert_eq!(int.limit_max, Some(5.0));
    }

    #[test]
    #[should_panic(expected = "dt must be positive")]
    fn zero_dt_panics() {
        DiscreteIntegrator::new(IntegrationMethod::ForwardEuler, 0.0, 0.0);
    }
}
