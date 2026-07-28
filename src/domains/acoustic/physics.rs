//! Acoustic physical constants and speed-of-sound models.

use crate::core::types::Scalar;

/// Speed of sound in air at 20°C (m/s).
pub const SPEED_OF_SOUND_AIR: Scalar = 343.0;

/// Speed of sound in water (m/s).
pub const SPEED_OF_SOUND_WATER: Scalar = 1482.0;

/// Speed of sound in steel (longitudinal, m/s).
pub const SPEED_OF_SOUND_STEEL: Scalar = 5900.0;

/// Characteristic impedance of air at 20°C (rayl).
pub const Z0_AIR: Scalar = 413.0;

/// Characteristic impedance of water (rayl).
pub const Z0_WATER: Scalar = 1.48e6;

/// Reference sound pressure in air (Pa).
pub const P_REF_AIR: Scalar = 20e-6;

/// Reference sound pressure in water (Pa).
pub const P_REF_WATER: Scalar = 1e-6;

/// Speed of sound in air as a function of temperature (°C).
///
/// c(T) = 331.3 · √(1 + T/273.15)
pub fn speed_of_sound_air(temperature_c: Scalar) -> Scalar {
    331.3 * (1.0 + temperature_c / 273.15).sqrt()
}

/// Speed of sound in water (simplified Mackenzie equation).
///
/// c(T, S, D) = 1448.96 + 4.591·T - 5.304e-2·T² + 2.374e-4·T³
///              + 1.340·(S-35) + 1.630e-2·D + 1.675e-7·D²
///              - 1.025e-2·T·(S-35) - 7.139e-13·T·D³
/// T: °C, S: salinity (ppt), D: depth (m)
pub fn speed_of_sound_water(temperature_c: Scalar, salinity_ppt: Scalar, depth_m: Scalar) -> Scalar {
    1448.96
        + 4.591 * temperature_c
        - 5.304e-2 * temperature_c * temperature_c
        + 2.374e-4 * temperature_c * temperature_c * temperature_c
        + 1.340 * (salinity_ppt - 35.0)
        + 1.630e-2 * depth_m
        + 1.675e-7 * depth_m * depth_m
        - 1.025e-2 * temperature_c * (salinity_ppt - 35.0)
        - 7.139e-13 * temperature_c * depth_m * depth_m * depth_m
}

/// Characteristic impedance: Z = ρ · c.
pub fn characteristic_impedance(density: Scalar, speed: Scalar) -> Scalar {
    density * speed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_of_sound_air_20c() {
        let c = speed_of_sound_air(20.0);
        assert!((c - 343.0).abs() / 343.0 < 0.02);
    }

    #[test]
    fn test_speed_of_sound_air_0c() {
        let c = speed_of_sound_air(0.0);
        assert!((c - 331.3).abs() < 0.1);
    }

    #[test]
    fn test_speed_of_sound_water_typical() {
        let c = speed_of_sound_water(20.0, 35.0, 0.0);
        assert!((c - 1520.0).abs() / 1520.0 < 0.02);
    }

    #[test]
    fn test_characteristic_impedance() {
        let z = characteristic_impedance(1.2, 343.0);
        assert!((z - 1.2 * 343.0).abs() < 1e-10);
    }

}
