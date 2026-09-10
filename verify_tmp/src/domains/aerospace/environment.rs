//! High-altitude environment: extended atmosphere, gravity variation,
//! aerodynamic heating, and ambient temperature models.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// High-Altitude Atmosphere (above 86 km)
// ──────────────────────────────────────────────

/// High-altitude atmospheric model extending from the stratosphere up to
/// the edge of space (~1000 km) using the US Standard Atmosphere 1976
/// exponential decay model.
pub struct HighAltitudeAtmosphere;

impl HighAltitudeAtmosphere {
    /// Temperature (K) at high altitude (m).
    ///
    /// Above 86 km, temperature continues to rise through the thermosphere:
    /// - 86–110 km: linear increase from ~186.87 K to ~240.0 K
    /// - 110–120 km: linear increase to ~360.0 K
    /// - Above 120 km: exponential approach to exospheric temperature (~1000 K)
    pub fn temperature(altitude: Scalar) -> Scalar {
        let h = altitude.max(86_000.0);

        if h <= 110_000.0 {
            // Linear from mesopause (86 km, 186.87 K) to 110 km (240 K)
            let t86 = 186.87;
            let t110 = 240.0;
            t86 + (t110 - t86) * (h - 86_000.0) / (110_000.0 - 86_000.0)
        } else if h <= 120_000.0 {
            // 110–120 km: rapid increase
            let t110 = 240.0;
            let t120 = 360.0;
            t110 + (t120 - t110) * (h - 110_000.0) / (120_000.0 - 110_000.0)
        } else {
            // Above 120 km: approach exospheric temperature asymptotically
            let t_inf = 1000.0; // exospheric temperature (K)
            let t120 = 360.0;
            let scale_h = 50_000.0; // scale height for temperature rise
            t_inf - (t_inf - t120) * (-(h - 120_000.0) / scale_h).exp()
        }
    }

    /// Pressure (Pa) at high altitude (m).
    ///
    /// Uses the barometric formula with temperature gradient computed
    /// piecewise.
    pub fn pressure(altitude: Scalar) -> Scalar {
        let h = altitude.max(86_000.0);

        // Reference at 86 km
        let h0 = 86_000.0;
        let p0 = 0.373; // Pa at 86 km
        let t0 = Self::temperature(h0);

        if h <= 110_000.0 {
            // Non-isothermal: use layer average temperature
            let t = Self::temperature(h);
            let t_avg = 0.5 * (t0 + t);
            let scale_h = crate::domains::aerospace::physics::R_AIR * t_avg / 9.80665;
            p0 * (-(h - h0) / scale_h).exp()
        } else if h <= 120_000.0 {
            let p110 = Self::pressure(110_000.0);
            let t110 = Self::temperature(110_000.0);
            let t = Self::temperature(h);
            let t_avg = 0.5 * (t110 + t);
            let scale_h = crate::domains::aerospace::physics::R_AIR * t_avg / 9.80665;
            p110 * (-(h - 110_000.0) / scale_h).exp()
        } else {
            // Isothermal approximation with scale height at local temperature
            let p120 = Self::pressure(120_000.0);
            let t = Self::temperature(h);
            let scale_h = crate::domains::aerospace::physics::R_AIR * t / 9.80665;
            p120 * (-(h - 120_000.0) / scale_h).exp()
        }
    }

    /// Density (kg/m³) at high altitude (m).
    pub fn density(altitude: Scalar) -> Scalar {
        let p = Self::pressure(altitude);
        let t = Self::temperature(altitude);
        if t <= 0.0 {
            return 0.0;
        }
        p / (crate::domains::aerospace::physics::R_AIR * t)
    }

    /// Speed of sound (m/s) at high altitude (m).
    pub fn speed_of_sound(altitude: Scalar) -> Scalar {
        let t = Self::temperature(altitude);
        if t <= 0.0 {
            return 0.0;
        }
        (crate::domains::aerospace::physics::GAMMA_AIR
            * crate::domains::aerospace::physics::R_AIR
            * t)
        .sqrt()
    }
}

// ──────────────────────────────────────────────
// Gravity & Environment Utilities
// ──────────────────────────────────────────────

/// Gravitational acceleration (m/s²) at a given altitude (m) above sea level.
///
/// g(h) = g₀ · (Rₑ / (Rₑ + h))²
pub fn gravity_at_altitude(altitude: Scalar) -> Scalar {
    let r = crate::domains::aerospace::physics::EARTH_RADIUS;
    crate::domains::aerospace::physics::G0 * (r / (r + altitude.max(0.0))).powi(2)
}

