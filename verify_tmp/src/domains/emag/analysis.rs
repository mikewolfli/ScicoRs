//! EM analysis: eddy current, hysteresis, shielding, radar equation.

use crate::core::types::Scalar;

/// Eddy current loss in thin plate approximation.
pub fn eddy_current_loss(
    freq: Scalar,
    b_peak: Scalar,
    thickness: Scalar,
    conductivity: Scalar,
    volume: Scalar,
) -> Scalar {
    if freq <= 0.0 || thickness <= 0.0 {
        return 0.0;
    }
    let pi_sq = std::f64::consts::PI * std::f64::consts::PI;
    pi_sq * freq * freq * b_peak * b_peak * thickness * thickness * volume / (6.0 * conductivity)
}

/// Hysteresis loss (Steinmetz equation): P_h = k·f^α·B^β.
pub fn hysteresis_loss(
    k: Scalar,
    freq: Scalar,
    b_peak: Scalar,
    alpha: Scalar,
    beta: Scalar,
) -> Scalar {
    k * f64::powf(freq, alpha) * f64::powf(b_peak, beta)
}

// Joule heating moved to thermal::coupling::joule_heating to avoid duplication.
// Previously defined here: P = I²·R.

/// Radiation efficiency: η = R_r/(R_r + R_loss).
pub fn radiation_efficiency(r_rad: Scalar, r_loss: Scalar) -> Scalar {
    let total = r_rad + r_loss;
    if total <= 0.0 {
        return 0.0;
    }
    r_rad / total
}

/// Antenna gain in dBi.
pub fn antenna_gain_dbi(directivity: Scalar, efficiency: Scalar) -> Scalar {
    10.0 * f64::log10(directivity * efficiency)
}

/// Simplified radar range equation.
pub fn radar_range_eq(
    pt: Scalar,
    gt: Scalar,
    gr: Scalar,
    sigma: Scalar,
    lambda: Scalar,
    snr_min: Scalar,
    losses: Scalar,
) -> Scalar {
    if snr_min <= 0.0 || lambda <= 0.0 {
        return 0.0;
    }
    let num = pt * gt * gr * lambda * lambda * sigma;
    let denom = (4.0 * std::f64::consts::PI).powi(3) * snr_min * losses;
    if denom <= 0.0 {
        return 0.0;
    }
    f64::powf(num / denom, 0.25)
}

/// Shielding effectiveness: SE(dB) = R + A + M.
pub fn shielding_effectiveness(
    freq: Scalar,
    _material: &str,
    thickness: Scalar,
    conductivity: Scalar,
    mu_r: Scalar,
) -> Scalar {
    let mu = 1.25663706212e-6 * mu_r;
    let sigma = conductivity;
    let omega = 2.0 * std::f64::consts::PI * freq;
    let sd = f64::sqrt(2.0 / (omega * mu * sigma));
    if sd <= 0.0 {
        return 0.0;
    }
    // Absorption loss: A = 8.686·t/δ dB
    let absorption = 8.686 * thickness / sd;
    // Reflection loss: R = 168 + 10·log₁₀(σ/(μ·f)) dB (simplified for plane wave far field)
    let reflection = 168.0 + 10.0 * f64::log10(sigma / (mu_r * freq));
    // Multiple reflection correction (simplified)
    let correction = if absorption > 10.0 {
        0.0
    } else {
        -10.0 * f64::log10(1.0 - 10.0_f64.powf(-absorption / 10.0))
    };
    absorption + reflection + correction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eddy_current_loss() {
        let p = eddy_current_loss(1e3, 0.1, 1e-3, 5.8e7, 1e-6);
        assert!(p >= 0.0);
    }

    #[test]
    fn test_hysteresis_loss() {
        let p = hysteresis_loss(0.01, 1e3, 0.1, 1.5, 2.0);
        assert!(p > 0.0);
    }

    #[test]
    fn test_joule_heating() {
        // Use crate-level re-export from thermal::coupling
        assert!((crate::joule_heating(10.0, 0.1) - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_radiation_efficiency() {
        let eta = radiation_efficiency(70.0, 5.0);
        assert!((eta - 70.0 / 75.0).abs() < 1e-10);
    }

    #[test]
    fn test_antenna_gain() {
        let g = antenna_gain_dbi(1.5, 0.8);
        assert!(g > 0.0);
    }

    #[test]
    fn test_radar_range() {
        let r = radar_range_eq(1000.0, 10.0, 10.0, 1.0, 0.03, 1e-12, 1.0);
        assert!(r > 0.0);
    }

    #[test]
    fn test_shielding_effectiveness() {
        let se = shielding_effectiveness(1e6, "copper", 1e-3, 5.8e7, 1.0);
        assert!(se > 50.0); // copper should provide good shielding at 1 MHz
    }
}
