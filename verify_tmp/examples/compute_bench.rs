//! Comprehensive compute benchmark — numpy/MKL-style ops across the module.
//!
//! Measures the real throughput of every heavy compute primitive at large
//! sizes so the weakest ops (still scalar loops) are visible next to the
//! SIMD-accelerated ones.
//!
//! Run with: `cargo run --release --example compute_bench`

use scico_rs::core::compute::fft::fft;
use scico_rs::core::compute::integration::{gauss_legendre, simpson, trapezoidal};
use scico_rs::core::compute::linalg::{cholesky, gemv, lu_solve, nrm2, qr_decompose, scal};
use scico_rs::core::compute::matrix::{
    determinant, inverse, mat_mul, mat_mul_parallel, solve_linear,
};
use scico_rs::core::compute::vector::dot;
use scico_rs::core::types::Scalar;
use std::time::Instant;

fn xorshift(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

fn rand_vec(n: usize) -> Vec<Scalar> {
    let mut x: u64 = 0x9E3779B97F4A7C15;
    (0..n)
        .map(|_| (xorshift(&mut x) as f64 / u64::MAX as f64) * 2.0 - 1.0)
        .collect()
}

fn rand_sym_pd(n: usize) -> Vec<Vec<Scalar>> {
    // B·Bᵀ + n·I is symmetric positive definite (for Cholesky).
    let b: Vec<Vec<Scalar>> = (0..n).map(|_| rand_vec(n)).collect();
    let mut m = vec![vec![0.0; n]; n];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, m_ij) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for (bik, bjk) in b[i].iter().zip(b[j].iter()) {
                s += bik * bjk;
            }
            *m_ij = s + if i == j { n as Scalar } else { 0.0 };
        }
    }
    m
}

fn time<F: FnMut()>(label: &str, flops: f64, reps: u32, mut f: F) {
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    println!(
        "{:<30} {:>10.2} ms   {:>10.1} GFLOPS",
        label,
        best * 1e3,
        flops / best / 1e9
    );
}

fn main() {
    println!("SCIcoRS compute benchmark (release build)");
    println!("-------------------------------------------------------------");

    // ── matrix multiply (SIMD + SIMD×rayon) ────────────────────────────────
    let n = 512;
    let a = rand_mat(n);
    let b = rand_mat(n);
    time(
        "mat_mul 512³ (SIMD)",
        2.0 * n as f64 * n as f64 * n as f64,
        8,
        || {
            let _ = mat_mul(&a, &b).unwrap();
        },
    );
    time(
        "mat_mul 512³ (SIMD+rayon)",
        2.0 * n as f64 * n as f64 * n as f64,
        8,
        || {
            let _ = mat_mul_parallel(&a, &b).unwrap();
        },
    );

    // ── BLAS-2 gemv (SIMD via m×k @ k×1) ───────────────────────────────────
    let g = 1024;
    let ag = rand_mat(g);
    let xg = rand_vec(g);
    time(
        "gemv 1024×1024 (SIMD)",
        2.0 * g as f64 * g as f64,
        32,
        || {
            let _ = gemv(&ag, &xg).unwrap();
        },
    );

    // ── BLAS-1 (adaptive rayon) ────────────────────────────────────────────
    let big = 8_000_000;
    let xv = rand_vec(big);
    let yv = rand_vec(big);
    time("dot 8M (adaptive)", 2.0 * big as f64, 32, || {
        let _ = dot(&xv, &yv);
    });
    time("scal 8M (adaptive)", big as f64, 32, || {
        let _ = scal(0.5, &xv).unwrap();
    });
    time("nrm2 8M (adaptive)", big as f64, 32, || {
        let _ = nrm2(&xv);
    });

    // ── LAPACK-style dense (O(n³) scalar loops — the next SIMD targets) ───
    let k = 256;
    let aq = rand_mat(k);
    let bq = rand_vec(k);
    time(
        "lu_solve 256 (scalar LU)",
        2.0 * k as f64 * k as f64 * k as f64 / 3.0,
        16,
        || {
            let _ = lu_solve(&aq, &bq).unwrap();
        },
    );
    let pd = rand_sym_pd(k);
    time(
        "cholesky 256 (blocked)",
        k as f64 * k as f64 * k as f64 / 3.0,
        16,
        || {
            let _ = cholesky(&pd).unwrap();
        },
    );
    let k2 = 512;
    let pd2 = rand_sym_pd(k2);
    time(
        "cholesky 512 (blocked+SIMD)",
        k2 as f64 * k2 as f64 * k2 as f64 / 3.0,
        8,
        || {
            let _ = cholesky(&pd2).unwrap();
        },
    );
    let m = 320;
    let am = rand_mat(m);
    time(
        "qr 320×320 (blocked CGS2)",
        2.0 * m as f64 * m as f64 * m as f64,
        16,
        || {
            let _ = qr_decompose(&am).unwrap();
        },
    );
    let d = 300;
    let ad = rand_mat(d);
    let bd = rand_vec(d);
    time(
        "determinant 300 (LU)",
        2.0 * d as f64 * d as f64 * d as f64 / 3.0,
        16,
        || {
            let _ = determinant(&ad).unwrap();
        },
    );
    time(
        "inverse 300 (LU-based)",
        2.0 * d as f64 * d as f64 * d as f64,
        8,
        || {
            let _ = inverse(&ad).unwrap();
        },
    );
    time(
        "solve_linear 300 (Gauss)",
        2.0 * d as f64 * d as f64 * d as f64 / 3.0,
        16,
        || {
            let _ = solve_linear(&ad, &bd).unwrap();
        },
    );

    // ── FFT ────────────────────────────────────────────────────────────────
    let fft_n = 1 << 16;
    let mut data = rand_vec(fft_n * 2);
    time(
        "fft 65536 (Cooley-Tukey)",
        fft_n as f64 * (fft_n as f64).log2() * 5.0,
        16,
        || {
            fft(&mut data).unwrap();
        },
    );

    // ── integration ────────────────────────────────────────────────────────
    let f = |x: Scalar| (x * x).sin() + 1.0;
    let reps = 64;
    time("trapezoidal 1M", 1e6, reps, || {
        let _ = trapezoidal(&f, 0.0, 10.0, 1_000_000);
    });
    time("simpson 1M", 1e6, reps, || {
        let _ = simpson(&f, 0.0, 10.0, 1_000_000);
    });
    time("gauss_legendre 1024", 1024.0, reps, || {
        let _ = gauss_legendre(&f, 0.0, 10.0, 1024);
    });

    println!("-------------------------------------------------------------");
    println!("mat_mul / gemv / BLAS-1 are SIMD/rayon; dense solves use vectorized");
    println!("LU/Cholesky + blocked CGS2 QR (gemm); FFT uses a twiddle table.");
    println!("Every heavy op now has a real, observable fast path.");
}

fn rand_mat(n: usize) -> Vec<Vec<Scalar>> {
    let mut x: u64 = 0x9E3779B97F4A7C15;
    (0..n)
        .map(|_| {
            (0..n)
                .map(|_| (xorshift(&mut x) as f64 / u64::MAX as f64) * 2.0 - 1.0)
                .collect()
        })
        .collect()
}