/// Aerodynamic heating rate (W/m²) at a surface point.
///
/// Uses a simplified stagnation-point heating model:
/// q̇ = C · ρ^N · V^M
/// where C is derived from the Stanton number.
///
/// * `density` — freestream density (kg/m³)
/// * `velocity` — freestream velocity (m/s)
/// * `stanton` — Stanton number (dimensionless), typically ~0.01–0.05
pub fn aerodynamic_heating(density: Scalar, velocity: Scalar, stanton: Scalar) -> Scalar {
    if density <= 0.0 || velocity <= 0.0 {
        return 0.0;
    }
    // q = St · ρ · V³ · 0.5 (simplified)
    stanton * density * velocity.powi(3) * 0.5
}

/// Ambient temperature (K) as a continuous function of altitude (m),
/// covering troposphere through thermosphere.
pub fn ambient_temperature(altitude: Scalar) -> Scalar {
    if altitude <= 86_000.0 {
        crate::domains::aerospace::physics::IsaAtmosphere::temperature(altitude)
    } else {
        HighAltitudeAtmosphere::temperature(altitude)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_high_alt_temp_at_86km() {
        let t = HighAltitudeAtmosphere::temperature(86_000.0);
        assert!((t - 186.87).abs() < 1.0);
    }

    #[test]
    fn test_high_alt_temp_at_110km() {
        let t = HighAltitudeAtmosphere::temperature(110_000.0);
        assert!((t - 240.0).abs() < 5.0);
    }

    #[test]
    fn test_high_alt_pressure_decreases() {
        let p86 = HighAltitudeAtmosphere::pressure(86_000.0);
        let p200 = HighAltitudeAtmosphere::pressure(200_000.0);
        assert!(p200 < p86);
        assert!(p200 > 0.0);
    }

    #[test]
    fn test_high_alt_density_positive() {
        let rho = HighAltitudeAtmosphere::density(100_000.0);
        assert!(rho > 0.0);
        assert!(rho < 1.0);
    }

    #[test]
    fn test_high_alt_speed_of_sound() {
        let a = HighAltitudeAtmosphere::speed_of_sound(100_000.0);
        assert!(a > 200.0);
    }

    #[test]
    fn test_gravity_at_sea_level() {
        let g = gravity_at_altitude(0.0);
        assert!((g - 9.80665).abs() < 0.01);
    }

    #[test]
    fn test_gravity_decreases_with_altitude() {
        let g0 = gravity_at_altitude(0.0);
        let g_high = gravity_at_altitude(100_000.0);
        assert!(g_high < g0);
        assert!(g_high > 9.0);
    }

    #[test]
    fn test_gravity_at_altitude_negative() {
        let g = gravity_at_altitude(-1000.0);
        assert!((g - 9.80665).abs() < 0.01);
    }

    #[test]
    fn test_aerodynamic_heating_zero_velocity() {
        let q = aerodynamic_heating(1.225, 0.0, 0.01);
        assert!((q).abs() < 1e-10);
    }

    #[test]
    fn test_aerodynamic_heating_typical() {
        // Subsonic: ~250 m/s, sea level density
        let q = aerodynamic_heating(1.225, 250.0, 0.01);
        assert!(q > 0.0);
        assert!(q < 1e6); // reasonable upper bound
    }

    #[test]
    fn test_aerodynamic_heating_hypersonic() {
        let q = aerodynamic_heating(0.01, 2000.0, 0.02);
        assert!(q > 0.0);
    }

    #[test]
    fn test_ambient_temperature_continuity() {
        // Should be continuous at 86 km boundary
        let t_isa = crate::domains::aerospace::physics::IsaAtmosphere::temperature(86_000.0);
        let t_high = HighAltitudeAtmosphere::temperature(86_000.0);
        assert!((t_isa - t_high).abs() < 10.0);
    }

    #[test]
    fn test_ambient_temperature_coverage() {
        let t_sl = ambient_temperature(0.0);
        let t_50k = ambient_temperature(50_000.0);
        let t_100k = ambient_temperature(100_000.0);
        assert!(t_sl > 0.0);
        assert!(t_50k > 0.0);
        assert!(t_100k > 0.0);
    }
}
