//! Aerospace physical constants and ISA (International Standard Atmosphere) model.
//!
//! Provides the `IsaAtmosphere` struct with static methods for computing atmospheric
//! properties (temperature, pressure, density, speed of sound, dynamic viscosity)
//! as functions of geometric altitude.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// ISA Standard Atmosphere Constants
// ──────────────────────────────────────────────

/// Sea-level standard temperature (K).
pub const ISA_SL_TEMP: Scalar = 288.15;

/// Sea-level standard pressure (Pa).
pub const ISA_SL_PRESSURE: Scalar = 101_325.0;

/// Sea-level standard density (kg/m³).
pub const ISA_SL_DENSITY: Scalar = 1.225;

/// Troposphere temperature lapse rate (K/m).
pub const ISA_LAPSE_RATE: Scalar = 0.0065;

/// Specific gas constant for dry air (J/(kg·K)).
pub const R_AIR: Scalar = 287.058;

/// Ratio of specific heats for air (dimensionless).
pub const GAMMA_AIR: Scalar = 1.4;

/// Standard gravitational acceleration at sea level (m/s²).
pub const G0: Scalar = 9.80665;

/// Mean Earth radius (m).
pub const EARTH_RADIUS: Scalar = 6_371_000.0;

/// Earth mass (kg).
pub const EARTH_MASS: Scalar = 5.9722e24;

/// Earth gravitational parameter (m³/s²).
pub const EARTH_GRAVITATIONAL_PARAMETER: Scalar = 3.986_004_418e14;

/// Earth rotation rate (rad/s).
pub const EARTH_ROTATION_RATE: Scalar = 7.292_115_0e-5;

// ──────────────────────────────────────────────
// ISA Layer Constants
// ──────────────────────────────────────────────

/// Sutherland's constant for air (K).
const SUTHERLAND_S: Scalar = 110.4;

/// Reference viscosity (Pa·s) at reference temperature.
const SUTHERLAND_MU0: Scalar = 1.716e-5;

/// Sutherland reference temperature (K).
const SUTHERLAND_T0: Scalar = 273.15;

/// ISA layer boundary altitudes (m).
const ISA_H_TROPOPAUSE: Scalar = 11_000.0;
const ISA_H_STRATOSPHERE1: Scalar = 20_000.0;
const ISA_H_STRATOSPHERE2: Scalar = 32_000.0;
const ISA_H_STRATOSPHERE3: Scalar = 47_000.0;
const ISA_H_MESOSPHERE1: Scalar = 51_000.0;
const ISA_H_MESOSPHERE2: Scalar = 71_000.0;
const ISA_H_MESOPAUSE: Scalar = 86_000.0;

/// Lapse rate for each layer (K/m). Positive means temperature decreases with altitude.
const LAPSE_TROP: Scalar = 0.0065;
const LAPSE_STRAT2: Scalar = -0.0010; // temperature increases
const LAPSE_STRAT3: Scalar = -0.0028;
const LAPSE_MESO2: Scalar = 0.0028;
const LAPSE_MESO3: Scalar = 0.0020;

// ──────────────────────────────────────────────
// ISA Atmosphere Implementation
// ──────────────────────────────────────────────

/// International Standard Atmosphere (ISA) model.
///
/// Computes atmospheric properties for altitudes from sea level up to 86 km.
/// Uses the standard ISA layer model with piecewise-linear temperature profile
/// in each atmospheric layer.
pub struct IsaAtmosphere;

impl IsaAtmosphere {
    /// Return the standard temperature (K) at a given geometric altitude (m).
    ///
    /// Uses the ISA layer model:
    /// - Troposphere (0–11 km):  T = T₀ – L·h
    /// - Tropopause (11–20 km):  T = 216.65 K (isothermal)
    /// - Stratosphere (20–32 km): T increases with L = –0.001 K/m
    /// - Stratosphere (32–47 km): T increases with L = –0.0028 K/m
    /// - Stratopause (47–51 km): T = 270.65 K (isothermal)
    /// - Mesosphere (51–71 km):  T decreases with L = 0.0028 K/m
    /// - Mesosphere (71–86 km):  T decreases with L = 0.0020 K/m
    /// - Above 86 km: clamped to mesopause temperature.
    pub fn temperature(altitude: Scalar) -> Scalar {
        let h = altitude.max(0.0);

        if h <= ISA_H_TROPOPAUSE {
            // Troposphere
            ISA_SL_TEMP - LAPSE_TROP * h
        } else if h <= ISA_H_STRATOSPHERE1 {
            // Tropopause (isothermal)
            IsaAtmosphere::temperature(ISA_H_TROPOPAUSE)
        } else if h <= ISA_H_STRATOSPHERE2 {
            // Lower stratosphere
            let t11 = IsaAtmosphere::temperature(ISA_H_TROPOPAUSE);
            t11 - LAPSE_STRAT2 * (h - ISA_H_STRATOSPHERE1)
        } else if h <= ISA_H_STRATOSPHERE3 {
            // Upper stratosphere
            let t20 = IsaAtmosphere::temperature(ISA_H_STRATOSPHERE2);
            t20 - LAPSE_STRAT3 * (h - ISA_H_STRATOSPHERE2)
        } else if h <= ISA_H_MESOSPHERE1 {
            // Stratopause (isothermal)
            IsaAtmosphere::temperature(ISA_H_STRATOSPHERE3)
        } else if h <= ISA_H_MESOSPHERE2 {
            // Lower mesosphere
            let t47 = IsaAtmosphere::temperature(ISA_H_STRATOSPHERE3);
            t47 - LAPSE_MESO2 * (h - ISA_H_MESOSPHERE1)
        } else if h <= ISA_H_MESOPAUSE {
            // Upper mesosphere
            let t51 = IsaAtmosphere::temperature(ISA_H_MESOSPHERE2);
            t51 - LAPSE_MESO3 * (h - ISA_H_MESOSPHERE2)
        } else {
            // Above 86 km — clamp
            IsaAtmosphere::temperature(ISA_H_MESOPAUSE)
        }
    }

