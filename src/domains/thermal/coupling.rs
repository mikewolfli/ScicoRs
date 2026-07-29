//! Multi-physics thermal coupling models.
//!
//! Provides interfaces between thermal and other physical domains:
//! electro-thermal (Joule heating), mechanical-thermal (friction heating,
//! thermal strain), and fluid-thermal (convective heat transfer).

use crate::core::types::Scalar;

/// Joule heating power from electric current: P = I² · R
///
/// Returns the power dissipated as heat (W).
pub fn joule_heating(current: Scalar, resistance: Scalar) -> Scalar {
    current * current * resistance
}

/// Friction heating power: P = F_friction · v
///
/// Returns the power dissipated as heat (W) from friction between
/// contacting surfaces.
pub fn friction_heating(friction_force: Scalar, velocity: Scalar) -> Scalar {
    friction_force * velocity
}

/// Convective heat transfer rate: Q = h · A · (T_surface - T_fluid)
///
/// Returns the heat transfer rate (W) between a surface and a fluid.
/// Positive when heat flows from surface to fluid.
pub fn convective_heat_transfer(h: Scalar, area: Scalar, t_surface: Scalar, t_fluid: Scalar) -> Scalar {
    h * area * (t_surface - t_fluid)
}

/// Thermal strain: ε = α · ΔT
///
/// Returns the linear thermal strain (dimensionless) for a material
/// with thermal expansion coefficient α (1/K) over temperature change ΔT.
pub fn thermal_strain(alpha: Scalar, delta_t: Scalar) -> Scalar {
    alpha * delta_t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_joule_heating_basic() {
        let p = joule_heating(10.0, 0.5);
        assert!((p - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_joule_heating_zero_current() {
        let p = joule_heating(0.0, 0.5);
        assert_eq!(p, 0.0);
    }

    #[test]
    fn test_joule_heating_zero_resistance() {
        let p = joule_heating(10.0, 0.0);
        assert_eq!(p, 0.0);
    }

    #[test]
    fn test_friction_heating_basic() {
        let p = friction_heating(100.0, 2.0);
        assert!((p - 200.0).abs() < 1e-10);
    }

    #[test]
    fn test_friction_heating_zero_velocity() {
        let p = friction_heating(100.0, 0.0);
        assert_eq!(p, 0.0);
    }

    #[test]
    fn test_friction_heating_negative_velocity() {
        let p = friction_heating(100.0, -2.0);
        assert!(p < 0.0);
    }

    #[test]
    fn test_convective_heat_transfer_positive() {
        let q = convective_heat_transfer(10.0, 2.0, 350.0, 300.0);
        assert!((q - 1000.0).abs() < 1e-10);
    }

    #[test]
    fn test_convective_heat_transfer_negative() {
        let q = convective_heat_transfer(10.0, 2.0, 300.0, 350.0);
        assert!((q + 1000.0).abs() < 1e-10);
    }

    #[test]
    fn test_convective_heat_transfer_equal_temps() {
        let q = convective_heat_transfer(10.0, 2.0, 300.0, 300.0);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_thermal_strain_basic() {
        let eps = thermal_strain(1.2e-5, 100.0);
        assert!((eps - 0.0012).abs() < 1e-10);
    }

    #[test]
    fn test_thermal_strain_zero_delta() {
        let eps = thermal_strain(1.2e-5, 0.0);
        assert_eq!(eps, 0.0);
    }

    #[test]
    fn test_thermal_strain_negative_delta() {
        let eps = thermal_strain(1.2e-5, -50.0);
        assert!(eps < 0.0);
    }
}
