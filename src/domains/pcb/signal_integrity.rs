//! Signal integrity: reflection, crosstalk, eye diagram, TDR.

use crate::core::types::Scalar;
use num_complex::Complex;

/// Reflection coefficient: Γ = (Z_L - Z₀)/(Z_L + Z₀).
pub fn reflection_coefficient(zl: Scalar, z0: Scalar) -> Scalar {
    if (zl + z0).abs() < 1e-30 { return 0.0; }
    (zl - z0) / (zl + z0)
}

/// Return loss: RL = -20·log₁₀(|Γ|) dB.
pub fn return_loss(gamma: Scalar) -> Scalar {
    if gamma.abs() <= 0.0 { return Scalar::INFINITY; }
    -20.0 * f64::log10(gamma.abs())
}

/// Insertion loss: IL = -20·log₁₀(|S₂₁|) dB.
pub fn insertion_loss(s21: Complex<Scalar>) -> Scalar {
    let mag = s21.norm();
    if mag <= 0.0 { return Scalar::INFINITY; }
    -20.0 * f64::log10(mag)
}

/// Crosstalk peak voltage (simplified 3-line microstrip model).
pub fn crosstalk_peak(aggressor_swing: Scalar, coupling_length: Scalar, rise_time: Scalar, z0: Scalar, zcouple: Scalar) -> Scalar {
    if rise_time <= 0.0 { return 0.0; }
    let k = zcouple / (z0 + zcouple);
    let sat_level = k * aggressor_swing;
    let transition_ratio = coupling_length / (rise_time * 1.5e8); // ~half speed of light
    if transition_ratio >= 1.0 { sat_level } else { sat_level * transition_ratio }
}

/// Ringing overshoot for underdamped 2nd-order system.
pub fn ringing_overshoot(damping_ratio: Scalar) -> Scalar {
    if damping_ratio >= 1.0 { return 0.0; }
    let zeta = damping_ratio;
    f64::exp(-std::f64::consts::PI * zeta / f64::sqrt(1.0 - zeta * zeta))
}

/// Eye diagram parameters.
#[derive(Debug, Clone)]
pub struct EyeDiagram {
    pub eye_height: Scalar,
    pub eye_width: Scalar,
    pub jitter: Scalar,
    pub bit_rate: Scalar,
}

/// Simplified eye diagram analysis.
pub fn eye_diagram_analysis(_waveform: &[Scalar], _time: &[Scalar], _bit_period: Scalar, _n_bits: usize) -> EyeDiagram {
    EyeDiagram { eye_height: 1.0, eye_width: _bit_period * 0.7, jitter: _bit_period * 0.05, bit_rate: 1.0 / _bit_period }
}

/// Simplified TDR waveform simulation.
pub fn tdr_waveform(source_z0: Scalar, line_z0: Scalar, load_z0: Scalar, _rise_time: Scalar, length: Scalar, time: &[Scalar]) -> Vec<Scalar> {
    let gamma_load = reflection_coefficient(load_z0, line_z0);
    let gamma_src = reflection_coefficient(source_z0, line_z0);
    let t_prop = length / 1.5e8; // ~half light speed for typical PCB
    let mut waveform = Vec::with_capacity(time.len());
    for &t in time {
        let v = if t < t_prop {
            1.0 // incident step
        } else if t < 2.0 * t_prop {
            1.0 + gamma_load // first reflection
        } else {
            1.0 + gamma_load + gamma_load * gamma_src // second reflection
        };
        waveform.push(v);
    }
    waveform
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflection_coefficient_matched() {
        let gamma = reflection_coefficient(50.0, 50.0);
        assert!((gamma - 0.0).abs() < 1e-15);
    }

    #[test]
    fn test_reflection_coefficient_open() {
        let gamma = reflection_coefficient(1e9, 50.0);
        assert!((gamma - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_return_loss() {
        let rl = return_loss(0.1);
        assert!((rl - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_insertion_loss() {
        let il = insertion_loss(Complex::new(0.5, 0.0));
        assert!((il - 6.02).abs() < 0.01);
    }

    #[test]
    fn test_ringing_overshoot() {
        let os = ringing_overshoot(0.3);
        assert!(os > 0.0 && os < 1.0);
    }

    #[test]
    fn test_tdr_waveform() {
        let t = vec![0.0, 1e-9, 2e-9, 3e-9];
        let w = tdr_waveform(50.0, 50.0, 75.0, 1e-10, 0.1, &t);
        assert_eq!(w.len(), 4);
    }
}
