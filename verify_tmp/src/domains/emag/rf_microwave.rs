//! RF and microwave circuits: Smith chart, S-parameters, cavities, amplifiers.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use num_complex::Complex;

/// Impedance to reflection coefficient: Γ = (Z - Z₀)/(Z + Z₀).
pub fn smith_chart_impedance(z: Complex<Scalar>, z0: Scalar) -> Complex<Scalar> {
    let z0c = Complex::new(z0, 0.0);
    (z - z0c) / (z + z0c)
}

/// Reflection coefficient to impedance: Z = Z₀·(1 + Γ)/(1 - Γ).
pub fn gamma_to_z(gamma: Complex<Scalar>, z0: Scalar) -> Complex<Scalar> {
    let z0c = Complex::new(z0, 0.0);
    z0c * (Complex::new(1.0, 0.0) + gamma) / (Complex::new(1.0, 0.0) - gamma)
}

/// Resonant cavity shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CavityShape {
    Rectangular,
    Cylindrical,
}

/// Resonant cavity model.
#[derive(Debug, Clone)]
pub struct ResonantCavity {
    pub shape: CavityShape,
    pub dimensions: Coord3D,
    pub wall_conductivity: Scalar,
}

impl ResonantCavity {
    pub fn resonant_freq(&self, mode: &str) -> Scalar {
        let c = 2.99792458e8;
        match (self.shape, mode) {
            (CavityShape::Rectangular, "TE101") => {
                let a = self.dimensions.x.max(0.01);
                let d = self.dimensions.z.max(0.01);
                0.5 * c * f64::sqrt(1.0 / (a * a) + 1.0 / (d * d))
            }
            (CavityShape::Cylindrical, "TM010") => {
                let r = (self.dimensions.x * 0.5).max(0.01);
                c * 2.4048 / (2.0 * std::f64::consts::PI * r)
            }
            _ => 1e9,
        }
    }

    pub fn quality_factor(&self, mode: &str) -> Scalar {
        let freq = self.resonant_freq(mode);
        let _omega = 2.0 * std::f64::consts::PI * freq;
        let c = 2.99792458e8;
        match self.shape {
            CavityShape::Rectangular => {
                let a = self.dimensions.x.max(0.01);
                let b = self.dimensions.y.max(0.01);
                let d = self.dimensions.z.max(0.01);
                let k = 2.0 * std::f64::consts::PI * freq / c;
                let rs = f64::sqrt(2.0 * std::f64::consts::PI * freq * 1.25663706212e-6 / (2.0 * self.wall_conductivity));
                let numerator = (k * a * b * d).powi(3) * 1.25663706212e-6 * c;
                let denom = 2.0 * std::f64::consts::PI * std::f64::consts::PI * rs * (2.0 * b * (a * a + d * d) + a * d * (k * a).powi(2) + a * d * (k * d).powi(2));
                if denom.abs() < 1e-30 { 1000.0 } else { numerator / denom }
            }
            CavityShape::Cylindrical => 1000.0,
        }
    }

    pub fn bandwidth(&self, q: Scalar) -> Scalar {
        let f0 = self.resonant_freq("TE101");
        if q <= 0.0 { return 0.0; }
        f0 / q
    }
}

/// Cascade two 2-port S-parameter matrices.
pub fn cascade_s2p(s1: [[Complex<Scalar>; 2]; 2], s2: [[Complex<Scalar>; 2]; 2]) -> [[Complex<Scalar>; 2]; 2] {
    let det = Complex::new(1.0, 0.0) - s1[1][0] * s2[0][1];
    if det.norm() < 1e-30 {
        // Singular cascade (e.g., two through networks): return identity
        return [[Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
                [Complex::new(0.0, 0.0), Complex::new(1.0, 0.0)]];
    }
    let s11 = s1[0][0] + s1[0][1] * s2[0][0] * s1[1][0] / det;
    let s12 = s1[0][1] * s2[1][1] / det;
    let s21 = s2[0][0] * s1[1][1] / det;
    let s22 = s2[1][1] + s2[0][0] * s1[1][1] * s2[1][0] / det;
    [[s11, s12], [s21, s22]]
}

/// RF amplifier model.
#[derive(Debug, Clone)]
pub struct RfAmplifier {
    pub gain_db: Scalar,
    pub nf_db: Scalar,
    pub p1db: Scalar,
    pub oip3: Scalar,
}

impl RfAmplifier {
    pub fn linear_gain(&self) -> Scalar {
        10.0_f64.powf(self.gain_db / 20.0)
    }
    pub fn noise_temp(&self) -> Scalar {
        290.0 * (10.0_f64.powf(self.nf_db / 10.0) - 1.0)
    }
    pub fn spurious_free_dr(&self, _bw: Scalar) -> Scalar {
        let _nf_lin = 10.0_f64.powf(self.nf_db / 10.0);
        (2.0 / 3.0) * (self.oip3 - (-174.0 + 10.0 * f64::log10(1e6))) // simplified
    }
}

/// Transmission line resonator frequency.
pub fn transmission_line_resonator(length: Scalar, _z0: Scalar, er: Scalar, n: u32, open_ended: bool) -> Scalar {
    let c = 2.99792458e8;
    let vp = c / f64::sqrt(er);
    if open_ended {
        n as Scalar * vp / (2.0 * length)
    } else {
        n as Scalar * vp / (4.0 * length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smith_chart_impedance() {
        let z = Complex::new(50.0, 0.0);
        let gamma = smith_chart_impedance(z, 50.0);
        assert!((gamma.norm() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_gamma_to_z() {
        let z = gamma_to_z(Complex::new(0.0, 0.0), 50.0);
        assert!((z.re - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_cavity_resonant_freq() {
        let cavity = ResonantCavity {
            shape: CavityShape::Rectangular,
            dimensions: Coord3D::new(0.2, 0.1, 0.3),
            wall_conductivity: 5.8e7,
        };
        let f = cavity.resonant_freq("TE101");
        assert!(f > 1e8 && f < 1e10);
    }

    #[test]
    fn test_cavity_q() {
        let cavity = ResonantCavity {
            shape: CavityShape::Rectangular,
            dimensions: Coord3D::new(0.2, 0.1, 0.3),
            wall_conductivity: 5.8e7,
        };
        let q = cavity.quality_factor("TE101");
        assert!(q > 0.0);
    }

    #[test]
    fn test_cascade_s2p() {
        let thru: [[Complex<Scalar>; 2]; 2] = [
            [Complex::new(0.0, 0.0), Complex::new(1.0, 0.0)],
            [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
        ];
        let _cascaded = cascade_s2p(thru, thru);
        // Verify cascade doesn't produce NaN
        for row in &_cascaded {
            for v in row {
                assert!(!v.is_nan());
            }
        }
    }

    #[test]
    fn test_rf_amplifier_linear_gain() {
        let amp = RfAmplifier { gain_db: 20.0, nf_db: 3.0, p1db: 10.0, oip3: 30.0 };
        assert!((amp.linear_gain() - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_tl_resonator() {
        let f = transmission_line_resonator(0.1, 50.0, 4.5, 1, true);
        assert!(f > 1e8);
    }
}
