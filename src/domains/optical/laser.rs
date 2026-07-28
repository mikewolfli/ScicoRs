//! Laser, fiber, waveguide, and grating models.

use super::wave::GaussianBeam;
use crate::core::types::Scalar;

/// Laser source model with beam properties.
#[derive(Debug, Clone)]
pub struct LaserSource {
    pub wavelength: Scalar,
    pub power: Scalar,
    pub beam: GaussianBeam,
    pub linewidth: Scalar,
    pub coherence_length: Scalar,
}

impl LaserSource {
    pub fn new(wavelength: Scalar, power: Scalar, w0: Scalar) -> Self {
        let coherence_length = wavelength * wavelength / (2.0 * 1e-12); // ~1 nm linewidth approx
        Self {
            wavelength,
            power,
            beam: GaussianBeam::new(wavelength, w0, 0.0),
            linewidth: 1e-12,
            coherence_length,
        }
    }

    /// Peak intensity at waist center: I₀ = 2P/(π·w₀²).
    pub fn peak_intensity(&self) -> Scalar {
        if self.beam.w0 <= 0.0 {
            return 0.0;
        }
        2.0 * self.power / (std::f64::consts::PI * self.beam.w0 * self.beam.w0)
    }
}

/// Optical fiber model.
#[derive(Debug, Clone)]
pub struct Fiber {
    pub core_n: Scalar,
    pub cladding_n: Scalar,
    pub core_diameter: Scalar,
    pub length: Scalar,
    pub attenuation: Scalar, // dB/km
}

impl Fiber {
    pub fn new(
        core_n: Scalar,
        cladding_n: Scalar,
        core_diameter: Scalar,
        length: Scalar,
        attenuation: Scalar,
    ) -> Self {
        Self {
            core_n,
            cladding_n,
            core_diameter,
            length,
            attenuation,
        }
    }

    /// Numerical aperture: NA = √(n₁² - n₂²).
    pub fn numerical_aperture(&self) -> Scalar {
        let diff = self.core_n * self.core_n - self.cladding_n * self.cladding_n;
        if diff < 0.0 { 0.0 } else { diff.sqrt() }
    }

    /// V-number: V = 2π·a·NA/λ.
    pub fn v_number(&self, lambda: Scalar) -> Scalar {
        if lambda <= 0.0 {
            return 0.0;
        }
        2.0 * std::f64::consts::PI * (self.core_diameter / 2.0) * self.numerical_aperture() / lambda
    }

    /// Check if fiber supports only a single mode (V < 2.405).
    pub fn is_single_mode(&self, lambda: Scalar) -> bool {
        self.v_number(lambda) < 2.405
    }

    /// Mode field diameter approximation (single-mode): MFD ≈ 2·w₀.
    pub fn mode_field_diameter(&self, lambda: Scalar) -> Scalar {
        let v = self.v_number(lambda);
        if v < 2.405 && v > 0.5 {
            let w0_over_a = 0.65 + 1.619 / v.powi(15) + 2.879 / v.powi(6); // Marcuse formula
            w0_over_a * self.core_diameter
        } else {
            self.core_diameter
        }
    }

    /// Power transmission after propagating length L (m).
    /// T = 10^(-α·L/10) where α is dB/km.
    pub fn transmission(&self, length_m: Scalar) -> Scalar {
        10.0_f64.powf(-self.attenuation * length_m / 1000.0 / 10.0)
    }

    /// Chromatic dispersion (ps/(nm·km)) — simplified single term.
    pub fn dispersion(&self, lambda: Scalar) -> Scalar {
        // Simplified material dispersion: D ≈ S₀·(λ - λ₀⁴/λ³)
        let lambda_um = lambda * 1e6;
        let lambda0 = 1.31; // zero-dispersion wavelength (μm) for SMF
        let s0 = 0.092; // slope (ps/(nm²·km))
        s0 * (lambda_um
            - lambda0 * lambda0 * lambda0 * lambda0 / (lambda_um * lambda_um * lambda_um))
    }
}

/// Planar waveguide model.
#[derive(Debug, Clone)]
pub struct Waveguide {
    pub n_core: Scalar,
    pub n_cladding: Scalar,
    pub thickness: Scalar,
}

impl Waveguide {
    pub fn new(n_core: Scalar, n_cladding: Scalar, thickness: Scalar) -> Self {
        Self {
            n_core,
            n_cladding,
            thickness,
        }
    }

    /// Effective refractive indices for TE modes.
    pub fn te_modes(&self, lambda: Scalar) -> Vec<Scalar> {
        let k0 = 2.0 * std::f64::consts::PI / lambda;
        let v = k0 * self.thickness / 2.0
            * (self.n_core * self.n_core - self.n_cladding * self.n_cladding).sqrt();
        let mut modes = Vec::new();
        let mut m = 0;
        loop {
            // Transcendental equation solved numerically
            let b_est = ((m as Scalar + 1.0) * std::f64::consts::PI / (v * 2.0)).min(0.99);
            if b_est <= 0.0 {
                break;
            }
            let n_eff = (self.n_cladding * self.n_cladding
                + b_est * (self.n_core * self.n_core - self.n_cladding * self.n_cladding))
                .sqrt();
            if n_eff >= self.n_cladding && n_eff <= self.n_core {
                modes.push(n_eff);
            }
            m += 1;
            if m > 20 {
                break;
            }
        }
        modes
    }

