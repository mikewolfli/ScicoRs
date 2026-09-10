//! Matrix multiply acceleration benchmark.
//!
//! Demonstrates the real SIMD acceleration wired into the compute path:
//! naive reference vs pure-Rust SIMD (`matrixmultiply`) vs SIMD+rayon.
//!
//! Run with: `cargo run --release --example matmul_bench`

use scico_rs::core::compute::matrix::{mat_mul, mat_mul_naive, mat_mul_parallel};
use scico_rs::core::types::Scalar;
use std::time::Instant;

fn rand_mat(m: usize, k: usize) -> Vec<Vec<Scalar>> {
    let mut x: u64 = 0x9E3779B97F4A7C15;
    (0..m)
        .map(|_| {
            (0..k)
                .map(|_| {
                    // xorshift64
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    (x as f64 / u64::MAX as f64) * 2.0 - 1.0
                })
                .collect()
        })
        .collect()
}

fn gflops(m: usize, k: usize, n: usize, secs: f64) -> f64 {
    2.0 * m as f64 * k as f64 * n as f64 / secs / 1e9
}

fn bench<F: Fn()>(label: &str, size: usize, f: F) -> f64 {
    let t = Instant::now();
    for _ in 0..8 {
        f();
    }
    let secs = t.elapsed().as_secs_f64() / 8.0;
    println!(
        "{:<26} {:>8.1} ms   {:>8.2} GFLOPS",
        label,
        secs * 1e3,
        gflops(size, size, size, secs)
    );
    secs
}

fn main() {
    let size = 512;
    let a = rand_mat(size, size);
    let b = rand_mat(size, size);

    println!("Matrix multiply benchmark  ({size}x{size}x{size} f64, release build)");
    println!("--------------------------------------------------------------");

    // Warm-up + reference.
    let _ = mat_mul_naive(&a, &b, size, size, size);
    let _ = mat_mul(&a, &b).unwrap();
    let _ = mat_mul_parallel(&a, &b).unwrap();

    let naive = bench("naive (reference)", size, || {
        let _ = mat_mul_naive(&a, &b, size, size, size);
    });
    let simd = bench("SIMD (matrixmultiply)", size, || {
        let _ = mat_mul(&a, &b).unwrap();
    });
    let simd_par = bench("SIMD + rayon (parallel)", size, || {
        let _ = mat_mul_parallel(&a, &b).unwrap();
    });

    // Correctness cross-check between all three.
    let c0 = mat_mul_naive(&a, &b, size, size, size);
    let c1 = mat_mul(&a, &b).unwrap();
    let c2 = mat_mul_parallel(&a, &b).unwrap();
    let max_diff = |c: &[Vec<Scalar>]| {
        c.iter()
            .enumerate()
            .map(|(i, row)| {
                row.iter()
                    .enumerate()
                    .map(|(j, v)| (v - c0[i][j]).abs())
                    .fold(0.0, f64::max)
            })
            .fold(0.0, f64::max)
    };
    println!("--------------------------------------------------------------");
    println!("max |Δ| SIMD        = {:.3e}", max_diff(&c1));
    println!("max |Δ| SIMD+rayon  = {:.3e}", max_diff(&c2));
    println!(
        "speedup  SIMD vs naive = {:.2}×",
        naive / simd
    );
    println!(
        "speedup  SIMD+rayon vs naive = {:.2}×",
        naive / simd_par
    );
}
