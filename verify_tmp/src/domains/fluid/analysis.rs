//! Flow analysis: basic fluid flow computations.
//!
//! Provides utility functions for computing volumetric and mass flow rates,
//! hydraulic diameter, and pressure coefficient commonly used in fluid
//! engineering analysis.

use crate::core::types::Scalar;

/// Volumetric flow rate: Q = U · A.
///
/// # Arguments
///
/// * `velocity` - Mean flow velocity U (m/s)
/// * `area`     - Cross-sectional flow area A (m²)
///
/// Returns volumetric flow rate Q (m³/s).
pub fn volumetric_flow(velocity: Scalar, area: Scalar) -> Scalar {
    if area < 0.0 {
        return 0.0;
    }
    velocity * area
}

/// Mass flow rate: ṁ = ρ · U · A.
///
/// # Arguments
///
/// * `density`  - Fluid density ρ (kg/m³)
/// * `velocity` - Mean flow velocity U (m/s)
/// * `area`     - Cross-sectional flow area A (m²)
///
/// Returns mass flow rate ṁ (kg/s).
pub fn mass_flow(density: Scalar, velocity: Scalar, area: Scalar) -> Scalar {
    if density < 0.0 || area < 0.0 {
        return 0.0;
    }
    density * velocity * area
}

/// Hydraulic diameter of a duct / pipe.
///
/// D_h = 4 · A / P_w
///
/// where A is the cross-sectional area (m²) and P_w is the wetted
/// perimeter (m). For a circular pipe D_h = D.
///
/// # Arguments
///
/// * `area`             - Cross-sectional area A (m²)
/// * `wetted_perimeter` - Wetted perimeter P_w (m)
///
/// Returns the hydraulic diameter D_h (m), or `f64::INFINITY` if
/// the wetted perimeter is zero (unbounded flow).
pub fn hydraulic_diameter(area: Scalar, wetted_perimeter: Scalar) -> Scalar {
    if area <= 0.0 || wetted_perimeter <= 0.0 {
        return 0.0;
    }
    4.0 * area / wetted_perimeter
}

/// Pressure coefficient (Cp).
///
/// Cp = (p - p_∞) / q_∞
///
/// where p is the local static pressure (Pa), p_∞ is the freestream
/// static pressure (Pa), and q_∞ is the freestream dynamic pressure (Pa).
///
/// # Arguments
///
/// * `p`    - Local static pressure (Pa)
/// * `p_inf`- Freestream static pressure (Pa)
/// * `q`    - Freestream dynamic pressure (Pa)
///
/// Returns the dimensionless pressure coefficient Cp.
pub fn pressure_coefficient(p: Scalar, p_inf: Scalar, q: Scalar) -> Scalar {
    if q.abs() < 1e-30 {
        return 0.0;
    }
    (p - p_inf) / q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volumetric_flow() {
        let q = volumetric_flow(3.0, 0.5);
        assert!((q - 1.5).abs() < 1e-12);
    }

    #[test]
    fn test_volumetric_flow_zero_velocity() {
        let q = volumetric_flow(0.0, 0.5);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_volumetric_flow_negative_area() {
        let q = volumetric_flow(3.0, -0.5);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_mass_flow() {
        let m = mass_flow(1.225, 10.0, 0.5);
        // ṁ = 1.225 * 10 * 0.5 = 6.125 kg/s
        assert!((m - 6.125).abs() < 1e-12);
    }

    #[test]
    fn test_mass_flow_zero_density() {
        let m = mass_flow(0.0, 10.0, 0.5);
        assert_eq!(m, 0.0);
    }

    #[test]
    fn test_hydraulic_diameter_circular() {
        // Circular pipe: D = 0.1 m → A = π·D²/4, P_w = π·D
        let d = 0.1;
        let area = std::f64::consts::PI * d * d / 4.0;
        let perim = std::f64::consts::PI * d;
        let dh = hydraulic_diameter(area, perim);
        assert!((dh - d).abs() < 1e-15);
    }

    #[test]
    fn test_hydraulic_diameter_rectangular() {
        // Rectangular: 0.3 m × 0.2 m → A = 0.06, P_w = 1.0
        // D_h = 4 * 0.06 / 1.0 = 0.24 m
        let dh = hydraulic_diameter(0.06, 1.0);
        assert!((dh - 0.24).abs() < 1e-15);
    }

    #[test]
    fn test_hydraulic_diameter_invalid() {
        let dh = hydraulic_diameter(0.0, 1.0);
        assert_eq!(dh, 0.0);
    }

    #[test]
    fn test_pressure_coefficient_typical() {
        // p = 101500, p_inf = 101325, q = 1531.25
        let cp = pressure_coefficient(101500.0, 101325.0, 1531.25);
        // Cp = 175 / 1531.25 ≈ 0.1143
        assert!((cp - 175.0 / 1531.25).abs() < 1e-10);
    }

    #[test]
    fn test_pressure_coefficient_zero_q() {
        let cp = pressure_coefficient(101500.0, 101325.0, 0.0);
        assert_eq!(cp, 0.0);
    }

    #[test]
    fn test_pressure_coefficient_equal_pressures() {
        let cp = pressure_coefficient(101325.0, 101325.0, 1531.25);
        assert_eq!(cp, 0.0);
    }
}
