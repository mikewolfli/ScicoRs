//! Structure-acoustic coupling, vibration noise transmission.

use crate::core::types::Scalar;

/// Radiation efficiency of a plate.
///
/// Simplified model based on modal radiation above critical frequency.
pub fn radiation_efficiency(
    plate_area: Scalar,
    freq: Scalar,
    critical_freq: Scalar,
    _c: Scalar,
) -> Scalar {
    if freq <= 0.0 {
        return 0.0;
    }
    if freq < critical_freq {
        // Below critical frequency: radiation efficiency < 1
        let ratio = freq / critical_freq;
        let sigma = 2.0 / (std::f64::consts::PI * plate_area.sqrt())
            * (ratio / (1.0 - ratio * ratio)).sqrt();
        sigma.clamp(0.0, 1.0)
    } else {
        // Above critical frequency: radiation efficiency ≈ 1
        1.0
    }
}

/// Critical frequency of a panel: f_c = c²/(1.8·t·c_L).
///
/// c = speed of sound in fluid, t = panel thickness, c_L = longitudinal wave speed.
pub fn critical_frequency(c_fluid: Scalar, thickness: Scalar, c_longitudinal: Scalar) -> Scalar {
    if thickness <= 0.0 || c_longitudinal <= 0.0 {
        return 0.0;
    }
    c_fluid * c_fluid / (1.8 * thickness * c_longitudinal)
}

/// Transmission loss (mass law): TL = 20·log₁₀(f·m) - 47 dB.
///
/// f = frequency (Hz), m = surface density (kg/m²).
pub fn transmission_loss_mass_law(freq: Scalar, surface_density: Scalar) -> Scalar {
    if freq <= 0.0 || surface_density <= 0.0 {
        return 0.0;
    }
    20.0 * (freq * surface_density).log10() - 47.0
}

/// Sound transmission loss considering coincidence effect.
///
/// Uses mass law below critical frequency, dip at critical frequency.
pub fn sound_transmission_loss(
    freq: Scalar,
    surface_density: Scalar,
    critical_freq_val: Scalar,
) -> Scalar {
    let tl_mass = transmission_loss_mass_law(freq, surface_density);
    if critical_freq_val <= 0.0 {
        return tl_mass;
    }
    let ratio = freq / critical_freq_val;
    if (ratio - 1.0).abs() < 0.05 {
        // Coincidence dip: subtract up to 10 dB
        tl_mass - 10.0 * (1.0 - (ratio - 1.0).abs() / 0.05)
    } else {
        tl_mass
    }
}

/// Vibration transfer function (SDOF system).
///
/// H(f) = 1 / (1 - (f/fₙ)² + 2j·ζ·(f/fₙ))
pub fn vibration_transfer_function(
    mass: Scalar,
    stiffness: Scalar,
    damping: Scalar,
    freq: Scalar,
) -> num_complex::Complex<Scalar> {
    if mass <= 0.0 || stiffness <= 0.0 {
        return num_complex::Complex::new(1.0, 0.0);
    }
    let fnat = (stiffness / mass).sqrt() / (2.0 * std::f64::consts::PI);
    if fnat <= 0.0 {
        return num_complex::Complex::new(1.0, 0.0);
    }
    let r = freq / fnat;
    let denom_re = 1.0 - r * r;
    let denom_im = 2.0 * damping * r;
    let denom_sq = denom_re * denom_re + denom_im * denom_im;
    if denom_sq <= 0.0 {
        return num_complex::Complex::new(Scalar::INFINITY, 0.0);
    }
    num_complex::Complex::new(denom_re / denom_sq, -denom_im / denom_sq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mass_law_positive() {
        let tl = transmission_loss_mass_law(1000.0, 10.0);
        let expected = 20.0 * (10000.0_f64).log10() - 47.0;
        assert!((tl - expected).abs() < 1e-10);
    }

    #[test]
    fn test_mass_law_doubling_density() {
        let tl1 = transmission_loss_mass_law(500.0, 10.0);
        let tl2 = transmission_loss_mass_law(500.0, 20.0);
        // Doubling density adds 20*log10(2) ≈ 6.02 dB
        assert!((tl2 - tl1 - 20.0 * 2.0_f64.log10()).abs() < 0.01);
    }

    #[test]
    fn test_critical_frequency() {
        // Steel panel 3mm thick
        let fc = critical_frequency(343.0, 0.003, 5900.0);
        assert!(fc > 0.0);
    }

    #[test]
    fn test_radiation_efficiency_above_critical() {
        let sigma = radiation_efficiency(0.25, 2000.0, 1000.0, 343.0);
        assert!((sigma - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_vibration_transfer_function_low_freq() {
        let h = vibration_transfer_function(10.0, 1e6, 0.05, 10.0);
        // Well below resonance, magnitude ≈ 1
        assert!((h.norm() - 1.0).abs() < 0.05);
    }

    #[test]
    fn test_sound_transmission_loss() {
        let tl = sound_transmission_loss(500.0, 10.0, 2000.0);
        assert!(tl > 0.0);
    }
}
