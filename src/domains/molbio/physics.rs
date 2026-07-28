//! Physical constants and molecular parameters for MD simulations.
//!
//! Provides fundamental physical constants, element mass lookup,
//! and standard bond parameter tables.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// 1. Fundamental Physical Constants
// ──────────────────────────────────────────────

/// Avogadro's number (mol⁻¹).
pub const AVOGADRO: Scalar = 6.02214076e23;

/// Boltzmann constant (J/K).
pub const KB: Scalar = 1.380649e-23;

/// Elementary charge (C).
pub const QE: Scalar = 1.602176634e-19;

/// Atomic mass unit to kg conversion factor.
pub const AMU_TO_KG: Scalar = 1.66053906660e-27;

/// Standard room temperature (K).
pub const T_300K: Scalar = 300.0;

/// 1 kcal/mol to kJ/mol.
pub const KCAL_TO_KJ: Scalar = 4.184;

/// 1 Angstrom = 1e-10 m.
pub const ANGSTROM: Scalar = 1e-10;

// ──────────────────────────────────────────────
// 2. Element Mass Table
// ──────────────────────────────────────────────

/// Return the atomic mass (amu) for a given element symbol.
///
/// Covers common elements found in biomolecules (H, C, N, O, P, S, etc.).
pub fn element_mass(symbol: &str) -> Option<Scalar> {
    match symbol {
        "H" => Some(1.008),
        "He" => Some(4.0026),
        "Li" => Some(6.94),
        "Be" => Some(9.0122),
        "B" => Some(10.81),
        "C" => Some(12.011),
        "N" => Some(14.007),
        "O" => Some(15.999),
        "F" => Some(18.998),
        "Ne" => Some(20.180),
        "Na" => Some(22.990),
        "Mg" => Some(24.305),
        "Al" => Some(26.982),
        "Si" => Some(28.085),
        "P" => Some(30.974),
        "S" => Some(32.065),
        "Cl" => Some(35.45),
        "K" => Some(39.098),
        "Ca" => Some(40.078),
        "Fe" => Some(55.845),
        "Cu" => Some(63.546),
        "Zn" => Some(65.38),
        "Se" => Some(78.971),
        "Br" => Some(79.904),
        "I" => Some(126.90),
        _ => None,
    }
}

// ──────────────────────────────────────────────
// 3. Bond Parameter Table
// ──────────────────────────────────────────────

/// Standard bond parameters (equilibrium length and force constant).
///
/// Returns (b0 in Angstrom, kb in kcal/(mol·Angstrom²)).
/// Based on CHARMM/AMBER common parameter values.
pub fn bond_parameters(type1: &str, type2: &str) -> Option<(Scalar, Scalar)> {
    // Normalize sorting for symmetric lookups
    let (a, b) = if type1 <= type2 {
        (type1, type2)
    } else {
        (type2, type1)
    };
    match (a, b) {
        ("C", "C") => Some((1.525, 310.0)),
        ("C", "H") => Some((1.090, 340.0)),
        ("C", "N") => Some((1.480, 337.0)),
        ("C", "O") => Some((1.430, 320.0)),
        ("C", "S") => Some((1.820, 225.0)),
        ("CA", "CA") => Some((1.400, 469.0)), // aromatic C-C
        ("C", "CA") => Some((1.480, 317.0)),
        ("C", "N2") => Some((1.340, 410.0)),  // amide C-N
        ("C", "O2") => Some((1.230, 570.0)),  // carbonyl C=O
        ("CA", "HA") => Some((1.080, 367.0)), // aromatic C-H
        ("CA", "N2") => Some((1.340, 483.0)),
        ("N", "H") => Some((1.010, 434.0)),
        ("N", "N2") => Some((1.380, 320.0)),
        ("N2", "H") => Some((1.010, 434.0)),
        ("O", "H") => Some((0.960, 450.0)),
        ("S", "H") => Some((1.340, 230.0)),
        ("S", "S") => Some((2.050, 200.0)), // disulfide
        _ => None,
    }
}

// ──────────────────────────────────────────────
// 4. Van der Waals Parameters
// ──────────────────────────────────────────────

/// Standard Lennard-Jones sigma (Angstrom) and epsilon (kcal/mol) for elements.
///
/// Based on CHARMM36 C27 atom type values.
pub fn lj_parameters(element: &str) -> Option<(Scalar, Scalar)> {
    match element {
        "H" => Some((1.3582, 0.0460)),
        "C" => Some((1.9080, 0.1094)),
        "N" => Some((1.8500, 0.1200)),
        "O" => Some((1.6612, 0.2100)),
        "P" => Some((2.1000, 0.2000)),
        "S" => Some((2.0000, 0.2500)),
        "F" => Some((1.6000, 0.1000)),
        "Cl" => Some((1.9000, 0.3000)),
        "Br" => Some((2.1000, 0.3500)),
        "I" => Some((2.3000, 0.4000)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_mass_carbon() {
        let m = element_mass("C").unwrap();
        assert!((m - 12.011).abs() < 1e-3);
    }

    #[test]
    fn test_element_mass_hydrogen() {
        let m = element_mass("H").unwrap();
        assert!((m - 1.008).abs() < 1e-3);
    }

    #[test]
    fn test_element_mass_oxygen() {
        let m = element_mass("O").unwrap();
        assert!((m - 15.999).abs() < 1e-3);
    }

    #[test]
    fn test_element_mass_unknown() {
        assert!(element_mass("Xx").is_none());
    }

    #[test]
    fn test_bond_parameters_cc() {
        let (b0, kb) = bond_parameters("C", "C").unwrap();
        assert!((b0 - 1.525).abs() < 1e-3);
        assert!((kb - 310.0).abs() < 1e-3);
    }

    #[test]
    fn test_bond_parameters_symmetric() {
        // Should work regardless of order
        let p1 = bond_parameters("H", "C");
        let p2 = bond_parameters("C", "H");
        assert_eq!(p1, p2);
    }
}
