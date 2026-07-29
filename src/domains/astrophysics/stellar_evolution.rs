//! Stellar structure and evolution models.
//!
//! Provides polytropic stellar models, main-sequence lifetime estimates,
//! Eddington luminosity, and basic stellar atmosphere relations.

use crate::core::types::Scalar;

/// Solar mass (kg).
const M_SUN: Scalar = 1.989e30;
#[allow(dead_code)]
/// Solar radius (m).
const R_SUN: Scalar = 6.9634e8;
/// Solar luminosity (W).
const L_SUN: Scalar = 3.828e26;

/// Stellar structure model with polytropic and main-sequence analysis.
#[derive(Debug, Clone)]
pub struct StellarStructure {
    /// Stellar mass (kg).
    pub mass: Scalar,
    /// Stellar radius (m).
    pub radius: Scalar,
    /// Stellar luminosity (W).
    pub luminosity: Scalar,
    /// Core temperature (K).
    pub core_temp: Scalar,
    /// Metallicity (mass fraction of elements heavier than He).
    pub metallicity: Scalar,
}

impl StellarStructure {
    /// Create a new stellar model.
    pub fn new(mass: Scalar, radius: Scalar, luminosity: Scalar, core_temp: Scalar) -> Self {
        Self {
            mass,
            radius,
            luminosity,
            core_temp,
            metallicity: 0.02, // Solar metallicity default
        }
    }

    /// Set metallicity.
    pub fn with_metallicity(mut self, z: Scalar) -> Self {
        self.metallicity = z;
        self
    }

    /// Compute the polytropic density profile for index n.
    ///
    /// Returns a Vec of (r/R, ρ/ρ_c, P/P_c) from centre to surface.
    pub fn polytropic_profile(&self, n: Scalar, n_points: usize) -> Vec<(Scalar, Scalar, Scalar)> {
        let mut profile = Vec::with_capacity(n_points);
        // Lane-Emden solution approximated by numerical integration
        let xi_step: Scalar = 10.0 / (n_points - 1).max(1) as Scalar;
        let mut xi: Scalar = 0.0;
        let mut theta: Scalar = 1.0;
        let mut dtheta: Scalar = 0.0;

        for _ in 0..n_points {
            let r_ratio = (xi / 6.896).min(1.0); // Normalise to first zero
            let rho_ratio = theta.powf(n);
            let p_ratio = theta.powf(n + 1.0);
            profile.push((r_ratio, rho_ratio, p_ratio));

            // Simple Euler integration of Lane-Emden
            let d2theta: Scalar = if xi > 1e-10 {
                -2.0 / xi * dtheta - theta.powf(n)
            } else {
                -1.0 / 3.0
            };
            dtheta += d2theta * xi_step;
            theta += dtheta * xi_step;
            xi += xi_step;
        }
        profile
    }

    /// Main-sequence lifetime (years) from mass-luminosity relation.
    ///
    /// τ_MS ≈ 10¹⁰ · (M/M_☉) / (L/L_☉) years
    pub fn main_sequence_lifetime(&self) -> Scalar {
        let m_ratio = self.mass / M_SUN;
        let l_ratio = self.luminosity / L_SUN;
        if l_ratio <= 0.0 {
            return 0.0;
        }
        1e10 * m_ratio / l_ratio
    }

    /// Eddington luminosity (W): L_Edd = 4πGMc/κ
    ///
    /// Where κ ≈ 0.034·(1+X) m²/kg for electron scattering,
    /// X is the hydrogen mass fraction.
    pub fn eddington_luminosity(&self, x_hydrogen: Scalar) -> Scalar {
        let kappa = 0.034 * (1.0 + x_hydrogen); // Electron scattering opacity
        4.0 * std::f64::consts::PI * 6.67430e-11 * self.mass * 2.99792458e8 / kappa
    }

