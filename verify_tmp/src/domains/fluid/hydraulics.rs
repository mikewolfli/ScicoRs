//! Hydraulics: open-channel, pressurised pipe, and transient flow models.
//!
//! Provides engineering formulas for Manning's open-channel flow,
//! orifice discharge, weir flow, and water hammer (Joukowsky) pressure surge.

use crate::core::types::Scalar;

/// Manning's equation for open-channel flow.
///
/// Q = (1 / n) · A · R_h^(2/3) · S^(1/2)
///
/// where Q is the volumetric flow rate (m³/s), A is the cross-sectional area
/// (m²), R_h is the hydraulic radius (m), S is the energy slope (m/m), and n
/// is the Manning roughness coefficient.
///
/// # Arguments
///
/// * `area`            - Cross-sectional flow area A (m²)
/// * `hydraulic_radius`- Hydraulic radius R_h = A / P_w (m)
/// * `slope`           - Channel / energy slope S (m/m)
/// * `n`               - Manning roughness coefficient (s/m^(1/3))
///
/// Returns the volumetric flow rate Q (m³/s).
pub fn manning_flow(area: Scalar, hydraulic_radius: Scalar, slope: Scalar, n: Scalar) -> Scalar {
    if area <= 0.0 || hydraulic_radius <= 0.0 || slope < 0.0 || n <= 0.0 {
        return 0.0;
    }
    (1.0 / n) * area * hydraulic_radius.powf(2.0 / 3.0) * slope.sqrt()
}

/// Orifice discharge (sharp-edged).
///
/// Q = C_d · A · √(2 · g · h)
///
/// where C_d is the discharge coefficient, A is the orifice area (m²),
/// h is the head above the orifice centreline (m), and g is gravitational
/// acceleration (m/s²).
///
/// # Arguments
///
/// * `cd`   - Discharge coefficient (typically 0.6–0.65)
/// * `area` - Orifice cross-sectional area (m²)
/// * `head` - Pressure head above orifice centre (m)
///
/// Returns the volumetric flow rate Q (m³/s).
pub fn orifice_flow(cd: Scalar, area: Scalar, head: Scalar) -> Scalar {
    if cd <= 0.0 || area <= 0.0 || head < 0.0 {
        return 0.0;
    }
    let g = crate::domains::fluid::physics::G;
    cd * area * (2.0 * g * head).sqrt()
}

/// Sharp-crested weir flow (suppressed rectangular weir).
///
/// Q = C_d · L · H^(3/2)
///
/// where C_d is the weir discharge coefficient (commonly ~1.84 in SI),
/// L is the crest length (m), and H is the head over the crest (m).
///
/// # Arguments
///
/// * `cd`           - Weir discharge coefficient (m^(1/2)/s)
/// * `crest_length` - Length of the weir crest L (m)
/// * `head`         - Head over the weir crest H (m)
///
/// Returns the volumetric flow rate Q (m³/s).
pub fn weir_flow(cd: Scalar, crest_length: Scalar, head: Scalar) -> Scalar {
    if cd <= 0.0 || crest_length <= 0.0 || head < 0.0 {
        return 0.0;
    }
    cd * crest_length * head.powf(1.5)
}

/// Water hammer (Joukowsky) pressure surge.
///
/// ΔP = ρ · a · ΔU
///
/// where ρ is the fluid density (kg/m³), a is the wave speed (m/s),
/// and ΔU is the instantaneous velocity change (m/s).
///
/// # Arguments
///
/// * `density`         - Fluid density ρ (kg/m³)
/// * `wave_speed`      - Pressure wave (water hammer) speed a (m/s)
/// * `velocity_change` - Sudden change in flow velocity ΔU (m/s)
///
/// Returns the pressure rise ΔP (Pa).
pub fn water_hammer_pressure(density: Scalar, wave_speed: Scalar, velocity_change: Scalar) -> Scalar {
    if density <= 0.0 || wave_speed <= 0.0 {
        return 0.0;
    }
    density * wave_speed * velocity_change.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manning_flow_typical() {
        // Rectangular channel: width = 2 m, depth = 1 m → A = 2, R_h = 0.5
        // S = 0.001, n = 0.015 (concrete)
        let q = manning_flow(2.0, 0.5, 0.001, 0.015);
        // Q = (1/0.015) * 2 * (0.5)^(2/3) * sqrt(0.001)
        let expected = (1.0 / 0.015) * 2.0 * (0.5_f64).powf(2.0 / 3.0) * (0.001_f64).sqrt();
        assert!((q - expected).abs() < 1e-10);
        // Approx: ~2.8 m³/s
        assert!(q > 2.0);
        assert!(q < 4.0);
    }

    #[test]
    fn test_manning_flow_invalid_n() {
        let q = manning_flow(2.0, 0.5, 0.001, 0.0);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_manning_flow_zero_slope() {
        let q = manning_flow(2.0, 0.5, 0.0, 0.015);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_orifice_flow_typical() {
        // C_d = 0.62, d = 0.05 m → A = π·d²/4, h = 2 m
        let area = std::f64::consts::PI * 0.05 * 0.05 / 4.0;
        let q = orifice_flow(0.62, area, 2.0);
        let g = crate::domains::fluid::physics::G;
        let expected = 0.62 * area * (2.0 * g * 2.0).sqrt();
        assert!((q - expected).abs() < 1e-12);
        // Approx: ~0.003 m³/s
        assert!(q > 0.002);
        assert!(q < 0.01);
    }

    #[test]
    fn test_orifice_flow_zero_head() {
        let q = orifice_flow(0.62, 0.01, 0.0);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_weir_flow_typical() {
        // C_d = 1.84, L = 1.5 m, H = 0.3 m
        let q = weir_flow(1.84, 1.5, 0.3);
        let expected = 1.84 * 1.5 * (0.3_f64).powf(1.5);
        assert!((q - expected).abs() < 1e-12);
        // Approx: ~0.454 m³/s
        assert!((q - 0.454).abs() < 0.01);
    }

    #[test]
    fn test_weir_flow_zero_head() {
        let q = weir_flow(1.84, 1.5, 0.0);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_water_hammer_typical() {
        // Water: ρ = 1000, a = 1200 m/s, ΔU = 2 m/s
        let dp = water_hammer_pressure(1000.0, 1200.0, 2.0);
        assert!((dp - 2.4e6).abs() < 1.0);
    }

    #[test]
    fn test_water_hammer_negative_delta_u() {
        let dp = water_hammer_pressure(1000.0, 1200.0, -2.0);
        // Pressure rise is always positive (magnitude)
        assert!((dp - 2.4e6).abs() < 1.0);
    }

    #[test]
    fn test_water_hammer_invalid_density() {
        let dp = water_hammer_pressure(0.0, 1200.0, 2.0);
        assert_eq!(dp, 0.0);
    }
}