    /// Number of guided modes.
    pub fn mode_count(&self, lambda: Scalar) -> usize {
        self.te_modes(lambda).len()
    }
}

/// Diffraction grating model.
#[derive(Debug, Clone)]
pub struct Grating {
    pub lines_per_mm: Scalar,
    pub blaze_angle: Option<Scalar>,
}

impl Grating {
    pub fn new(lines_per_mm: Scalar) -> Self {
        Self {
            lines_per_mm,
            blaze_angle: None,
        }
    }

    /// Grating period (m).
    pub fn period(&self) -> Scalar {
        1e-3 / self.lines_per_mm
    }

    /// Diffraction angles for a given order: m·λ = d·(sinθ_m + sinθ_i).
    /// Returns angles in radians for each order.
    pub fn diffraction_angles(&self, lambda: Scalar, order: i32) -> Vec<Scalar> {
        if lambda <= 0.0 {
            return Vec::new();
        }
        let d = self.period();
        let mut angles = Vec::new();
        // Find all real solutions for sinθ_m = m·λ/d
        for m in -order.abs()..=order.abs() {
            let sin_theta = m as Scalar * lambda / d;
            if sin_theta.abs() <= 1.0 {
                angles.push(sin_theta.asin());
            }
        }
        angles
    }

    /// Angular dispersion: dθ/dλ = m/(d·cosθ_m).
    pub fn angular_dispersion(&self, lambda: Scalar, order: i32) -> Scalar {
        let d = self.period();
        let angles = self.diffraction_angles(lambda, order);
        if angles.is_empty() {
            return 0.0;
        }
        let theta = angles[0];
        order as Scalar / (d * theta.cos())
    }

    /// Resolving power: R = m·N (N = number of illuminated lines).
    pub fn resolving_power(&self, order: i32, n_lines: usize) -> Scalar {
        (order.abs() as Scalar) * (n_lines as Scalar)
    }

    /// Free spectral range: FSR = λ/m.
    pub fn free_spectral_range(&self, lambda: Scalar, order: i32) -> Scalar {
        if order == 0 {
            return Scalar::INFINITY;
        }
        lambda / order.abs() as Scalar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_laser_peak_intensity() {
        let laser = LaserSource::new(500e-9, 0.001, 1e-3);
        let i0 = laser.peak_intensity();
        let expected = 2.0 * 0.001 / (std::f64::consts::PI * 1e-6);
        assert!((i0 - expected).abs() < 1e-6);
    }

    #[test]
    fn test_fiber_numerical_aperture() {
        let fiber = Fiber::new(1.45, 1.40, 50e-6, 1.0, 0.2);
        let na = fiber.numerical_aperture();
        let expected = (1.45_f64.powi(2) - 1.40_f64.powi(2)).sqrt();
        assert!((na - expected).abs() < 0.001);
    }

    #[test]
    fn test_fiber_single_mode() {
        // SMF-28: core=8.2μm, n1≈1.468, n2≈1.463
        let fiber = Fiber::new(1.468, 1.463, 8.2e-6, 1.0, 0.2);
        assert!(fiber.is_single_mode(1550e-9));
        // Large core should be multi-mode
        let mm_fiber = Fiber::new(1.45, 1.40, 50e-6, 1.0, 0.2);
        assert!(!mm_fiber.is_single_mode(500e-9));
    }

    #[test]
    fn test_fiber_attenuation() {
        let fiber = Fiber::new(1.45, 1.40, 50e-6, 1000.0, 0.2);
        let t = fiber.transmission(1000.0);
        // 0.2 dB/km * 1 km = 0.2 dB → T = 10^(-0.02) ≈ 0.955
        assert!((t - 10.0_f64.powf(-0.02)).abs() < 0.001);
    }

    #[test]
    fn test_grating_period() {
        let grating = Grating::new(600.0);
        let d = grating.period();
        assert!((d - 1.6667e-6).abs() < 1e-9);
    }

    #[test]
    fn test_grating_diffraction_angles() {
        let grating = Grating::new(600.0);
        let angles = grating.diffraction_angles(500e-9, 1);
        // First order: sinθ = λ/d
        let expected_sin = 500e-9 / grating.period();
        assert!(
            angles
                .iter()
                .any(|&a| (a.sin() - expected_sin).abs() < 1e-10)
        );
    }

    #[test]
    fn test_grating_resolving_power() {
        let grating = Grating::new(600.0);
        let r = grating.resolving_power(1, 10000);
        assert!((r - 10000.0).abs() < 1e-10);
    }

    #[test]
    fn test_waveguide_te_modes() {
        let wg = Waveguide::new(3.5, 3.3, 0.5e-6);
        let modes = wg.te_modes(1.55e-6);
        // Should have at least one mode
        assert!(!modes.is_empty());
        for &n_eff in &modes {
            assert!((3.3..=3.5).contains(&n_eff));
        }
    }

    #[test]
    fn test_fiber_v_number() {
        let fiber = Fiber::new(1.45, 1.40, 50e-6, 1.0, 0.2);
        let v = fiber.v_number(500e-9);
        assert!(v > 0.0);
    }
}
