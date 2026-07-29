//! Astrophysics physical constants.

use crate::core::types::Scalar;

/// Gravitational constant G (m³/(kg·s²))
pub const GRAVITATIONAL: Scalar = 6.67430e-11;

/// Speed of light c (m/s)
pub const C: Scalar = 299792458.0;

/// Solar mass M☉ (kg)
pub const SOLAR_MASS: Scalar = 1.98847e30;

/// Solar radius R☉ (m)
pub const SOLAR_RADIUS: Scalar = 6.957e8;

/// Earth mass M⊕ (kg)
pub const EARTH_MASS: Scalar = 5.9722e24;

/// Earth radius R⊕ (m)
pub const EARTH_RADIUS: Scalar = 6371000.0;

/// Earth orbital radius 1 AU (m)
pub const AU: Scalar = 1.495978707e11;

/// Parsec pc (m)
pub const PARSEC: Scalar = 3.085677581e16;

/// Light year ly (m)
pub const LIGHT_YEAR: Scalar = 9.460730472e15;

/// Hubble constant H₀ (km/s/Mpc) — approximate
pub const HUBBLE_CONSTANT: Scalar = 70.0;

/// Solar luminosity L☉ (W)
pub const SOLAR_LUMINOSITY: Scalar = 3.828e26;

/// Solar surface temperature (K)
pub const SOLAR_TEMPERATURE: Scalar = 5772.0;

/// Standard gravitational parameter GM⊕ (m³/s²)
pub const EARTH_GM: Scalar = 3.986004418e14;

/// Solar standard gravitational parameter GM☉ (m³/s²)
pub const SOLAR_GM: Scalar = 1.32712442099e20;

/// J2 for Earth (oblateness perturbation)
pub const EARTH_J2: Scalar = 1.08263e-3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solar_mass_positive() {
        assert!(SOLAR_MASS > 0.0);
    }

    #[test]
    fn test_earth_mass_positive() {
        assert!(EARTH_MASS > 0.0);
    }

    #[test]
    fn test_au_value() {
        assert!((AU - 1.495978707e11).abs() < 1.0);
    }

    #[test]
    fn test_speed_of_light() {
        assert!((C - 299792458.0).abs() < 1.0);
    }

    #[test]
    fn test_parsec_vs_light_year() {
        assert!(PARSEC > LIGHT_YEAR);
    }
}
