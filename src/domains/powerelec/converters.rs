//! Power converter topologies: Buck, Boost, inverter, rectifier, PWM.

use crate::core::types::Scalar;
use super::devices::PowerMosfet;

/// PWM signal: compares control voltage with triangle carrier.
pub fn pwm_signal(control_voltage: Scalar, carrier_amplitude: Scalar, _carrier_freq: Scalar, time: Scalar) -> Scalar {
    let carrier = carrier_amplitude * (2.0 * std::f64::consts::PI * time).sin();
    if control_voltage > carrier { 1.0 } else { 0.0 }
}

/// Single-phase diode rectifier.
pub fn single_phase_rectifier(v_ac_peak: Scalar, r_load: Scalar, diode_vf: Scalar) -> (Scalar, Scalar) {
    if r_load <= 0.0 { return (0.0, 0.0); }
    let v_dc = v_ac_peak - 2.0 * diode_vf;
    (v_dc, v_dc / r_load)
}

/// Three-phase diode rectifier.
pub fn three_phase_rectifier(v_ac_line_rms: Scalar, r_load: Scalar) -> (Scalar, Scalar) {
    if r_load <= 0.0 { return (0.0, 0.0); }
    let v_dc = v_ac_line_rms * f64::sqrt(2.0) * 3.0 / std::f64::consts::PI;
    (v_dc, v_dc / r_load)
}

/// Buck (step-down) DC-DC converter.
#[derive(Debug, Clone)]
pub struct BuckConverter {
    pub vin: Scalar, pub vout: Scalar,
    pub l: Scalar, pub c: Scalar,
    pub fs: Scalar, pub esr: Scalar,
}

impl BuckConverter {
    pub fn duty_cycle(&self) -> Scalar { self.vout / self.vin }
    pub fn ripple_current(&self) -> Scalar {
        if self.fs <= 0.0 || self.l <= 0.0 { return 0.0; }
        (self.vin - self.vout) * self.duty_cycle() / (self.l * self.fs)
    }
    pub fn ripple_voltage(&self) -> Scalar {
        if self.c <= 0.0 { return 0.0; }
        let dv_c = self.ripple_current() / (8.0 * self.c * self.fs);
        let dv_esr = self.ripple_current() * self.esr;
        f64::sqrt(dv_c * dv_c + dv_esr * dv_esr)
    }
    pub fn efficiency(&self, i_out: Scalar) -> Scalar {
        let p_out = self.vout * i_out;
        if p_out <= 0.0 { return 0.0; }
        let p_loss = i_out * i_out * 0.01; // simplified conduction loss
        p_out / (p_out + p_loss)
    }
}

/// Boost (step-up) DC-DC converter.
#[derive(Debug, Clone)]
pub struct BoostConverter {
    pub vin: Scalar, pub vout: Scalar,
    pub l: Scalar, pub c: Scalar,
    pub fs: Scalar, pub esr: Scalar,
}

impl BoostConverter {
    pub fn duty_cycle(&self) -> Scalar { 1.0 - self.vin / self.vout }
    pub fn ripple_current(&self) -> Scalar {
        if self.fs <= 0.0 || self.l <= 0.0 { return 0.0; }
        self.vin * self.duty_cycle() / (self.l * self.fs)
    }
    pub fn ripple_voltage(&self) -> Scalar {
        if self.c <= 0.0 { return 0.0; }
        let dv_c = self.ripple_current() / (8.0 * self.c * self.fs);
        let dv_esr = self.ripple_current() * self.esr;
        f64::sqrt(dv_c * dv_c + dv_esr * dv_esr)
    }
    pub fn efficiency(&self, i_out: Scalar) -> Scalar {
        let p_out = self.vout * i_out;
        if p_out <= 0.0 { return 0.0; }
        let p_loss = i_out * i_out * 0.02;
        p_out / (p_out + p_loss)
    }
}

/// Full-bridge inverter (SPWM).
#[derive(Debug, Clone)]
pub struct FullBridgeInverter {
    pub v_dc: Scalar,
    pub modulation_index: Scalar,
    pub carrier_freq: Scalar,
    pub output_freq: Scalar,
}

impl FullBridgeInverter {
    pub fn fundamental_output(&self) -> Scalar {
        self.modulation_index * self.v_dc / f64::sqrt(2.0)
    }
    pub fn thd_estimate(&self) -> Scalar {
        0.1 + 0.5 * f64::sqrt(1.0 - self.modulation_index)
    }
    pub fn switching_losses(&self, i_out: Scalar, device: &PowerMosfet) -> Scalar {
        4.0 * device.switching_loss(i_out, self.v_dc, self.carrier_freq)
    }
    pub fn conduction_losses(&self, i_out: Scalar, device: &PowerMosfet) -> Scalar {
        4.0 * device.conduction_loss(i_out, device.r_ds_on, 0.5)
    }
}

/// Chopper mode selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChopperMode { Buck, Boost, BuckBoost }

/// Buck/Boost chopper.
#[derive(Debug, Clone)]
pub struct Chopper {
    pub mode: ChopperMode,
    pub vin: Scalar, pub vout: Scalar,
    pub l: Scalar, pub c: Scalar,
    pub fs: Scalar,
}

/// Re-export for power_integrity compatibility (same function).
pub fn buck_ripple_voltage(vin: Scalar, vout: Scalar, l: Scalar, c: Scalar, freq: Scalar, esr: Scalar) -> Scalar {
    if freq <= 0.0 || l <= 0.0 || c <= 0.0 { return 0.0; }
    let d = vout / vin;
    let di = (vin - vout) * d / (l * freq);
    let dv_c = di / (8.0 * c * freq);
    let dv_esr = di * esr;
    f64::sqrt(dv_c * dv_c + dv_esr * dv_esr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pwm_signal() {
        let v = pwm_signal(0.0, 1.0, 1e3, 0.0);
        assert!((v - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_buck_duty_cycle() {
        let b = BuckConverter { vin: 12.0, vout: 5.0, l: 10e-6, c: 22e-6, fs: 500e3, esr: 0.005 };
        assert!((b.duty_cycle() - 5.0/12.0).abs() < 1e-10);
    }

    #[test]
    fn test_boost_duty_cycle() {
        let b = BoostConverter { vin: 5.0, vout: 12.0, l: 10e-6, c: 22e-6, fs: 500e3, esr: 0.005 };
        assert!((b.duty_cycle() - (1.0 - 5.0/12.0)).abs() < 1e-10);
    }

    #[test]
    fn test_single_phase_rectifier() {
        let (v, i) = single_phase_rectifier(170.0, 10.0, 0.8);
        assert!((v - 168.4).abs() < 0.1);
        assert!(i > 0.0);
    }

    #[test]
    fn test_three_phase_rectifier() {
        let (v, i) = three_phase_rectifier(230.0, 10.0);
        assert!(v > 300.0);
        assert!(i > 0.0);
    }

    #[test]
    fn test_full_bridge_inverter() {
        let inv = FullBridgeInverter { v_dc: 400.0, modulation_index: 0.8, carrier_freq: 10e3, output_freq: 50.0 };
        assert!(inv.fundamental_output() > 0.0);
        assert!(inv.thd_estimate() > 0.0);
    }
}
