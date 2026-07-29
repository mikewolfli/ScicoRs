//! Dual Active Bridge (DAB) DC-DC converter.

use crate::core::types::Scalar;
use std::f64::consts::PI;

/// DAB converter with phase-shift modulation.
#[derive(Debug, Clone)]
pub struct DabConverter {
    pub vin: Scalar, pub vout: Scalar,
    pub inductance: Scalar, pub fs: Scalar,
    pub phase_shift: Scalar,
}

impl DabConverter {
    pub fn new(vin: Scalar, vout: Scalar, inductance: Scalar, fs: Scalar) -> Self {
        Self { vin, vout, inductance, fs, phase_shift: 0.0 }
    }
    pub fn power_flow(&self) -> Scalar {
        let d = (self.phase_shift / PI).clamp(-0.5, 0.5);
        self.vin * self.vout * d * (1.0 - 2.0 * d.abs()) / (2.0 * self.inductance * self.fs).max(1e-30)
    }
    pub fn zvs_condition(&self) -> bool {
        let d = (self.phase_shift / PI).clamp(-0.5, 0.5);
        (self.vin - self.vout * (2.0 * d - 1.0)).abs() < self.vin * 0.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_dab_new() { let c = DabConverter::new(400.0, 400.0, 10e-6, 100e3); assert!((c.vin - 400.0).abs() < 1.0); }
    #[test]
    fn test_power_flow_zero() { let c = DabConverter::new(400.0, 400.0, 10e-6, 100e3); assert!((c.power_flow() - 0.0).abs() < 1.0); }
    #[test]
    fn test_power_flow_nonzero() { let mut c = DabConverter::new(400.0, 400.0, 10e-6, 100e3); c.phase_shift = 0.5; assert!(c.power_flow() > 0.0); }
    #[test]
    fn test_zvs_condition() { let c = DabConverter::new(400.0, 400.0, 10e-6, 100e3); let _ = c.zvs_condition(); }
}