    /// Return the static pressure (Pa) at a given geometric altitude (m).
    ///
    /// Uses the barometric formula for each layer:
    /// - Troposphere (non-isothermal):  p = p₀ · (T / T₀)^(g₀ / (R · L))
    /// - Isothermal layers:            p = pᵢ · exp(–g₀ · (h – hᵢ) / (R · Tᵢ))
    pub fn pressure(altitude: Scalar) -> Scalar {
        let h = altitude.max(0.0);

        if h <= ISA_H_TROPOPAUSE {
            // Troposphere
            let t = IsaAtmosphere::temperature(h);
            let t0 = ISA_SL_TEMP;
            let exponent = G0 / (R_AIR * LAPSE_TROP);
            ISA_SL_PRESSURE * (t / t0).powf(exponent)
        } else if h <= ISA_H_STRATOSPHERE1 {
            // Tropopause (isothermal)
            let p11 = IsaAtmosphere::pressure(ISA_H_TROPOPAUSE);
            let t11 = IsaAtmosphere::temperature(ISA_H_TROPOPAUSE);
            p11 * (-G0 * (h - ISA_H_TROPOPAUSE) / (R_AIR * t11)).exp()
        } else if h <= ISA_H_STRATOSPHERE2 {
            // Lower stratosphere (non-isothermal, negative lapse)
            let h0 = ISA_H_STRATOSPHERE1;
            let p0 = IsaAtmosphere::pressure(h0);
            let t0 = IsaAtmosphere::temperature(h0);
            let t = IsaAtmosphere::temperature(h);
            let exponent = -G0 / (R_AIR * LAPSE_STRAT2);
            p0 * (t / t0).powf(exponent)
        } else if h <= ISA_H_STRATOSPHERE3 {
            let h0 = ISA_H_STRATOSPHERE2;
            let p0 = IsaAtmosphere::pressure(h0);
            let t0 = IsaAtmosphere::temperature(h0);
            let t = IsaAtmosphere::temperature(h);
            let exponent = -G0 / (R_AIR * LAPSE_STRAT3);
            p0 * (t / t0).powf(exponent)
        } else if h <= ISA_H_MESOSPHERE1 {
            // Stratopause (isothermal)
            let h0 = ISA_H_STRATOSPHERE3;
            let p0 = IsaAtmosphere::pressure(h0);
            let t0 = IsaAtmosphere::temperature(h0);
            p0 * (-G0 * (h - h0) / (R_AIR * t0)).exp()
        } else if h <= ISA_H_MESOSPHERE2 {
            let h0 = ISA_H_MESOSPHERE1;
            let p0 = IsaAtmosphere::pressure(h0);
            let t0 = IsaAtmosphere::temperature(h0);
            let t = IsaAtmosphere::temperature(h);
            let exponent = -G0 / (R_AIR * LAPSE_MESO2);
            p0 * (t / t0).powf(exponent)
        } else if h <= ISA_H_MESOPAUSE {
            let h0 = ISA_H_MESOSPHERE2;
            let p0 = IsaAtmosphere::pressure(h0);
            let t0 = IsaAtmosphere::temperature(h0);
            let t = IsaAtmosphere::temperature(h);
            let exponent = -G0 / (R_AIR * LAPSE_MESO3);
            p0 * (t / t0).powf(exponent)
        } else {
            // Above 86 km — exponential extrapolation
            let h0 = ISA_H_MESOPAUSE;
            let p0 = IsaAtmosphere::pressure(h0);
            let t0 = IsaAtmosphere::temperature(h0);
            p0 * (-G0 * (h - h0) / (R_AIR * t0)).exp()
        }
    }

