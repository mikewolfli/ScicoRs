//! Convective heat transfer models for natural and forced convection.
//!
//! Provides Nusselt number correlations for natural convection (vertical
//! plates), forced convection (laminar and turbulent internal flows),
//! Grashof number, convection coefficient, and nucleate boiling.

use crate::core::types::Scalar;

/// Nusselt number correlation for natural convection on a vertical plate.
///
/// For laminar flow (Ra < 10⁹): Nu = 0.59 · Ra^(1/4)
/// For turbulent flow (Ra ≥ 10⁹): Nu = 0.10 · Ra^(1/3)
/// where Ra = Gr · Pr.
pub fn natural_convection_nu(gr: Scalar, pr: Scalar, laminar: bool) -> Scalar {
    let ra = gr * pr;
    if ra <= 0.0 {
        return 0.0;
    }
    if laminar {
        0.59 * ra.powf(0.25)
    } else {
        0.10 * ra.powf(1.0 / 3.0)
    }
}

/// Nusselt number correlation for turbulent forced convection in pipes.
///
/// Dittus-Boelter correlation:
///   Nu = 0.023 · Re^(4/5) · Pr^n
/// where n = 0.4 for heating (fluid being heated), n = 0.3 for cooling.
pub fn forced_convection_nu_turbulent(re: Scalar, pr: Scalar, heating: bool) -> Scalar {
    if re <= 0.0 || pr <= 0.0 {
        return 0.0;
    }
    let n = if heating { 0.4 } else { 0.3 };
    0.023 * re.powf(0.8) * pr.powf(n)
}

/// Nusselt number correlation for laminar forced convection in pipes.
///
/// Nu = 3.66 for constant wall temperature (fully developed),
/// with a correction for entrance effects:
///   Nu = 3.66 + 0.065 · (Re · Pr · d/L) / (1 + 0.04 · (Re · Pr · d/L)^(2/3))
pub fn forced_convection_nu_laminar(re: Scalar, pr: Scalar, d_l_ratio: Scalar) -> Scalar {
    if re <= 0.0 || pr <= 0.0 || d_l_ratio <= 0.0 {
        return 3.66;
    }
    let gz = re * pr * d_l_ratio; // Graetz number approximation
    3.66 + 0.065 * gz / (1.0 + 0.04 * gz.powf(2.0 / 3.0))
}

/// Grashof number: Gr = g · β · ΔT · L³ / ν²
///
/// Where g is gravitational acceleration, β is thermal expansion coefficient,
/// ΔT is temperature difference, L is characteristic length, ν is kinematic
/// viscosity.
pub fn grashof_number(g: Scalar, beta: Scalar, delta_t: Scalar, l: Scalar, nu: Scalar) -> Scalar {
    if nu <= 0.0 || l <= 0.0 {
        return 0.0;
    }
    g * beta * delta_t * l.powi(3) / (nu * nu)
}

/// Convective heat transfer coefficient: h = Nu · k / L
///
/// Where Nu is the Nusselt number, k is the thermal conductivity of the
/// fluid (W/(m·K)), and L is the characteristic length (m).
pub fn convection_coefficient(nu: Scalar, k: Scalar, l: Scalar) -> Scalar {
    if k <= 0.0 || l <= 0.0 {
        return 0.0;
    }
    nu * k / l
}

/// Nucleate boiling heat transfer coefficient (W/(m²·K)).
///
/// Simplified correlation based on the Rohsenow or Mostinski approach.
/// Supports common fluids: "water", "r134a", "r22".
pub fn nucleate_boiling_h(delta_t_sat: Scalar, fluid: &str) -> Scalar {
    if delta_t_sat <= 0.0 {
        return 0.0;
    }
    match fluid {
        "water" => {
            // Simplified: ~5.56 * ΔT_sat^3 for water
            5.56 * delta_t_sat.powi(3)
        }
        "r134a" => {
            2.5 * delta_t_sat.powi(2)
        }
        "r22" => {
            3.0 * delta_t_sat.powi(2)
        }
        _ => {
            // Generic correlation
            1.5 * delta_t_sat.powi(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_convection_nu_laminar() {
        // Typical values: Gr=1e7, Pr=0.7 (air)
        let nu = natural_convection_nu(1e7, 0.7, true);
        let ra: Scalar = 1e7 * 0.7;
        let expected = 0.59 * ra.powf(0.25);
        assert!((nu - expected).abs() < 1e-10);
    }

    #[test]
    fn test_natural_convection_nu_turbulent() {
        let nu = natural_convection_nu(1e10, 0.7, false);
        let ra: Scalar = 1e10 * 0.7;
        let expected = 0.10 * ra.powf(1.0 / 3.0);
        assert!((nu - expected).abs() < 1e-6);
    }

    #[test]
    fn test_natural_convection_nu_zero_ra() {
        let nu = natural_convection_nu(0.0, 0.7, true);
        assert_eq!(nu, 0.0);
    }

    #[test]
    fn test_forced_convection_nu_turbulent() {
        // Re=1e5, Pr=0.7 (air), heating
        let nu = forced_convection_nu_turbulent(1e5, 0.7, true);
        let expected = 0.023 * 1e5_f64.powf(0.8) * 0.7_f64.powf(0.4);
        assert!((nu - expected).abs() < 1e-10);
    }

    #[test]
    fn test_forced_convection_nu_turbulent_cooling() {
        let nu = forced_convection_nu_turbulent(1e5, 0.7, false);
        let expected = 0.023 * 1e5_f64.powf(0.8) * 0.7_f64.powf(0.3);
        assert!((nu - expected).abs() < 1e-10);
    }

    #[test]
    fn test_forced_convection_nu_laminar() {
        let nu = forced_convection_nu_laminar(2000.0, 0.7, 0.1);
        assert!(nu >= 3.66);
    }

    #[test]
    fn test_forced_convection_nu_laminar_default() {
        let nu = forced_convection_nu_laminar(0.0, 0.0, 0.0);
        assert!((nu - 3.66).abs() < 1e-10);
    }

    #[test]
    fn test_grashof_number() {
        // Air: g=9.81, β=1/300, ΔT=10, L=0.1, ν=1.5e-5
        let gr = grashof_number(9.81, 1.0 / 300.0, 10.0, 0.1, 1.5e-5);
        let expected = 9.81 * (1.0 / 300.0) * 10.0 * 0.001 / (2.25e-10);
        assert!((gr - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_grashof_number_zero_nu() {
        let gr = grashof_number(9.81, 0.003, 10.0, 0.1, 0.0);
        assert_eq!(gr, 0.0);
    }

    #[test]
    fn test_convection_coefficient() {
        // Air: Nu=50, k=0.026, L=0.1
        let h = convection_coefficient(50.0, 0.026, 0.1);
        assert!((h - 13.0).abs() < 1e-10);
    }

    #[test]
    fn test_convection_coefficient_zero() {
        let h = convection_coefficient(50.0, 0.0, 0.1);
        assert_eq!(h, 0.0);
    }

    #[test]
    fn test_nucleate_boiling_water() {
        let h = nucleate_boiling_h(10.0, "water");
        assert!((h - 5560.0).abs() < 1.0);
    }

    #[test]
    fn test_nucleate_boiling_unknown_fluid() {
        let h = nucleate_boiling_h(10.0, "unknown");
        assert!((h - 150.0).abs() < 1.0);
    }

    #[test]
    fn test_nucleate_boiling_zero_delta() {
        let h = nucleate_boiling_h(0.0, "water");
        assert_eq!(h, 0.0);
    }
}
