//! Quantum physics constants for simulation.

use crate::core::types::Scalar;

/// Reduced Planck constant ℏ (J·s)
pub const HBAR: Scalar = 1.054571817e-34;

/// Planck constant h (J·s)
pub const PLANCK: Scalar = 6.62607015e-34;

/// Boltzmann constant k_B (J/K)
pub const BOLTZMANN: Scalar = 1.380649e-23;

/// Fine-structure constant α
pub const FINE_STRUCTURE: Scalar = 7.2973525693e-3;

/// Elementary charge e (C)
pub const ELEMENTARY_CHARGE: Scalar = 1.602176634e-19;

/// Bohr magneton μ_B (J/T)
pub const BOHR_MAGNETON: Scalar = 9.2740100783e-24;

/// Electron mass m_e (kg)
pub const ELECTRON_MASS: Scalar = 9.1093837e-31;

/// Proton mass m_p (kg)
pub const PROTON_MASS: Scalar = 1.6726219e-27;

/// Speed of light c (m/s)
pub const SPEED_OF_LIGHT: Scalar = 299792458.0;

/// Vacuum permittivity ε₀ (F/m)
pub const VACUUM_PERMITTIVITY: Scalar = 8.854187817e-12;

/// Vacuum permeability μ₀ (N/A²)
pub const VACUUM_PERMEABILITY: Scalar = 1.2566370612e-6;

/// Rydberg constant R_∞ (1/m)
pub const RYDBERG: Scalar = 10973731.568157;

/// Bohr radius a₀ (m)
pub const BOHR_RADIUS: Scalar = 5.29177210903e-11;

/// Avogadro constant N_A (mol⁻¹)
pub const AVOGADRO: Scalar = 6.02214076e23;

/// Stefan-Boltzmann constant σ (W/(m²·K⁴))
pub const STEFAN_BOLTZMANN: Scalar = 5.670374419e-8;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hbar_value() {
        assert!(HBAR > 0.0);
        assert!(HBAR < 1e-33);
    }

    #[test]
    fn test_fine_structure_positive() {
        assert!(FINE_STRUCTURE > 0.0);
        assert!(FINE_STRUCTURE < 0.01);
    }

    #[test]
    fn test_speed_of_light() {
        assert!((SPEED_OF_LIGHT - 299792458.0).abs() < 1.0);
    }

    #[test]
    fn test_bohr_radius_positive() {
        assert!(BOHR_RADIUS > 0.0);
    }
}
