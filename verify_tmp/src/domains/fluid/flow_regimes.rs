//! Flow regime classification and modelling.
//!
//! Provides functions for determining laminar / transitional / turbulent
//! flow regimes, turbulent viscosity models, friction factor correlations,
//! and multiphase flow properties.

use crate::core::types::Scalar;

/// Classification of internal flow regime based on Reynolds number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlowRegime {
    /// Re < 2300 — smooth, layered flow.
    Laminar,
    /// 2300 ≤ Re ≤ 4000 — uncertain regime between laminar and turbulent.
    Transitional,
    /// Re > 4000 — chaotic, eddying flow.
    Turbulent,
}

/// Determine the flow regime from the Reynolds number.
///
/// - Re < 2300 → `Laminar`
/// - 2300 ≤ Re ≤ 4000 → `Transitional`
/// - Re > 4000 → `Turbulent`
pub fn flow_regime(re: Scalar) -> FlowRegime {
    if re < 2300.0 {
        FlowRegime::Laminar
    } else if re <= 4000.0 {
        FlowRegime::Transitional
    } else {
        FlowRegime::Turbulent
    }
}

/// Mixing-length turbulent (eddy) viscosity using the Prandtl mixing-length model.
///
/// μ_t = ρ · ℓ² · |∂u/∂y|
///
/// where ℓ = κ · y is the mixing length, κ is the von Kármán constant (~0.41),
/// and y is the distance from the wall.
///
/// # Arguments
///
/// * `velocity_gradient` - Mean velocity gradient ∂u/∂y (1/s)
/// * `wall_distance`     - Distance from the wall y (m)
/// * `kappa`             - Von Kármán constant (typically 0.41)
///
/// Returns the turbulent (eddy) viscosity (Pa·s).
pub fn mixing_length_turbulent_viscosity(
    velocity_gradient: Scalar,
    wall_distance: Scalar,
    kappa: Scalar,
) -> Scalar {
    if wall_distance <= 0.0 || kappa <= 0.0 {
        return 0.0;
    }
    let mixing_length = kappa * wall_distance;
    mixing_length * mixing_length * velocity_gradient.abs()
}

/// Darcy friction factor using the Colebrook–White equation (turbulent) or
/// the laminar formula f = 64 / Re.
///
/// # Arguments
///
/// * `re`        - Reynolds number (dimensionless)
/// * `roughness` - Absolute wall roughness ε (m)
/// * `diameter`  - Pipe internal diameter D (m)
///
/// Returns the Darcy friction factor f (dimensionless).
pub fn darcy_friction_factor(re: Scalar, roughness: Scalar, diameter: Scalar) -> Scalar {
    if re <= 0.0 || diameter <= 0.0 {
        return 0.0;
    }
    if re < 2300.0 {
        // Hagen–Poiseuille
        64.0 / re
    } else {
        // Colebrook–White (implicit, solved via 10 iterations of Swamee–Jain explicit approx.)
        let rel_rough = roughness / diameter;
        let mut f: Scalar = 0.02; // initial guess
        for _ in 0..10 {
            let sqrt_f = f.sqrt();
            let lhs = -2.0 * (rel_rough / 3.7 + 2.51 / (re * sqrt_f)).log10();
            f = 1.0 / (lhs * lhs);
            if f.is_nan() || f.is_infinite() {
                f = 0.02;
                break;
            }
        }
        f
    }
}

/// Pressure drop in a straight pipe (Darcy–Weisbach equation).
///
/// ΔP = f · (L / D) · (ρ · U² / 2)
///
/// # Arguments
///
/// * `f`        - Darcy friction factor (dimensionless)
/// * `length`   - Pipe length L (m)
/// * `diameter` - Pipe internal diameter D (m)
/// * `density`  - Fluid density ρ (kg/m³)
/// * `velocity` - Mean flow velocity U (m/s)
///
/// Returns the pressure drop ΔP (Pa).
pub fn pipe_pressure_drop(
    f: Scalar,
    length: Scalar,
    diameter: Scalar,
    density: Scalar,
    velocity: Scalar,
) -> Scalar {
    if diameter <= 0.0 || length < 0.0 {
        return 0.0;
    }
    if f <= 0.0 || density <= 0.0 {
        return 0.0;
    }
    f * (length / diameter) * (0.5 * density * velocity * velocity)
}

/// Homogeneous (no-slip) density of a gas–liquid mixture.
///
/// ρ_h = α_g · ρ_g + (1 - α_g) · ρ_l
///
/// # Arguments
///
/// * `alpha_gas`  - Gas volume fraction α_g (0–1)
/// * `rho_gas`    - Gas density (kg/m³)
/// * `rho_liquid` - Liquid density (kg/m³)
pub fn homogeneous_density(alpha_gas: Scalar, rho_gas: Scalar, rho_liquid: Scalar) -> Scalar {
    if alpha_gas < 0.0 {
        return rho_liquid;
    }
    if alpha_gas > 1.0 {
        return rho_gas;
    }
    alpha_gas * rho_gas + (1.0 - alpha_gas) * rho_liquid
}

