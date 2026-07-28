//! Electromagnetic field physical constants.

use crate::core::types::Scalar;

/// Speed of light in vacuum (m/s).
pub const C: Scalar = 299792458.0;

/// Vacuum permittivity (F/m).
pub const EPSILON_0: Scalar = 8.854187817e-12;

/// Vacuum permeability (H/m).
pub const MU_0: Scalar = 1.25663706212e-6;

/// Vacuum impedance (Ω).
pub const Z0: Scalar = 376.730313668;

/// Wave number: k = 2π/λ.
pub fn wave_number(lambda: Scalar) -> Scalar {
    if lambda <= 0.0 { return 0.0; }
    2.0 * std::f64::consts::PI / lambda
}

/// Wavelength: λ = c/f.
pub fn wavelength(freq: Scalar) -> Scalar {
    if freq <= 0.0 { return Scalar::INFINITY; }
    C / freq
}

/// Skin depth: δ = √(2/(ω·μ·σ)).
pub fn skin_depth(freq: Scalar, mu: Scalar, sigma: Scalar) -> Scalar {
    if freq <= 0.0 || sigma <= 0.0 { return Scalar::INFINITY; }
    let omega = 2.0 * std::f64::consts::PI * freq;
    f64::sqrt(2.0 / (omega * mu * sigma))
}

/// Wave impedance in medium: η = √(μ/ε).
pub fn wave_impedance(mu: Scalar, epsilon: Scalar) -> Scalar {
    if epsilon <= 0.0 { return 0.0; }
    f64::sqrt(mu / epsilon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wavelength() {
        let lam = wavelength(1e9); // 1 GHz
        assert!((lam - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_wave_number() {
        let k = wave_number(0.3);
        assert!((k - 2.0 * std::f64::consts::PI / 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_skin_depth_copper() {
        let mu0 = 1.25663706212e-6;
        let sigma_cu = 5.8e7;
        let sd = skin_depth(1e6, mu0, sigma_cu);
        assert!(sd > 1e-6 && sd < 1e-3); // ~66 μm at 1 MHz
    }

    #[test]
    fn test_wave_impedance_vacuum() {
        let eta = wave_impedance(MU_0, EPSILON_0);
        assert!((eta - Z0).abs() / Z0 < 1e-6);
    }
}
