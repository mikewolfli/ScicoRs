//! Biophysical constants and medium property functions for cell culture simulation.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// 1. Biophysical Constants
// ──────────────────────────────────────────────

/// Water diffusion coefficient at 37°C (m²/s).
pub const DIFFUSION_WATER_37C: Scalar = 2.5e-9;

/// Typical mammalian cell diameter (m).
pub const TYPICAL_CELL_DIAMETER: Scalar = 15e-6;

/// Typical mammalian cell volume (m³).
pub const TYPICAL_CELL_VOLUME: Scalar = 1.0e-15;

/// Typical cell doubling time (s) — ~22 hours.
pub const TYPICAL_DOUBLING_TIME: Scalar = 79200.0;

/// Oxygen diffusion coefficient in water (m²/s).
pub const O2_DIFFUSION_COEFFICIENT: Scalar = 2.1e-9;

/// Glucose diffusion coefficient in water (m²/s).
pub const GLUCOSE_DIFFUSION_COEFFICIENT: Scalar = 6.7e-10;

/// CO₂ diffusion coefficient in water (m²/s).
pub const CO2_DIFFUSION_COEFFICIENT: Scalar = 1.9e-9;

/// Typical seeding density (cells/mL).
pub const TYPICAL_SEEDING_DENSITY: Scalar = 1e5;

/// Maximum cell density (cells/mL) — contact inhibition limit.
pub const MAX_CELL_DENSITY: Scalar = 2e6;

/// Avogadro constant.
pub const AVOGADRO_CELL: Scalar = 6.02214076e23;

// ──────────────────────────────────────────────
// 2. Medium Property Functions
// ──────────────────────────────────────────────

/// Density of water (kg/m³) at given temperature (K).
pub fn water_density(temp: Scalar) -> Scalar {
    // Approximate polynomial for water density 0-100°C
    let tc: Scalar = temp - 273.15;
    999.842594 + 6.793952e-2 * tc - 9.095290e-3 * tc.powi(2)
        + 1.001685e-4 * tc.powi(3) - 1.120083e-6 * tc.powi(4)
        + 6.536332e-9 * tc.powi(5)
}

/// Dynamic viscosity of water (Pa·s) at given temperature (K).
pub fn water_viscosity(temp: Scalar) -> Scalar {
    let tc: Scalar = temp - 273.15;
    // Vogel equation approximation
    2.414e-5 * 10.0_f64.powf(247.8 / (tc + 140.0))
}

/// Oxygen solubility (mol/(L·atm)) at given temperature (K).
pub fn o2_solubility(temp: Scalar) -> Scalar {
    // Henry's constant for O2 in water
    1.3e-3 * (-1500.0 * (1.0 / temp - 1.0 / 298.15)).exp()
}

/// CO₂ solubility (mol/(L·atm)) at given temperature (K).
pub fn co2_solubility(temp: Scalar) -> Scalar {
    // Henry's constant for CO2 in water
    3.4e-2 * (-2400.0 * (1.0 / temp - 1.0 / 298.15)).exp()
}

/// Diffusion coefficient (m²/s) for common medium molecules.
pub fn diffusion_coefficient(molecule: &str, temp: Scalar) -> Option<Scalar> {
    // Stokes-Einstein scaling with temperature and viscosity
    let eta_37 = water_viscosity(310.15); // reference viscosity at 37°C
    let eta_t = water_viscosity(temp);
    let temp_scale = (temp / 310.15) * (eta_37 / eta_t);

    let d0 = match molecule {
        "O2" | "oxygen" => Some(O2_DIFFUSION_COEFFICIENT),
        "CO2" => Some(CO2_DIFFUSION_COEFFICIENT),
        "glucose" | "Glucose" => Some(GLUCOSE_DIFFUSION_COEFFICIENT),
        "water" | "Water" | "H2O" => Some(DIFFUSION_WATER_37C),
        "lactate" | "Lactate" => Some(1.1e-9),
        "glutamine" | "Glutamine" => Some(5.8e-10),
        "ammonia" | "NH3" => Some(1.6e-9),
        "ions" | "NaCl" => Some(1.5e-9),
        _ => None,
    };
    d0.map(|d| d * temp_scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_water_density_4c() {
        let rho = water_density(277.15); // 4°C
        // Water density near 4°C is ~1000 kg/m³
        assert!((rho - 1000.0).abs() < 5.0);
    }

    #[test]
    fn test_water_viscosity_37c() {
        let eta = water_viscosity(310.15); // 37°C
        // Water viscosity ~0.0007 Pa·s at 37°C
        assert!(eta > 0.0);
        assert!(eta < 0.001);
    }

    #[test]
    fn test_o2_solubility_37c() {
        let s = o2_solubility(310.15);
        assert!(s > 0.0);
        assert!(s < 1.0);
    }

    #[test]
    fn test_diffusion_coefficient_glucose() {
        let d = diffusion_coefficient("glucose", 310.15).unwrap();
        assert!((d - GLUCOSE_DIFFUSION_COEFFICIENT).abs() < 1e-11);
    }

    #[test]
    fn test_diffusion_coefficient_unknown() {
        assert!(diffusion_coefficient("unknown_mol", 310.15).is_none());
    }
}
