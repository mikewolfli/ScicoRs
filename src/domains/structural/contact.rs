//! Contact mechanics: point-to-point distance, Hertzian contact,
//! Coulomb friction, bolt preload, and constraint modelling.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Euclidean distance between two 3D points.
pub fn point_to_point_distance(p1: &Coord3D, p2: &Coord3D) -> Scalar {
    p1.distance(p2)
}

/// Hertzian contact stress for two spheres in contact.
///
/// # Parameters
/// - `f` — normal contact force (N).
/// - `r1`, `r2` — radii of the two spheres (m).
/// - `e1`, `e2` — Young's moduli (Pa).
/// - `nu1`, `nu2` — Poisson's ratios.
///
/// Returns the maximum contact pressure (Pa).
pub fn hertz_contact_stress(
    f: Scalar,
    r1: Scalar,
    r2: Scalar,
    e1: Scalar,
    e2: Scalar,
    nu1: Scalar,
    nu2: Scalar,
) -> Scalar {
    if f <= 0.0 || r1 <= 0.0 || r2 <= 0.0 {
        return 0.0;
    }

    // Equivalent radius: 1/R = 1/r1 + 1/r2
    let r_eq = 1.0 / (1.0 / r1 + 1.0 / r2);

    // Equivalent modulus: 1/E* = (1-ν₁²)/E₁ + (1-ν₂²)/E₂
    let e_star_inv = (1.0 - nu1 * nu1) / e1 + (1.0 - nu2 * nu2) / e2;
    if e_star_inv <= 0.0 {
        return 0.0;
    }
    let e_star = 1.0 / e_star_inv;

    // Contact radius: a = (3·F·R / (4·E*))^(1/3)
    let a = (3.0 * f * r_eq / (4.0 * e_star)).cbrt();

    // Maximum pressure: p₀ = 3·F / (2·π·a²)
    3.0 * f / (2.0 * std::f64::consts::PI * a * a)
}

/// Coulomb friction force.
///
/// Returns the maximum friction force (N) for a given normal force
/// and friction coefficient.
pub fn coulomb_friction(normal_force: Scalar, mu: Scalar) -> Scalar {
    if normal_force <= 0.0 || mu <= 0.0 {
        return 0.0;
    }
    mu * normal_force
}

/// Bolt preload from applied torque.
///
/// Standard relation: F = T / (k·d)
/// where `T` is torque (N·m), `d` is bolt diameter (m), and `k` is the
/// nut factor (typically 0.20 for lubricated steel bolts).
pub fn bolt_preload(torque: Scalar, diameter: Scalar, k_factor: Scalar) -> Scalar {
    if diameter <= 0.0 || k_factor <= 0.0 {
        return 0.0;
    }
    torque / (k_factor * diameter)
}

/// Check whether a point is within a spherical clearance zone of another.
pub fn is_in_contact(p1: &Coord3D, p2: &Coord3D, clearance: Scalar) -> bool {
    point_to_point_distance(p1, p2) <= clearance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_to_point_distance() {
        let p1 = Coord3D::new(0.0, 0.0, 0.0);
        let p2 = Coord3D::new(3.0, 4.0, 0.0);
        let d = point_to_point_distance(&p1, &p2);
        assert!((d - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_to_point_distance_3d() {
        let p1 = Coord3D::new(1.0, 2.0, 3.0);
        let p2 = Coord3D::new(4.0, 6.0, 3.0);
        let d = point_to_point_distance(&p1, &p2);
        assert!((d - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_hertz_contact_stress_positive() {
        let p = hertz_contact_stress(100.0, 0.01, 0.01, 200.0e9, 200.0e9, 0.3, 0.3);
        assert!(p > 0.0);
        assert!(p < 1e10); // sanity check
    }

    #[test]
    fn test_hertz_contact_stress_zero_force() {
        let p = hertz_contact_stress(0.0, 0.01, 0.01, 200.0e9, 200.0e9, 0.3, 0.3);
        assert!((p - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_coulomb_friction() {
        let f = coulomb_friction(100.0, 0.3);
        assert!((f - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_coulomb_friction_zero_normal() {
        let f = coulomb_friction(0.0, 0.3);
        assert!((f - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_bolt_preload() {
        // T = 100 N·m, d = 10 mm = 0.01 m, k = 0.2
        let f = bolt_preload(100.0, 0.01, 0.2);
        assert!((f - 50000.0).abs() < 1.0);
    }

    #[test]
    fn test_bolt_preload_zero_diameter() {
        let f = bolt_preload(100.0, 0.0, 0.2);
        assert!((f - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_is_in_contact() {
        let p1 = Coord3D::new(0.0, 0.0, 0.0);
        let p2 = Coord3D::new(0.005, 0.0, 0.0);
        assert!(is_in_contact(&p1, &p2, 0.01));
        assert!(!is_in_contact(&p1, &p2, 0.001));
    }
}
