//! Aircraft aerodynamics: thin airfoil theory, finite-wing corrections,
//! shock wave relations, and the `AircraftAerodynamics` struct.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// Thin Airfoil Theory
// ──────────────────────────────────────────────

/// Lift coefficient from thin airfoil theory.
///
/// Cₗ = 2π · α  (valid for small angles, |α| < ~15°)
pub fn thin_airfoil_cl(alpha_rad: Scalar) -> Scalar {
    2.0 * core::f64::consts::PI * alpha_rad
}

/// Drag coefficient using the drag polar model.
///
/// C_d = C_d₀ + Cₗ² / (π · AR · e)
pub fn airfoil_cd(cl: Scalar, cd0: Scalar, ar: Scalar, e: Scalar) -> Scalar {
    if ar <= 0.0 || e <= 0.0 {
        return cd0;
    }
    cd0 + cl * cl / (core::f64::consts::PI * ar * e)
}

/// Pitching moment coefficient about the quarter-chord.
///
/// C_m = –π/2 · (α – camber)  (simplified)
pub fn airfoil_cm(alpha_rad: Scalar, camber: Scalar) -> Scalar {
    -0.5 * core::f64::consts::PI * (alpha_rad - camber)
}

// ──────────────────────────────────────────────
// Aircraft Aerodynamics
// ──────────────────────────────────────────────

/// Aerodynamic configuration of an aircraft.
///
/// Encapsulates wing geometry and zero-lift drag parameters needed to
/// compute lift, drag, and stall characteristics.
pub struct AircraftAerodynamics {
    /// Wing reference area (m²).
    pub wing_area: Scalar,
    /// Wing aspect ratio (b² / S).
    pub aspect_ratio: Scalar,
    /// Zero-lift drag coefficient.
    pub cd0: Scalar,
    /// Oswald efficiency factor (0 < e ≤ 1).
    pub oswald: Scalar,
    /// Lift curve slope (1/rad).
    pub cl_alpha: Scalar,
    /// Stall angle of attack (rad).
    pub alpha_stall: Scalar,
}

impl AircraftAerodynamics {
    /// Compute lift coefficient at a given angle of attack (rad).
    ///
    /// Below stall: linear lift-curve slope.
    /// At or beyond stall: Cₗ = Cₗ_max · sign(α), with a reduced post-stall slope.
    pub fn cl(&self, alpha: Scalar) -> Scalar {
        let alpha_abs = alpha.abs();
        let sign = if alpha >= 0.0 { 1.0 } else { -1.0 };

        if alpha_abs <= self.alpha_stall {
            self.cl_alpha * alpha
        } else {
            let cl_max = self.cl_alpha * self.alpha_stall;
            let post_slope = 0.1 * self.cl_alpha;
            let cl_post = cl_max + post_slope * (alpha_abs - self.alpha_stall);
            sign * cl_post
        }
    }

    /// Compute drag coefficient at a given angle of attack (rad).
    ///
    /// Uses the drag polar: C_d = C_d₀ + Cₗ² / (π · AR · e)
    pub fn cd(&self, alpha: Scalar) -> Scalar {
        let cl_val = self.cl(alpha);
        self.cd0 + cl_val * cl_val / (core::f64::consts::PI * self.aspect_ratio * self.oswald)
    }

    /// Compute lift-to-drag ratio at a given angle of attack (rad).
    pub fn lift_to_drag(&self, alpha: Scalar) -> Scalar {
        let cl_val = self.cl(alpha);
        let cd_val = self.cd(alpha);
        if cd_val.abs() < 1e-30 {
            return 0.0;
        }
        cl_val / cd_val
    }

    /// Compute stall speed (m/s) for a given weight (N) and air density (kg/m³).
    ///
    /// V_stall = √(2 · W / (ρ · S · Cₗ_max))
    pub fn stall_speed(&self, weight: Scalar, density: Scalar) -> Scalar {
        if density <= 0.0 || self.wing_area <= 0.0 {
            return Scalar::INFINITY;
        }
        let cl_max = self.cl_alpha * self.alpha_stall;
        if cl_max <= 0.0 {
            return Scalar::INFINITY;
        }
        (2.0 * weight / (density * self.wing_area * cl_max)).sqrt()
    }
}

