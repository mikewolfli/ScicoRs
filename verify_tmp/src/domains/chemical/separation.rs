//! Separation process models for chemical engineering.
//!
//! Provides models for distillation (Fenske equation, minimum reflux),
//! absorption, liquid-liquid extraction, and flash distillation
//! (Rachford-Rice equation).

use crate::core::types::Scalar;

/// Fenske equation for minimum number of theoretical stages in distillation.
///
/// N_min = ln[(x_D/(1-x_D))·((1-x_B)/x_B)] / ln(α)
pub fn fenske_equation(alpha: Scalar, x_d: Scalar, x_b: Scalar) -> Scalar {
    if alpha <= 0.0 || alpha == 1.0 || x_d <= 0.0 || x_d >= 1.0 || x_b <= 0.0 || x_b >= 1.0 {
        return f64::NAN;
    }
    (x_d * (1.0 - x_b) / ((1.0 - x_d) * x_b)).ln() / alpha.ln()
}

/// Minimum reflux ratio for binary distillation (Underwood method approximation).
///
/// The pinch composition is found from the intersection of the q-line
/// `y = q/(q−1)·x − x_F/(q−1)` with the relative-volatility equilibrium curve
/// `y = αx/(1+(α−1)x)`, then `R_min = (1/(α−1))·(x_D/x_p − α(1−x_D)/(1−x_p))`.
pub fn minimum_reflux_ratio(alpha: Scalar, x_f: Scalar, x_d: Scalar, q: Scalar) -> Scalar {
    if alpha <= 0.0 || alpha == 1.0 || x_f <= 0.0 || x_f >= 1.0 || x_d <= 0.0 || x_d >= 1.0 {
        return f64::NAN;
    }
    // Quadratic for the pinch composition (q-line ∩ equilibrium curve):
    //   q(α−1)·x² + [q − x_F(α−1) − α(q−1)]·x − x_F = 0
    let a = q * (alpha - 1.0);
    let b = q - x_f * (alpha - 1.0) - alpha * (q - 1.0);
    let c = -x_f;
    let disc = (b * b - 4.0 * a * c).max(0.0);
    let x_pinch = if a.abs() < 1e-15 {
        // q=0 (saturated vapour): x = -c/b
        -c / b
    } else {
        (-b + disc.sqrt()) / (2.0 * a)
    };
    let x_pinch = x_pinch.clamp(1e-9, 1.0 - 1e-9);
    (1.0 / (alpha - 1.0)) * (x_d / x_pinch - alpha * (1.0 - x_d) / (1.0 - x_pinch))
}

/// Absorption factor for gas absorption columns.
///
/// A = L/(G·K)
pub fn absorption_factor(l: Scalar, g: Scalar, k: Scalar) -> Scalar {
    if g <= 0.0 || k <= 0.0 {
        return 0.0;
    }
    l / (g * k)
}

/// Distribution coefficient for liquid-liquid extraction.
///
/// K_D = C_org / C_aq
pub fn distribution_coefficient(c_org: Scalar, c_aq: Scalar) -> Scalar {
    if c_aq != 0.0 {
        c_org / c_aq
    } else {
        0.0
    }
}

/// Rachford-Rice flash equation.
///
/// F(ψ) = Σᵢ zᵢ·(Kᵢ - 1) / (1 + ψ·(Kᵢ - 1))
///
/// where ψ is the vapor fraction and Kᵢ are equilibrium K-values.
/// Returns the value of the Rachford-Rice function.
pub fn rachford_rice(vapor_frac: Scalar, z_i: &[Scalar], k_i: &[Scalar]) -> Scalar {
    assert_eq!(
        z_i.len(),
        k_i.len(),
        "z_i and k_i must have the same length"
    );

    let mut sum = 0.0;
    for i in 0..z_i.len() {
        let denominator = 1.0 + vapor_frac * (k_i[i] - 1.0);
        if denominator.abs() < 1e-15 {
            // Avoid division by zero
            continue;
        }
        sum += z_i[i] * (k_i[i] - 1.0) / denominator;
    }
    sum
}

