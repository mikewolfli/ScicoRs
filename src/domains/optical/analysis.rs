//! Optical system analysis: resolution, aberrations, MTF, efficiency.

use crate::core::types::Scalar;

/// Rayleigh criterion angular resolution (rad): θ = 1.22·λ/D.
pub fn rayleigh_criterion(diameter: Scalar, lambda: Scalar) -> Scalar {
    if diameter <= 0.0 || lambda <= 0.0 {
        return 0.0;
    }
    1.22 * lambda / diameter
}

/// Simplified aberration coefficients.
#[derive(Debug, Clone)]
pub struct AberrationEstimator {
    pub spherical: Scalar,
    pub coma: Scalar,
    pub astigmatism: Scalar,
}

impl AberrationEstimator {
    pub fn new(spherical: Scalar, coma: Scalar, astigmatism: Scalar) -> Self {
        Self { spherical, coma, astigmatism }
    }

    /// Estimate RMS wavefront error for given field angle.
    pub fn rms_error(&self, field_angle: Scalar, aperture_radius: Scalar) -> Scalar {
        let w_spherical = self.spherical * aperture_radius.powi(4);
        let w_coma = self.coma * field_angle * aperture_radius.powi(3);
        let w_astig = self.astigmatism * field_angle.powi(2) * aperture_radius.powi(2);
        (w_spherical * w_spherical + w_coma * w_coma + w_astig * w_astig).sqrt()
    }

    /// Strehl ratio approximation: S ≈ exp(-(2π·σ/λ)²).
    pub fn strehl_ratio(&self, field_angle: Scalar, aperture_radius: Scalar, lambda: Scalar) -> Scalar {
        if lambda <= 0.0 {
            return 0.0;
        }
        let sigma = self.rms_error(field_angle, aperture_radius);
        (-(2.0 * std::f64::consts::PI * sigma / lambda).powi(2)).exp()
    }
}

/// Modulation Transfer Function (simplified diffraction-limited).
///
/// For a circular aperture, MTF(f) = 2/π·(acos(f/f_c) - f/f_c·√(1-(f/f_c)²))
/// where f_c = D/(λ·f) is the cutoff frequency.
pub fn modulation_transfer_function(spatial_freq: Scalar, aperture: Scalar, lambda: Scalar) -> Scalar {
    if lambda <= 0.0 || aperture <= 0.0 {
        return 0.0;
    }
    let f_c = aperture / lambda; // cutoff frequency (cycles/m)
    if spatial_freq <= 0.0 {
        return 1.0;
    }
    if spatial_freq >= f_c {
        return 0.0;
    }
    let nu = spatial_freq / f_c;
    let acos_nu = nu.acos();
    2.0 / std::f64::consts::PI * (acos_nu - nu * (1.0 - nu * nu).sqrt())
}

/// System transmittance: product of individual element transmittances.
pub fn system_transmittance(elements: &[Scalar]) -> Scalar {
    let mut t = 1.0;
    for &el in elements {
        t *= el;
    }
    t
}

/// Optical efficiency: η = P_out / P_in.
pub fn optical_efficiency(transmitted_power: Scalar, incident_power: Scalar) -> Scalar {
    if incident_power <= 0.0 {
        return 0.0;
    }
    transmitted_power / incident_power
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rayleigh_criterion() {
        let theta = rayleigh_criterion(0.1, 500e-9);
        let expected = 1.22 * 500e-9 / 0.1;
        assert!((theta - expected).abs() < 1e-15);
    }

    #[test]
    fn test_mtf_diffraction_limited() {
        let mtf0 = modulation_transfer_function(0.0, 0.05, 500e-9);
        assert!((mtf0 - 1.0).abs() < 1e-10);
        let f_c = 0.05 / 500e-9;
        let mtf_cutoff = modulation_transfer_function(f_c * 0.999, 0.05, 500e-9);
        assert!(mtf_cutoff > 0.0 && mtf_cutoff < 1.0);
        let mtf_beyond = modulation_transfer_function(f_c * 1.1, 0.05, 500e-9);
        assert!((mtf_beyond - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_system_transmittance() {
        let t = system_transmittance(&[0.9, 0.8, 0.95]);
        assert!((t - 0.684).abs() < 0.001);
    }

    #[test]
    fn test_optical_efficiency() {
        let eta = optical_efficiency(0.8, 1.0);
        assert!((eta - 0.8).abs() < 1e-12);
    }

    #[test]
    fn test_aberration_strehl_ratio() {
        let ab = AberrationEstimator::new(0.01, 0.005, 0.003);
        let s = ab.strehl_ratio(0.05, 0.01, 500e-9);
        assert!(s > 0.0 && s <= 1.0);
    }

    #[test]
    fn test_aberration_rms_error() {
        let ab = AberrationEstimator::new(0.01, 0.0, 0.0);
        let rms = ab.rms_error(0.0, 0.01);
        let expected = 0.01 * 0.01_f64.powi(4);
        assert!((rms - expected).abs() < 1e-12);
    }
}
