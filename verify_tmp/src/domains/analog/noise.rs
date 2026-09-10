//! Basic noise analysis for circuit simulation.
//!
//! Provides noise power spectral density (PSD) calculations for
//! thermal noise (resistors), shot noise (diodes/BJTs), and
//! flicker noise (MOSFETs).

use crate::core::types::Scalar;

// Physical constants (defined locally to avoid cross-module dependency).
const K_B: Scalar = 1.380649e-23;
const Q: Scalar = 1.602176634e-19;

/// Thermal noise power spectral density (V²/Hz).
///
/// For a resistor R at temperature T: Sv = 4*k*T*R
///
/// # Arguments
/// * `resistance` - Resistance (Ω)
/// * `temp` - Temperature (K)
pub fn thermal_noise_psd(resistance: Scalar, temp: Scalar) -> Scalar {
    if resistance <= 0.0 || temp <= 0.0 {
        return 0.0;
    }
    4.0 * K_B * temp * resistance
}

/// Thermal noise current spectral density (A²/Hz).
///
/// For a conductor G = 1/R: Si = 4*k*T*G
pub fn thermal_noise_current_psd(conductance: Scalar, temp: Scalar) -> Scalar {
    if conductance <= 0.0 || temp <= 0.0 {
        return 0.0;
    }
    4.0 * K_B * temp * conductance
}

/// Shot noise power spectral density (A²/Hz).
///
/// For a DC current I through a junction: Si = 2*q*I
///
/// # Arguments
/// * `current` - DC current (A)
pub fn shot_noise_psd(current: Scalar) -> Scalar {
    if current <= 0.0 {
        return 0.0;
    }
    2.0 * Q * current.abs()
}

/// Flicker noise power spectral density (A²/Hz).
///
/// For a MOSFET: Si = KF * Id^AF / f
///
/// # Arguments
/// * `kf` - Flicker noise coefficient
/// * `af` - Flicker noise exponent
/// * `current` - DC drain current (A)
/// * `freq` - Frequency (Hz)
pub fn flicker_noise_psd(kf: Scalar, af: Scalar, current: Scalar, freq: Scalar) -> Scalar {
    if freq <= 0.0 || current <= 0.0 {
        return 0.0;
    }
    kf * current.abs().powf(af) / freq
}

/// Total RMS noise voltage over a bandwidth (V).
///
/// # Arguments
/// * `psd` - Noise power spectral density (V²/Hz)
/// * `f_start` - Start frequency (Hz)
/// * `f_stop` - Stop frequency (Hz)
pub fn rms_noise_voltage(psd: Scalar, f_start: Scalar, f_stop: Scalar) -> Scalar {
    if f_stop <= f_start || f_start < 0.0 {
        return 0.0;
    }
    (psd * (f_stop - f_start)).sqrt()
}

/// Signal-to-noise ratio (dB).
///
/// # Arguments
/// * `signal_rms` - RMS signal voltage (V)
/// * `noise_rms` - RMS noise voltage (V)
pub fn snr_db(signal_rms: Scalar, noise_rms: Scalar) -> Scalar {
    if noise_rms <= 0.0 || signal_rms <= 0.0 {
        return f64::NEG_INFINITY;
    }
    20.0 * (signal_rms / noise_rms).log10()
}

/// Noise figure (dB): NF = SNR_in(dB) - SNR_out(dB).
pub fn noise_figure_db(snr_in_db: Scalar, snr_out_db: Scalar) -> Scalar {
    snr_in_db - snr_out_db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_noise_psd_resistor() {
        // 1kΩ resistor at 300K
        let sv = thermal_noise_psd(1000.0, 300.0);
        // 4 * 1.38e-23 * 300 * 1000 = 1.656e-17 V²/Hz
        assert!((sv - 1.656e-17).abs() < 1e-19);
    }

    #[test]
    fn test_thermal_noise_zero_resistance() {
        let sv = thermal_noise_psd(0.0, 300.0);
        assert!((sv).abs() < 1e-30);
    }

    #[test]
    fn test_shot_noise_psd() {
        // 1mA DC current
        let si = shot_noise_psd(0.001);
        // 2 * 1.602e-19 * 0.001 = 3.204e-22 A²/Hz
        assert!((si - 3.204e-22).abs() < 1e-24);
    }

    #[test]
    fn test_shot_noise_zero_current() {
        let si = shot_noise_psd(0.0);
        assert!((si).abs() < 1e-30);
    }

    #[test]
    fn test_flicker_noise_psd() {
        let si = flicker_noise_psd(1e-10, 1.0, 0.001, 100.0);
        assert!(si > 0.0);
        // Should scale as 1/f
        let si_low_freq = flicker_noise_psd(1e-10, 1.0, 0.001, 10.0);
        assert!(si_low_freq > si);
    }

    #[test]
    fn test_rms_noise() {
        let v_rms = rms_noise_voltage(1e-12, 100.0, 10000.0);
        assert!(v_rms > 0.0);
        // Vrms = sqrt(1e-12 * 9900) ≈ 9.95e-5
        assert!((v_rms - 9.9498e-5).abs() < 1e-7);
    }

    #[test]
    fn test_snr_db_positive() {
        let snr = snr_db(1.0, 0.001);
        // 20 * log10(1000) = 60 dB
        assert!((snr - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_noise_figure() {
        let nf = noise_figure_db(60.0, 40.0);
        assert!((nf - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_thermal_noise_current_psd() {
        // 1mS conductance at 300K
        let si = thermal_noise_current_psd(0.001, 300.0);
        assert!(si > 0.0);
    }

    #[test]
    fn test_snr_zero_noise() {
        let snr = snr_db(1.0, 0.0);
        assert!(snr.is_infinite() && snr.is_sign_negative());
    }
}
