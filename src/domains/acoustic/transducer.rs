//! Acoustic transducer models: loudspeaker, microphone, accelerometer.

use crate::core::types::Scalar;

/// Loudspeaker model using Thiele-Small parameters (piston model).
#[derive(Debug, Clone)]
pub struct Loudspeaker {
    pub sd: Scalar,
    pub mms: Scalar,
    pub cms: Scalar,
    pub rms: Scalar,
    pub bl: Scalar,
    pub re: Scalar,
    pub le: Scalar,
}

impl Loudspeaker {
    pub fn new(sd: Scalar, mms: Scalar, cms: Scalar, rms: Scalar, bl: Scalar, re: Scalar, le: Scalar) -> Self {
        Self { sd, mms, cms, rms, bl, re, le }
    }

    /// Fundamental resonance frequency: fₛ = 1/(2π·√(Mms·Cms)).
    pub fn fundamental_resonance(&self) -> Scalar {
        if self.mms <= 0.0 || self.cms <= 0.0 {
            return 0.0;
        }
        1.0 / (2.0 * std::f64::consts::PI * (self.mms * self.cms).sqrt())
    }

    /// Electrical impedance at given frequency.
    pub fn electrical_impedance(&self, freq: Scalar) -> num_complex::Complex<Scalar> {
        let omega = 2.0 * std::f64::consts::PI * freq;
        // Mechanical impedance Zm = Rms + j(ω·Mms - 1/(ω·Cms))
        let zm_re = self.rms;
        let zm_im = omega * self.mms - 1.0 / (omega * self.cms);
        // Electrical impedance Ze = Re + jω·Le + (Bl)²/Zm
        let zm_sq = zm_re * zm_re + zm_im * zm_im;
        let ze_re = self.re + self.bl * self.bl * zm_re / zm_sq;
        let ze_im = omega * self.le - self.bl * self.bl * zm_im / zm_sq;
        num_complex::Complex::new(ze_re, ze_im)
    }

    /// Sound pressure level at 1 m for given voltage (dB SPL).
    pub fn sound_pressure(&self, freq: Scalar, voltage: Scalar, distance: Scalar) -> Scalar {
        if distance <= 0.0 {
            return 0.0;
        }
        let z = self.electrical_impedance(freq);
        let current = voltage / z.norm();
        let force = self.bl * current;
        let omega = 2.0 * std::f64::consts::PI * freq;
        let zm_re = self.rms;
        let zm_im = omega * self.mms - 1.0 / (omega * self.cms);
        let velocity = force / (num_complex::Complex::new(zm_re, zm_im).norm());
        let volume_velocity = velocity * self.sd;
        // p = ρ·f·U/(2·r) for a monopole
        let pressure = 1.2 * freq * volume_velocity / (2.0 * distance);
        20.0 * (pressure / 20e-6).log10()
    }

    /// Reference efficiency: η = ρ·(Bl)²·Sd²/(2π·c·Re·Mms²).
    pub fn efficiency(&self) -> Scalar {
        if self.re <= 0.0 || self.mms <= 0.0 {
            return 0.0;
        }
        1.2 * self.bl * self.bl * self.sd * self.sd
            / (2.0 * std::f64::consts::PI * 343.0 * self.re * self.mms * self.mms)
    }
}

/// Capacitive microphone model.
#[derive(Debug, Clone)]
pub struct Microphone {
    pub sensitivity: Scalar,
    pub frequency_response: Vec<(Scalar, Scalar)>,
}

impl Microphone {
    pub fn new(sensitivity: Scalar) -> Self {
        Self { sensitivity, frequency_response: Vec::new() }
    }

    pub fn output_voltage(&self, sound_pressure_pa: Scalar) -> Scalar {
        self.sensitivity * sound_pressure_pa / 1000.0 // mV from mV/Pa
    }

    pub fn frequency_correction(&self, freq: Scalar) -> Scalar {
        if self.frequency_response.is_empty() {
            return 0.0;
        }
        // Find nearest response point
        let mut best = self.frequency_response[0].1;
        let mut min_diff = (freq - self.frequency_response[0].0).abs();
        for &(f, g) in &self.frequency_response {
            let diff = (freq - f).abs();
            if diff < min_diff {
                min_diff = diff;
                best = g;
            }
        }
        best
    }
}

/// Accelerometer sensor model.
#[derive(Debug, Clone)]
pub struct Accelerometer {
    pub sensitivity: Scalar,
    pub resonant_freq: Scalar,
    pub damping_ratio: Scalar,
}

impl Accelerometer {
    pub fn new(sensitivity: Scalar, resonant_freq: Scalar, damping_ratio: Scalar) -> Self {
        Self { sensitivity, resonant_freq, damping_ratio }
    }

    /// Output voltage for given acceleration (in g).
    pub fn output_voltage(&self, acceleration_g: Scalar) -> Scalar {
        self.sensitivity * acceleration_g
    }

    /// Frequency response magnitude at given frequency.
    pub fn frequency_response(&self, freq: Scalar) -> Scalar {
        if self.resonant_freq <= 0.0 {
            return 1.0;
        }
        let r = freq / self.resonant_freq;
        let denom = (1.0 - r * r).powi(2) + (2.0 * self.damping_ratio * r).powi(2);
        if denom <= 0.0 {
            return Scalar::INFINITY;
        }
        1.0 / denom.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loudspeaker_resonance() {
        let ls = Loudspeaker::new(0.03, 0.01, 2.5e-4, 1.0, 5.0, 6.0, 0.5e-3);
        let fs = ls.fundamental_resonance();
        let expected = 1.0 / (2.0 * std::f64::consts::PI * f64::sqrt(0.01 * 2.5e-4));
        assert!((fs - expected).abs() / expected < 0.001);
    }

    #[test]
    fn test_loudspeaker_impedance() {
        let ls = Loudspeaker::new(0.03, 0.01, 2.5e-4, 1.0, 5.0, 6.0, 0.5e-3);
        let z = ls.electrical_impedance(100.0);
        assert!(z.norm() > 0.0);
    }

    #[test]
    fn test_loudspeaker_efficiency() {
        let ls = Loudspeaker::new(0.03, 0.01, 2.5e-4, 1.0, 5.0, 6.0, 0.5e-3);
        let eta = ls.efficiency();
        assert!(eta > 0.0 && eta < 1.0);
    }

    #[test]
    fn test_microphone_output() {
        let mic = Microphone::new(50.0);
        let v = mic.output_voltage(1.0);
        assert!((v - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_accelerometer_output() {
        let acc = Accelerometer::new(100.0, 20000.0, 0.7);
        let v = acc.output_voltage(1.0);
        assert!((v - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_accelerometer_frequency_response() {
        let acc = Accelerometer::new(100.0, 10000.0, 0.7);
        let h_lo = acc.frequency_response(100.0); // well below resonance
        assert!((h_lo - 1.0).abs() < 0.01);
    }
}
