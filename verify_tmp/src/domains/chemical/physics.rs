//! Fundamental physical constants for chemical engineering.
//!
//! Provides gas constant, standard atmosphere, Avogadro's number,
//! Faraday constant, and related property functions.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// Physical Constants
// ──────────────────────────────────────────────

/// Universal gas constant (J/(mol·K)).
pub const R: Scalar = 8.314462618;

/// Standard atmosphere (Pa).
pub const ATM: Scalar = 101325.0;

/// Avogadro constant (mol⁻¹).
pub const AVOGADRO: Scalar = 6.02214076e23;

/// Standard temperature (K) — 0°C.
pub const T_STP: Scalar = 273.15;

/// Molar volume of ideal gas at STP (m³/mol).
pub const MOLAR_VOLUME_STP: Scalar = 0.022414;

/// Faraday constant (C/mol).
pub const FARADAY: Scalar = 96485.33212;

/// Triple point of water (K).
pub const WATER_TRIPLE_POINT: Scalar = 273.16;

// ──────────────────────────────────────────────
// Property Functions
// ──────────────────────────────────────────────

/// Ideal gas law: PV = nRT.
/// Returns pressure (Pa) for given moles, volume (m³), and temperature (K).
pub fn ideal_gas_pressure(n: Scalar, v: Scalar, t: Scalar) -> Scalar {
    if v <= 0.0 || t <= 0.0 {
        return 0.0;
    }
    n * R * t / v
}

/// Ideal gas law: PV = nRT.
/// Returns volume (m³) for given moles, pressure (Pa), and temperature (K).
pub fn ideal_gas_volume(n: Scalar, p: Scalar, t: Scalar) -> Scalar {
    if p <= 0.0 || t <= 0.0 {
        return 0.0;
    }
    n * R * t / p
}

/// Ideal gas law: PV = nRT.
/// Returns temperature (K) for given moles, pressure (Pa), and volume (m³).
pub fn ideal_gas_temperature(n: Scalar, p: Scalar, v: Scalar) -> Scalar {
    if n <= 0.0 || v <= 0.0 {
        return 0.0;
    }
    p * v / (n * R)
}

/// Van der Waals equation pressure correction.
/// P = (nRT)/(V - nb) - a(n/V)²
pub fn van_der_waals_pressure(
    n: Scalar,
    v: Scalar,
    t: Scalar,
    a: Scalar,
    b: Scalar,
) -> Scalar {
    if v <= 0.0 || t <= 0.0 || (v - n * b) <= 0.0 {
        return 0.0;
    }
    (n * R * t) / (v - n * b) - a * (n / v).powi(2)
}

/// Reduced pressure from critical properties.
pub fn reduced_pressure(p: Scalar, p_critical: Scalar) -> Scalar {
    if p_critical <= 0.0 {
        return 0.0;
    }
    p / p_critical
}

/// Reduced temperature from critical properties.
pub fn reduced_temperature(t: Scalar, t_critical: Scalar) -> Scalar {
    if t_critical <= 0.0 {
        return 0.0;
    }
    t / t_critical
}

/// Water density (kg/m³) at given temperature (K), 0–100°C range.
pub fn water_density(temp: Scalar) -> Scalar {
    let tc: Scalar = temp - 273.15;
    if tc < 0.0 || tc > 100.0 {
        // Extended range approximation
        let tc_clamped = tc.clamp(0.0, 100.0);
        999.842594 + 6.793952e-2 * tc_clamped - 9.095290e-3 * tc_clamped.powi(2)
            + 1.001685e-4 * tc_clamped.powi(3)
            - 1.120083e-6 * tc_clamped.powi(4)
            + 6.536332e-9 * tc_clamped.powi(5)
    } else {
        999.842594 + 6.793952e-2 * tc - 9.095290e-3 * tc.powi(2)
            + 1.001685e-4 * tc.powi(3)
            - 1.120083e-6 * tc.powi(4)
            + 6.536332e-9 * tc.powi(5)
    }
}

/// Water dynamic viscosity (Pa·s) at given temperature (K).
pub fn water_viscosity(temp: Scalar) -> Scalar {
    let tc: Scalar = temp - 273.15;
    if tc < 0.0 {
        return 1.8e-3; // ice-like viscosity lower bound
    }
    2.414e-5 * 10.0_f64.powf(247.8 / (tc + 140.0))
}

/// Convert Celsius to Kelvin.
pub fn celsius_to_kelvin(c: Scalar) -> Scalar {
    c + 273.15
}

/// Convert Kelvin to Celsius.
pub fn kelvin_to_celsius(k: Scalar) -> Scalar {
    k - 273.15
}

#[cfg(test)]
mod tests {
    #![allow(clippy::let_and_return, clippy::manual_range_contains, clippy::single_match, clippy::unnecessary_unwrap)]
    use super::*;

    #[test]
    fn test_ideal_gas_pressure() {
        // 1 mole at STP: P = nRT/V = 1*8.314*273.15/0.022414 ≈ 101325 Pa
        let p = ideal_gas_pressure(1.0, MOLAR_VOLUME_STP, T_STP);
        assert!((p - ATM).abs() / ATM < 1e-3);
    }

    #[test]
    fn test_ideal_gas_zero_volume() {
        assert_eq!(ideal_gas_pressure(1.0, 0.0, 300.0), 0.0);
    }

    #[test]
    fn test_van_der_waals_pressure() {
        // CO₂: a=0.364, b=4.27e-5, n=1, v=0.1, T=300K
        let p = van_der_waals_pressure(1.0, 0.1, 300.0, 0.364, 4.27e-5);
        assert!(p > 20000.0);
        assert!(p < 30000.0);
    }

    #[test]
    fn test_reduced_properties() {
        assert!((reduced_pressure(ATM, 2.0 * ATM) - 0.5).abs() < 1e-12);
        assert!((reduced_temperature(300.0, 600.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_water_density_4c() {
        let rho = water_density(277.15);
        assert!((rho - 1000.0).abs() < 5.0);
    }

    #[test]
    fn test_water_viscosity_37c() {
        let eta = water_viscosity(310.15);
        assert!(eta > 0.0);
        assert!(eta < 0.001);
    }

    #[test]
    fn test_temperature_conversion() {
        assert!((celsius_to_kelvin(0.0) - T_STP).abs() < 1e-12);
        assert!((kelvin_to_celsius(T_STP)).abs() < 1e-12);
        assert!((celsius_to_kelvin(100.0) - 373.15).abs() < 1e-12);
    }

    #[test]
    fn test_constants() {
        assert!(R > 0.0);
        assert!(ATM > 0.0);
        assert!(AVOGADRO > 0.0);
        assert!(FARADAY > 0.0);
        assert!((WATER_TRIPLE_POINT - 273.16).abs() < 1e-12);
    }
}