/// Terminal velocity of a single gas bubble rising in a stagnant liquid
/// (Hadamard–Rybczynski regime for small, clean bubbles).
///
/// U_t = (2 · (ρ_l - ρ_g) · g · d²) / (9 · μ_l) · (μ_l + μ_g) / (2·μ_l + 3·μ_g)
///
/// For an inviscid gas (μ_g → 0) this simplifies to:
/// U_t = (ρ_l - ρ_g) · g · d² / (12 · μ_l)
///
/// # Arguments
///
/// * `bubble_diameter` - Bubble diameter d (m)
/// * `rho_l`           - Liquid density (kg/m³)
/// * `rho_g`           - Gas density (kg/m³)
/// * `mu_l`            - Liquid dynamic viscosity (Pa·s)
///
/// Returns terminal velocity (m/s).
pub fn bubble_terminal_velocity(
    bubble_diameter: Scalar,
    rho_l: Scalar,
    rho_g: Scalar,
    mu_l: Scalar,
) -> Scalar {
    if bubble_diameter <= 0.0 || mu_l <= 0.0 {
        return 0.0;
    }
    let delta_rho = rho_l - rho_g;
    if delta_rho <= 0.0 {
        return 0.0;
    }
    let g = crate::domains::fluid::physics::G;
    // Simplified form for inviscid gas: U_t = Δρ · g · d² / (12 · μ_l)
    delta_rho * g * bubble_diameter * bubble_diameter / (12.0 * mu_l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_regime_laminar() {
        assert_eq!(flow_regime(1000.0), FlowRegime::Laminar);
    }

    #[test]
    fn test_flow_regime_transitional() {
        assert_eq!(flow_regime(3000.0), FlowRegime::Transitional);
    }

    #[test]
    fn test_flow_regime_turbulent() {
        assert_eq!(flow_regime(10000.0), FlowRegime::Turbulent);
    }

    #[test]
    fn test_flow_regime_boundary_low() {
        assert_eq!(flow_regime(2300.0), FlowRegime::Transitional);
    }

    #[test]
    fn test_mixing_length_viscosity() {
        let mu_t = mixing_length_turbulent_viscosity(100.0, 0.01, 0.41);
        // ℓ = 0.41 * 0.01 = 0.0041; μ_t = (0.0041)² * 100 = 1.681e-3
        let expected = Scalar::powi(0.41 * 0.01, 2) * 100.0;
        assert!((mu_t - expected).abs() < 1e-12);
    }

    #[test]
    fn test_mixing_length_viscosity_zero_distance() {
        let mu_t = mixing_length_turbulent_viscosity(100.0, 0.0, 0.41);
        assert_eq!(mu_t, 0.0);
    }

    #[test]
    fn test_darcy_friction_factor_laminar() {
        let f = darcy_friction_factor(1000.0, 0.0, 0.1);
        assert!((f - 0.064).abs() < 1e-10);
    }

    #[test]
    fn test_darcy_friction_factor_turbulent() {
        let f = darcy_friction_factor(100_000.0, 0.0001, 0.1);
        assert!(f > 0.0);
        assert!(f < 0.05);
    }

    #[test]
    fn test_darcy_friction_factor_invalid() {
        let f = darcy_friction_factor(0.0, 0.0, 0.1);
        assert_eq!(f, 0.0);
    }

    #[test]
    fn test_pipe_pressure_drop() {
        let dp = pipe_pressure_drop(0.02, 10.0, 0.1, 1000.0, 2.0);
        // ΔP = 0.02 * (10/0.1) * 0.5 * 1000 * 4 = 0.02 * 100 * 2000 = 4000 Pa
        assert!((dp - 4000.0).abs() < 1e-9);
    }

    #[test]
    fn test_pipe_pressure_drop_zero_length() {
        let dp = pipe_pressure_drop(0.02, 0.0, 0.1, 1000.0, 2.0);
        assert_eq!(dp, 0.0);
    }

    #[test]
    fn test_homogeneous_density() {
        let rho = homogeneous_density(0.3, 1.2, 1000.0);
        // ρ_h = 0.3*1.2 + 0.7*1000 = 0.36 + 700 = 700.36
        assert!((rho - 700.36).abs() < 1e-10);
    }

    #[test]
    fn test_homogeneous_density_pure_liquid() {
        let rho = homogeneous_density(0.0, 1.2, 1000.0);
        assert_eq!(rho, 1000.0);
    }

    #[test]
    fn test_homogeneous_density_pure_gas() {
        let rho = homogeneous_density(1.0, 1.2, 1000.0);
        assert_eq!(rho, 1.2);
    }

    #[test]
    fn test_homogeneous_density_out_of_range() {
        let rho = homogeneous_density(-0.1, 1.2, 1000.0);
        assert_eq!(rho, 1000.0);
    }

    #[test]
    fn test_bubble_terminal_velocity() {
        // d = 1 mm = 0.001 m, water-air at STP
        let ut = bubble_terminal_velocity(0.001, 1000.0, 1.2, 1.002e-3);
        // U_t ≈ (1000-1.2)*9.81*(1e-3)² / (12*1.002e-3) ≈ 0.815 m/s
        assert!(ut > 0.5);
        assert!(ut < 1.2);
    }

    #[test]
    fn test_bubble_terminal_velocity_invalid() {
        let ut = bubble_terminal_velocity(0.0, 1000.0, 1.2, 1.002e-3);
        assert_eq!(ut, 0.0);
    }
}
