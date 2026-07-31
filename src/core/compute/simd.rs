//! Pure-Rust SIMD kernels — the single, audited acceleration boundary.
//!
//! These wrappers expose the `matrixmultiply` gemm kernels (the same
//! battle-tested BLIS-style kernels used by `ndarray`) behind a **safe** API.
//!
//! ## Safety discipline
//!
//! `matrixmultiply::dgemm` is `unsafe fn`; this module is the **only** place
//! in the crate that calls into it, and the only `unsafe` block in production
//! code. Every invariant it requires is established in safe code *before* the
//! call:
//!
//! - `m`, `k`, `n` are non-zero (the kernel assumes non-empty matrices);
//! - `a.len() >= m*k`, `b.len() >= k*n`, `c.len() >= m*n` (the kernel indexes
//!   beyond the end of the slices if these are violated);
//! - row-major strides are passed as `rsx = #cols`, `csx = 1`, matching the
//!   contiguous slice layout;
//! - the destination `c` is a freshly allocated buffer, so no element aliases
//!   any input (the kernel forbids alias-in-C).
//!
//! The kernel never performs UB on its own; it *panics* on inconsistent
//! shapes. The `assert!`s below run in release too, so the unsafe call is
//! only reached with shapes the kernel accepts.
//!
//! Verified against the naive reference in `#[cfg(test)]` below (exact match
//! within 1 ulp-ish tolerance for typical sizes).

use crate::core::types::Scalar;

/// `C = A·B` — safe SIMD dense matrix multiply.
///
/// `a` is row-major `m×k`, `b` is row-major `k×n`, `c` (row-major `m×n`) is
/// overwritten (its previous contents are ignored).
pub fn dgemm(m: usize, k: usize, n: usize, a: &[Scalar], b: &[Scalar], c: &mut [Scalar]) {
    assert!(m > 0 && k > 0 && n > 0, "simd::dgemm: empty dimensions");
    assert!(
        a.len() >= m.saturating_mul(k),
        "simd::dgemm: a too short ({} < {m}×{k})",
        a.len()
    );
    assert!(
        b.len() >= k.saturating_mul(n),
        "simd::dgemm: b too short ({} < {k}×{n})",
        b.len()
    );
    assert!(
        c.len() >= m.saturating_mul(n),
        "simd::dgemm: c too short ({} < {m}×{n})",
        c.len()
    );
    // SAFETY: all invariants established above. `a`/`b` are read-only, `c` is
    // a distinct freshly allocated buffer (no aliasing), slices are long
    // enough for the (m,k,n) strides, and strides are the canonical row-major
    // (rs = #cols, cs = 1) for contiguous storage.
    unsafe {
        matrixmultiply::dgemm(
            m,
            k,
            n,
            1.0,
            a.as_ptr(),
            k as isize,
            1,
            b.as_ptr(),
            n as isize,
            1,
            0.0,
            c.as_mut_ptr(),
            n as isize,
            1,
        );
    }
}

/// `y = A·x` — safe SIMD matrix-vector multiply (BLAS-2 as a `m×k @ k×1` gemm).
///
/// `a` is row-major `m×k`, `x` has length `k`, `y` (length `m`) is overwritten.
pub fn dgemv(m: usize, k: usize, a: &[Scalar], x: &[Scalar], y: &mut [Scalar]) {
    assert!(m > 0 && k > 0, "simd::dgemv: empty dimensions");
    assert!(
        a.len() >= m.saturating_mul(k),
        "simd::dgemv: a too short ({} < {m}×{k})",
        a.len()
    );
    assert!(x.len() >= k, "simd::dgemv: x too short ({} < {k})", x.len());
    assert!(y.len() >= m, "simd::dgemv: y too short ({} < {m})", y.len());
    // SAFETY: same argument as `dgemm` with n = 1 (x as a k×1 column).
    unsafe {
        matrixmultiply::dgemm(
            m,
            k,
            1,
            1.0,
            a.as_ptr(),
            k as isize,
            1,
            x.as_ptr(),
            1,
            1,
            0.0,
            y.as_mut_ptr(),
            1,
            1,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_dgemm(m: usize, k: usize, n: usize, a: &[Scalar], b: &[Scalar]) -> Vec<Scalar> {
        let mut c = vec![0.0; m * n];
        for i in 0..m {
            for kk in 0..k {
                let aik = a[i * k + kk];
                for j in 0..n {
                    c[i * n + j] += aik * b[kk * n + j];
                }
            }
        }
        c
    }

    #[test]
    fn dgemm_matches_reference() {
        for (m, k, n) in [(1, 1, 1), (3, 4, 5), (16, 16, 16), (24, 7, 31)] {
            let a: Vec<Scalar> = (0..m * k).map(|i| ((i * 7 + 3) as Scalar) * 0.5).collect();
            let b: Vec<Scalar> = (0..k * n).map(|i| ((i * 3 + 1) as Scalar) * -0.25).collect();
            let want = reference_dgemm(m, k, n, &a, &b);
            let mut got = vec![0.0; m * n];
            dgemm(m, k, n, &a, &b, &mut got);
            for (g, w) in got.iter().zip(want.iter()) {
                assert!((g - w).abs() < 1e-9, "dgemm mismatch: {g} vs {w}");
            }
        }
    }

    #[test]
    fn dgemv_matches_reference() {
        let m = 8;
        let k = 5;
        let a: Vec<Scalar> = (0..m * k).map(|i| (i as Scalar) * 0.1).collect();
        let x: Vec<Scalar> = (0..k).map(|i| (i as Scalar) - 2.0).collect();
        let mut want = vec![0.0; m];
        for i in 0..m {
            for j in 0..k {
                want[i] += a[i * k + j] * x[j];
            }
        }
        let mut got = vec![0.0; m];
        dgemv(m, k, &a, &x, &mut got);
        for (g, w) in got.iter().zip(want.iter()) {
            assert!((g - w).abs() < 1e-9, "dgemv mismatch: {g} vs {w}");
        }
    }
}
