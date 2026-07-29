//! Combustion and reactive flow models.
//!
//! Provides adiabatic flame temperature, laminar flame speed,
//! explosive limits, and auto-catalytic conversion models.

use crate::core::types::Scalar;

/// Adiabatic flame temperature estimate.
///
/// T_ad = T_initial + HHV_fuel / (Cp_products · (1 + excess_air))
///
/// where HHV is the higher heating value (J/kg fuel),
/// Cp_products is the average specific heat of combustion products (J/(kg·K)),
/// and excess_air is the fractional excess air (e.g., 0.2 for 20% excess).
pub fn adiabatic_flame_temperature(
    fuel_hhv: Scalar,
    cp_products: Scalar,
    t_initial: Scalar,
    excess_air: Scalar,
) -> Scalar {
    if cp_products <= 0.0 || fuel_hhv < 0.0 {
        return t_initial;
    }
    let afr = 17.0; // stoichiometric air-fuel ratio (methane)
    t_initial + fuel_hhv / (cp_products * (1.0 + excess_air) * afr)
}

/// Laminar flame speed approximation (methane-air like).
///
/// S_L = S_L0 · (T_u / T_u0)^1.5 · (P / P0)^(-0.3)
///
/// Simplified: S_L ≈ 0.4 · (T_unburned / 300)^1.5
pub fn laminar_flame_speed(
    unburned_temp: Scalar,
    _pressure: Scalar,
    _equivalence_ratio: Scalar,
) -> Scalar {
    if unburned_temp <= 0.0 {
        return 0.0;
    }
    0.4 * (unburned_temp / 300.0).powf(1.5)
}

/// Flammability (explosive) limits for common gases.
///
/// Returns (lower_flammable_limit, upper_flammable_limit) as
/// volume fraction in air, or None if the gas is not recognized.
///
/// # Supported Gases
///
/// - "methane": 5%–15%
/// - "hydrogen": 4%–75%
/// - "propane": 2.1%–9.5%
/// - "acetylene": 2.5%–80%
/// - "carbon_monoxide": 12.5%–74%
pub fn explosive_limits(gas_name: &str) -> Option<(Scalar, Scalar)> {
    match gas_name {
        "methane" => Some((0.05, 0.15)),
        "hydrogen" => Some((0.04, 0.75)),
        "propane" => Some((0.021, 0.095)),
        "acetylene" => Some((0.025, 0.80)),
        "carbon_monoxide" => Some((0.125, 0.74)),
        _ => None,
    }
}

/// Auto-catalytic conversion model with time evolution.
///
/// X(t) = X₀·(1 - X₀)·k·t / (1 + X₀·k·t)
///
/// where X is conversion, k is rate constant, t is time.
/// This represents a sigmoidal conversion curve typical of
/// autocatalytic reactions.
pub fn auto_catalytic_conversion(conversion: Scalar, k: Scalar, t: Scalar) -> Scalar {
    if conversion <= 0.0 || k <= 0.0 || t <= 0.0 {
        return 0.0;
    }
    conversion * (1.0 - conversion) * k * t / (1.0 + conversion * k * t)
}

/// Stoichiometric oxygen requirement for complete combustion of a hydrocarbon.
///
/// C_xH_y + (x + y/4)O₂ → xCO₂ + (y/2)H₂O
/// Returns moles of O₂ per mole of fuel.
pub fn stoichiometric_oxygen(carbon: usize, hydrogen: usize) -> Scalar {
    carbon as Scalar + hydrogen as Scalar / 4.0
}

/// Stoichiometric air requirement from oxygen requirement.
/// Air is ~21% O₂ by mole.
pub fn stoichiometric_air(o2_moles: Scalar) -> Scalar {
    o2_moles / 0.21
}

