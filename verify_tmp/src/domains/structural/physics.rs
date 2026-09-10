//! Material mechanical constants for structural analysis.
//!
//! Provides common engineering material properties and
//! stress–strain constitutive relations (Hooke's law in 1D and 3D,
//! von Mises equivalent stress).

use crate::core::types::Scalar;

/// Mechanical properties of a structural material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialProperties {
    /// Young's modulus (Pa).
    pub young_modulus: Scalar,
    /// Poisson's ratio (dimensionless).
    pub poisson_ratio: Scalar,
    /// Density (kg/m³).
    pub density: Scalar,
    /// Yield strength (Pa).
    pub yield_strength: Scalar,
    /// Ultimate tensile strength (Pa).
    pub ultimate_strength: Scalar,
    /// Coefficient of thermal expansion (1/K).
    pub thermal_expansion: Scalar,
}

impl MaterialProperties {
    /// Create a new material properties set.
    pub fn new(
        young_modulus: Scalar,
        poisson_ratio: Scalar,
        density: Scalar,
        yield_strength: Scalar,
        ultimate_strength: Scalar,
        thermal_expansion: Scalar,
    ) -> Self {
        Self {
            young_modulus,
            poisson_ratio,
            density,
            yield_strength,
            ultimate_strength,
            thermal_expansion,
        }
    }

    /// Shear modulus G = E / [2(1+ν)].
    pub fn shear_modulus(&self) -> Scalar {
        let denom = 2.0 * (1.0 + self.poisson_ratio);
        if denom.abs() < 1e-15 {
            return Scalar::INFINITY;
        }
        self.young_modulus / denom
    }

    /// Bulk modulus K = E / [3(1-2ν)].
    pub fn bulk_modulus(&self) -> Scalar {
        let denom = 3.0 * (1.0 - 2.0 * self.poisson_ratio);
        if denom.abs() < 1e-15 {
            return Scalar::INFINITY;
        }
        self.young_modulus / denom
    }
}

// ──────────────────────────────────────────────
//  Common Engineering Materials
// ──────────────────────────────────────────────

/// Standard structural steel (A992 / S355).
pub fn steel_structural() -> MaterialProperties {
    MaterialProperties {
        young_modulus: 200.0e9,    // 200 GPa
        poisson_ratio: 0.30,
        density: 7850.0,           // kg/m³
        yield_strength: 345.0e6,   // 345 MPa
        ultimate_strength: 450.0e6, // 450 MPa
        thermal_expansion: 12.0e-6, // 12 μm/m·K
    }
}

/// Aluminum alloy 6061-T6.
pub fn aluminum_6061() -> MaterialProperties {
    MaterialProperties {
        young_modulus: 68.9e9,     // 68.9 GPa
        poisson_ratio: 0.33,
        density: 2700.0,           // kg/m³
        yield_strength: 276.0e6,   // 276 MPa
        ultimate_strength: 310.0e6, // 310 MPa
        thermal_expansion: 23.6e-6, // 23.6 μm/m·K
    }
}

/// Normal-weight concrete (30 MPa compressive strength).
pub fn concrete_30mpa() -> MaterialProperties {
    MaterialProperties {
        young_modulus: 25.0e9,     // 25 GPa (ACI 318 approximation)
        poisson_ratio: 0.20,
        density: 2400.0,           // kg/m³
        yield_strength: 30.0e6,    // 30 MPa compressive
        ultimate_strength: 3.0e6,  // ~3 MPa tensile (≈ 0.1 f'c)
        thermal_expansion: 10.0e-6,
    }
}

/// Titanium alloy Ti-6Al-4V (Grade 5).
pub fn titanium_ti6al4v() -> MaterialProperties {
    MaterialProperties {
        young_modulus: 110.0e9,    // 110 GPa
        poisson_ratio: 0.31,
        density: 4430.0,           // kg/m³
        yield_strength: 830.0e6,   // 830 MPa
        ultimate_strength: 900.0e6, // 900 MPa
        thermal_expansion: 8.6e-6,
    }
}

// ──────────────────────────────────────────────
//  Constitutive Relations
// ──────────────────────────────────────────────

/// 1D Hooke's law: σ = E · ε.
pub fn hookes_law_1d(strain: Scalar, e: Scalar) -> Scalar {
    e * strain
}

