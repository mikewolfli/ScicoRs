//! Aerospace mission analysis: L/D ratio, Breguet range equation,
//! rate of climb, and wing loading.

use crate::core::types::Scalar;

/// Lift-to-drag ratio.
///
/// L/D = lift / drag
pub fn lift_to_drag_ratio(lift: Scalar, drag: Scalar) -> Scalar {
    if drag.abs() < 1e-30 {
        if lift.abs() < 1e-30 {
            return 1.0; // neutral
        }
        return Scalar::INFINITY;
    }
    lift / drag
}

/// Breguet range equation for jet aircraft (m).
///
/// R = (V / (SFC · g₀)) · (L/D) · ln(W_initial / W_final)
///
/// * `velocity` — cruise velocity (m/s)
/// * `ld_ratio` — lift-to-drag ratio (dimensionless)
/// * `sfc` — thrust-specific fuel consumption (kg/(N·s))
/// * `w_initial` — initial weight (N)
/// * `w_final` — final weight (N)
pub fn breguet_range(
    velocity: Scalar,
    ld_ratio: Scalar,
    sfc: Scalar,
    w_initial: Scalar,
    w_final: Scalar,
) -> Scalar {
    if sfc <= 0.0 || w_final <= 0.0 || w_initial <= w_final {
        return 0.0;
    }
    (velocity / (sfc * 9.80665)) * ld_ratio * (w_initial / w_final).ln()
}

/// Rate of climb (m/s).
///
/// ROC = (T – D) · V / W
///
/// * `thrust` — net thrust (N)
/// * `drag` — total drag (N)
/// * `velocity` — flight velocity (m/s)
/// * `weight` — aircraft weight (N)
pub fn rate_of_climb(thrust: Scalar, drag: Scalar, velocity: Scalar, weight: Scalar) -> Scalar {
    if weight <= 0.0 {
        return 0.0;
    }
    (thrust - drag) * velocity / weight
}

/// Wing loading (N/m² or Pa).
///
/// W/S = weight / wing_area
pub fn wing_loading(weight: Scalar, wing_area: Scalar) -> Scalar {
    if wing_area <= 0.0 {
        return Scalar::INFINITY;
    }
    weight / wing_area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lift_to_drag_ratio_typical() {
        let ld = lift_to_drag_ratio(50_000.0, 5_000.0);
        assert!((ld - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_lift_to_drag_ratio_zero_drag() {
        let ld = lift_to_drag_ratio(1000.0, 0.0);
        assert!(ld.is_infinite());
    }

    #[test]
    fn test_lift_to_drag_ratio_both_zero() {
        let ld = lift_to_drag_ratio(0.0, 0.0);
        assert!((ld - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_breguet_range_typical() {
        // Boeing 737-ish: V=230 m/s, L/D=18, SFC=1.5e-5, W0=700kN, W1=500kN
        let range = breguet_range(230.0, 18.0, 1.5e-5, 700_000.0, 500_000.0);
        // Should be ~5000-6000 km
        assert!(range > 4_000_000.0);
        assert!(range < 12_000_000.0);
    }

    #[test]
    fn test_breguet_range_zero_sfc() {
        let range = breguet_range(230.0, 18.0, 0.0, 700_000.0, 500_000.0);
        assert!((range).abs() < 1e-10);
    }

    #[test]
    fn test_breguet_range_equal_weights() {
        let range = breguet_range(230.0, 18.0, 1.5e-5, 700_000.0, 700_000.0);
        assert!((range).abs() < 1e-10);
    }

    #[test]
    fn test_rate_of_climb_positive() {
        let roc = rate_of_climb(100_000.0, 60_000.0, 100.0, 500_000.0);
        // (100000 - 60000) * 100 / 500000 = 8 m/s
        assert!((roc - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_rate_of_climb_zero_weight() {
        let roc = rate_of_climb(100_000.0, 60_000.0, 100.0, 0.0);
        assert!((roc).abs() < 1e-10);
    }

    #[test]
    fn test_rate_of_climb_negative() {
        let roc = rate_of_climb(50_000.0, 80_000.0, 100.0, 500_000.0);
        assert!(roc < 0.0);
    }

    #[test]
    fn test_wing_loading_typical() {
        let wl = wing_loading(700_000.0, 125.0);
        assert!((wl - 5600.0).abs() < 0.1);
    }

    #[test]
    fn test_wing_loading_zero_area() {
        let wl = wing_loading(100_000.0, 0.0);
        assert!(wl.is_infinite());
    }
}
