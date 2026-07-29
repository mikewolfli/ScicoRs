//! Aerodynamics: lift, drag, and boundary-layer models.
//!
//! Provides standard aerodynamics relations for subsonic flow:
//! lift and drag coefficients, forces, dynamic pressure, and
//! turbulent boundary-layer thickness.

use crate::core::types::Scalar;

/// Lift coefficient using a linear lift-curve slope.
///
/// C_L = C_Lα · α
///
/// # Arguments
///
/// * `cl_alpha` - Lift-curve slope (1/rad); typically 2π for thin airfoils
/// * `alpha_rad` - Angle of attack (rad)
///
/// Returns the lift coefficient C_L (dimensionless).
pub fn lift_coefficient(cl_alpha: Scalar, alpha_rad: Scalar) -> Scalar {
    cl_alpha * alpha_rad
}

/// Drag coefficient using the drag polar (parabolic approximation).
///
/// C_D = C_D0 + C_L² / (π · AR · e)
///
/// # Arguments
///
/// * `cd0`         - Zero-lift (parasitic) drag coefficient
/// * `cl`          - Lift coefficient C_L
/// * `aspect_ratio`- Wing aspect ratio AR = b² / S
/// * `oswald`      - Oswald efficiency factor e (0 < e ≤ 1)
///
/// Returns the drag coefficient C_D (dimensionless).
pub fn drag_coefficient(cd0: Scalar, cl: Scalar, aspect_ratio: Scalar, oswald: Scalar) -> Scalar {
    if aspect_ratio <= 0.0 || oswald <= 0.0 {
        return cd0;
    }
    let induced = cl * cl / (std::f64::consts::PI * aspect_ratio * oswald);
    cd0 + induced
}

/// Lift force.
///
/// L = 0.5 · ρ · U² · S · C_L
///
/// # Arguments
///
/// * `density`  - Freestream fluid density ρ (kg/m³)
/// * `velocity` - Freestream velocity U (m/s)
/// * `area`     - Reference (wing) area S (m²)
/// * `cl`       - Lift coefficient C_L
///
/// Returns the lift force L (N).
pub fn lift_force(density: Scalar, velocity: Scalar, area: Scalar, cl: Scalar) -> Scalar {
    if density <= 0.0 || area <= 0.0 {
        return 0.0;
    }
    0.5 * density * velocity * velocity * area * cl
}

/// Drag force.
///
/// D = 0.5 · ρ · U² · S · C_D
///
/// # Arguments
///
/// * `density`  - Freestream fluid density ρ (kg/m³)
/// * `velocity` - Freestream velocity U (m/s)
/// * `area`     - Reference (wing) area S (m²)
/// * `cd`       - Drag coefficient C_D
///
/// Returns the drag force D (N).
pub fn drag_force(density: Scalar, velocity: Scalar, area: Scalar, cd: Scalar) -> Scalar {
    if density <= 0.0 || area <= 0.0 {
        return 0.0;
    }
    0.5 * density * velocity * velocity * area * cd
}

/// Dynamic (stagnation) pressure.
///
/// q = 0.5 · ρ · U²
///
/// # Arguments
///
/// * `density`  - Fluid density ρ (kg/m³)
/// * `velocity` - Flow velocity U (m/s)
///
/// Returns the dynamic pressure q (Pa).
pub fn dynamic_pressure(density: Scalar, velocity: Scalar) -> Scalar {
    0.5 * density * velocity * velocity
}

/// Turbulent boundary-layer thickness (flat plate, 1/7th power law).
///
/// δ ≈ 0.37 · x / Re_x^(1/5)
///
/// # Arguments
///
/// * `x`    - Streamwise distance from leading edge (m)
/// * `re_x` - Local Reynolds number Re_x = ρ U x / μ
///
/// Returns the 99% boundary-layer thickness δ (m).
pub fn turbulent_boundary_layer_thickness(x: Scalar, re_x: Scalar) -> Scalar {
    if x <= 0.0 || re_x <= 0.0 {
        return 0.0;
    }
    0.37 * x / re_x.powf(0.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lift_coefficient_linear() {
        // Thin airfoil: C_Lα = 2π ≈ 6.28, α = 5° ≈ 0.08727 rad
        let cl = lift_coefficient(2.0 * std::f64::consts::PI, 0.08727);
        let expected = 2.0 * std::f64::consts::PI * 0.08727;
        assert!((cl - expected).abs() < 1e-4);
    }

    #[test]
    fn test_lift_coefficient_zero_alpha() {
        let cl = lift_coefficient(6.28, 0.0);
        assert_eq!(cl, 0.0);
    }

    #[test]
    fn test_drag_coefficient() {
        let cd = drag_coefficient(0.02, 0.5, 8.0, 0.85);
        // C_D = 0.02 + 0.25 / (π * 8 * 0.85) ≈ 0.02 + 0.0117 ≈ 0.0317
        let induced = 0.25 / (std::f64::consts::PI * 8.0 * 0.85);
        let expected = 0.02 + induced;
        assert!((cd - expected).abs() < 1e-10);
    }

    #[test]
    fn test_drag_coefficient_zero_aspect() {
        let cd = drag_coefficient(0.02, 0.5, 0.0, 0.85);
        assert_eq!(cd, 0.02);
    }

    #[test]
    fn test_lift_force() {
        // ρ = 1.225, U = 50, S = 16, C_L = 0.5
        let l = lift_force(1.225, 50.0, 16.0, 0.5);
        // L = 0.5 * 1.225 * 2500 * 16 * 0.5 = 12250 N
        assert!((l - 12250.0).abs() < 1e-9);
    }

    #[test]
    fn test_lift_force_zero_velocity() {
        let l = lift_force(1.225, 0.0, 16.0, 0.5);
        assert_eq!(l, 0.0);
    }

    #[test]
    fn test_drag_force() {
        let d = drag_force(1.225, 50.0, 16.0, 0.03);
        // D = 0.5 * 1.225 * 2500 * 16 * 0.03 = 735 N
        assert!((d - 735.0).abs() < 1e-9);
    }

    #[test]
    fn test_drag_force_zero_area() {
        let d = drag_force(1.225, 50.0, 0.0, 0.03);
        assert_eq!(d, 0.0);
    }

    #[test]
    fn test_dynamic_pressure() {
        let q = dynamic_pressure(1.225, 50.0);
        // q = 0.5 * 1.225 * 2500 = 1531.25 Pa
        assert!((q - 1531.25).abs() < 1e-10);
    }

    #[test]
    fn test_dynamic_pressure_zero_velocity() {
        let q = dynamic_pressure(1.225, 0.0);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_turbulent_boundary_layer_thickness() {
        let delta = turbulent_boundary_layer_thickness(1.0, 1.0e6);
        // δ ≈ 0.37 * 1.0 / (1e6)^0.2 = 0.37 / 15.85 ≈ 0.0233 m
        assert!((delta - 0.0233).abs() < 0.001);
    }

    #[test]
    fn test_turbulent_boundary_layer_invalid() {
        let delta = turbulent_boundary_layer_thickness(0.0, 1.0e6);
        assert_eq!(delta, 0.0);
    }
}
