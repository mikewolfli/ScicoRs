//! Thermal radiation models: Stefan-Boltzmann law, radiation exchange
//! between surfaces, and view factor calculations for common geometries.

use crate::core::types::Scalar;

/// Stefan-Boltzmann law: radiative heat flux from a black body.
///
/// q = ε · σ · T⁴
/// Returns the radiative heat flux (W/m²).
pub fn stefan_boltzmann(emissivity: Scalar, temperature: Scalar) -> Scalar {
    if emissivity <= 0.0 || temperature <= 0.0 {
        return 0.0;
    }
    emissivity * super::physics::SIGMA_SB * temperature.powi(4)
}

/// Radiative heat exchange between two gray surfaces.
///
/// Q = (σ · (T₁⁴ - T₂⁴)) / ((1-ε₁)/(A₁·ε₁) + 1/(A₁·F₁₂) + (1-ε₂)/(A₂·ε₂))
///
/// Simplified for the case where both surfaces are large and close
/// (A₁ = A₂ = A, F₁₂ = 1):
///   Q = σ · A · (T₁⁴ - T₂⁴) / (1/ε₁ + 1/ε₂ - 1)
pub fn radiation_exchange(a1: Scalar, eps1: Scalar, eps2: Scalar, t1: Scalar, t2: Scalar) -> Scalar {
    if a1 <= 0.0 || eps1 <= 0.0 || eps2 <= 0.0 {
        return 0.0;
    }
    let sigma = super::physics::SIGMA_SB;
    let t1_4 = t1.powi(4);
    let t2_4 = t2.powi(4);
    let denom = 1.0 / eps1 + 1.0 / eps2 - 1.0;
    if denom <= 0.0 {
        return 0.0;
    }
    sigma * a1 * (t1_4 - t2_4) / denom
}

/// View factor between two parallel coaxial disks.
///
/// F₁₂ = 0.5 · (S - √(S² - 4·(R₂/R₁)²))
/// where S = 1 + (1 + R₂²) / R₁², R₁ = r₁/L, R₂ = r₂/L
pub fn view_factor_parallel_disks(r1: Scalar, r2: Scalar, distance: Scalar) -> Scalar {
    if r1 <= 0.0 || r2 <= 0.0 || distance <= 0.0 {
        return 0.0;
    }
    let r1_ratio = r1 / distance;
    let r2_ratio = r2 / distance;
    let s = 1.0 + (1.0 + r2_ratio * r2_ratio) / (r1_ratio * r1_ratio);
    let sqrt_term = (s * s - 4.0 * (r2_ratio / r1_ratio).powi(2)).sqrt();
    0.5 * (s - sqrt_term)
}

/// View factor between two perpendicular rectangles sharing a common edge.
///
/// F₁₂ = (1/(π·W)) · [W·arctan(1/W) + H·arctan(1/H)
///        - √(H²+W²)·arctan(1/√(H²+W²))
///        + 0.25·ln(((1+W²)(1+H²))/(1+W²+H²))
///        · (W²·(1+W²+H²)/((1+W²)(W²+H²))^W²)
///        · (H²·(1+W²+H²)/((1+H²)(W²+H²))^H²)]
/// where W = w/L, H = h/L (w,h = rectangle dimensions, L = common edge)
pub fn view_factor_perpendicular_rectangles(l: Scalar, w: Scalar, h: Scalar) -> Scalar {
    if l <= 0.0 || w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let w_ratio = w / l;
    let h_ratio = h / l;
    let wh_sq = w_ratio * w_ratio + h_ratio * h_ratio;

    let term1 = w_ratio * (1.0 / w_ratio).atan();
    let term2 = h_ratio * (1.0 / h_ratio).atan();
    let term3 = wh_sq.sqrt() * (1.0 / wh_sq.sqrt()).atan();

    // Full Incropera (Table 13.1) logarithm with the W² and H² power terms:
    //   ln{ A · [B]^(W²) · [C]^(H²) }
    let w2 = w_ratio * w_ratio;
    let h2 = h_ratio * h_ratio;
    let sum = 1.0 + w2 + h2;
    let a = ((1.0 + w2) * (1.0 + h2)) / sum;
    let b = (w2 * sum) / ((1.0 + w2) * (w2 + h2));
    let c = (h2 * sum) / ((1.0 + h2) * (w2 + h2));
    let log_arg = a * b.powf(w2) * c.powf(h2);
    let ln_part = if log_arg > 0.0 { 0.25 * log_arg.ln() } else { 0.0 };

    let result = (term1 + term2 - term3 + ln_part) / (std::f64::consts::PI * w_ratio);
    result.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stefan_boltzmann_positive() {
        // Black body at 300 K
        let q = stefan_boltzmann(1.0, 300.0);
        assert!(q > 0.0);
        let expected = 5.670374419e-8 * 300.0_f64.powi(4);
        assert!((q - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_stefan_boltzmann_zero_temp() {
        let q = stefan_boltzmann(1.0, 0.0);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_stefan_boltzmann_zero_emissivity() {
        let q = stefan_boltzmann(0.0, 300.0);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_radiation_exchange_equal_temps() {
        let q = radiation_exchange(1.0, 0.9, 0.9, 300.0, 300.0);
        assert!((q).abs() < 1e-10);
    }

    #[test]
    fn test_radiation_exchange_positive() {
        // Two parallel plates: A=1 m², ε₁=ε₂=0.9, T₁=400 K, T₂=300 K
        let q = radiation_exchange(1.0, 0.9, 0.9, 400.0, 300.0);
        assert!(q > 0.0);
    }

    #[test]
    fn test_radiation_exchange_zero_area() {
        let q = radiation_exchange(0.0, 0.9, 0.9, 400.0, 300.0);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_view_factor_parallel_disks_equal() {
        let f = view_factor_parallel_disks(0.1, 0.1, 0.2);
        assert!(f > 0.0 && f < 1.0);
    }

    #[test]
    fn test_view_factor_parallel_disks_zero_distance() {
        let f = view_factor_parallel_disks(0.1, 0.1, 0.0);
        assert_eq!(f, 0.0);
    }

    #[test]
    fn test_view_factor_parallel_disks_small_distance() {
        // Very close disks should have F ≈ 1
        let f = view_factor_parallel_disks(1.0, 1.0, 0.01);
        assert!(f > 0.99);
    }

    #[test]
    fn test_view_factor_perpendicular_rectangles_basic() {
        let f = view_factor_perpendicular_rectangles(1.0, 1.0, 1.0);
        assert!(f > 0.0 && f < 0.5);
    }

    #[test]
    fn test_view_factor_perpendicular_rectangles_zero_l() {
        let f = view_factor_perpendicular_rectangles(0.0, 1.0, 1.0);
        assert_eq!(f, 0.0);
    }
}
