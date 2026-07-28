//! Wave optics: interference, diffraction, polarization, Gaussian beams.
//!
//! Provides models for double-slit/thin-film interference, single-slit/circular
//! aperture/grating diffraction, Fresnel equations, Malus' law, and Gaussian beams.

use crate::core::types::Scalar;

/// Polarization state of light.
#[derive(Debug, Clone, PartialEq)]
pub enum CircularPolarization {
    LeftHanded,
    RightHanded,
}

/// Polarization state description.
#[derive(Debug, Clone, PartialEq)]
pub enum PolarizationState {
    Linear { angle: Scalar },
    Circular { handedness: CircularPolarization },
    Elliptical { psi: Scalar, chi: Scalar },
    Unpolarized,
}

/// 2D wavefront amplitude/phase distribution.
#[derive(Debug, Clone)]
pub struct Wavefront {
    pub wavelength: Scalar,
    pub amplitude: Vec<Vec<Scalar>>,
    pub phase: Vec<Vec<Scalar>>,
    pub grid_size: (usize, usize),
    pub spacing: Scalar,
}

/// Double-slit interference intensity at screen position x.
///
/// I(x) = I₀ · cos²(π·d·x/(λ·L))
/// where d = slit separation, L = screen distance.
pub fn double_slit_intensity(
    x: Scalar,
    d: Scalar,
    lambda: Scalar,
    l: Scalar,
    i0: Scalar,
) -> Scalar {
    if lambda <= 0.0 || l <= 0.0 {
        return 0.0;
    }
    let arg = std::f64::consts::PI * d * x / (lambda * l);
    i0 * arg.cos().powi(2)
}

/// Thin-film interference intensity (normal incidence).
///
/// Returns normalized intensity accounting for film thickness and wavelength.
pub fn thin_film_interference(
    n_film: Scalar,
    thickness: Scalar,
    lambda: Scalar,
    n_incident: Scalar,
) -> Scalar {
    if lambda <= 0.0 {
        return 0.0;
    }
    let delta = 2.0 * n_film * thickness / lambda;
    let phase_shift = std::f64::consts::PI * 2.0 * delta;
    // Add π phase shift if n_film > n_incident (reflection at first interface)
    let extra_phase = if n_film > n_incident {
        std::f64::consts::PI
    } else {
        0.0
    };
    let total_phase = phase_shift + extra_phase;
    (1.0 + total_phase.cos()) / 2.0
}

/// Michelson interferometer output intensity.
///
/// I = I₀ · (1 + cos(2π·Δ/λ)) / 2
/// where Δ = path difference.
pub fn michelson_intensity(lambda: Scalar, path_diff: Scalar, i0: Scalar) -> Scalar {
    if lambda <= 0.0 {
        return 0.0;
    }
    let phase = 2.0 * std::f64::consts::PI * path_diff / lambda;
    i0 * (1.0 + phase.cos()) / 2.0
}

/// Single-slit Fraunhofer diffraction intensity.
///
/// I(θ) = I₀ · [sin(π·a·sinθ/λ) / (π·a·sinθ/λ)]²
pub fn single_slit_diffraction(
    theta: Scalar,
    slit_width: Scalar,
    lambda: Scalar,
    i0: Scalar,
) -> Scalar {
    if lambda <= 0.0 {
        return 0.0;
    }
    let arg = std::f64::consts::PI * slit_width * theta.sin() / lambda;
    if arg.abs() < 1e-15 {
        return i0;
    }
    i0 * (arg.sin() / arg).powi(2)
}

/// Circular aperture Fraunhofer diffraction (Airy pattern).
///
/// I(θ) = I₀ · [2·J₁(k·a·sinθ)/(k·a·sinθ)]²
/// Simplified: uses first zero at θ = 1.22·λ/D.
pub fn circular_aperture_diffraction(
    theta: Scalar,
    diameter: Scalar,
    lambda: Scalar,
    i0: Scalar,
) -> Scalar {
    if lambda <= 0.0 || diameter <= 0.0 {
        return 0.0;
    }
    let k_a = std::f64::consts::PI * diameter / lambda;
    let u = k_a * theta.sin();
    if u.abs() < 1e-15 {
        return i0;
    }
    // Approximation of Airy pattern using sinc-like behavior
    let j1_over_u = if u.abs() > 0.01 {
        // J₁(u)/u ≈ sin(u)/u² - cos(u)/u for large u
        // Use simplified: (2*J₁(u)/u)² ≈ (sin(u-0.25π)/u^1.5)² for far field
        let sinc = u.sin() / u;
        sinc * sinc
    } else {
        1.0 - u * u / 8.0 // small u expansion
    };
    i0 * j1_over_u * j1_over_u
}

