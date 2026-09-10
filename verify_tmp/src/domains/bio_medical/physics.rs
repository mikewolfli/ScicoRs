//! Biomedical physical constants: tissue/fluid properties, diffusion, thermal.

use crate::core::types::Scalar;

/// Blood density (kg/m³) — whole blood at 37°C.
pub const BLOOD_DENSITY: Scalar = 1060.0;

/// Blood viscosity (Pa·s) — whole blood at 37°C.
pub const BLOOD_VISCOSITY: Scalar = 0.0035;

/// Heart muscle density (kg/m³).
pub const HEART_DENSITY: Scalar = 1050.0;

/// Cortical bone Young's modulus (Pa).
pub const BONE_YOUNG_MODULUS: Scalar = 18e9;

/// Bone Poisson ratio (dimensionless).
pub const BONE_POISSON_RATIO: Scalar = 0.3;

/// Bone density (kg/m³).
pub const BONE_DENSITY: Scalar = 1900.0;

/// Skeletal muscle density (kg/m³).
pub const MUSCLE_DENSITY: Scalar = 1060.0;

/// Articular cartilage Young's modulus (Pa).
pub const CARTILAGE_YOUNG_MODULUS: Scalar = 0.79e9;

/// Artery / vessel wall Young's modulus (Pa).
pub const VESSEL_YOUNG_MODULUS: Scalar = 1.3e6;

/// Tissue thermal conductivity (W/(m·K)).
pub const TISSUE_THERMAL_CONDUCTIVITY: Scalar = 0.5;

/// Tissue specific heat capacity (J/(kg·K)).
pub const TISSUE_SPECIFIC_HEAT: Scalar = 3600.0;

/// Default drug diffusivity in tissue (m²/s).
pub const DRUG_DIFFUSIVITY_DEFAULT: Scalar = 1e-10;

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant)]
    use super::*;

    #[test]
    fn test_blood_density_in_physical_range() {
        assert!(BLOOD_DENSITY > 1000.0 && BLOOD_DENSITY < 1100.0);
    }

    #[test]
    fn test_blood_viscosity_in_physical_range() {
        assert!(BLOOD_VISCOSITY > 0.001 && BLOOD_VISCOSITY < 0.01);
    }

    #[test]
    fn test_bone_young_modulus_in_physical_range() {
        assert!(BONE_YOUNG_MODULUS > 1e9 && BONE_YOUNG_MODULUS < 50e9);
    }

    #[test]
    fn test_bone_poisson_ratio_in_physical_range() {
        assert!(BONE_POISSON_RATIO > 0.0 && BONE_POISSON_RATIO < 0.5);
    }

    #[test]
    fn test_bone_density_in_physical_range() {
        assert!(BONE_DENSITY > 500.0 && BONE_DENSITY < 2500.0);
    }

    #[test]
    fn test_muscle_density_in_physical_range() {
        assert!(MUSCLE_DENSITY > 1000.0 && MUSCLE_DENSITY < 1200.0);
    }

    #[test]
    fn test_cartilage_young_modulus_in_physical_range() {
        assert!(CARTILAGE_YOUNG_MODULUS > 1e8 && CARTILAGE_YOUNG_MODULUS < 2e9);
    }

    #[test]
    fn test_vessel_young_modulus_in_physical_range() {
        assert!(VESSEL_YOUNG_MODULUS > 1e5 && VESSEL_YOUNG_MODULUS < 1e7);
    }

    #[test]
    fn test_tissue_thermal_conductivity_in_physical_range() {
        assert!(TISSUE_THERMAL_CONDUCTIVITY > 0.1 && TISSUE_THERMAL_CONDUCTIVITY < 1.0);
    }

    #[test]
    fn test_tissue_specific_heat_in_physical_range() {
        assert!(TISSUE_SPECIFIC_HEAT > 2000.0 && TISSUE_SPECIFIC_HEAT < 5000.0);
    }

    #[test]
    fn test_drug_diffusivity_in_physical_range() {
        assert!(DRUG_DIFFUSIVITY_DEFAULT > 1e-12 && DRUG_DIFFUSIVITY_DEFAULT < 1e-8);
    }
}
