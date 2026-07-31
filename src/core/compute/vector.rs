//! Vector operations — dot product, cross product, norm, interpolation.
//!
//! Centralized vector computation primitives. Domain modules should use
//! these rather than implementing their own.

use crate::core::types::Scalar;

/// Dot product of two vectors.
///
/// Uses the adaptive backend for large inputs (rayon / registered GPU).
pub fn dot(a: &[Scalar], b: &[Scalar]) -> Option<Scalar> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    // Route through the adaptive dispatcher for large vectors.
    if a.len()
        >= crate::core::compute::backend::global()
            .config()
            .parallel_threshold
    {
        return crate::core::compute::backend::global().dot(a, b).ok();
    }
    let mut s = 0.0;
    for i in 0..a.len() {
        s += a[i] * b[i];
    }
    Some(s)
}

/// Cross product of two 3D vectors.
pub fn cross(a: &[Scalar; 3], b: &[Scalar; 3]) -> [Scalar; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Euclidean norm (L2) of a vector (adaptive BLAS-1 `nrm2`).
pub fn norm(v: &[Scalar]) -> Scalar {
    crate::core::compute::linalg::nrm2(v)
}

/// Squared Euclidean norm (L2²).
pub fn norm_squared(v: &[Scalar]) -> Scalar {
    let mut s = 0.0;
    for &x in v {
        s += x * x;
    }
    s
}

/// Normalize a vector to unit length. Returns None if the vector is zero.
pub fn normalize(v: &[Scalar]) -> Option<Vec<Scalar>> {
    let n = norm(v);
    if n < 1e-15 {
        return None;
    }
    Some(v.iter().map(|&x| x / n).collect())
}

/// Euclidean distance between two points.
pub fn distance(a: &[Scalar], b: &[Scalar]) -> Option<Scalar> {
    if a.len() != b.len() {
        return None;
    }
    let mut s = 0.0;
    for i in 0..a.len() {
        let d = a[i] - b[i];
        s += d * d;
    }
    Some(s.sqrt())
}

/// Linear interpolation: y = y0 + (x - x0) * (y1 - y0) / (x1 - x0).
pub fn lerp(x: Scalar, x0: Scalar, x1: Scalar, y0: Scalar, y1: Scalar) -> Scalar {
    if (x1 - x0).abs() < 1e-15 {
        return y0;
    }
    y0 + (x - x0) * (y1 - y0) / (x1 - x0)
}

/// 1D linear interpolation on a sorted grid.
///
/// Given sorted x values and corresponding y values, interpolate at point `xq`.
/// Extrapolates using the nearest endpoint if `xq` is outside the range.
pub fn interp1(x: &[Scalar], y: &[Scalar], xq: Scalar) -> Option<Scalar> {
    if x.len() != y.len() || x.is_empty() {
        return None;
    }
    if x.len() == 1 {
        return Some(y[0]);
    }
    // Extrapolate left
    if xq <= x[0] {
        return Some(y[0]);
    }
    // Extrapolate right
    if xq >= x[x.len() - 1] {
        return Some(y[y.len() - 1]);
    }
    // Binary search for the interval
    let mut lo = 0;
    let mut hi = x.len() - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if xq < x[mid] {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(lerp(xq, x[lo], x[hi], y[lo], y[hi]))
}

/// Cubic spline interpolation (natural spline).
///
/// Returns interpolated values at query points `xq`.
pub fn spline_interp(x: &[Scalar], y: &[Scalar], xq: &[Scalar]) -> Option<Vec<Scalar>> {
    let n = x.len();
    if n != y.len() || n < 2 {
        return None;
    }

    // Tridiagonal system for natural cubic spline
    let mut h = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        h.push(x[i + 1] - x[i]);
    }

    let mut alpha = vec![0.0; n];
    for i in 1..n - 1 {
        alpha[i] = 3.0 * ((y[i + 1] - y[i]) / h[i] - (y[i] - y[i - 1]) / h[i - 1]);
    }

    let mut l = vec![1.0; n];
    let mut mu = vec![0.0; n];
    let mut z = vec![0.0; n];

    for i in 1..n - 1 {
        l[i] = 2.0 * (x[i + 1] - x[i - 1]) - h[i - 1] * mu[i - 1];
        if l[i].abs() < 1e-15 {
            return None;
        }
        mu[i] = h[i] / l[i];
        z[i] = (alpha[i] - h[i - 1] * z[i - 1]) / l[i];
    }

    // Back substitution
    let mut c = vec![0.0; n];
    let mut b = vec![0.0; n];
    let mut d = vec![0.0; n];
    for j in (0..n - 1).rev() {
        c[j] = z[j] - mu[j] * c[j + 1];
        b[j] = (y[j + 1] - y[j]) / h[j] - h[j] * (c[j + 1] + 2.0 * c[j]) / 3.0;
        d[j] = (c[j + 1] - c[j]) / (3.0 * h[j]);
    }

    // Evaluate at query points
    let mut result = Vec::with_capacity(xq.len());
    for &xq_val in xq {
        if xq_val <= x[0] {
            result.push(y[0]);
            continue;
        }
        if xq_val >= x[n - 1] {
            result.push(y[n - 1]);
            continue;
        }
        let mut lo = 0;
        let mut hi = n - 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if xq_val < x[mid] {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let dx = xq_val - x[lo];
        result.push(y[lo] + b[lo] * dx + c[lo] * dx * dx + d[lo] * dx * dx * dx);
    }
    Some(result)
}

/// Vector addition: c = a + b.
pub fn vec_add(a: &[Scalar], b: &[Scalar]) -> Option<Vec<Scalar>> {
    if a.len() != b.len() {
        return None;
    }
    Some(a.iter().zip(b.iter()).map(|(&x, &y)| x + y).collect())
}

/// Vector subtraction: c = a - b.
pub fn vec_sub(a: &[Scalar], b: &[Scalar]) -> Option<Vec<Scalar>> {
    if a.len() != b.len() {
        return None;
    }
    Some(a.iter().zip(b.iter()).map(|(&x, &y)| x - y).collect())
}

/// Scalar-vector multiply.
pub fn vec_scale(v: &[Scalar], s: Scalar) -> Vec<Scalar> {
    v.iter().map(|&x| x * s).collect()
}

/// Element-wise product (Hadamard product).
pub fn vec_hadamard(a: &[Scalar], b: &[Scalar]) -> Option<Vec<Scalar>> {
    if a.len() != b.len() {
        return None;
    }
    Some(a.iter().zip(b.iter()).map(|(&x, &y)| x * y).collect())
}

/// Vector range: generate evenly spaced values from `start` to `end` with `n` points.
pub fn linspace(start: Scalar, end: Scalar, n: usize) -> Vec<Scalar> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![start];
    }
    let step = (end - start) / (n - 1) as Scalar;
    (0..n).map(|i| start + i as Scalar * step).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        assert!((dot(&a, &b).unwrap() - 32.0).abs() < 1e-10);
    }

    #[test]
    fn test_cross_product() {
        let a = [1.0, 0.0, 0.0];
        let b = [0.0, 1.0, 0.0];
        let c = cross(&a, &b);
        assert!((c[0] - 0.0).abs() < 1e-10);
        assert!((c[1] - 0.0).abs() < 1e-10);
        assert!((c[2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_norm() {
        let v = vec![3.0, 4.0];
        assert!((norm(&v) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalize() {
        let v = vec![3.0, 4.0];
        let n = normalize(&v).unwrap();
        assert!((n[0] - 0.6).abs() < 1e-10);
        assert!((n[1] - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_normalize_zero() {
        let v = vec![0.0, 0.0];
        assert!(normalize(&v).is_none());
    }

    #[test]
    fn test_interp1() {
        let x = vec![0.0, 1.0, 2.0, 3.0];
        let y = vec![0.0, 10.0, 20.0, 30.0];
        assert!((interp1(&x, &y, 1.5).unwrap() - 15.0).abs() < 1e-10);
        assert!((interp1(&x, &y, 0.0).unwrap() - 0.0).abs() < 1e-10);
        assert!((interp1(&x, &y, 3.0).unwrap() - 30.0).abs() < 1e-10);
        assert!((interp1(&x, &y, -1.0).unwrap() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_linspace() {
        let v = linspace(0.0, 10.0, 6);
        assert_eq!(v.len(), 6);
        assert!((v[0] - 0.0).abs() < 1e-10);
        assert!((v[5] - 10.0).abs() < 1e-10);
        assert!((v[2] - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_add() {
        let a = vec![1.0, 2.0];
        let b = vec![3.0, 4.0];
        let c = vec_add(&a, &b).unwrap();
        assert!((c[0] - 4.0).abs() < 1e-10);
        assert!((c[1] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_spline_interp() {
        let x = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let y = vec![0.0, 1.0, 0.0, 1.0, 0.0];
        let xq = vec![0.5, 1.5, 2.5, 3.5];
        let result = spline_interp(&x, &y, &xq).unwrap();
        assert_eq!(result.len(), 4);
        // All interpolated values should be between 0 and 1
        for &v in &result {
            assert!(v >= -0.1 && v <= 1.1);
        }
    }

    #[test]
    fn test_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        assert!((distance(&a, &b).unwrap() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_hadamard() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let c = vec_hadamard(&a, &b).unwrap();
        assert!((c[0] - 4.0).abs() < 1e-10);
        assert!((c[1] - 10.0).abs() < 1e-10);
        assert!((c[2] - 18.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec_scale() {
        let v = vec![1.0, 2.0, 3.0];
        let s = vec_scale(&v, 2.0);
        assert!((s[0] - 2.0).abs() < 1e-10);
        assert!((s[2] - 6.0).abs() < 1e-10);
    }
}
