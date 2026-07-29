//! Propulsion systems: rocket motor and turbojet engine performance models.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// Rocket Propulsion
// ──────────────────────────────────────────────

/// Rocket thrust (N).
///
/// F = ṁ · v_e + (p_e – p_∞) · A_e
pub fn rocket_thrust(
    mass_flow: Scalar,
    exit_velocity: Scalar,
    exit_pressure: Scalar,
    ambient_pressure: Scalar,
    exit_area: Scalar,
) -> Scalar {
    mass_flow * exit_velocity + (exit_pressure - ambient_pressure) * exit_area
}

/// Characteristic velocity c* (m/s), a measure of combustion chamber performance.
///
/// c* = p_c · A_t / ṁ
pub fn characteristic_velocity(chamber_pressure: Scalar, throat_area: Scalar, mass_flow: Scalar) -> Scalar {
    if mass_flow <= 0.0 {
        return 0.0;
    }
    chamber_pressure * throat_area / mass_flow
}

/// Specific impulse (s).
///
/// I_sp = F / (ṁ · g₀)
pub fn specific_impulse(thrust: Scalar, mass_flow: Scalar) -> Scalar {
    if mass_flow <= 0.0 {
        return 0.0;
    }
    thrust / (mass_flow * 9.80665)
}

/// Nozzle area ratio (exit-to-throat) from Mach number at exit.
///
/// A_e / A_t = (1/M) · [(2/(γ+1)) · (1 + (γ–1)/2 · M²)]^((γ+1)/(2·(γ–1)))
pub fn nozzle_area_ratio(mach: Scalar, gamma: Scalar) -> Scalar {
    if mach <= 0.0 {
        return Scalar::INFINITY;
    }
    let gp1 = gamma + 1.0;
    let gm1 = gamma - 1.0;
    let exponent = gp1 / (2.0 * gm1);
    let term = (2.0 / gp1) * (1.0 + 0.5 * gm1 * mach * mach);
    (1.0 / mach) * term.powf(exponent)
}

/// Isentropic flow relations for temperature, pressure, and density ratios.
///
/// Returns (T/T₀, p/p₀, ρ/ρ₀) where subscript 0 denotes stagnation conditions.
pub fn isentropic_flow(mach: Scalar, gamma: Scalar) -> (Scalar, Scalar, Scalar) {
    let gm1 = gamma - 1.0;
    let factor = 1.0 + 0.5 * gm1 * mach * mach;
    let t_ratio = 1.0 / factor;
    let p_ratio = t_ratio.powf(gamma / gm1);
    let rho_ratio = t_ratio.powf(1.0 / gm1);
    (t_ratio, p_ratio, rho_ratio)
}

// ──────────────────────────────────────────────
// Turbojet / Air-Breathing Propulsion
// ──────────────────────────────────────────────

/// Turbojet thrust (N).
///
/// F = (ṁ_air + ṁ_fuel) · v_e – ṁ_air · v_∞
pub fn turbojet_thrust(
    mass_flow_air: Scalar,
    mass_flow_fuel: Scalar,
    exhaust_velocity: Scalar,
    flight_velocity: Scalar,
) -> Scalar {
    (mass_flow_air + mass_flow_fuel) * exhaust_velocity - mass_flow_air * flight_velocity
}

/// Thrust-specific fuel consumption (kg/(N·s)).
///
/// TSFC = ṁ_fuel / F
pub fn thrust_specific_fuel_consumption(fuel_flow: Scalar, thrust: Scalar) -> Scalar {
    if thrust <= 0.0 {
        return Scalar::INFINITY;
    }
    fuel_flow / thrust
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rocket_thrust_vacuum() {
        let f = rocket_thrust(10.0, 3000.0, 0.0, 0.0, 1.0);
        assert!((f - 30_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_rocket_thrust_with_pressure_term() {
        let f = rocket_thrust(10.0, 3000.0, 50_000.0, 0.0, 1.0);
        assert!((f - 80_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_rocket_thrust_sea_level() {
        let f = rocket_thrust(10.0, 3000.0, 50_000.0, 101_325.0, 1.0);
        assert!((f + 21_325.0).abs() < 1e-6);
    }

    #[test]
    fn test_characteristic_velocity() {
        let c_star = characteristic_velocity(10e6, 0.1, 50.0);
        assert!((c_star - 20_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_specific_impulse() {
        let isp = specific_impulse(500_000.0, 200.0);
        assert!((isp - 254.93).abs() < 0.1);
    }

    #[test]
    fn test_nozzle_area_ratio_mach1() {
        let ar = nozzle_area_ratio(1.0, 1.4);
        assert!((ar - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_nozzle_area_ratio_supersonic() {
        let ar = nozzle_area_ratio(2.0, 1.4);
        assert!(ar > 1.0);
        assert!((ar - 1.687).abs() < 0.01);
    }

    #[test]
    fn test_isentropic_flow_mach0() {
        let (t, p, rho) = isentropic_flow(0.0, 1.4);
        assert!((t - 1.0).abs() < 1e-10);
        assert!((p - 1.0).abs() < 1e-10);
        assert!((rho - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_isentropic_flow_mach1() {
        let (t, p, rho) = isentropic_flow(1.0, 1.4);
        assert!((t - 0.83333).abs() < 0.001);
        assert!((p - 0.528).abs() < 0.01);
        assert!((rho - 0.634).abs() < 0.01);
    }

    #[test]
    fn test_turbojet_thrust_static() {
        let f = turbojet_thrust(100.0, 2.0, 500.0, 0.0);
        assert!((f - 51_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_turbojet_thrust_forward() {
        let f = turbojet_thrust(100.0, 2.0, 500.0, 250.0);
        assert!((f - 26_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_tsfc_typical() {
        let tsfc = thrust_specific_fuel_consumption(0.5, 50_000.0);
        assert!((tsfc - 1e-5).abs() < 1e-8);
    }

    #[test]
    fn test_tsfc_zero_thrust() {
        let tsfc = thrust_specific_fuel_consumption(0.5, 0.0);
        assert!(tsfc.is_infinite());
    }
}