/// Solve Rachford-Rice equation for vapor fraction using Newton's method.
///
/// Returns the vapor fraction ψ in [0, 1] that satisfies the flash equation.
pub fn solve_rachford_rice(z_i: &[Scalar], k_i: &[Scalar]) -> Option<Scalar> {
    let n = z_i.len();
    assert_eq!(n, k_i.len());

    // Check if all K_i are essentially 1.0 (no unique solution)
    if k_i.iter().all(|&k| (k - 1.0).abs() < 1e-10) {
        return None;
    }

    // Bracket the solution
    let f0 = rachford_rice(0.0, z_i, k_i);
    let f1 = rachford_rice(1.0, z_i, k_i);

    // If both have same sign, no solution in [0,1]
    if f0 * f1 >= 0.0 {
        // Check if solution is very close to boundaries
        if f0.abs() < 1e-10 {
            return Some(0.0);
        }
        if f1.abs() < 1e-10 {
            return Some(1.0);
        }
        return None;
    }

    // Newton's method
    let mut psi = 0.5;
    for _ in 0..100 {
        let f = rachford_rice(psi, z_i, k_i);

        if f.abs() < 1e-10 {
            return Some(psi.clamp(0.0, 1.0));
        }

        // Derivative: dF/dψ = -Σ z_i·(K_i - 1)² / (1 + ψ·(K_i - 1))²
        let mut df = 0.0;
        for i in 0..n {
            let denom = 1.0 + psi * (k_i[i] - 1.0);
            if denom.abs() < 1e-15 {
                continue;
            }
            df -= z_i[i] * (k_i[i] - 1.0).powi(2) / (denom * denom);
        }

        if df.abs() < 1e-15 {
            return None;
        }

        psi -= f / df;
        psi = psi.clamp(0.0, 1.0);
    }

    // Check final residual
    let f_final = rachford_rice(psi, z_i, k_i);
    if f_final.abs() < 1e-6 {
        Some(psi)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fenske_equation() {
        // α=2.5, x_D=0.95, x_B=0.05
        let n = fenske_equation(2.5, 0.95, 0.05);
        assert!(n > 0.0);
        assert!(n < 10.0);
    }

    #[test]
    fn test_fenske_invalid_alpha() {
        assert!(fenske_equation(1.0, 0.95, 0.05).is_nan());
    }

    #[test]
    fn test_minimum_reflux_ratio() {
        let r_min = minimum_reflux_ratio(2.5, 0.5, 0.95, 1.0);
        assert!(r_min > 0.0);
        assert!(r_min < 10.0);
    }

    #[test]
    fn test_absorption_factor() {
        let a = absorption_factor(100.0, 50.0, 2.0);
        assert!((a - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_absorption_factor_zero_g() {
        assert_eq!(absorption_factor(100.0, 0.0, 2.0), 0.0);
    }

    #[test]
    fn test_distribution_coefficient() {
        assert!((distribution_coefficient(10.0, 5.0) - 2.0).abs() < 1e-12);
        assert_eq!(distribution_coefficient(10.0, 0.0), 0.0);
    }

    #[test]
    fn test_rachford_rice() {
        // Binary mixture: z=[0.5, 0.5], K=[2.0, 0.5]
        let z = vec![0.5, 0.5];
        let k = vec![2.0, 0.5];
        let f0 = rachford_rice(0.0, &z, &k);
        // At ψ=0: F = 0.5*(1)/1 + 0.5*(-0.5)/1 = 0.5 - 0.25 = 0.25
        assert!((f0 - 0.25).abs() < 1e-12);
    }

    #[test]
    fn test_solve_rachford_rice() {
        let z = vec![0.5, 0.5];
        let k = vec![2.0, 0.5];
        let psi = solve_rachford_rice(&z, &k);
        assert!(psi.is_some());
        let psi_val = psi.unwrap();
        assert!(psi_val > 0.0);
        assert!(psi_val < 1.0);
        // Verify by plugging back
        let residual = rachford_rice(psi_val, &z, &k);
        assert!(residual.abs() < 1e-6);
    }

    #[test]
    fn test_solve_rachford_rice_no_solution() {
        // All K_i = 1.0 means no separation possible
        let z = vec![0.5, 0.5];
        let k = vec![1.0, 1.0];
        let psi = solve_rachford_rice(&z, &k);
        assert!(psi.is_none());
    }
}
