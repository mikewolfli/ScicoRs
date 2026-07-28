//! PCB transmission line models: microstrip, stripline, CPW, S-parameters.

use crate::core::types::Scalar;
use num_complex::Complex;

/// Microstrip characteristic impedance (IPC-2141 approximation).
pub fn microstrip_z0(w: Scalar, h: Scalar, t: Scalar, er: Scalar) -> Scalar {
    if h <= 0.0 {
        return 0.0;
    }
    let we = w + t * (1.0 + 1.0 / er) / std::f64::consts::PI
        * (4.0 * std::f64::consts::E
            / (t * t / h / h + 1.0 / (std::f64::consts::PI * w / t + 1.1).powi(2)))
        .ln();
    let eta0 = 376.73;
    let eff_er = (er + 1.0) / 2.0 + (er - 1.0) / 2.0 * f64::powi(1.0 + 12.0 * h / we, -1i32);
    eta0 / (2.0 * std::f64::consts::PI * f64::sqrt(2.0 * (1.0 + eff_er)))
        * (4.0 * h / we + f64::sqrt(16.0 * h * h / (we * we) + 2.0)).ln()
}

/// Stripline characteristic impedance.
pub fn stripline_z0(w: Scalar, _h: Scalar, t: Scalar, er: Scalar, b: Scalar) -> Scalar {
    if b <= 0.0 {
        return 0.0;
    }
    let _eta0 = 376.73;
    let we = w + t * (1.0 + 1.0 / er) / std::f64::consts::PI
        * (4.0 * std::f64::consts::E
            / (t * t / b / b + 1.0 / (std::f64::consts::PI * w / t + 1.1).powi(2)))
        .ln();
    let cf = 2.0 * we / (b - t)
        + (b - t) / (b - t + 2.0 * we / std::f64::consts::PI * (2.0 * b / t + 1.0).ln());
    30.0 / f64::sqrt(er) * cf
}

/// Coplanar waveguide characteristic impedance (simplified).
pub fn cpw_z0(w: Scalar, gap: Scalar, _h: Scalar, er: Scalar) -> Scalar {
    if w <= 0.0 || gap <= 0.0 {
        return 0.0;
    }
    let eta0 = 376.73;
    let ke = w / (w + 2.0 * gap);
    let k1 = f64::sqrt(1.0 - ke * ke);
    let ratio = if ke < 0.707 {
        std::f64::consts::PI / (1.0 + k1 / ke).ln()
    } else {
        (1.0 + ke / k1).ln() / std::f64::consts::PI
    };
    let eff_er = (er + 1.0) / 2.0;
    eta0 * ratio / (4.0 * f64::sqrt(eff_er))
}

/// Propagation delay per meter: t_pd = sqrt(er)/c.
pub fn propagation_delay(er: Scalar) -> Scalar {
    f64::sqrt(er) / 2.99792458e8
}

/// Transmission line model using lumped LC segments.
#[derive(Debug, Clone)]
pub struct TransmissionLine {
    pub z0: Scalar,
    pub length: Scalar,
    pub er: Scalar,
    pub attenuation: Scalar,
    pub segments: usize,
}

impl TransmissionLine {
    pub fn new(z0: Scalar, length: Scalar, er: Scalar) -> Self {
        Self {
            z0,
            length,
            er,
            attenuation: 0.0,
            segments: 10,
        }
    }

    /// Propagation delay for the entire line.
    pub fn propagation_delay(&self) -> Scalar {
        f64::sqrt(self.er) * self.length / 2.99792458e8
    }

    /// Electrical length in wavelengths.
    pub fn electrical_length(&self, freq: Scalar) -> Scalar {
        if freq <= 0.0 {
            return 0.0;
        }
        let wavelength = 2.99792458e8 / (freq * f64::sqrt(self.er));
        self.length / wavelength
    }

    /// Input impedance for a given load impedance.
    pub fn input_impedance(&self, freq: Scalar, zl: Complex<Scalar>) -> Complex<Scalar> {
        let beta = 2.0 * std::f64::consts::PI * freq * f64::sqrt(self.er) / 2.99792458e8;
        let el = beta * self.length;
        let z0c = Complex::new(self.z0, 0.0);
        z0c * (zl * Complex::new(el.cos(), 0.0) + z0c * Complex::new(0.0, el.sin()))
            / (z0c * Complex::new(el.cos(), 0.0) + zl * Complex::new(0.0, el.sin()))
    }

    /// S11 reflection coefficient.
    pub fn s11(&self, freq: Scalar, z0: Scalar, zl: Complex<Scalar>) -> Complex<Scalar> {
        let zin = self.input_impedance(freq, zl);
        let z0c = Complex::new(z0, 0.0);
        (zin - z0c) / (zin + z0c)
    }

    /// S21 transmission coefficient.
    pub fn s21(&self, freq: Scalar, _z0: Scalar) -> Complex<Scalar> {
        let gamma = self.attenuation / 8.686 * self.length; // Nepers
        let _beta = 2.0 * std::f64::consts::PI * freq * f64::sqrt(self.er) / 2.99792458e8;
        let k = if self.attenuation > 0.0 {
            10.0_f64.powf(-self.attenuation * self.length / 20.0)
        } else {
            1.0
        };
        Complex::new(k * (-gamma * self.length).cos(), 0.0)
            - Complex::new(0.0, k * (-gamma * self.length).sin())
    }
}

/// Convert 2-port S-parameters to T-parameters.
pub fn s2p_to_t_params(
    s11: Complex<Scalar>,
    s12: Complex<Scalar>,
    s21: Complex<Scalar>,
    s22: Complex<Scalar>,
    _z0: Scalar,
) -> [[Complex<Scalar>; 2]; 2] {
    let det = s21;
    [
        [s11 / det, (s12 * s22 - s11 * s21) / det],
        [1.0 / det, -s22 / det],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_microstrip_z0_positive() {
        let z0 = microstrip_z0(0.5e-3, 0.2e-3, 35e-6, 4.5);
        assert!(z0 > 0.0);
    }

    #[test]
    fn test_propagation_delay() {
        let pd = propagation_delay(4.5);
        assert!(pd > 0.0);
    }

    #[test]
    fn test_transmission_line_delay() {
        let tl = TransmissionLine::new(50.0, 0.1, 4.5);
        assert!(tl.propagation_delay() > 0.0);
    }

    #[test]
    fn test_transmission_line_electrical_length() {
        let tl = TransmissionLine::new(50.0, 1.0, 1.0);
        let el = tl.electrical_length(1e8);
        assert!(el > 0.0);
    }

    #[test]
    fn test_s2p_to_t_params() {
        let s11 = Complex::new(0.1, 0.0);
        let s21 = Complex::new(0.9, 0.0);
        let t = s2p_to_t_params(
            s11,
            Complex::new(0.0, 0.0),
            s21,
            Complex::new(0.1, 0.0),
            50.0,
        );
        assert!((t[0][0].norm() - 0.111).abs() < 0.01);
    }
}