    /// Return the air density (kg/m³) at a given geometric altitude (m).
    ///
    /// Uses the ideal gas law: ρ = p / (R · T)
    pub fn density(altitude: Scalar) -> Scalar {
        let p = IsaAtmosphere::pressure(altitude);
        let t = IsaAtmosphere::temperature(altitude);
        if t <= 0.0 {
            return 0.0;
        }
        p / (R_AIR * t)
    }

    /// Return the speed of sound (m/s) at a given geometric altitude (m).
    ///
    /// a = √(γ · R · T)
    pub fn speed_of_sound(altitude: Scalar) -> Scalar {
        let t = IsaAtmosphere::temperature(altitude);
        if t <= 0.0 {
            return 0.0;
        }
        (GAMMA_AIR * R_AIR * t).sqrt()
    }

    /// Return the dynamic viscosity (Pa·s) at a given geometric altitude (m).
    ///
    /// Uses Sutherland's law:
    /// μ = μ₀ · (T / T₀)^(3/2) · (T₀ + S) / (T + S)
    pub fn dynamic_viscosity(altitude: Scalar) -> Scalar {
        let t = IsaAtmosphere::temperature(altitude);
        if t <= 0.0 {
            return SUTHERLAND_MU0;
        }
        SUTHERLAND_MU0 * (t / SUTHERLAND_T0).powf(1.5) * (SUTHERLAND_T0 + SUTHERLAND_S)
            / (t + SUTHERLAND_S)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isa_sl_temperature() {
        let t = IsaAtmosphere::temperature(0.0);
        assert!((t - ISA_SL_TEMP).abs() < 0.01);
    }

    #[test]
    fn test_isa_sl_pressure() {
        let p = IsaAtmosphere::pressure(0.0);
        assert!((p - ISA_SL_PRESSURE).abs() < 1.0);
    }

    #[test]
    fn test_isa_sl_density() {
        let rho = IsaAtmosphere::density(0.0);
        assert!((rho - ISA_SL_DENSITY).abs() < 0.001);
    }

    #[test]
    fn test_isa_sl_speed_of_sound() {
        let a = IsaAtmosphere::speed_of_sound(0.0);
        assert!((a - 340.0).abs() < 5.0);
    }

    #[test]
    fn test_isa_sl_viscosity() {
        let mu = IsaAtmosphere::dynamic_viscosity(0.0);
        assert!((mu - 1.8e-5).abs() < 0.2e-5);
    }

    #[test]
    fn test_isa_tropopause_temperature() {
        let t = IsaAtmosphere::temperature(11_000.0);
        assert!((t - 216.65).abs() < 0.1);
    }

    #[test]
    fn test_isa_tropopause_pressure() {
        let p = IsaAtmosphere::pressure(11_000.0);
        assert!((p - 22632.0).abs() / 22632.0 < 0.02);
    }

    #[test]
    fn test_isa_stratosphere_20k() {
        let t = IsaAtmosphere::temperature(20_000.0);
        assert!((t - 216.65).abs() < 0.1);
    }

    #[test]
    fn test_isa_negative_altitude_clamped() {
        let t = IsaAtmosphere::temperature(-100.0);
        assert!((t - ISA_SL_TEMP).abs() < 0.01);
    }

    #[test]
    fn test_isa_density_decreases_with_altitude() {
        let rho0 = IsaAtmosphere::density(0.0);
        let rho11 = IsaAtmosphere::density(11_000.0);
        assert!(rho11 < rho0);
        assert!(rho11 > 0.0);
    }

    #[test]
    fn test_isa_pressure_decreases_with_altitude() {
        let p0 = IsaAtmosphere::pressure(0.0);
        let p20 = IsaAtmosphere::pressure(20_000.0);
        assert!(p20 < p0);
        assert!(p20 > 0.0);
    }

    #[test]
    fn test_isa_speed_of_sound_11km() {
        let a = IsaAtmosphere::speed_of_sound(11_000.0);
        assert!((a - 295.0).abs() < 5.0);
    }

    #[test]
    fn test_isa_viscosity_increases_with_temp() {
        let mu_sl = IsaAtmosphere::dynamic_viscosity(0.0);
        let mu_high = IsaAtmosphere::dynamic_viscosity(30_000.0);
        // At 30km temperature is lower than sea level, so viscosity is lower
        assert!(mu_high < mu_sl);
        assert!(mu_sl > 0.0);
    }

    #[test]
    fn test_isa_constants_are_plausible() {
        assert!(EARTH_MASS > 0.0);
        assert!(EARTH_GRAVITATIONAL_PARAMETER > 0.0);
        assert!(EARTH_ROTATION_RATE > 0.0);
        assert!(EARTH_RADIUS > 0.0);
        assert!(G0 > 0.0);
        assert!(R_AIR > 0.0);
        assert!(GAMMA_AIR > 1.0);
    }
}
