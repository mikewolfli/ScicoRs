//! Signal integrity: reflection, crosstalk, eye diagram, TDR.

use crate::core::types::Scalar;
use num_complex::Complex;

/// Reflection coefficient: Γ = (Z_L - Z₀)/(Z_L + Z₀).
pub fn reflection_coefficient(zl: Scalar, z0: Scalar) -> Scalar {
    if (zl + z0).abs() < 1e-30 {
        return 0.0;
    }
    (zl - z0) / (zl + z0)
}

/// Return loss: RL = -20·log₁₀(|Γ|) dB.
pub fn return_loss(gamma: Scalar) -> Scalar {
    if gamma.abs() <= 0.0 {
        return Scalar::INFINITY;
    }
    -20.0 * f64::log10(gamma.abs())
}

/// Insertion loss: IL = -20·log₁₀(|S₂₁|) dB.
pub fn insertion_loss(s21: Complex<Scalar>) -> Scalar {
    let mag = s21.norm();
    if mag <= 0.0 {
        return Scalar::INFINITY;
    }
    -20.0 * f64::log10(mag)
}

/// Crosstalk peak voltage (simplified 3-line microstrip model).
pub fn crosstalk_peak(
    aggressor_swing: Scalar,
    coupling_length: Scalar,
    rise_time: Scalar,
    z0: Scalar,
    zcouple: Scalar,
) -> Scalar {
    if rise_time <= 0.0 {
        return 0.0;
    }
    let k = zcouple / (z0 + zcouple);
    let sat_level = k * aggressor_swing;
    let transition_ratio = coupling_length / (rise_time * 1.5e8); // ~half speed of light
    if transition_ratio >= 1.0 {
        sat_level
    } else {
        sat_level * transition_ratio
    }
}

/// Ringing overshoot for underdamped 2nd-order system.
pub fn ringing_overshoot(damping_ratio: Scalar) -> Scalar {
    if damping_ratio >= 1.0 {
        return 0.0;
    }
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

/// Simplified eye diagram analysis computed from the actual waveform.
///
/// Eye height is the separation between the mean high-rail and low-rail
/// samples; eye width is the mean crossing-to-crossing interval expressed as a
/// fraction of the bit period; jitter is the standard deviation of the
/// mid-level crossing times.
pub fn eye_diagram_analysis(
    waveform: &[Scalar],
    time: &[Scalar],
    bit_period: Scalar,
    _n_bits: usize,
) -> EyeDiagram {
    let bit_rate = if bit_period > 0.0 {
        1.0 / bit_period
    } else {
        0.0
    };
    if waveform.is_empty() || time.len() != waveform.len() || bit_period <= 0.0 {
        return EyeDiagram {
            eye_height: 0.0,
            eye_width: 0.0,
            jitter: 0.0,
            bit_rate,
        };
    }

    let mut vmin = Scalar::INFINITY;
    let mut vmax = Scalar::NEG_INFINITY;
    for &v in waveform {
        vmin = vmin.min(v);
        vmax = vmax.max(v);
    }
    if vmax <= vmin {
        return EyeDiagram {
            eye_height: 0.0,
            eye_width: bit_period,
            jitter: 0.0,
            bit_rate,
        };
    }

    let mid = 0.5 * (vmin + vmax);
    let high_thresh = vmin + 0.7 * (vmax - vmin);
    let low_thresh = vmin + 0.3 * (vmax - vmin);

    let mut high_sum = 0.0;
    let mut high_n = 0.0;
    let mut low_sum = 0.0;
    let mut low_n = 0.0;
    let mut crossings: Vec<Scalar> = Vec::new();

    for i in 0..waveform.len() {
        let v = waveform[i];
        if v >= high_thresh {
            high_sum += v;
            high_n += 1.0;
        }
        if v <= low_thresh {
            low_sum += v;
            low_n += 1.0;
        }
        if i > 0 {
            let prev = waveform[i - 1];
            if (v >= mid) != (prev >= mid) {
                let dt = time[i] - time[i - 1];
                if dt > 0.0 && (v - prev).abs() > 1e-30 {
                    let t = time[i - 1] + dt * (mid - prev) / (v - prev);
                    crossings.push(t);
                }
            }
        }
    }

    let high_mean = if high_n > 0.0 {
        high_sum / high_n
    } else {
        high_thresh
    };
    let low_mean = if low_n > 0.0 {
        low_sum / low_n
    } else {
        low_thresh
    };
    let eye_height = (high_mean - low_mean).max(0.0);

    let mut eye_width_frac = 1.0;
    if crossings.len() >= 2 {
        let mut intervals: Vec<Scalar> = Vec::new();
        for w in crossings.windows(2) {
            let d = (w[1] - w[0]).abs();
            if d > 0.0 && d < 1.5 * bit_period {
                intervals.push(d);
            }
        }
        if !intervals.is_empty() {
            let mean_interval: Scalar =
                intervals.iter().sum::<Scalar>() / intervals.len() as Scalar;
            eye_width_frac = (mean_interval / bit_period).clamp(0.0, 1.0);
        }
    }

    let mut jitter = 0.0;
    if crossings.len() >= 2 {
        let mean_c: Scalar = crossings.iter().sum::<Scalar>() / crossings.len() as Scalar;
        let var: Scalar = crossings
            .iter()
            .map(|c| (c - mean_c) * (c - mean_c))
            .sum::<Scalar>()
            / crossings.len() as Scalar;
        jitter = var.sqrt();
    }

    EyeDiagram {
        eye_height,
        eye_width: eye_width_frac * bit_period,
        jitter,
        bit_rate,
    }
}

/// Simplified TDR waveform simulation.
pub fn tdr_waveform(
    source_z0: Scalar,
    line_z0: Scalar,
    load_z0: Scalar,
    _rise_time: Scalar,
    length: Scalar,
    time: &[Scalar],
) -> Vec<Scalar> {
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
