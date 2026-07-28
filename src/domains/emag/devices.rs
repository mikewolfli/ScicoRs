//! Electromagnetic devices: coils, transformers, antennas, magnets.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Coil inductance (multilayer solenoid approximation).
pub fn coil_inductance(n_turns: Scalar, radius: Scalar, _length: Scalar, _layers: u32) -> Scalar {
    if radius <= 0.0 { return 0.0; }
    let r_m = radius;
    1.25663706212e-6 * n_turns * n_turns * std::f64::consts::PI * r_m * r_m
}

/// Mutual inductance: M = k·√(L₁·L₂).
pub fn mutual_inductance(l1: Scalar, l2: Scalar, k: Scalar) -> Scalar {
    k * f64::sqrt(l1 * l2)
}

/// Simplified transformer model.
#[derive(Debug, Clone)]
pub struct Transformer {
    pub n1: Scalar, pub n2: Scalar,
    pub lm: Scalar, pub ll: Scalar,
    pub r1: Scalar, pub r2: Scalar,
}

impl Transformer {
    pub fn turns_ratio(&self) -> Scalar { self.n1 / self.n2 }
    pub fn open_circuit_test(&self, v1: Scalar, freq: Scalar) -> (Scalar, Scalar, Scalar) {
        let omega = 2.0 * std::f64::consts::PI * freq;
        let i_mag = v1 / (omega * self.lm);
        let p_core = v1 * v1 / 1e6;
        (v1, i_mag, p_core)
    }
    pub fn short_circuit_test(&self, _v1: Scalar, _freq: Scalar) -> (Scalar, Scalar, Scalar) {
        (self.r1 + self.r2, self.ll, (self.r1 + self.r2) * 100.0)
    }
}

/// Permanent magnet model.
#[derive(Debug, Clone)]
pub struct PermanentMagnet {
    pub br: Scalar,
    pub hc: Scalar,
    pub volume: Scalar,
    pub shape: MagnetShape,
}

/// Magnet shape variants.
#[derive(Debug, Clone)]
pub enum MagnetShape {
    Cylindrical { radius: Scalar, height: Scalar },
    Block { dims: Coord3D },
}

/// Dipole antenna model.
#[derive(Debug, Clone)]
pub struct DipoleAntenna {
    pub length: Scalar,
    pub freq: Scalar,
}

impl DipoleAntenna {
    pub fn radiation_resistance(&self) -> Scalar {
        let c = 2.99792458e8;
        let lambda = c / self.freq;
        let ratio = self.length / lambda;
        if ratio < 0.1 {
            80.0 * std::f64::consts::PI * std::f64::consts::PI * ratio * ratio
        } else if (ratio - 0.5).abs() < 0.01 {
            73.0 // half-wave dipole
        } else {
            73.0 * (1.0 - 0.5 * (2.0 * std::f64::consts::PI * ratio).cos()) / (1.0 - (2.0 * std::f64::consts::PI * ratio).cos())
        }
    }
    pub fn directivity(&self) -> Scalar {
        let c = 2.99792458e8;
        let lambda = c / self.freq;
        let ratio = self.length / lambda;
        if (ratio - 0.5).abs() < 0.1 { 1.5 } else { 1.5 + 0.5 * (1.0 - ratio / 0.5) }
    }
    pub fn gain(&self, efficiency: Scalar) -> Scalar {
        efficiency * self.directivity()
    }
    pub fn radiation_pattern(&self, theta: Scalar) -> Scalar {
        let c = 2.99792458e8;
        let lambda = c / self.freq;
        let beta_l = 2.0 * std::f64::consts::PI * self.length / lambda;
        let num = f64::cos(beta_l * 0.5 * f64::cos(theta)) - f64::cos(beta_l * 0.5);
        let denom = f64::sin(theta);
        if denom.abs() < 1e-10 { return 0.0; }
        (num / denom).abs()
    }
    pub fn bandwidth(&self, _swr_max: Scalar) -> Scalar {
        self.freq * 0.08
    }
}

/// Alias for Antenna.
pub type Antenna = DipoleAntenna;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coil_inductance() {
        let l = coil_inductance(100.0, 0.01, 0.02, 1);
        assert!(l > 0.0);
    }

    #[test]
    fn test_mutual_inductance() {
        let m = mutual_inductance(1e-3, 1e-3, 0.9);
        assert!((m - 9e-4).abs() < 1e-6);
    }

    #[test]
    fn test_transformer_turns_ratio() {
        let t = Transformer { n1: 100.0, n2: 10.0, lm: 0.1, ll: 0.001, r1: 0.1, r2: 0.01 };
        assert!((t.turns_ratio() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_dipole_radiation_resistance() {
        let c = 2.99792458e8;
        let ant = DipoleAntenna { length: c / 2e9 / 2.0, freq: 2e9 }; // half-wave at 2 GHz
        let rr = ant.radiation_resistance();
        assert!(rr > 50.0 && rr < 100.0);
    }

    #[test]
    fn test_dipole_directivity() {
        let ant = DipoleAntenna { length: 0.15, freq: 1e9 };
        let d = ant.directivity();
        assert!(d > 1.0);
    }

    #[test]
    fn test_dipole_radiation_pattern() {
        let ant = DipoleAntenna { length: 0.15, freq: 1e9 };
        let p = ant.radiation_pattern(std::f64::consts::FRAC_PI_2);
        assert!(p >= 0.0);
    }
}