/// Multi-slit grating diffraction intensity.
///
/// I(θ) = I₀ · [sin(N·π·d·sinθ/λ) / (N·sin(π·d·sinθ/λ))]² · [sin(π·a·sinθ/λ)/(π·a·sinθ/λ)]²
pub fn grating_diffraction(
    theta: Scalar,
    d: Scalar,
    n_slits: usize,
    lambda: Scalar,
    i0: Scalar,
) -> Scalar {
    if lambda <= 0.0 || n_slits == 0 {
        return 0.0;
    }
    let beta = std::f64::consts::PI * d * theta.sin() / lambda;
    let envelope = if beta.abs() < 1e-15 {
        1.0
    } else {
        let n = n_slits as Scalar;
        let num = (n * beta).sin();
        let den = n * beta.sin();
        (num / den).powi(2)
    };
    envelope * i0
}

/// Malus' law: I = I₀ · cos²θ.
pub fn malus_law(intensity: Scalar, angle: Scalar) -> Scalar {
    intensity * angle.cos().powi(2)
}

/// Brewster angle: θ_B = arctan(n₂/n₁).
pub fn brewster_angle(n1: Scalar, n2: Scalar) -> Scalar {
    (n2 / n1).atan()
}

/// Fresnel reflection coefficient (s and p polarization).
pub fn fresnel_reflection(
    n1: Scalar,
    n2: Scalar,
    theta_i: Scalar,
    _polarization: &PolarizationState,
) -> Scalar {
    // For normal incidence: R = ((n1 - n2)/(n1 + n2))²
    if theta_i.abs() < 1e-12 {
        let r = (n1 - n2) / (n1 + n2);
        return r * r;
    }
    let sin_t = n1 / n2 * theta_i.sin();
    if sin_t.abs() > 1.0 {
        return 1.0; // TIR
    }
    let theta_t = sin_t.asin();
    // Average of s and p reflectivity as simplification
    let rs = ((n1 * theta_i.cos() - n2 * theta_t.cos())
        / (n1 * theta_i.cos() + n2 * theta_t.cos()))
    .powi(2);
    let rp = ((n1 * theta_t.cos() - n2 * theta_i.cos())
        / (n1 * theta_t.cos() + n2 * theta_i.cos()))
    .powi(2);
    (rs + rp) / 2.0
}

/// Fresnel transmission coefficient.
pub fn fresnel_transmission(
    n1: Scalar,
    n2: Scalar,
    theta_i: Scalar,
    polarization: &PolarizationState,
) -> Scalar {
    1.0 - fresnel_reflection(n1, n2, theta_i, polarization)
}

/// Gaussian beam parameters.
#[derive(Debug, Clone)]
pub struct GaussianBeam {
    pub wavelength: Scalar,
    pub w0: Scalar, // waist radius (m)
    pub z0: Scalar, // waist position (m)
}

impl GaussianBeam {
    pub fn new(wavelength: Scalar, w0: Scalar, z0: Scalar) -> Self {
        Self { wavelength, w0, z0 }
    }

    /// Rayleigh range: z_R = π·w₀²/λ.
    pub fn rayleigh_range(&self) -> Scalar {
        if self.wavelength <= 0.0 {
            return 0.0;
        }
        std::f64::consts::PI * self.w0 * self.w0 / self.wavelength
    }

    /// Beam radius at position z: w(z) = w₀·√(1 + (z-z₀)²/z_R²).
    pub fn beam_radius(&self, z: Scalar) -> Scalar {
        let zr = self.rayleigh_range();
        if zr <= 0.0 {
            return self.w0;
        }
        let dz = z - self.z0;
        self.w0 * (1.0 + (dz * dz) / (zr * zr)).sqrt()
    }

    /// Radius of curvature at position z: R(z) = z·(1 + (z_R/z)²).
    pub fn curvature_radius(&self, z: Scalar) -> Scalar {
        let zr = self.rayleigh_range();
        let dz = z - self.z0;
        if dz.abs() < 1e-15 {
            return Scalar::INFINITY;
        }
        dz * (1.0 + (zr / dz).powi(2))
    }

    /// Gouy phase shift: ζ(z) = arctan(z/z_R).
    pub fn gouy_phase(&self, z: Scalar) -> Scalar {
        let zr = self.rayleigh_range();
        if zr <= 0.0 {
            return 0.0;
        }
        ((z - self.z0) / zr).atan()
    }

    /// Intensity at radial position r, axial position z.
    pub fn intensity(&self, r: Scalar, z: Scalar) -> Scalar {
        let wz = self.beam_radius(z);
        if wz <= 0.0 {
            return 0.0;
        }
        let i0 = 2.0 / (std::f64::consts::PI * self.w0 * self.w0);
        i0 * (-2.0 * r * r / (wz * wz)).exp()
    }

