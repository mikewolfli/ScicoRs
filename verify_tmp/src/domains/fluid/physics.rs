//! Fluid physical constants and property functions for CFD.
//!
//! Provides fundamental fluid property constants and models for
//! single- and multi-phase flow simulations.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
//  Physical Constants
// ──────────────────────────────────────────────

/// Specific gas constant for dry air (J/(kg·K)).
pub const AIR_GAS_CONSTANT: Scalar = 287.058;

/// Ratio of specific heats for air (dimensionless).
pub const AIR_GAMMA: Scalar = 1.4;

/// Density of water at STP (kg/m³).
pub const WATER_DENSITY: Scalar = 1000.0;

/// Dynamic viscosity of water at 20°C (Pa·s).
pub const WATER_VISCOSITY: Scalar = 1.002e-3;

/// Density of air at STP (kg/m³).
pub const AIR_DENSITY_STP: Scalar = 1.225;

/// Standard gravitational acceleration (m/s²).
pub const G: Scalar = 9.80665;

/// Boltzmann constant (J/K) — available for future plasma/thermal calculations.
pub const K_B: Scalar = 1.380649e-23;

/// Electron mass (kg).
const M_E: Scalar = 9.10938356e-31;

/// Elementary charge (C).
const E_CHARGE: Scalar = 1.602176634e-19;

// ──────────────────────────────────────────────
//  Functions
// ──────────────────────────────────────────────

/// Kinematic viscosity ν = μ / ρ.
///
/// # Arguments
///
/// * `dynamic` - Dynamic viscosity μ (Pa·s)
/// * `density` - Fluid density ρ (kg/m³)
///
/// Returns kinematic viscosity ν (m²/s), or `f64::INFINITY` if density ≤ 0.
pub fn kinematic_viscosity(dynamic: Scalar, density: Scalar) -> Scalar {
    if density <= 0.0 {
        return Scalar::INFINITY;
    }
    dynamic / density
}

/// Plasma (electron) frequency: ω_p = √(n_e · e² / (ε₀ · m_e)).
///
/// Useful for weakly ionised gas flows and MHD coupling.
///
/// # Arguments
///
/// * `electron_density` - Electron number density (m⁻³)
///
/// Returns angular plasma frequency (rad/s).
pub fn plasma_frequency(electron_density: Scalar) -> Scalar {
    if electron_density <= 0.0 {
        return 0.0;
    }
    // Vacuum permittivity ε₀
    let eps0 = 8.854187817e-12;
    (electron_density * E_CHARGE * E_CHARGE / (eps0 * M_E)).sqrt()
}

/// High-temperature air properties using a two-temperature model.
///
/// Returns a `HighTempAirProps` struct containing the effective specific
/// heat ratio and effective speed of sound for air in thermal non-equilibrium.
///
/// # Arguments
///
/// * `t_translational` - Translational / rotational temperature (K)
/// * `t_vibrational`   - Vibrational temperature (K)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighTempAirProps {
    /// Effective ratio of specific heats γ_eff.
    pub gamma_effective: Scalar,
    /// Effective speed of sound c_eff (m/s).
    pub speed_of_sound_effective: Scalar,
}

pub fn high_temp_air_properties(t_translational: Scalar, t_vibrational: Scalar) -> HighTempAirProps {
    if t_translational <= 0.0 || t_vibrational <= 0.0 {
        return HighTempAirProps {
            gamma_effective: AIR_GAMMA,
            speed_of_sound_effective: (AIR_GAMMA * AIR_GAS_CONSTANT * 300.0).sqrt(),
        };
    }
    // Simplified two-temperature model:
    //   γ_eff ≈ 1 + (γ₀ - 1) / (1 + f_vib)
    // where f_vib ≈ exp(-θ_v / T_vib) · (T_tr / T_vib) · ...
    //   θ_v ≈ 2256 K (characteristic vibrational temperature of N₂)
    let theta_v = 2256.0;
    let c_vib = t_vibrational / theta_v;
    let f_vib = if c_vib > 1e-12 {
        let exp_theta = (-theta_v / t_vibrational).exp();
        exp_theta * (t_translational / t_vibrational)
    } else {
        0.0
    };
    let gamma_eff = 1.0 + (AIR_GAMMA - 1.0) / (1.0 + f_vib);
    let r_specific = AIR_GAS_CONSTANT;
    let c_eff = (gamma_eff * r_specific * t_translational).sqrt();
    HighTempAirProps {
        gamma_effective: gamma_eff,
        speed_of_sound_effective: c_eff,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kinematic_viscosity_water() {
        let nu = kinematic_viscosity(WATER_VISCOSITY, WATER_DENSITY);
        let expected = 1.002e-6; // ≈ 1.002×10⁻⁶ m²/s
        assert!((nu - expected).abs() / expected < 1e-3);
    }

    #[test]
    fn test_kinematic_viscosity_zero_density() {
        let nu = kinematic_viscosity(1.0, 0.0);
        assert!(nu.is_infinite());
    }

    #[test]
    fn test_plasma_frequency_typical() {
        // Typical tokamak edge density ~ 1e19 m⁻³
        let wp = plasma_frequency(1.0e19);
        assert!(wp > 0.0);
        // Order of magnitude: ~ 5.6e11 rad/s
        assert!((wp / 1.78e11 - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_plasma_frequency_zero() {
        let wp = plasma_frequency(0.0);
        assert_eq!(wp, 0.0);
    }

    #[test]
    fn test_high_temp_air_properties_equilibrium() {
        let props = high_temp_air_properties(3000.0, 3000.0);
        // At equilibrium, gamma should be lower than 1.4
        assert!(props.gamma_effective < AIR_GAMMA);
        assert!(props.gamma_effective > 1.1);
        assert!(props.speed_of_sound_effective > 500.0);
    }

    #[test]
    fn test_high_temp_air_properties_cold() {
        let props = high_temp_air_properties(300.0, 300.0);
        // Near STP, gamma should be close to 1.4
        assert!((props.gamma_effective - AIR_GAMMA).abs() < 0.01);
    }

    #[test]
    fn test_high_temp_air_properties_non_equilibrium() {
        let props = high_temp_air_properties(5000.0, 1000.0);
        // Cold vibrations → frozen vibrational mode → gamma closer to 1.4
        assert!(props.gamma_effective > 1.2);
    }

    #[test]
    fn test_high_temp_air_properties_invalid() {
        let props = high_temp_air_properties(-100.0, 300.0);
        assert!((props.gamma_effective - AIR_GAMMA).abs() < 0.01);
    }
}
