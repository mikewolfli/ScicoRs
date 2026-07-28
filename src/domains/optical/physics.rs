//! Optical physical constants, spectral bands, and refractive index models.

use crate::core::types::Scalar;

/// Speed of light in vacuum (m/s).
pub const C: Scalar = 299792458.0;

/// Planck constant (J·s).
pub const H_PLANCK: Scalar = 6.62607015e-34;

/// Vacuum permittivity (F/m).
pub const EPSILON_0: Scalar = 8.854187817e-12;

/// Vacuum permeability (H/m).
pub const MU_0: Scalar = 1.25663706212e-6;

/// Spectral band classification by wavelength range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpectralBand {
    /// 10-400 nm
    Ultraviolet,
    /// 400-700 nm
    Visible,
    /// 700-2500 nm
    NearInfrared,
    /// 2.5-50 μm
    MidInfrared,
    /// 50-1000 μm
    FarInfrared,
}

/// Classify wavelength (in meters) into a spectral band.
pub fn classify_spectral_band(lambda: Scalar) -> SpectralBand {
    let nm = lambda * 1e9;
    if nm < 400.0 {
        SpectralBand::Ultraviolet
    } else if nm < 700.0 {
        SpectralBand::Visible
    } else if nm < 2500.0 {
        SpectralBand::NearInfrared
    } else if nm < 50_000.0 {
        SpectralBand::MidInfrared
    } else {
        SpectralBand::FarInfrared
    }
}

/// Convert wavelength (m) to frequency (Hz): f = c/λ.
pub fn wavelength_to_freq(lambda: Scalar) -> Scalar {
    if lambda <= 0.0 {
        return 0.0;
    }
    C / lambda
}

/// Convert frequency (Hz) to wavelength (m): λ = c/f.
pub fn freq_to_wavelength(freq: Scalar) -> Scalar {
    if freq <= 0.0 {
        return 0.0;
    }
    C / freq
}

/// Photon energy (J): E = h·c/λ.
pub fn photon_energy(lambda: Scalar) -> Scalar {
    if lambda <= 0.0 {
        return 0.0;
    }
    H_PLANCK * C / lambda
}

/// Trait for wavelength-dependent refractive index models.
pub trait RefractiveIndex: Send + Sync {
    /// Refractive index at the given wavelength (m).
    fn n(&self, wavelength: Scalar) -> Scalar;
}

/// Constant refractive index independent of wavelength.
pub struct ConstantRefractiveIndex {
    pub n: Scalar,
}

impl RefractiveIndex for ConstantRefractiveIndex {
    fn n(&self, _wavelength: Scalar) -> Scalar {
        self.n
    }
}

/// Sellmeier dispersion model.
///
/// n²(λ) = 1 + Σᵢ Bᵢ·λ² / (λ² - Cᵢ)
/// where λ is in μm.
pub struct SellmeierModel {
    /// Vector of (B_i, C_i) coefficient pairs.
    pub coefficients: Vec<(Scalar, Scalar)>,
}

impl RefractiveIndex for SellmeierModel {
    fn n(&self, wavelength: Scalar) -> Scalar {
        let lambda_um = wavelength * 1e6; // convert m to μm
        let lambda2 = lambda_um * lambda_um;
        let mut sum = 1.0;
        for &(b, c) in &self.coefficients {
            let denom = lambda2 - c;
            if denom.abs() < 1e-20 {
                continue;
            }
            sum += b * lambda2 / denom;
        }
        sum.max(1.0).sqrt()
    }
}

/// Fused silica (SiO₂) Sellmeier model.
pub fn fused_silica() -> SellmeierModel {
    SellmeierModel {
        coefficients: vec![
            (0.6961663, 0.0684043_f64.powi(2)),
            (0.4079426, 0.1162414_f64.powi(2)),
            (0.8974794, 9.896161_f64.powi(2)),
        ],
    }
}

/// BK7 glass Sellmeier model.
pub fn bk7_glass() -> SellmeierModel {
    SellmeierModel {
        coefficients: vec![
            (1.03961212, 0.00600069867),
            (0.231792344, 0.0200179144),
            (1.01046945, 103.560653),
        ],
    }
}

/// Silicon (Si) constant refractive index (near IR, ~3.5).
pub fn silicon_n() -> ConstantRefractiveIndex {
    ConstantRefractiveIndex { n: 3.5 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wavelength_to_freq() {
        let lambda = 500e-9; // 500 nm
        let f = wavelength_to_freq(lambda);
        let expected = C / lambda;
        assert!((f - expected).abs() < 1e-6);
    }

    #[test]
    fn test_freq_to_wavelength() {
        let f = 3e14; // 300 THz
        let lambda = freq_to_wavelength(f);
        let expected = C / f;
        assert!((lambda - expected).abs() < 1e-20);
    }

    #[test]
    fn test_photon_energy_visible() {
        // 500 nm photon energy ≈ 3.97e-19 J
        let e = photon_energy(500e-9);
        assert!((e - 3.97e-19).abs() / 3.97e-19 < 0.02);
    }

    #[test]
    fn test_zero_wavelength_returns_zero() {
        assert_eq!(wavelength_to_freq(0.0), 0.0);
        assert_eq!(photon_energy(0.0), 0.0);
    }

    #[test]
    fn test_constant_refractive_index() {
        let ri = ConstantRefractiveIndex { n: 1.5 };
        assert!((ri.n(500e-9) - 1.5).abs() < 1e-12);
    }

    #[test]
    fn test_sellmeier_fused_silica() {
        let silica = fused_silica();
        let n = silica.n(587.6e-9); // 587.6 nm (HeNe)
        assert!((n - 1.458).abs() < 0.01);
    }

    #[test]
    fn test_sellmeier_bk7() {
        let bk7 = bk7_glass();
        let n = bk7.n(587.6e-9);
        assert!((n - 1.5168).abs() < 0.01);
    }

    #[test]
    fn test_classify_spectral_band() {
        assert_eq!(classify_spectral_band(300e-9), SpectralBand::Ultraviolet);
        assert_eq!(classify_spectral_band(500e-9), SpectralBand::Visible);
        assert_eq!(classify_spectral_band(1000e-9), SpectralBand::NearInfrared);
        assert_eq!(classify_spectral_band(10e-6), SpectralBand::MidInfrared);
        assert_eq!(classify_spectral_band(100e-6), SpectralBand::FarInfrared);
    }
}