    /// Beam divergence angle (half-angle, far field): θ = λ/(π·w₀).
    pub fn divergence_angle(&self) -> Scalar {
        if self.w0 <= 0.0 {
            return 0.0;
        }
        self.wavelength / (std::f64::consts::PI * self.w0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_double_slit_constructive_interference() {
        // Central maximum (x=0) should be I₀
        let i = double_slit_intensity(0.0, 1e-3, 500e-9, 1.0, 1.0);
        assert!((i - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_double_slit_destructive_interference() {
        // First minimum at x = λ·L/(2d)
        let d = 1e-3;
        let lambda = 500e-9;
        let l = 1.0;
        let x_min = lambda * l / (2.0 * d);
        let i = double_slit_intensity(x_min, d, lambda, l, 1.0);
        assert!(i < 0.01);
    }

    #[test]
    fn test_single_slit_central_maximum() {
        let i = single_slit_diffraction(0.0, 0.1e-3, 500e-9, 1.0);
        assert!((i - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_single_slit_first_minimum() {
        let a = 0.1e-3;
        let lambda = 500e-9;
        // First minimum at sinθ = λ/a
        let theta = f64::asin(lambda / a);
        let i = single_slit_diffraction(theta, a, lambda, 1.0);
        assert!(i < 0.01);
    }

    #[test]
    fn test_malus_law() {
        assert!((malus_law(1.0, 0.0) - 1.0).abs() < 1e-12);
        assert!((malus_law(1.0, std::f64::consts::FRAC_PI_2) - 0.0).abs() < 1e-12);
        assert!((malus_law(1.0, std::f64::consts::FRAC_PI_4) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_brewster_angle() {
        let theta_b = brewster_angle(1.0, 1.5);
        assert!((theta_b - (1.5_f64).atan()).abs() < 1e-12);
    }

    #[test]
    fn test_fresnel_normal_incidence() {
        let r = fresnel_reflection(1.0, 1.5, 0.0, &PolarizationState::Unpolarized);
        let expected = f64::powi((1.0 - 1.5) / (1.0 + 1.5), 2);
        assert!((r - expected).abs() < 1e-12);
    }

    #[test]
    fn test_gaussian_beam_rayleigh_range() {
        let beam = GaussianBeam::new(500e-9, 1e-3, 0.0);
        let zr = beam.rayleigh_range();
        let expected = std::f64::consts::PI * 1e-6 / 500e-9;
        assert!((zr - expected).abs() < 1e-6);
    }

    #[test]
    fn test_gaussian_beam_waist() {
        let beam = GaussianBeam::new(500e-9, 1e-3, 0.0);
        let w_at_waist = beam.beam_radius(0.0);
        assert!((w_at_waist - 1e-3).abs() < 1e-12);
    }

    #[test]
    fn test_gaussian_beam_far_field() {
        let beam = GaussianBeam::new(500e-9, 1e-3, 0.0);
        let zr = beam.rayleigh_range();
        let w_far = beam.beam_radius(zr * 10.0);
        // In far field, w(z) ≈ w₀·z/z_R
        let expected = 1e-3 * 10.0 * zr / zr;
        assert!((w_far / expected - 1.0).abs() < 0.02);
    }

    #[test]
    fn test_michelson_intensity() {
        let i = michelson_intensity(500e-9, 0.0, 1.0);
        assert!((i - 1.0).abs() < 1e-12);
        let i_half = michelson_intensity(500e-9, 250e-9, 1.0); // λ/2 path diff
        assert!((i_half - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_thin_film_interference() {
        // Half-wave thickness gives constructive interference
        let n_film = 1.38;
        let lambda = 550e-9;
        let thickness = lambda / (2.0 * n_film); // half-wave
        let i = thin_film_interference(n_film, thickness, lambda, 1.0);
        // Half-wave: 2*n*d = λ, delta = 1, phase = 2π, cos(2π) = 1
        // (1 + cos(2π + π_extra)) / 2 — for n_film > n_incident, extra_phase = π
        // So cos(2π + π) = cos(3π) = -1, giving (1 + (-1))/2 = 0
        // When n_film < n_incident, extra_phase = 0, cos(2π) = 1, giving 1
        // MgF₂ (1.38) > air (1.0) → 0.0
        assert!((i - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_grating_diffraction_orders() {
        let d = 1.0 / 600e3; // 600 lines/mm
        let lambda = 500e-9;
        let n_slits = 10;
        // Central maximum
        let i0 = grating_diffraction(0.0, d, n_slits, lambda, 1.0);
        assert!((i0 - 1.0).abs() < 1e-10);
        // First order: sinθ = λ/d
        let theta1 = (lambda / d).asin();
        let i1 = grating_diffraction(theta1, d, n_slits, lambda, 1.0);
        assert!(i1 > 0.5);
    }

    #[test]
    fn test_circular_aperture_central() {
        let i = circular_aperture_diffraction(0.0, 0.01, 500e-9, 1.0);
        assert!((i - 1.0).abs() < 1e-10);
    }
}