/// 3D Hooke's law for isotropic linear elasticity.
///
/// The strain tensor is given in Voigt notation: [εₓₓ, εᵧᵧ, ε₂₂, γₓᵧ, γₓ₂, γᵧ₂].
/// Returns the stress tensor in Voigt notation: [σₓₓ, σᵧᵧ, σ₂₂, τₓᵧ, τₓ₂, τᵧ₂].
pub fn hookes_law_3d(strain: &[Scalar; 6], e: Scalar, nu: Scalar) -> [Scalar; 6] {
    let c1 = e / ((1.0 + nu) * (1.0 - 2.0 * nu));
    let c2 = 1.0 - nu;
    let c3 = nu;
    let c4 = (1.0 - 2.0 * nu) / 2.0; // shear factor

    let sxx = c1 * (c2 * strain[0] + c3 * (strain[1] + strain[2]));
    let syy = c1 * (c2 * strain[1] + c3 * (strain[0] + strain[2]));
    let szz = c1 * (c2 * strain[2] + c3 * (strain[0] + strain[1]));
    let txy = c1 * c4 * strain[3];
    let txz = c1 * c4 * strain[4];
    let tyz = c1 * c4 * strain[5];

    [sxx, syy, szz, txy, txz, tyz]
}

/// Von Mises equivalent stress from the full stress tensor in Voigt notation.
///
/// Input: [σₓₓ, σᵧᵧ, σ₂₂, τₓᵧ, τₓ₂, τᵧ₂].
pub fn von_mises_stress(sigma: &[Scalar; 6]) -> Scalar {
    let sxx = sigma[0];
    let syy = sigma[1];
    let szz = sigma[2];
    let txy = sigma[3];
    let txz = sigma[4];
    let tyz = sigma[5];

    let a = (sxx - syy).powi(2);
    let b = (syy - szz).powi(2);
    let c = (szz - sxx).powi(2);
    let d = 6.0 * (txy.powi(2) + txz.powi(2) + tyz.powi(2));

    ((a + b + c + d) / 2.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steel_properties() {
        let s = steel_structural();
        assert!((s.young_modulus - 200.0e9).abs() < 1.0);
        assert!((s.poisson_ratio - 0.30).abs() < 1e-6);
    }

    #[test]
    fn test_aluminum_properties() {
        let al = aluminum_6061();
        assert!((al.young_modulus - 68.9e9).abs() < 1.0);
        assert!((al.density - 2700.0).abs() < 1.0);
    }

    #[test]
    fn test_concrete_properties() {
        let c = concrete_30mpa();
        assert!((c.young_modulus - 25.0e9).abs() < 1.0);
    }

    #[test]
    fn test_titanium_properties() {
        let ti = titanium_ti6al4v();
        assert!((ti.young_modulus - 110.0e9).abs() < 1.0);
    }

    #[test]
    fn test_shear_modulus_steel() {
        let s = steel_structural();
        let g = s.shear_modulus();
        let expected = 200.0e9 / (2.0 * (1.0 + 0.30));
        assert!((g - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_hookes_law_1d() {
        let e = 200.0e9;
        let strain = 0.001;
        let stress = hookes_law_1d(strain, e);
        assert!((stress - 200.0e6).abs() < 1.0);
    }

    #[test]
    fn test_hookes_law_3d_uniaxial() {
        // Uniaxial stress in x: only ε_xx is non-zero
        let e = 200.0e9;
        let nu = 0.3;
        let eps = 0.001;
        let strain = [eps, -nu * eps, -nu * eps, 0.0, 0.0, 0.0];
        let stress = hookes_law_3d(&strain, e, nu);
        // σ_xx should be E·ε = 200 MPa
        assert!((stress[0] - 200.0e6).abs() < 1.0);
        // σ_yy and σ_zz should be ~0 for uniaxial stress
        assert!(stress[1].abs() < 1.0);
        assert!(stress[2].abs() < 1.0);
    }

    #[test]
    fn test_von_mises_uniaxial() {
        // Uniaxial tension: only σ_xx non-zero
        let sigma = [300.0e6, 0.0, 0.0, 0.0, 0.0, 0.0];
        let vm = von_mises_stress(&sigma);
        assert!((vm - 300.0e6).abs() < 1.0);
    }

    #[test]
    fn test_von_mises_shear() {
        // Pure shear: only τ_xy non-zero
        let sigma = [0.0, 0.0, 0.0, 100.0e6, 0.0, 0.0];
        let vm = von_mises_stress(&sigma);
        let expected = (3.0_f64).sqrt() * 100.0e6;
        assert!((vm - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_material_bulk_modulus() {
        let s = steel_structural();
        let k = s.bulk_modulus();
        let expected = 200.0e9 / (3.0 * (1.0 - 2.0 * 0.30));
        assert!((k - expected).abs() / expected < 1e-10);
    }
}
