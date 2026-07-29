//! Tissue material models: linear elastic and hyperelastic.

use crate::core::types::Scalar;

/// Material properties of biological tissue.
pub struct TissueMaterial {
    pub young_modulus: Scalar,
    pub poisson_ratio: Scalar,
    pub density: Scalar,
    pub yield_stress: Scalar,
    pub is_hyperelastic: bool,
}

impl TissueMaterial {
    pub fn new(
        young_modulus: Scalar,
        poisson_ratio: Scalar,
        density: Scalar,
        yield_stress: Scalar,
        is_hyperelastic: bool,
    ) -> Self {
        Self {
            young_modulus,
            poisson_ratio,
            density,
            yield_stress,
            is_hyperelastic,
        }
    }
}

/// Tissue mechanics helper functions.
pub struct TissueMechanics;

impl TissueMechanics {
    /// Linear elastic stress: σ = E·ε
    pub fn stress(strain: Scalar, material: &TissueMaterial) -> Scalar {
        material.young_modulus * strain
    }

    /// Elastic (Young's) modulus of the material.
    pub fn elastic_modulus(material: &TissueMaterial) -> Scalar {
        material.young_modulus
    }

    /// Neo-Hookean hyperelastic stress (1D simplification).
    ///
    /// σ = μ·(λ - 1/λ²) + K·ln(λ)
    /// where λ is the stretch ratio, μ is the shear modulus, K is the bulk modulus.
    pub fn neo_hookean_stress(stretch: Scalar, mu: Scalar, bulk_modulus: Scalar) -> Scalar {
        if stretch <= 0.0 {
            return -mu * 1e10; // near-infinite compressive stress
        }
        mu * (stretch - 1.0 / (stretch * stretch)) + bulk_modulus * stretch.ln()
    }
}

/// Cortical bone material (dense outer layer).
pub fn cortical_bone() -> TissueMaterial {
    TissueMaterial {
        young_modulus: 18e9,
        poisson_ratio: 0.3,
        density: 1900.0,
        yield_stress: 170e6,
        is_hyperelastic: false,
    }
}

/// Trabecular (spongy) bone material.
pub fn trabecular_bone() -> TissueMaterial {
    TissueMaterial {
        young_modulus: 1.0e9,
        poisson_ratio: 0.3,
        density: 600.0,
        yield_stress: 10e6,
        is_hyperelastic: false,
    }
}

/// Skeletal muscle tissue.
pub fn skeletal_muscle() -> TissueMaterial {
    TissueMaterial {
        young_modulus: 1.0e6,
        poisson_ratio: 0.4,
        density: 1060.0,
        yield_stress: 100e3,
        is_hyperelastic: false,
    }
}

/// Articular cartilage tissue.
pub fn articular_cartilage() -> TissueMaterial {
    TissueMaterial {
        young_modulus: 0.79e9,
        poisson_ratio: 0.45,
        density: 1100.0,
        yield_stress: 15e6,
        is_hyperelastic: false,
    }
}

/// Artery / vessel wall tissue (hyperelastic).
pub fn artery_wall() -> TissueMaterial {
    TissueMaterial {
        young_modulus: 1.3e6,
        poisson_ratio: 0.45,
        density: 1070.0,
        yield_stress: 1.0e6,
        is_hyperelastic: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cortical_bone_properties() {
        let b = cortical_bone();
        assert!((b.young_modulus - 18e9).abs() < 1.0);
        assert!((b.density - 1900.0).abs() < 1.0);
        assert!(!b.is_hyperelastic);
    }

    #[test]
    fn test_trabecular_bone_properties() {
        let b = trabecular_bone();
        assert!((b.young_modulus - 1.0e9).abs() < 1.0);
        assert!((b.density - 600.0).abs() < 1.0);
    }

    #[test]
    fn test_skeletal_muscle_properties() {
        let m = skeletal_muscle();
        assert!((m.young_modulus - 1.0e6).abs() < 1.0);
        assert!((m.density - 1060.0).abs() < 1.0);
    }

    #[test]
    fn test_articular_cartilage_properties() {
        let c = articular_cartilage();
        assert!((c.young_modulus - 0.79e9).abs() < 1.0);
        assert!((c.density - 1100.0).abs() < 1.0);
    }

    #[test]
    fn test_artery_wall_properties() {
        let a = artery_wall();
        assert!((a.young_modulus - 1.3e6).abs() < 1.0);
        assert!(a.is_hyperelastic);
    }

    #[test]
    fn test_linear_stress() {
        let bone = cortical_bone();
        let strain = 0.001;
        let s = TissueMechanics::stress(strain, &bone);
        let expected = bone.young_modulus * strain;
        assert!((s - expected).abs() < 1.0);
    }

    #[test]
    fn test_elastic_modulus() {
        let bone = cortical_bone();
        assert!((TissueMechanics::elastic_modulus(&bone) - 18e9).abs() < 1.0);
    }

    #[test]
    fn test_neo_hookean_stress_zero_stretch() {
        let s = TissueMechanics::neo_hookean_stress(1.0, 1e6, 2e9);
        assert!((s).abs() < 1.0);
    }

    #[test]
    fn test_neo_hookean_stress_positive_stretch() {
        let s = TissueMechanics::neo_hookean_stress(1.1, 1e6, 2e9);
        assert!(s > 0.0);
    }

    #[test]
    fn test_new_tissue_material() {
        let t = TissueMaterial::new(1.0e9, 0.3, 1000.0, 50e6, false);
        assert!((t.young_modulus - 1.0e9).abs() < 1.0);
        assert!((t.poisson_ratio - 0.3).abs() < 1e-10);
        assert!((t.density - 1000.0).abs() < 1.0);
        assert!((t.yield_stress - 50e6).abs() < 1.0);
        assert!(!t.is_hyperelastic);
    }
}
