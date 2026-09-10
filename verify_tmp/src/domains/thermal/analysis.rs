//! Thermal system analysis: heat sinks, heat pipes, radiators, cooling systems.
//!
//! Provides engineering analysis functions for thermal management design:
//! heat sink thermal resistance, heat pipe effective conductivity, cooling
//! coefficient of performance, and temperature gradient.

use crate::core::types::Scalar;

/// Heat sink thermal resistance (K/W).
///
/// R_th = 1 / (h · η_fin · A_fin + h · A_base)
///
/// Simplified model: R_th = 1 / (h · A_total · η)
/// where h is the convection coefficient, A_total is the total surface area,
/// and η is the overall fin efficiency.
pub fn heatsink_thermal_resistance(
    air_flow: Scalar,
    fin_area: Scalar,
    fin_efficiency: Scalar,
    h: Scalar,
) -> Scalar {
    if air_flow <= 0.0 || fin_area <= 0.0 || fin_efficiency <= 0.0 || h <= 0.0 {
        return Scalar::INFINITY;
    }
    // Effective heat transfer area includes fin area and base plate area
    let effective_area = fin_area * fin_efficiency;
    let total_heat_transfer = h * effective_area;
    if total_heat_transfer <= 0.0 {
        return Scalar::INFINITY;
    }
    1.0 / total_heat_transfer
}

/// Effective thermal conductivity of a heat pipe (W/(m·K)).
///
/// k_eff = Q · L / (A_cross · ΔT)
///
/// where Q is the heat input (W), L is the heat pipe length (m),
/// A_cross is the cross-sectional area (m²), and ΔT is the temperature
/// difference between evaporator and condenser (K).
pub fn heat_pipe_effective_k(
    heat_input: Scalar,
    delta_t: Scalar,
    length: Scalar,
    cross_section: Scalar,
) -> Scalar {
    if delta_t <= 0.0 || length <= 0.0 || cross_section <= 0.0 || heat_input < 0.0 {
        return 0.0;
    }
    heat_input * length / (cross_section * delta_t)
}

/// Coefficient of Performance (COP) for a cooling system.
///
/// COP = Q_cooling / P_electrical
///
/// where Q_cooling is the cooling power (W) and P_electrical is the
/// electrical power input (W).
pub fn cooling_cop(cooling_power: Scalar, electrical_power: Scalar) -> Scalar {
    if electrical_power <= 0.0 || cooling_power <= 0.0 {
        return 0.0;
    }
    cooling_power / electrical_power
}

/// Temperature gradient along a direction (K/m).
///
/// dT/dx = (T₂ - T₁) / distance
pub fn temperature_gradient(t1: Scalar, t2: Scalar, distance: Scalar) -> Scalar {
    if distance <= 0.0 {
        return 0.0;
    }
    (t2 - t1) / distance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heatsink_thermal_resistance_basic() {
        // Typical CPU heatsink: air_flow=0.01 m³/s, fin_area=0.5 m², η=0.9, h=50 W/(m²·K)
        let r_th = heatsink_thermal_resistance(0.01, 0.5, 0.9, 50.0);
        assert!(r_th.is_finite());
        assert!(r_th > 0.0);
    }

    #[test]
    fn test_heatsink_thermal_resistance_no_flow() {
        let r_th = heatsink_thermal_resistance(0.0, 0.5, 0.9, 50.0);
        assert!(r_th.is_infinite());
    }

    #[test]
    fn test_heatsink_thermal_resistance_zero_area() {
        let r_th = heatsink_thermal_resistance(0.01, 0.0, 0.9, 50.0);
        assert!(r_th.is_infinite());
    }

    #[test]
    fn test_heat_pipe_effective_k_basic() {
        // Q=100 W, ΔT=5 K, L=0.2 m, A=1e-4 m²
        let k = heat_pipe_effective_k(100.0, 5.0, 0.2, 1e-4);
        // k_eff = 100 * 0.2 / (1e-4 * 5) = 40000 W/(m·K)
        assert!((k - 40000.0).abs() < 1.0);
    }

    #[test]
    fn test_heat_pipe_effective_k_zero_delta() {
        let k = heat_pipe_effective_k(100.0, 0.0, 0.2, 1e-4);
        assert_eq!(k, 0.0);
    }

    #[test]
    fn test_heat_pipe_effective_k_high_conductivity() {
        let k = heat_pipe_effective_k(100.0, 1.0, 0.2, 1e-4);
        // Should be much higher than copper (401)
        assert!(k > 401.0);
    }

    #[test]
    fn test_cooling_cop_basic() {
        let cop = cooling_cop(1000.0, 250.0);
        assert!((cop - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_cooling_cop_zero_power() {
        let cop = cooling_cop(1000.0, 0.0);
        assert_eq!(cop, 0.0);
    }

    #[test]
    fn test_cooling_cop_ideal() {
        // Ideal Carnot COP for cooling: T_cold / (T_hot - T_cold)
        // Not directly tested here, just sanity
        let cop = cooling_cop(500.0, 100.0);
        assert!((cop - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_temperature_gradient_basic() {
        let grad = temperature_gradient(300.0, 350.0, 0.1);
        assert!((grad - 500.0).abs() < 1e-10);
    }

    #[test]
    fn test_temperature_gradient_zero_distance() {
        let grad = temperature_gradient(300.0, 350.0, 0.0);
        assert_eq!(grad, 0.0);
    }

    #[test]
    fn test_temperature_gradient_negative() {
        let grad = temperature_gradient(350.0, 300.0, 0.1);
        assert!((grad + 500.0).abs() < 1e-10);
    }
}