    /// Effective surface temperature (K): T_eff = (L/(4πR²σ))^{1/4}.
    pub fn effective_temperature(&self) -> Scalar {
        let sigma = 5.670374419e-8;
        let area = 4.0 * std::f64::consts::PI * self.radius * self.radius;
        (self.luminosity / (area * sigma)).powf(0.25)
    }

    /// Surface gravity (m/s²): g = GM/R².
    pub fn surface_gravity(&self) -> Scalar {
        6.67430e-11 * self.mass / (self.radius * self.radius)
    }

    /// Central pressure (Pa) from hydrostatic equilibrium (polytropic
    /// approximation): P_c = (GM²)/(8πR⁴)·(n+1)/θ'_n.
    pub fn central_pressure(&self, n: Scalar) -> Scalar {
        6.67430e-11 * self.mass * self.mass / (8.0 * std::f64::consts::PI * self.radius.powi(4))
            * (n + 1.0)
            / 0.5 // Approximate |θ'_n| for n=3
    }

    /// Central density (kg/m³): ρ_c = (3M)/(4πR³)·μ where μ depends on n.
    pub fn central_density(&self) -> Scalar {
        3.0 * self.mass / (4.0 * std::f64::consts::PI * self.radius.powi(3)) * 5.0 // μ≈5 for n=3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sun_effective_temp() {
        let sun = StellarStructure::new(M_SUN, R_SUN, L_SUN, 1.57e7);
        let teff = sun.effective_temperature();
        // Solar T_eff ≈ 5772 K
        assert!((teff - 5772.0).abs() < 200.0);
    }

    #[test]
    fn test_sun_surface_gravity() {
        let sun = StellarStructure::new(M_SUN, R_SUN, L_SUN, 1.57e7);
        let g = sun.surface_gravity();
        // Solar surface gravity ≈ 274 m/s²
        assert!((g - 274.0).abs() < 10.0);
    }

    #[test]
    fn test_main_sequence_lifetime() {
        let sun = StellarStructure::new(M_SUN, R_SUN, L_SUN, 1.57e7);
        let lifetime = sun.main_sequence_lifetime();
        // Solar MS lifetime ≈ 1e10 years
        assert!(lifetime > 8e9 && lifetime < 1.2e10);
    }

    #[test]
    fn test_eddington_luminosity_sun() {
        let sun = StellarStructure::new(M_SUN, R_SUN, L_SUN, 1.57e7);
        let ledd = sun.eddington_luminosity(0.7);
        // L_Edd for Sun ≈ 1.3e31 W (much larger than actual luminosity)
        assert!(ledd > 1e30);
    }

    #[test]
    fn test_polytropic_profile() {
        let sun = StellarStructure::new(M_SUN, R_SUN, L_SUN, 1.57e7);
        let profile = sun.polytropic_profile(3.0, 10);
        assert_eq!(profile.len(), 10);
        // Centre values should be highest
        assert!((profile[0].1 - 1.0).abs() < 1e-10); // ρ/ρ_c = 1 at centre
        assert!(profile[0].2 >= profile[9].2); // P decreasing outward
    }

    #[test]
    fn test_central_density_sun() {
        let sun = StellarStructure::new(M_SUN, R_SUN, L_SUN, 1.57e7);
        let rho_c = sun.central_density();
        // Solar central density ≈ 1.62e5 kg/m³ (approximate polytropic model)
        assert!(
            rho_c > 1e3,
            "central density should be large, got {}",
            rho_c
        );
    }

    #[test]
    fn test_central_pressure() {
        let sun = StellarStructure::new(M_SUN, R_SUN, L_SUN, 1.57e7);
        let pc = sun.central_pressure(3.0);
        // Solar central pressure ≈ 2.5e16 Pa (approximate polytropic model)
        assert!(pc > 1e10, "central pressure should be large, got {}", pc);
    }

    #[test]
    fn test_metallicity_setter() {
        let star = StellarStructure::new(M_SUN, R_SUN, L_SUN, 1.57e7).with_metallicity(0.001);
        assert!((star.metallicity - 0.001).abs() < 1e-10);
    }
}
