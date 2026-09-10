//! Thermal physical constants for heat transfer simulations.
//!
//! Provides fundamental constants: Stefan-Boltzmann constant, gravitational
//! acceleration, thermal conductivities of common materials (air, water,
//! copper, aluminum), fluid properties (viscosity), and latent heats.

use crate::core::types::Scalar;

/// Stefan-Boltzmann constant (W·m⁻²·K⁻⁴).
pub const SIGMA_SB: Scalar = 5.670374419e-8;

/// Standard gravitational acceleration (m/s²).
pub const G: Scalar = 9.80665;

/// Thermal conductivity of dry air at 20°C (W/(m·K)).
pub const AIR_THERMAL_CONDUCTIVITY: Scalar = 0.026;

/// Dynamic viscosity of air at 20°C (Pa·s).
pub const AIR_DYNAMIC_VISCOSITY: Scalar = 1.85e-5;

/// Thermal conductivity of water at 20°C (W/(m·K)).
pub const WATER_THERMAL_CONDUCTIVITY: Scalar = 0.6;

/// Dynamic viscosity of water at 20°C (Pa·s).
pub const WATER_DYNAMIC_VISCOSITY: Scalar = 1.002e-3;

/// Thermal conductivity of copper at 20°C (W/(m·K)).
pub const COPPER_THERMAL_CONDUCTIVITY: Scalar = 401.0;

/// Thermal conductivity of aluminum at 20°C (W/(m·K)).
pub const ALUMINUM_THERMAL_CONDUCTIVITY: Scalar = 237.0;

/// Latent heat of fusion of water (J/kg).
pub const WATER_FUSION_LATENT_HEAT: Scalar = 334000.0;

/// Latent heat of vaporization of water (J/kg).
pub const WATER_VAPORIZATION_LATENT_HEAT: Scalar = 2260000.0;

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant)]
    use super::*;

    #[test]
    fn test_sigma_sb_positive() {
        assert!(SIGMA_SB > 0.0);
    }

    #[test]
    fn test_thermal_conductivities_positive() {
        assert!(AIR_THERMAL_CONDUCTIVITY > 0.0);
        assert!(WATER_THERMAL_CONDUCTIVITY > 0.0);
        assert!(COPPER_THERMAL_CONDUCTIVITY > 0.0);
        assert!(ALUMINUM_THERMAL_CONDUCTIVITY > 0.0);
    }

    #[test]
    fn test_latent_heats_positive() {
        assert!(WATER_FUSION_LATENT_HEAT > 0.0);
        assert!(WATER_VAPORIZATION_LATENT_HEAT > 0.0);
    }

    #[test]
    fn test_metals_higher_conductivity_than_air() {
        assert!(COPPER_THERMAL_CONDUCTIVITY > AIR_THERMAL_CONDUCTIVITY);
        assert!(ALUMINUM_THERMAL_CONDUCTIVITY > AIR_THERMAL_CONDUCTIVITY);
    }

    #[test]
    fn test_copper_conductivity_greater_than_aluminum() {
        assert!(COPPER_THERMAL_CONDUCTIVITY > ALUMINUM_THERMAL_CONDUCTIVITY);
    }
}
