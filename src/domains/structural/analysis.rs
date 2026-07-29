//! Structural analysis: safety factors, buckling loads, beam deflection.

use crate::core::types::Scalar;

/// Safety factor: SF = ultimate_stress / allowable_stress.
///
/// Values SF > 1.0 indicate the design is safe under the allowable stress.
/// SF < 1.0 indicates overstress / potential failure.
pub fn safety_factor(ultimate_stress: Scalar, allowable_stress: Scalar) -> Scalar {
    if allowable_stress <= 0.0 {
        if ultimate_stress <= 0.0 {
            return 1.0;
        }
        return Scalar::INFINITY;
    }
    ultimate_stress / allowable_stress
}

/// Euler buckling load for a slender column.
///
/// P_cr = π²·E·I / (K·L)²
///
/// # Parameters
/// - `e` — Young's modulus (Pa).
/// - `i` — area moment of inertia (m⁴).
/// - `l` — column length (m).
/// - `k` — effective length factor (0.5 for fixed-fixed, 1.0 for pinned-pinned,
///   2.0 for fixed-free, 0.7 for fixed-pinned).
pub fn euler_buckling_load(e: Scalar, i: Scalar, l: Scalar, k: Scalar) -> Scalar {
    if e <= 0.0 || i <= 0.0 || l <= 0.0 || k <= 0.0 {
        return 0.0;
    }
    let kl = k * l;
    std::f64::consts::PI * std::f64::consts::PI * e * i / (kl * kl)
}

/// Static deflection of a simply supported beam with a point load.
///
/// For a beam of length `l` with a point load `f` at position `x` along
/// the beam (measured from the left support):
///
/// If x ≤ a (where a is the load position):
///     v(x) = (F·b·x) / (6·E·I·L) · (L² - x² - b²)
/// where a = L/2 (centre load), or custom position.
///
/// Simplified for central point load: a = b = L/2:
///     v(x) = (F·x) / (48·E·I) · (3·L² - 4·x²)   for x ≤ L/2
///
/// # Parameters
/// - `f` — point load (N).
/// - `l` — beam length (m).
/// - `e` — Young's modulus (Pa).
/// - `i` — area moment of inertia (m⁴).
/// - `x` — position along beam where deflection is evaluated (m).
pub fn beam_deflection_simple(f: Scalar, l: Scalar, e: Scalar, i: Scalar, x: Scalar) -> Scalar {
    if e <= 0.0 || i <= 0.0 || l <= 0.0 {
        return 0.0;
    }
    let x = x.clamp(0.0, l);

    // Central point load case
    let half = l / 2.0;
    if x <= half {
        // v(x) = F·x·(3·L² - 4·x²) / (48·E·I)
        f * x * (3.0 * l * l - 4.0 * x * x) / (48.0 * e * i)
    } else {
        // Symmetric
        let x2 = l - x;
        f * x2 * (3.0 * l * l - 4.0 * x2 * x2) / (48.0 * e * i)
    }
}

/// Axial stress: σ = F / A.
pub fn axial_stress(force: Scalar, area: Scalar) -> Scalar {
    if area <= 0.0 {
        return 0.0;
    }
    force / area
}

/// Bending stress: σ = M·y / I.
pub fn bending_stress(moment: Scalar, distance_from_neutral_axis: Scalar, moment_of_inertia: Scalar) -> Scalar {
    if moment_of_inertia <= 0.0 {
        return 0.0;
    }
    moment * distance_from_neutral_axis / moment_of_inertia
}

/// Maximum mid-span deflection of a simply supported beam with uniformly
/// distributed load: v_max = 5·w·L⁴ / (384·E·I).
pub fn beam_deflection_udl(w: Scalar, l: Scalar, e: Scalar, i: Scalar) -> Scalar {
    if e <= 0.0 || i <= 0.0 || l <= 0.0 {
        return 0.0;
    }
    5.0 * w * l.powi(4) / (384.0 * e * i)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::doc_lazy_continuation)]
    use super::*;

    #[test]
    fn test_safety_factor_safe() {
        let sf = safety_factor(500.0e6, 250.0e6);
        assert!((sf - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_safety_factor_unsafe() {
        let sf = safety_factor(200.0e6, 300.0e6);
        assert!((sf - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_safety_factor_zero_allowable() {
        let sf = safety_factor(100.0, 0.0);
        assert!(sf.is_infinite());
    }

    #[test]
    fn test_euler_buckling_pinned() {
        // Steel column: E=200 GPa, I=1e-6 m⁴, L=3 m, K=1.0 (pinned-pinned)
        let p = euler_buckling_load(200.0e9, 1e-6, 3.0, 1.0);
        let expected = std::f64::consts::PI * std::f64::consts::PI * 200.0e9 * 1e-6 / 9.0;
        assert!((p - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_euler_buckling_fixed() {
        // Same column, fixed-fixed: K=0.5
        let p = euler_buckling_load(200.0e9, 1e-6, 3.0, 0.5);
        let expected = std::f64::consts::PI * std::f64::consts::PI * 200.0e9 * 1e-6 / (1.5 * 1.5);
        assert!((p - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_euler_buckling_zero_parameters() {
        assert!((euler_buckling_load(0.0, 1e-6, 3.0, 1.0) - 0.0).abs() < 1e-10);
        assert!((euler_buckling_load(200.0e9, 0.0, 3.0, 1.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_beam_deflection_centre() {
        // Steel beam: F=1000 N, L=2 m, E=200 GPa, I=1e-6 m⁴
        // Max deflection at centre (x=L/2=1.0):
        // v_max = F·L³ / (48·E·I) = 1000 * 8 / (48 * 200e9 * 1e-6)
        let v = beam_deflection_simple(1000.0, 2.0, 200.0e9, 1e-6, 1.0);
        let expected = 1000.0 * 8.0 / (48.0 * 200.0e9 * 1e-6);
        assert!((v - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_beam_deflection_at_support() {
        let v = beam_deflection_simple(1000.0, 2.0, 200.0e9, 1e-6, 0.0);
        assert!((v - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_beam_deflection_symmetric() {
        let v1 = beam_deflection_simple(1000.0, 2.0, 200.0e9, 1e-6, 0.5);
        let v2 = beam_deflection_simple(1000.0, 2.0, 200.0e9, 1e-6, 1.5);
        assert!((v1 - v2).abs() < 1e-10);
    }

    #[test]
    fn test_axial_stress() {
        let s = axial_stress(5000.0, 0.01);
        assert!((s - 500000.0).abs() < 1.0);
    }

    #[test]
    fn test_bending_stress() {
        let s = bending_stress(1000.0, 0.05, 1e-5);
        assert!((s - 5.0e6).abs() < 1.0);
    }

    #[test]
    fn test_beam_deflection_udl() {
        // Steel beam: w=1000 N/m, L=2 m, E=200 GPa, I=1e-6 m⁴
        // v_max = 5*1000*16/(384*200e9*1e-6)
        let v = beam_deflection_udl(1000.0, 2.0, 200.0e9, 1e-6);
        let expected = 5.0 * 1000.0 * 16.0 / (384.0 * 200.0e9 * 1e-6);
        assert!((v - expected).abs() / expected < 1e-10);
    }
}
