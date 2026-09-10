//! LLC resonant converter DC-gain analysis and ZVS region.

use crate::core::types::Scalar;
use std::f64::consts::PI;

/// LLC resonant converter model.
#[derive(Debug, Clone)]
pub struct LlcConverter {
    pub vin: Scalar, pub vout: Scalar,
    pub lr: Scalar, pub cr: Scalar, pub lm: Scalar,
    pub fs: Scalar,
}

impl LlcConverter {
    pub fn new(vin: Scalar, vout: Scalar, lr: Scalar, cr: Scalar, lm: Scalar, fs: Scalar) -> Self {
        Self { vin, vout, lr, cr, lm, fs }
    }
    pub fn resonant_freq(&self) -> Scalar { 1.0 / (2.0 * PI * (self.lr * self.cr).sqrt()) }
    pub fn gain_curve(&self, fnorm: Scalar, q: Scalar) -> Scalar {
        let f = fnorm;
        let l = self.lm / self.lr.max(1e-30);
        let denom = ((1.0 + l - l / (f * f)).powi(2) + (q * q * (f - 1.0 / f).powi(2))).sqrt();
        if denom < 1e-30 { 0.0 } else { 1.0 / denom }
    }
    pub fn dc_gain(&self, load: Scalar) -> Scalar {
        let _ = self.vout;
        let fr = self.resonant_freq();
        let fnorm = self.fs / fr;
        let r_ac = 8.0 * self.vout * self.vout / (PI * PI * load.max(1e-30));
        let q = (self.lr / self.cr).sqrt() / r_ac.max(1e-30);
        self.gain_curve(fnorm, q)
    }
    pub fn zvs_region(&self) -> (Scalar, Scalar) {
        let fr = self.resonant_freq();
        (fr * 0.5, fr * 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_llc_new() { let c = LlcConverter::new(400.0, 48.0, 50e-6, 100e-9, 200e-6, 100e3); assert!(c.vin > 0.0); }
    #[test]
    fn test_resonant_freq() { let c = LlcConverter::new(400.0, 48.0, 50e-6, 100e-9, 200e-6, 100e3); assert!(c.resonant_freq() > 0.0); }
    #[test]
    fn test_gain_curve() { let c = LlcConverter::new(400.0, 48.0, 50e-6, 100e-9, 200e-6, 100e3); let g = c.gain_curve(1.0, 0.5); assert!(g > 0.0); }
    #[test]
    fn test_dc_gain() { let c = LlcConverter::new(400.0, 48.0, 50e-6, 100e-9, 200e-6, 100e3); let g = c.dc_gain(10.0); assert!(g > 0.0); }
}
