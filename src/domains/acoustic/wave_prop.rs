//! Sound field propagation models: plane/spherical waves, attenuation, SPL.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Sound field type for propagation calculations.
#[derive(Debug, Clone)]
pub enum SoundField {
    PlaneWave {
        amplitude: Scalar,
        frequency: Scalar,
        direction: Coord3D,
    },
    SphericalWave {
        amplitude: Scalar,
        frequency: Scalar,
        source: Coord3D,
    },
    FarField {
        amplitude: Scalar,
        frequency: Scalar,
        distance: Scalar,
    },
}

/// Sound pressure level in dB: SPL = 20·log₁₀(p_rms / p_ref).
pub fn sound_pressure_level(p_rms: Scalar, p_ref: Scalar) -> Scalar {
    if p_rms <= 0.0 || p_ref <= 0.0 {
        return f64::NEG_INFINITY;
    }
    20.0 * (p_rms / p_ref).log10()
}

/// Spherical spreading loss: -20·log₁₀(r / r_ref) dB.
pub fn spherical_spreading(r: Scalar, r_ref: Scalar) -> Scalar {
    if r <= 0.0 || r_ref <= 0.0 {
        return 0.0;
    }
    -20.0 * (r / r_ref).log10()
}

/// Air absorption attenuation coefficient (dB/m) per ISO 9613-1.
///
/// Simplified model for common temperature and humidity.
pub fn air_attenuation_coefficient(freq: Scalar, temp_c: Scalar, humidity_pct: Scalar) -> Scalar {
    // Simplified from ISO 9613-1
    let t_kelvin = temp_c + 273.15;
    let t_ratio = t_kelvin / 293.15;
    let p_sat = 10.0_f64.powf(8.07131 - 1730.63 / (233.426 + temp_c)); // mmHg
    let h = humidity_pct * p_sat / 760.0; // mole fraction
    let frn = (t_ratio.sqrt()) * (24.0 + 4.04e4 * h * (0.02 + h) / (0.391 + h));
    let fro = (t_ratio.sqrt()) * (9.0 + 280.0 * h * (-4.17 * (t_ratio - 1.0)).exp());
    let freq_sq = freq * freq;
    1.0e-3
        * freq_sq
        * (1.562 * (frn / freq).powi(-2).exp() / (frn + freq_sq / frn)
            + 0.148 * (fro / freq).powi(-2).exp() / (fro + freq_sq / fro))
}

/// SPL at distance r from reference distance r_ref.
pub fn spl_at_distance(spl_ref: Scalar, r_ref: Scalar, r: Scalar, absorption: Scalar) -> Scalar {
    let spreading_loss = spherical_spreading(r, r_ref);
    let air_loss = absorption * (r - r_ref).max(0.0);
    spl_ref + spreading_loss - air_loss
}

/// Sound intensity: I = p² / (ρ·c) in W/m².
pub fn sound_intensity(p_rms: Scalar, impedance: Scalar) -> Scalar {
    if impedance <= 0.0 {
        return 0.0;
    }
    p_rms * p_rms / impedance
}

/// Sound power: W = I · A in W.
pub fn sound_power(intensity: Scalar, area: Scalar) -> Scalar {
    intensity * area
}

/// Convert SPL to RMS pressure: p_rms = p_ref · 10^(SPL/20).
pub fn spl_to_pressure(spl_db: Scalar, p_ref: Scalar) -> Scalar {
    p_ref * 10.0_f64.powf(spl_db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spl_94db_is_1pa() {
        // 94 dB SPL = 1 Pa (with p_ref = 20 μPa)
        let spl = sound_pressure_level(1.0, 20e-6);
        assert!((spl - 94.0).abs() < 0.1);
    }

    #[test]
    fn test_spherical_spreading_doubling() {
        // Doubling distance ≈ -6 dB
        let loss = spherical_spreading(2.0, 1.0);
        // -20*log10(2) ≈ -6.02 dB
        assert!((loss + 6.02).abs() < 0.01);
    }

    #[test]
    fn test_sound_intensity() {
        let i = sound_intensity(1.0, 413.0);
        assert!((i - 1.0 / 413.0).abs() < 1e-10);
    }

    #[test]
    fn test_sound_power() {
        let w = sound_power(1.0, 2.0);
        assert!((w - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_spl_to_pressure() {
        let p = spl_to_pressure(94.0, 20e-6);
        assert!((p - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_air_attenuation_non_negative() {
        let alpha = air_attenuation_coefficient(1000.0, 20.0, 50.0);
        assert!(alpha >= 0.0);
    }

    #[test]
    fn test_spl_at_distance() {
        let spl = spl_at_distance(94.0, 1.0, 2.0, 0.0);
        // Only spherical spreading: 94 - 20*log10(2) ≈ 87.98 dB
        assert!((spl - 87.98).abs() < 0.01);
    }
}
