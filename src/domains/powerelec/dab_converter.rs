// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Dual Active Bridge (DAB) DC-DC converter.

use crate::core::types::Scalar;
use std::f64::consts::PI;

/// DAB converter with phase-shift modulation.
#[derive(Debug, Clone)]
pub struct DabConverter {
    pub vin: Scalar,
    pub vout: Scalar,
    pub inductance: Scalar,
    pub fs: Scalar,
    pub phase_shift: Scalar,
}

impl DabConverter {
    pub fn new(vin: Scalar, vout: Scalar, inductance: Scalar, fs: Scalar) -> Self {
        Self {
            vin,
            vout,
            inductance,
            fs,
            phase_shift: 0.0,
        }
    }
    pub fn power_flow(&self) -> Scalar {
        // Canonical DAB power transfer: P = Vin·Vout·d·(1 − |d|) / (2·L·fs)
        // with d = φ/π ∈ [−1, 1]. Maximum power occurs at |d| = 0.5.
        let d = (self.phase_shift / PI).clamp(-1.0, 1.0);
        self.vin * self.vout * d * (1.0 - d.abs()) / (2.0 * self.inductance * self.fs).max(1e-30)
    }
    pub fn zvs_condition(&self) -> bool {
        let d = (self.phase_shift / PI).clamp(-1.0, 1.0);
        (self.vin - self.vout * (2.0 * d - 1.0)).abs() < self.vin * 0.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_dab_new() {
        let c = DabConverter::new(400.0, 400.0, 10e-6, 100e3);
        assert!((c.vin - 400.0).abs() < 1.0);
    }
    #[test]
    fn test_power_flow_zero() {
        let c = DabConverter::new(400.0, 400.0, 10e-6, 100e3);
        assert!((c.power_flow() - 0.0).abs() < 1.0);
    }
    #[test]
    fn test_power_flow_nonzero() {
        let mut c = DabConverter::new(400.0, 400.0, 10e-6, 100e3);
        c.phase_shift = 0.5;
        assert!(c.power_flow() > 0.0);
    }
    #[test]
    fn test_zvs_condition() {
        // The condition is |Vin - Vout·(2d - 1)| < 0.1·Vin. Work through the
        // real arithmetic to pin the expected outcomes rather than discarding
        // the boolean (which previously made the test vacuous).
        let c = DabConverter::new(400.0, 400.0, 10e-6, 100e3);

        // At d = 0 the bracket is Vout·(−1) = −400, so |400 − (−400)| = 800,
        // far outside the 40 V window: a matched converter at zero phase shift
        // is NOT in ZVS.
        assert!(
            !c.zvs_condition(),
            "matched converter at d=0 sits outside the ZVS window"
        );

        // The bracket vanishes when Vout·(2d − 1) = Vin, i.e. d = (1 + Vin/Vout)/2
        // = 1.0 for Vin = Vout. At d = 1 the difference is exactly zero, so ZVS
        // holds — this is the boundary the condition is designed to detect.
        let mut c_on = DabConverter::new(400.0, 400.0, 10e-6, 100e3);
        c_on.phase_shift = PI; // d = 1
        assert!(
            c_on.zvs_condition(),
            "d=1 on a matched converter must satisfy ZVS"
        );

        // Halfway between the two, at d = 0.5, the bracket is 0 and the
        // difference is 400 — outside the window again.
        let mut c_mid = DabConverter::new(400.0, 400.0, 10e-6, 100e3);
        c_mid.phase_shift = PI / 2.0;
        assert!(!c_mid.zvs_condition());
    }

    #[test]
    fn test_power_flow_is_maximal_at_quarter_phase_shift() {
        // P(d) = Vin·Vout·d(1-|d|)/(2Lfs) peaks at d = 0.5, i.e. phase = π/2.
        let mut c = DabConverter::new(400.0, 400.0, 10e-6, 100e3);
        c.phase_shift = PI / 2.0;
        let peak = c.power_flow();
        for frac in [0.25, 0.75] {
            c.phase_shift = PI * frac;
            let p = c.power_flow();
            assert!(p < peak, "P({frac}π) = {p} should be below the peak {peak}");
        }
        // Reversing the phase reverses the power flow.
        c.phase_shift = -PI / 2.0;
        assert!(c.power_flow() < 0.0);
    }
}
