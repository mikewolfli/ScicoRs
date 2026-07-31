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
        Self { kp, ki, integral: 0.0, output_min: min, output_max: max }
    }

    pub fn update(&mut self, error: Scalar, dt: Scalar) -> Scalar {
        if dt <= 0.0 { return 0.0; }
        self.integral += error * dt;
        self.integral = self.integral.clamp(self.output_min / self.ki.max(1e-10), self.output_max / self.ki.max(1e-10));
        let output = self.kp * error + self.ki * self.integral;
        output.clamp(self.output_min, self.output_max)
    }

    pub fn reset(&mut self) { self.integral = 0.0; }
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

    pub fn update(&mut self, omega_ref: Scalar, omega: Scalar, i_d: Scalar,
                  i_q: Scalar, _theta_e: Scalar, dt: Scalar) -> (Scalar, Scalar) {
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
        if v_dc <= 0.0 { return [0.0, 0.0, 0.0]; }
        let t1 = 0.5 + v_alpha / v_dc;
        let t2 = 0.5 + (f64::sqrt(3.0) * v_alpha - v_beta) / (2.0 * v_dc);
        let t3 = 0.5 + (-f64::sqrt(3.0) * v_alpha - v_beta) / (2.0 * v_dc);
        [t1.clamp(0.0, 1.0), t2.clamp(0.0, 1.0), t3.clamp(0.0, 1.0)]
    }
}

/// Drive system efficiency.
pub fn drive_efficiency(input_power: Scalar, output_power: Scalar, motor_loss: Scalar, converter_loss: Scalar) -> Scalar {
    let _total_loss = motor_loss + converter_loss;
    let total_in = input_power;
    if total_in <= 0.0 { return 0.0; }
    output_power / total_in
}

/// Torque-speed curve for a motor type.
///
/// Linear DC-motor model: `T(ω) = T_stall · (1 − ω/ω_max)`. `params` is
/// `[stall_torque (N·m), no_load_speed (rad/s)]`; the no-load speed scales
/// with the bus voltage `v_dc` (relative to a 100 V reference).
pub fn torque_speed_curve(_motor_type: &str, params: &[Scalar], v_dc: Scalar) -> Vec<(Scalar, Scalar)> {
    let stall = params.first().copied().unwrap_or(10.0).max(0.0);
    let no_load = params.get(1).copied().unwrap_or(300.0).max(0.0);
    let speed_scale = if v_dc > 0.0 { (v_dc / 100.0).max(0.0) } else { 1.0 };
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
    fn test_drive_efficiency() {
        let eta = drive_efficiency(1000.0, 900.0, 50.0, 30.0);
        assert!((eta - 0.9).abs() < 0.01);
    }
}