/// Calculate equivalence ratio from actual fuel/air ratio and stoichiometric ratio.
///
/// φ = (F/A)_actual / (F/A)_stoich
/// φ < 1: lean mixture, φ = 1: stoichiometric, φ > 1: rich mixture
pub fn equivalence_ratio(fuel_air_actual: Scalar, fuel_air_stoich: Scalar) -> Scalar {
    if fuel_air_stoich <= 0.0 {
        return 0.0;
    }
    fuel_air_actual / fuel_air_stoich
}

/// Higher heating value estimate for common fuels (MJ/kg).
pub fn fuel_hhv(fuel_name: &str) -> Option<Scalar> {
    match fuel_name {
        "methane" => Some(55.5e6),
        "hydrogen" => Some(141.8e6),
        "propane" => Some(50.3e6),
        "acetylene" => Some(48.2e6),
        "carbon_monoxide" => Some(10.1e6),
        "ethanol" => Some(29.7e6),
        "gasoline" => Some(44.0e6),
        "diesel" => Some(45.5e6),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adiabatic_flame_temperature() {
        // Methane HHV ~55.5 MJ/kg, Cp ~1.2 kJ/(kg·K), 10% excess air
        let t_ad = adiabatic_flame_temperature(55.5e6, 1200.0, 298.0, 0.1);
        // Should be significantly higher than ambient
        assert!(t_ad > 1000.0);
        assert!(t_ad < 5000.0);
    }

    #[test]
    fn test_adiabatic_flame_zero_cp() {
        let t_ad = adiabatic_flame_temperature(55.5e6, 0.0, 298.0, 0.1);
        assert_eq!(t_ad, 298.0);
    }

    #[test]
    fn test_laminar_flame_speed() {
        let s = laminar_flame_speed(600.0, 1.0, 1.0);
        assert!(s > 0.0);
        // At 600K, S_L ~ 0.4 * (600/300)^1.5 = 0.4 * 2^1.5 = 0.4 * 2.828 = 1.13
        assert!((s - 0.4 * Scalar::powf(600.0 / 300.0, 1.5)).abs() < 1e-12);
    }

    #[test]
    fn test_explosive_limits_methane() {
        let limits = explosive_limits("methane");
        assert!(limits.is_some());
        let (lfl, ufl) = limits.unwrap();
        assert!((lfl - 0.05).abs() < 1e-10);
        assert!((ufl - 0.15).abs() < 1e-10);
    }

    #[test]
    fn test_explosive_limits_unknown() {
        assert!(explosive_limits("unknown_gas").is_none());
    }

    #[test]
    fn test_explosive_limits_all() {
        for gas in &["methane", "hydrogen", "propane", "acetylene", "carbon_monoxide"] {
            assert!(explosive_limits(gas).is_some(), "gas {gas} should have limits");
        }
    }

    #[test]
    fn test_auto_catalytic_conversion() {
        let x = auto_catalytic_conversion(0.01, 0.1, 10.0);
        assert!(x > 0.0);
        assert!(x < 1.0);
    }

    #[test]
    fn test_auto_catalytic_zero_input() {
        assert_eq!(auto_catalytic_conversion(0.0, 0.1, 10.0), 0.0);
        assert_eq!(auto_catalytic_conversion(0.5, 0.0, 10.0), 0.0);
    }

    #[test]
    fn test_stoichiometric_oxygen() {
        // Methane CH₄: 1 + 4/4 = 2
        assert!((stoichiometric_oxygen(1, 4) - 2.0).abs() < 1e-12);
        // Propane C₃H₈: 3 + 8/4 = 5
        assert!((stoichiometric_oxygen(3, 8) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_equivalence_ratio() {
        // Stoichiometric
        assert!((equivalence_ratio(1.0, 1.0) - 1.0).abs() < 1e-12);
        // Lean
        assert!(equivalence_ratio(0.5, 1.0) < 1.0);
        // Rich
        assert!(equivalence_ratio(1.5, 1.0) > 1.0);
    }

    #[test]
    fn test_fuel_hhv() {
        assert!(fuel_hhv("methane").unwrap() > 0.0);
        assert!(fuel_hhv("unknown").is_none());
    }
}