// ──────────────────────────────────────────────
// Shock Wave Relations
// ──────────────────────────────────────────────

/// Oblique shock wave angle β (rad) for a given Mach number and flow
/// deflection angle θ (rad), using the θ-β-M relation.
///
/// Returns `None` if the shock is detached (no real solution).
pub fn oblique_shock_angle(mach: Scalar, deflection_angle: Scalar, gamma: Scalar) -> Option<Scalar> {
    if mach <= 1.0 || deflection_angle <= 0.0 {
        return None;
    }

    let m2 = mach * mach;
    let g = gamma;
    let theta = deflection_angle;

    // Initial guess: Mach angle + half the deflection
    let mut beta = (1.0 / mach).asin() + 0.5 * theta;

    for _ in 0..100 {
        let sb = beta.sin();
        let cb = beta.cos();
        let sb2 = sb * sb;
        let tan_b = sb / cb;

        let numerator = 2.0 * (m2 * sb2 - 1.0) / tan_b;
        let denominator = m2 * (g + 2.0 * cb * cb - 1.0) + 2.0;
        let f = numerator / denominator - theta;

        if f.abs() < 1e-12 {
            return Some(beta);
        }

        // Finite-difference derivative
        let eps = 1e-8;
        let beta2 = beta + eps;
        let sb2_e = beta2.sin();
        let cb2_e = beta2.cos();
        let tan_b2 = sb2_e / cb2_e;
        let sb2_2 = sb2_e * sb2_e;
        let numerator2 = 2.0 * (m2 * sb2_2 - 1.0) / tan_b2;
        let denominator2 = m2 * (g + 2.0 * cb2_e * cb2_e - 1.0) + 2.0;
        let f2 = numerator2 / denominator2 - theta;
        let df = (f2 - f) / eps;

        if df.abs() < 1e-30 {
            break;
        }

        beta -= f / df;

        // Clamp to physical range [μ, π/2]
        let mu = (1.0 / mach).asin();
        if beta < mu {
            beta = mu;
        }
        if beta > core::f64::consts::FRAC_PI_2 {
            beta = core::f64::consts::FRAC_PI_2;
        }
    }

    // Verify solution
    let sb = beta.sin();
    let cb = beta.cos();
    let tan_b = sb / cb;
    let sb2 = sb * sb;
    let num_val = 2.0 * (m2 * sb2 - 1.0) / tan_b;
    let denom_val = m2 * (g + 2.0 * cb * cb - 1.0) + 2.0;
    let residual = (num_val / denom_val - theta).abs();

    if residual < 1e-6 {
        Some(beta)
    } else {
        None
    }
}

/// Normal shock wave pressure ratio.
///
/// p₂/p₁ = 1 + (2γ/(γ+1)) · (M² – 1)
pub fn normal_shock_pressure_ratio(mach: Scalar, gamma: Scalar) -> Scalar {
    if mach < 1.0 {
        return 1.0;
    }
    1.0 + (2.0 * gamma / (gamma + 1.0)) * (mach * mach - 1.0)
}

