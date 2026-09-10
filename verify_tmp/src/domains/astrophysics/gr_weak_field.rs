//! Weak-field general relativistic corrections.
//!
//! Provides post-Newtonian corrections for precision orbital mechanics:
//! Schwarzschild radius, perihelion precession, gravitational time dilation,
//! light deflection, and Shapiro delay.

use crate::core::types::Scalar;

/// Gravitational constant (m³·kg⁻¹·s⁻²).
const G: Scalar = 6.67430e-11;
/// Speed of light (m/s).
const C: Scalar = 2.99792458e8;

/// Weak-field general relativistic corrections around a central mass.
#[derive(Debug, Clone, Copy)]
pub struct GRCorrection {
    /// Central mass (kg).
    pub mass_central: Scalar,
}

impl GRCorrection {
    /// Create a new GR correction model.
    pub fn new(mass_central: Scalar) -> Self {
        assert!(mass_central > 0.0, "Central mass must be positive");
        Self { mass_central }
    }

    /// Schwarzschild radius (m): r_s = 2GM/c².
    pub fn schwarzschild_radius(&self) -> Scalar {
        2.0 * G * self.mass_central / (C * C)
    }

    /// Perihelion precession per orbit (radians).
    ///
    /// Δφ = 6πGM / (a(1-e²)c²)
    /// where `a` is the semi-major axis and `e` the eccentricity.
    pub fn perihelion_precession(&self, semi_major: Scalar, eccentricity: Scalar) -> Scalar {
        6.0 * std::f64::consts::PI * G * self.mass_central
            / (semi_major * (1.0 - eccentricity * eccentricity) * C * C)
    }

    /// Gravitational time dilation factor at radius `r` from centre.
    ///
    /// dτ/dt = √(1 - r_s/r), where r_s is the Schwarzschild radius.
    pub fn gravitational_time_dilation(&self, r: Scalar) -> Scalar {
        let rs = self.schwarzschild_radius();
        if r <= rs {
            return 0.0; // Inside event horizon
        }
        (1.0 - rs / r).sqrt()
    }

    /// Light deflection angle (radians) for a ray passing at impact
    /// parameter `b` from the central mass.
    ///
    /// α = 4GM / (b·c²)
    pub fn light_deflection(&self, impact_parameter: Scalar) -> Scalar {
        if impact_parameter <= 0.0 {
            return 0.0;
        }
        4.0 * G * self.mass_central / (impact_parameter * C * C)
    }

    /// Shapiro time delay (seconds) for a signal travelling from
    /// `r_source` to `r_observer` past the central mass.
    ///
    /// Δt = (2GM/c³) · ln(4·r_source·r_observer / b²)
    pub fn shapiro_delay(&self, r_source: Scalar, r_observer: Scalar) -> Scalar {
        if r_source <= 0.0 || r_observer <= 0.0 {
            return 0.0;
        }
        let rs = self.schwarzschild_radius();
        let term = 4.0 * r_source * r_observer / (rs * rs);
        if term <= 1.0 {
            return 0.0;
        }
        rs / C * term.ln()
    }

    /// Orbital velocity at radius `r` for a circular orbit (includes GR
    /// correction to Newtonian: v = √(GM/(r - r_s))).
    pub fn orbital_velocity(&self, r: Scalar) -> Scalar {
        let rs = self.schwarzschild_radius();
        if r <= rs {
            return 0.0;
        }
        (G * self.mass_central / (r - rs)).sqrt()
    }

    /// ISCO (Innermost Stable Circular Orbit) radius: r_ISCO = 3·r_s.
    pub fn isco_radius(&self) -> Scalar {
        3.0 * self.schwarzschild_radius()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solar_schwarzschild() {
        let sun = GRCorrection::new(1.989e30);
        let rs = sun.schwarzschild_radius();
        // Solar Schwarzschild radius ≈ 2953 m
        assert!((rs - 2953.0).abs() < 10.0);
    }

    #[test]
    fn test_earth_schwarzschild() {
        let earth = GRCorrection::new(5.972e24);
        let rs = earth.schwarzschild_radius();
        // Earth Schwarzschild radius ≈ 8.87 mm
        assert!((rs - 0.00887).abs() < 0.001);
    }

    #[test]
    fn test_mercury_perihelion() {
        let sun = GRCorrection::new(1.989e30);
        // Mercury: a = 5.79e10 m, e = 0.2056
        let precession = sun.perihelion_precession(5.79e10, 0.2056);
        // ~5.0e-7 rad/orbit for Mercury
        assert!(precession > 4.5e-7 && precession < 5.5e-7);
    }

    #[test]
    fn test_time_dilation_surface() {
        let earth = GRCorrection::new(5.972e24);
        let dt = earth.gravitational_time_dilation(6.371e6);
        // At Earth surface: very close to 1.0
        assert!(dt > 0.999999999);
        assert!(dt <= 1.0);
    }

    #[test]
    fn test_time_dilation_inside_horizon() {
        let sun = GRCorrection::new(1.989e30);
        let dt = sun.gravitational_time_dilation(1000.0); // inside rs
        assert!((dt - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_light_deflection_sun() {
        let sun = GRCorrection::new(1.989e30);
        // Light grazing Sun's surface: b ≈ 6.96e8 m
        let alpha = sun.light_deflection(6.96e8);
        // ≈ 1.75 arcseconds = 8.5e-6 rad
        assert!(alpha > 8.0e-6 && alpha < 9.0e-6);
    }

    #[test]
    fn test_shapiro_delay() {
        let sun = GRCorrection::new(1.989e30);
        let delay = sun.shapiro_delay(1.5e11, 1.5e11); // Earth-Sun-Earth
        assert!(delay > 0.0);
    }

    #[test]
    fn test_orbital_velocity_earth() {
        let earth = GRCorrection::new(5.972e24);
        // LEO at 200 km: r = 6.571e6 m
        let v = earth.orbital_velocity(6.571e6 + 200e3);
        assert!(v > 7000.0 && v < 8000.0);
    }

    #[test]
    fn test_isco_radius() {
        let sun = GRCorrection::new(1.989e30);
        let isco = sun.isco_radius();
        assert!((isco - 3.0 * sun.schwarzschild_radius()).abs() < 1e-10);
    }
}
