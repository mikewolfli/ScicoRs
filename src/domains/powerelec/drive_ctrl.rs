// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Drive control: PI controller, FOC, efficiency analysis.

use crate::core::types::Scalar;

/// PI controller with anti-windup.
#[derive(Debug, Clone)]
pub struct PiController {
    pub kp: Scalar,
    pub ki: Scalar,
    pub integral: Scalar,
    pub output_min: Scalar,
    pub output_max: Scalar,
}

impl PiController {
    pub fn new(kp: Scalar, ki: Scalar, min: Scalar, max: Scalar) -> Self {
        Self {
            kp,
            ki,
            integral: 0.0,
            output_min: min,
            output_max: max,
        }
    }

    pub fn update(&mut self, error: Scalar, dt: Scalar) -> Scalar {
        if dt <= 0.0 {
            return 0.0;
        }
        self.integral += error * dt;
        self.integral = self.integral.clamp(
            self.output_min / self.ki.max(1e-10),
            self.output_max / self.ki.max(1e-10),
        );
        let output = self.kp * error + self.ki * self.integral;
        output.clamp(self.output_min, self.output_max)
    }

    pub fn reset(&mut self) {
        self.integral = 0.0;
    }
}

/// Field-Oriented Control for PMSM.
#[derive(Debug, Clone)]
pub struct FocController {
    pub asr: PiController,
    pub acr_d: PiController,
    pub acr_q: PiController,
}

impl FocController {
    pub fn new(asr: PiController, acr_d: PiController, acr_q: PiController) -> Self {
        Self { asr, acr_d, acr_q }
    }

    pub fn update(
        &mut self,
        omega_ref: Scalar,
        omega: Scalar,
        i_d: Scalar,
        i_q: Scalar,
        _theta_e: Scalar,
        dt: Scalar,
    ) -> (Scalar, Scalar) {
        let i_q_ref = self.asr.update(omega_ref - omega, dt);
        let v_d = self.acr_d.update(0.0 - i_d, dt);
        let v_q = self.acr_q.update(i_q_ref - i_q, dt);
        (v_d, v_q)
    }

    /// Inverse Park transform: (v_d, v_q) → (v_α, v_β).
    pub fn inv_park_transform(v_d: Scalar, v_q: Scalar, theta: Scalar) -> (Scalar, Scalar) {
        let ct = f64::cos(theta);
        let st = f64::sin(theta);
        (v_d * ct - v_q * st, v_d * st + v_q * ct)
    }

    /// Space Vector PWM (simplified).
    pub fn svpwm(v_alpha: Scalar, v_beta: Scalar, v_dc: Scalar) -> [Scalar; 3] {
        if v_dc <= 0.0 {
            return [0.0, 0.0, 0.0];
        }
        let t1 = 0.5 + v_alpha / v_dc;
        let t2 = 0.5 + (f64::sqrt(3.0) * v_alpha - v_beta) / (2.0 * v_dc);
        let t3 = 0.5 + (-f64::sqrt(3.0) * v_alpha - v_beta) / (2.0 * v_dc);
        [t1.clamp(0.0, 1.0), t2.clamp(0.0, 1.0), t3.clamp(0.0, 1.0)]
    }
}

/// Drive system efficiency: the fraction of input power delivered as output.
///
/// `input_power` is the electrical power drawn from the bus and is the
/// denominator. `motor_loss` and `converter_loss` are the powers dissipated in
/// the machine and the converter; they are subtracted from the input to give the
/// useful output, so the losses genuinely reduce the reported efficiency:
///
/// ```text
/// η = (input_power − motor_loss − converter_loss) / input_power
/// ```
///
/// `output_power` is accepted for callers that also measure the shaft power and
/// is used to cross-check the loss model: when it is positive and the losses are
/// not given, the losses are inferred as `input_power − output_power`. This keeps
/// the result consistent whether a caller supplies the measured losses or the
/// measured output.
///
/// The result is clamped to `[0, 1]`. Returns `0.0` for a non-positive or
/// non-finite input power, or when the losses exceed the input.
pub fn drive_efficiency(
    input_power: Scalar,
    output_power: Scalar,
    motor_loss: Scalar,
    converter_loss: Scalar,
) -> Scalar {
    if !input_power.is_finite() || input_power <= 0.0 {
        return 0.0;
    }
    let declared = motor_loss.max(0.0) + converter_loss.max(0.0);
    // Fall back to the measured output only when no loss figures were supplied;
    // otherwise the two would be double-counted.
    let total_loss = if declared > 0.0 {
        declared
    } else if output_power.is_finite() && output_power > 0.0 {
        (input_power - output_power).max(0.0)
    } else {
        0.0
    };
    ((input_power - total_loss) / input_power).clamp(0.0, 1.0)
}