/// Prandtl-Meyer expansion angle (rad).
///
/// ν(M) = √((γ+1)/(γ-1)) · arctan(√((γ-1)/(γ+1) · (M² – 1)))
///        – arctan(√(M² – 1))
pub fn prandtl_meyer_angle(mach: Scalar, gamma: Scalar) -> Scalar {
    if mach < 1.0 {
        return 0.0;
    }
    let m2 = mach * mach;
    let gp1 = gamma + 1.0;
    let gm1 = gamma - 1.0;
    let sqrt_term = ((gm1 / gp1) * (m2 - 1.0)).sqrt();
    (gp1 / gm1).sqrt() * sqrt_term.atan() - (m2 - 1.0).sqrt().atan()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thin_airfoil_cl_zero() {
        let cl = thin_airfoil_cl(0.0);
        assert!((cl).abs() < 1e-15);
    }

    #[test]
    fn test_thin_airfoil_cl_small_angle() {
        let cl = thin_airfoil_cl(0.1);
        let expected = 2.0 * core::f64::consts::PI * 0.1;
        assert!((cl - expected).abs() < 1e-12);
    }

    #[test]
    fn test_airfoil_cd_parasitic_only() {
        let cd = airfoil_cd(0.0, 0.02, 8.0, 0.85);
        assert!((cd - 0.02).abs() < 1e-12);
    }

    #[test]
    fn test_airfoil_cd_with_lift() {
        let cd = airfoil_cd(0.5, 0.02, 8.0, 0.85);
        let induced = 0.5 * 0.5 / (core::f64::consts::PI * 8.0 * 0.85);
        assert!((cd - (0.02 + induced)).abs() < 1e-12);
    }

    #[test]
    fn test_airfoil_cm_symmetric() {
        let cm = airfoil_cm(0.05, 0.0);
        assert!(cm < 0.0);
    }

    #[test]
    fn test_aircraft_aerodynamics_cl_linear() {
        let aero = AircraftAerodynamics {
            wing_area: 50.0,
            aspect_ratio: 8.0,
            cd0: 0.02,
            oswald: 0.85,
            cl_alpha: 2.0 * core::f64::consts::PI,
            alpha_stall: 0.25,
        };
        let cl = aero.cl(0.1);
        let expected = 2.0 * core::f64::consts::PI * 0.1;
        assert!((cl - expected).abs() < 1e-10);
    }

    #[test]
    fn test_aircraft_aerodynamics_cl_stall() {
        let aero = AircraftAerodynamics {
            wing_area: 50.0,
            aspect_ratio: 8.0,
            cd0: 0.02,
            oswald: 0.85,
            cl_alpha: 2.0 * core::f64::consts::PI,
            alpha_stall: 0.25,
        };
        let cl_stall = aero.cl(0.25);
        let cl_post = aero.cl(0.35);
        assert!(cl_post > cl_stall * 0.5);
    }

    #[test]
    fn test_lift_to_drag_positive() {
        let aero = AircraftAerodynamics {
            wing_area: 50.0,
            aspect_ratio: 8.0,
            cd0: 0.02,
            oswald: 0.85,
            cl_alpha: 2.0 * core::f64::consts::PI,
            alpha_stall: 0.25,
        };
        let ld = aero.lift_to_drag(0.05);
        assert!(ld > 0.0);
    }

    #[test]
    fn test_stall_speed_positive() {
        let aero = AircraftAerodynamics {
            wing_area: 50.0,
            aspect_ratio: 8.0,
            cd0: 0.02,
            oswald: 0.85,
            cl_alpha: 2.0 * core::f64::consts::PI,
            alpha_stall: 0.25,
        };
        let vs = aero.stall_speed(500_000.0, 1.225);
        assert!(vs > 0.0);
        assert!(vs < 200.0);
    }

    #[test]
    fn test_normal_shock_pressure_ratio() {
        let pr = normal_shock_pressure_ratio(2.0, 1.4);
        assert!((pr - 4.5).abs() < 1e-10);
    }

    #[test]
    fn test_normal_shock_subsonic() {
        let pr = normal_shock_pressure_ratio(0.8, 1.4);
        assert!((pr - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_prandtl_meyer_angle_at_m1() {
        let nu = prandtl_meyer_angle(1.0, 1.4);
        assert!((nu).abs() < 1e-10);
    }

    #[test]
    fn test_prandtl_meyer_angle_supersonic() {
        let nu = prandtl_meyer_angle(2.0, 1.4);
        assert!(nu > 0.0);
    }

    #[test]
    fn test_oblique_shock_attached() {
        let beta = oblique_shock_angle(2.0, 0.1745, 1.4);
        assert!(beta.is_some());
        if let Some(b) = beta {
            assert!(b > 0.1745);
            assert!(b < core::f64::consts::FRAC_PI_2);
        }
    }

    #[test]
    fn test_oblique_shock_large_deflection() {
        let beta = oblique_shock_angle(1.5, 0.8, 1.4);
        assert!(beta.is_none() || beta.unwrap() > 0.5);
    }
}
