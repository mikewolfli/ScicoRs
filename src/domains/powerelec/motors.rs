//! Electric motor models: DC, stepper, PMSM, induction.

use crate::core::types::Scalar;

/// DC motor model (armature + mechanical dynamics).
#[derive(Debug, Clone)]
pub struct DcMotor {
    pub ra: Scalar, pub la: Scalar,
    pub ke: Scalar, pub kt: Scalar,
    pub j: Scalar, pub b: Scalar,
}

impl DcMotor {
    pub fn back_emf(&self, omega: Scalar) -> Scalar { self.ke * omega }
    pub fn torque(&self, i_a: Scalar) -> Scalar { self.kt * i_a }
    pub fn electrical_eq(&self, v_a: Scalar, i_a: Scalar, omega: Scalar) -> Scalar {
        (v_a - self.ra * i_a - self.back_emf(omega)) / self.la
    }
    pub fn mechanical_eq(&self, t_em: Scalar, t_load: Scalar, omega: Scalar) -> Scalar {
        (t_em - t_load - self.b * omega) / self.j
    }
    pub fn steady_state_speed(&self, v_a: Scalar, t_load: Scalar) -> Scalar {
        if self.ke * self.kt + self.b * self.ra <= 0.0 { return 0.0; }
        (self.kt * v_a - self.ra * t_load) / (self.ke * self.kt + self.b * self.ra)
    }
}

/// Stepper motor (simplified).
#[derive(Debug, Clone)]
pub struct StepperMotor {
    pub steps_per_rev: u32,
    pub phase_resistance: Scalar,
    pub phase_inductance: Scalar,
    pub holding_torque: Scalar,
}

impl StepperMotor {
    pub fn step_angle(&self) -> Scalar { 360.0 / self.steps_per_rev as Scalar }
    pub fn pull_out_torque(&self, _speed_rps: Scalar) -> Scalar {
        self.holding_torque * 0.8 // simplified derating
    }
}

/// PMSM (dq-axis model).
#[derive(Debug, Clone)]
pub struct Pmsm {
    pub rs: Scalar, pub ld: Scalar, pub lq: Scalar,
    pub flux_pm: Scalar, pub pole_pairs: u32, pub j: Scalar,
}

impl Pmsm {
    pub fn electrical_eq_d(&self, i_d: Scalar, i_q: Scalar, omega_e: Scalar, v_d: Scalar) -> Scalar {
        (v_d - self.rs * i_d + omega_e * self.lq * i_q) / self.ld
    }
    pub fn electrical_eq_q(&self, i_d: Scalar, i_q: Scalar, omega_e: Scalar, v_q: Scalar) -> Scalar {
        (v_q - self.rs * i_q - omega_e * (self.ld * i_d + self.flux_pm)) / self.lq
    }
    pub fn torque(&self, i_d: Scalar, i_q: Scalar) -> Scalar {
        1.5 * self.pole_pairs as Scalar * (self.flux_pm * i_q + (self.ld - self.lq) * i_d * i_q)
    }
    pub fn mechanical_eq(&self, t_e: Scalar, t_load: Scalar, _omega_m: Scalar) -> Scalar {
        (t_e - t_load) / self.j
    }
}

/// Induction motor (steady-state equivalent circuit).
#[derive(Debug, Clone)]
pub struct InductionMotor {
    pub rs: Scalar, pub rr: Scalar,
    pub ls: Scalar, pub lr: Scalar, pub lm: Scalar,
    pub pole_pairs: u32,
}

impl InductionMotor {
    pub fn slip(&self, sync_speed: Scalar, rotor_speed: Scalar) -> Scalar {
        if sync_speed.abs() < 1e-10 { return 0.0; }
        (sync_speed - rotor_speed) / sync_speed
    }
    pub fn torque_slip(&self, v_phase: Scalar, slip: Scalar, freq: Scalar) -> Scalar {
        if slip.abs() < 1e-10 { return 0.0; }
        let omega_s = 2.0 * std::f64::consts::PI * freq;
        let x_ls = omega_s * self.ls;
        let _x_lr = omega_s * self.lr;
        let x_m = omega_s * self.lm;
        let r2_s = self.rr / slip;
        let denom = (self.rs + x_m * r2_s / (x_m + r2_s / slip)).powi(2)
            + (x_ls + x_m * (r2_s / slip) / (x_m + r2_s / slip)).powi(2);
        if denom <= 0.0 { 0.0 } else { 3.0 * v_phase * v_phase * r2_s / (omega_s * denom) }
    }
    pub fn breakdown_torque(&self, v_phase: Scalar, freq: Scalar) -> Scalar {
        let slip_max = self.rr / f64::sqrt(self.rs * self.rs + (freq * 2.0 * std::f64::consts::PI * (self.ls + self.lr)).powi(2));
        self.torque_slip(v_phase, slip_max, freq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dc_motor_back_emf() {
        let m = DcMotor { ra: 0.5, la: 0.01, ke: 0.05, kt: 0.05, j: 0.001, b: 0.001 };
        assert!((m.back_emf(100.0) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_dc_motor_torque() {
        let m = DcMotor { ra: 0.5, la: 0.01, ke: 0.05, kt: 0.05, j: 0.001, b: 0.001 };
        assert!((m.torque(10.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_dc_motor_steady_state() {
        let m = DcMotor { ra: 0.5, la: 0.01, ke: 0.05, kt: 0.05, j: 0.001, b: 0.001 };
        let omega = m.steady_state_speed(12.0, 0.1);
        assert!(omega > 0.0);
    }

    #[test]
    fn test_stepper_step_angle() {
        let s = StepperMotor { steps_per_rev: 200, phase_resistance: 2.0, phase_inductance: 0.005, holding_torque: 1.0 };
        assert!((s.step_angle() - 1.8).abs() < 1e-10);
    }

    #[test]
    fn test_pmsm_torque() {
        let m = Pmsm { rs: 0.1, ld: 0.5e-3, lq: 0.8e-3, flux_pm: 0.1, pole_pairs: 4, j: 0.001 };
        let t = m.torque(0.0, 10.0);
        assert!((t - 1.5 * 4.0 * 0.1 * 10.0).abs() < 1e-10);
    }
}