/// Torque-speed curve for a motor type.
///
/// Linear DC-motor model: `T(ω) = T_stall · (1 − ω/ω_max)`. `params` is
/// `[stall_torque (N·m), no_load_speed (rad/s)]`; the no-load speed scales
/// with the bus voltage `v_dc` (relative to a 100 V reference).
pub fn torque_speed_curve(
    _motor_type: &str,
    params: &[Scalar],
    v_dc: Scalar,
) -> Vec<(Scalar, Scalar)> {
    let stall = params.first().copied().unwrap_or(10.0).max(0.0);
    let no_load = params.get(1).copied().unwrap_or(300.0).max(0.0);
    let speed_scale = if v_dc > 0.0 {
        (v_dc / 100.0).max(0.0)
    } else {
        1.0
    };
    let omega_max = (no_load * speed_scale).max(1e-6);
    (0..=10)
        .map(|i| {
            let omega = omega_max * i as Scalar / 10.0;
            let torque = stall * (1.0 - omega / omega_max).max(0.0);
            (omega, torque)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pi_controller_step() {
        let mut pi = PiController::new(1.0, 0.1, -10.0, 10.0);
        let out = pi.update(1.0, 0.01);
        assert!(out > 0.0);
    }

    #[test]
    fn test_pi_controller_reset() {
        let mut pi = PiController::new(1.0, 0.1, -10.0, 10.0);
        pi.update(1.0, 0.01);
        pi.reset();
        assert!((pi.integral - 0.0).abs() < 1e-15);
    }

    #[test]
    fn test_inv_park_transform() {
        let (va, vb) = FocController::inv_park_transform(1.0, 0.0, 0.0);
        assert!((va - 1.0).abs() < 1e-10);
        assert!((vb - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_svpwm() {
        let pwm = FocController::svpwm(100.0, 0.0, 400.0);
        assert!(pwm[0] > 0.0 && pwm[0] < 1.0);
    }

    #[test]
    fn test_drive_efficiency_includes_losses() {
        // 1000 W drawn from the bus with 50 W motor loss and 30 W converter
        // loss → 92% reaches the shaft. Before the fix this returned
        // output/input = 0.9 while ignoring the loss arguments entirely.
        let eta = drive_efficiency(1000.0, 900.0, 50.0, 30.0);
        assert!(
            (eta - 0.92).abs() < 1e-9,
            "expected 0.92 (losses reduce efficiency), got {eta}"
        );
    }

    #[test]
    fn test_drive_efficiency_infers_losses_from_measured_output() {
        // With no loss figures supplied, the losses are inferred from the gap
        // between input and output.
        let eta = drive_efficiency(1000.0, 850.0, 0.0, 0.0);
        assert!((eta - 0.85).abs() < 1e-9, "expected 0.85, got {eta}");
    }

    #[test]
    fn test_drive_efficiency_loss_free_is_unity() {
        let eta = drive_efficiency(1000.0, 1000.0, 0.0, 0.0);
        assert!((eta - 1.0).abs() < 1e-12, "expected 1.0, got {eta}");
    }

    #[test]
    fn test_drive_efficiency_losses_reduce_efficiency() {
        // More loss must lower efficiency; this is exactly what the discarded
        // loss term previously prevented.
        let low_loss = drive_efficiency(1000.0, 0.0, 10.0, 10.0);
        let high_loss = drive_efficiency(1000.0, 0.0, 100.0, 100.0);
        assert!(
            high_loss < low_loss,
            "{high_loss} should be below {low_loss}"
        );
        assert!((low_loss - 0.98).abs() < 1e-9);
        assert!((high_loss - 0.8).abs() < 1e-9);
    }

    #[test]
    fn test_drive_efficiency_degenerate_inputs() {
        assert_eq!(drive_efficiency(0.0, 0.0, 0.0, 0.0), 0.0);
        assert_eq!(drive_efficiency(-5.0, 0.0, 0.0, 0.0), 0.0);
        assert_eq!(drive_efficiency(Scalar::NAN, 1.0, 0.0, 0.0), 0.0);
        // Losses exceeding the input cannot yield a negative efficiency.
        assert_eq!(drive_efficiency(100.0, 0.0, 500.0, 500.0), 0.0);
        // Negative loss arguments are not physical and are clamped away.
        assert_eq!(drive_efficiency(100.0, 0.0, -50.0, -50.0), 1.0);
    }
}
