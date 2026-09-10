//! Numerical integration (quadrature) methods.
//!
//! Provides trapezoidal, Simpson's, and Gauss-Legendre quadrature for
//! integrating functions over finite intervals.

#![allow(clippy::manual_is_multiple_of)]

use crate::core::types::Scalar;

/// Trapezoidal rule integration.
///
/// Integrates the function `f` over [a, b] using `n` sub-intervals.
/// Error ~ O(1/n²).
pub fn trapezoidal(f: &dyn Fn(Scalar) -> Scalar, a: Scalar, b: Scalar, n: usize) -> Scalar {
    if n == 0 || (b - a).abs() < 1e-15 {
        return 0.0;
    }
    let h = (b - a) / n as Scalar;
    let mut sum = 0.5 * (f(a) + f(b));
    for i in 1..n {
        sum += f(a + i as Scalar * h);
    }
    sum * h
}

/// Simpson's rule integration.
///
/// Integrates the function `f` over [a, b] using `n` sub-intervals (n must be even).
/// Error ~ O(1/n⁴).
pub fn simpson(f: &dyn Fn(Scalar) -> Scalar, a: Scalar, b: Scalar, n: usize) -> Scalar {
    if n == 0 || (n & 1) != 0 {
        // Fall back to trapezoidal if n is odd
        return trapezoidal(f, a, b, n);
    }
    if (b - a).abs() < 1e-15 {
        return 0.0;
    }
    let h = (b - a) / n as Scalar;
    let mut sum = f(a) + f(b);
    for i in 1..n {
        let x = a + i as Scalar * h;
        if i % 2 == 0 {
            sum += 2.0 * f(x);
        } else {
            sum += 4.0 * f(x);
        }
    }
    sum * h / 3.0
}

/// Gauss-Legendre quadrature nodes and weights for n=2,3,4,5.
struct GaussLegendreData {
    nodes: Vec<Scalar>,
    weights: Vec<Scalar>,
}

fn gauss_legendre_data(n: usize) -> Option<GaussLegendreData> {
    match n {
        2 => Some(GaussLegendreData {
            nodes: vec![-0.5773502691896257, 0.5773502691896257],
            weights: vec![1.0, 1.0],
        }),
        3 => Some(GaussLegendreData {
            nodes: vec![-0.7745966692414834, 0.0, 0.7745966692414834],
            weights: vec![0.5555555555555556, 0.8888888888888889, 0.5555555555555556],
        }),
        4 => Some(GaussLegendreData {
            nodes: vec![
                -0.8611363115940526,
                -0.3399810435848563,
                0.3399810435848563,
                0.8611363115940526,
            ],
            weights: vec![
                0.3478548451374538,
                0.6521451548625461,
                0.6521451548625461,
                0.3478548451374538,
            ],
        }),
        5 => Some(GaussLegendreData {
            nodes: vec![
                -0.9061798459386640,
                -0.5384693101056831,
                0.0,
                0.5384693101056831,
                0.9061798459386640,
            ],
            weights: vec![
                0.2369268850561891,
                0.4786286704993665,
                0.5688888888888889,
                0.4786286704993665,
                0.2369268850561891,
            ],
        }),
        _ => None,
    }
}

/// Gauss-Legendre quadrature.
///
/// Integrates the function `f` over [a, b] using `n`-point Gauss-Legendre rule.
/// Supports n = 2, 3, 4, 5. Falls back to trapezoidal for other n.
pub fn gauss_legendre(f: &dyn Fn(Scalar) -> Scalar, a: Scalar, b: Scalar, n: usize) -> Scalar {
    let data = match gauss_legendre_data(n) {
        Some(d) => d,
        None => return trapezoidal(f, a, b, n.max(1)),
    };

    if (b - a).abs() < 1e-15 {
        return 0.0;
    }

    let mid = (a + b) / 2.0;
    let half = (b - a) / 2.0;
    let mut sum = 0.0;
    for i in 0..n {
        let x = mid + half * data.nodes[i];
        sum += data.weights[i] * f(x);
    }
    sum * half
}

/// Adaptive Simpson's quadrature with recursive subdivision.
///
/// Recursively subdivides the interval until the estimated error is below `tol`.
pub fn adaptive_simpson(
    f: &dyn Fn(Scalar) -> Scalar,
    a: Scalar,
    b: Scalar,
    tol: Scalar,
    max_depth: usize,
) -> Scalar {
    if max_depth == 0 {
        return simpson(f, a, b, 2);
    }
    let c = (a + b) / 2.0;
    let whole = simpson(f, a, b, 2); // 2 sub-intervals = 3 points
    let left = simpson(f, a, c, 2);
    let right = simpson(f, c, b, 2);
    let error = (left + right - whole).abs() / 15.0;

    if error < tol || (b - a).abs() < 1e-15 {
        return left + right + (left + right - whole) / 15.0;
    }

    adaptive_simpson(f, a, c, tol / 2.0, max_depth - 1)
        + adaptive_simpson(f, c, b, tol / 2.0, max_depth - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trapezoidal_constant() {
        let result = trapezoidal(&|_| 1.0, 0.0, 1.0, 100);
        assert!((result - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_trapezoidal_linear() {
        let result = trapezoidal(&|x| x, 0.0, 1.0, 100);
        assert!((result - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_simpson_quadratic() {
        // ∫₀¹ x² dx = 1/3
        let result = simpson(&|x| x * x, 0.0, 1.0, 10);
        assert!((result - 1.0 / 3.0).abs() < 1e-4);
    }

    #[test]
    fn test_simpson_sin() {
        // ∫₀^π sin(x) dx = 2
        let result = simpson(&|x| x.sin(), 0.0, std::f64::consts::PI, 100);
        assert!((result - 2.0).abs() < 1e-4);
    }

    #[test]
    fn test_gauss_legendre_quadratic() {
        let result = gauss_legendre(&|x| x * x, 0.0, 1.0, 3);
        assert!((result - 1.0 / 3.0).abs() < 1e-4);
    }

    #[test]
    fn test_adaptive_simpson() {
        let result = adaptive_simpson(&|x| x.sin(), 0.0, std::f64::consts::PI, 1e-8, 20);
        assert!((result - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_trapezoidal_zero_interval() {
        let result = trapezoidal(&|x| x, 1.0, 1.0, 10);
        assert!((result - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_simpson_odd_n() {
        // Should fall back to trapezoidal
        let result = simpson(&|x| x, 0.0, 1.0, 5);
        assert!((result - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_gauss_legendre_exponential() {
        // ∫₀¹ e^x dx = e - 1 ≈ 1.71828
        let result = gauss_legendre(&|x| x.exp(), 0.0, 1.0, 5);
        assert!((result - (std::f64::consts::E - 1.0)).abs() < 1e-4);
    }
}
