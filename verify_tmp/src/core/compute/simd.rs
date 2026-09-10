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

/// `C ← α·A·B + β·C` — safe SIMD gemm over **strided sub-matrices**.
///
/// `a`/`b`/`c` are slices whose first element is the logical (0,0) of the
/// respective matrix; each is indexed as `ptr[i·rs + j·cs]`. Row-major
/// contiguous blocks therefore pass `rs = #cols, cs = 1`; a column block of a
/// row-major buffer passes `rs = full_row_stride, cs = 1` (its slice starts at
/// the block's first element). All strides must be non-negative.
///
/// This powers the in-place trailing-block update in the blocked Cholesky,
/// eliminating the extract/transpose/write-back copies of a naive `syrk`.
// The 14-argument signature mirrors the CBLAS `cblas_dgemm` / matrixmultiply
// `dgemm` contract (m,k,n,α + 3×matrix+strides + β); each argument is required.
#[allow(clippy::too_many_arguments)]
pub fn dgemm_strided(
    m: usize,
    k: usize,
    n: usize,
    alpha: Scalar,
    a: &[Scalar],
    rsa: isize,
    csa: isize,
    b: &[Scalar],
    rsb: isize,
    csb: isize,
    beta: Scalar,
    c: &mut [Scalar],
    rsc: isize,
    csc: isize,
) {
    assert!(
        m > 0 && k > 0 && n > 0,
        "simd::dgemm_strided: empty dimensions"
    );
    assert!(
        rsa >= 0 && csa >= 0 && rsb >= 0 && csb >= 0 && rsc >= 0 && csc >= 0,
        "simd::dgemm_strided: negative strides not supported"
    );
    // Highest element offset touched inside each slice (strides are isize;
    // products fit usize for realistic sizes).
    let maxa = (m - 1) * rsa as usize + (k - 1) * csa as usize;
    let maxb = (k - 1) * rsb as usize + (n - 1) * csb as usize;
    let maxc = (m - 1) * rsc as usize + (n - 1) * csc as usize;
    assert!(
        a.len() > maxa,
        "simd::dgemm_strided: a too short ({} <= {maxa})",
        a.len()
    );
    assert!(
        b.len() > maxb,
        "simd::dgemm_strided: b too short ({} <= {maxb})",
        b.len()
    );
    assert!(
        c.len() > maxc,
        "simd::dgemm_strided: c too short ({} <= {maxc})",
        c.len()
    );
    // SAFETY: all invariants established above — the kernel only touches
    // `a[0..maxa]`, `b[0..maxb]`, `c[0..maxc]`, all within bounds; `a`/`b` are
    // read-only; the caller guarantees `c`'s region does not overlap `a`/`b`
    // (blocked Cholesky passes disjoint column ranges of one buffer).
    unsafe {
        matrixmultiply::dgemm(
            m,
            k,
            n,
            alpha,
            a.as_ptr(),
            rsa,
            csa,
            b.as_ptr(),
            rsb,
            csb,
            beta,
            c.as_mut_ptr(),
            rsc,
            csc,
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
            let b: Vec<Scalar> = (0..k * n)
                .map(|i| ((i * 3 + 1) as Scalar) * -0.25)
                .collect();
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

    #[test]
    fn dgemm_strided_matches_contiguous() {
        // A column block of a row-major buffer (rs = full width, cs = 1) must
        // equal the contiguous result, and an in-place αAB+βC update must
        // equal the manual computation.
        let m = 12;
        let k = 8;
        let n = 10;
        let full = 20; // buffer row width (wider than the block).
        // Build a row-major buffer with the A-block at columns 3..3+k.
        let mut buf = vec![0.0; m * full];
        for i in 0..m {
            for j in 0..k {
                buf[i * full + 3 + j] = ((i * 7 + j * 3) as Scalar) * 0.5;
            }
        }
        // B contiguous k×n.
        let b: Vec<Scalar> = (0..k * n)
            .map(|i| ((i * 5 + 1) as Scalar) * -0.25)
            .collect();
        // C region at columns 3..3+n.
        for i in 0..m {
            for j in 0..n {
                buf[i * full + 3 + j] += ((i + j) as Scalar) * 0.1;
            }
        }
        let a_slice = &buf[3..]; // first element of the A block is buf[0*full+3]
        let mut c_slice = buf[3..].to_vec();
        dgemm_strided(
            m,
            k,
            n,
            1.0,
            a_slice,
            full as isize,
            1,
            &b,
            n as isize,
            1,
            0.0,
            &mut c_slice,
            full as isize,
            1,
        );
        // Reference: contiguous version of the same block.
        let mut a_contig = Vec::with_capacity(m * k);
        for i in 0..m {
            for j in 0..k {
                a_contig.push(buf[i * full + 3 + j]);
            }
        }
        let mut c_contig = vec![0.0; m * n];
        dgemm(m, k, n, &a_contig, &b, &mut c_contig);
        for i in 0..m {
            for j in 0..n {
                assert!(
                    (c_slice[i * full + j] - c_contig[i * n + j]).abs() < 1e-9,
                    "strided mismatch at ({i},{j})"
                );
            }
        }
    }

    #[test]
    fn dgemm_strided_alpha_beta_update() {
        // C ← -A·B + C in place, with strided C, equals the scalar reference.
        let m = 6;
        let k = 4;
        let n = 5;
        let a: Vec<Scalar> = (0..m * k).map(|i| ((i % 7) as Scalar) * 0.3).collect();
        let b: Vec<Scalar> = (0..k * n).map(|i| ((i % 5) as Scalar) * -0.2).collect();
        let mut c: Vec<Scalar> = (0..m * n)
            .map(|i| ((i % 3) as Scalar) * 0.7 + 1.0)
            .collect();
        let mut want = c.clone();
        dgemm_strided(
            m, k, n, -1.0, &a, k as isize, 1, &b, n as isize, 1, 1.0, &mut c, n as isize, 1,
        );
        for i in 0..m {
            for j in 0..n {
                let mut ab = 0.0;
                for t in 0..k {
                    ab += a[i * k + t] * b[t * n + j];
                }
                want[i * n + j] -= ab;
                assert!(
                    (c[i * n + j] - want[i * n + j]).abs() < 1e-9,
                    "update mismatch ({i},{j})"
                );
            }
        }
    }
}
