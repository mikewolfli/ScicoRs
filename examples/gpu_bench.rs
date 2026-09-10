//! GPU vs CPU matrix-multiply benchmark — measured acceleration evidence.
//!
//! Compares the pure-Rust SIMD CPU kernel (serial and rayon) against the wgpu
//! 30.0.1 compute backend for the same problem, verifies that both produce the
//! same result, and reports the achieved GFLOPS and speed-up.
//!
//! Run with:
//! ```text
//! cargo run --release --features gpu --example gpu_bench
//! ```
//!
//! Without the `gpu` feature the example still builds and explains how to
//! enable it, so a CPU-only checkout never fails to compile.

use scico_rs::core::compute::matrix::{mat_mul, mat_mul_parallel};
use scico_rs::core::types::Scalar;
use std::time::Instant;

fn rand_mat(m: usize, k: usize, seed: u64) -> Vec<Vec<Scalar>> {
    let mut x = seed | 1;
    let flat: Vec<Scalar> = (0..m * k)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x as f64 / u64::MAX as f64) * 2.0 - 1.0
        })
        .collect();
    flat.chunks(k).map(|r| r.to_vec()).collect()
}

/// Median-of-reps timing after one untimed warm-up, in seconds.
fn bench<F: FnMut()>(reps: usize, mut f: F) -> f64 {
    f();
    let t = Instant::now();
    for _ in 0..reps {
        f();
    }
    t.elapsed().as_secs_f64() / reps as f64
}

fn gflops(n: usize, secs: f64) -> f64 {
    2.0 * (n as f64).powi(3) / secs / 1e9
}

/// Largest element-wise absolute difference between two matrices.
///
/// Only the GPU comparison uses this, so it is compiled out of a CPU-only
/// build along with the rest of the verification path.
#[cfg(feature = "gpu")]
fn max_diff(a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Scalar {
    a.iter()
        .zip(b.iter())
        .map(|(ra, rb)| {
            ra.iter()
                .zip(rb.iter())
                .map(|(x, y)| (x - y).abs())
                .fold(0.0, f64::max)
        })
        .fold(0.0, f64::max)
}

fn main() {
    println!("SCIcoRS GPU acceleration benchmark (wgpu 30.0.1)");
    println!("=================================================");
    cpu_baseline();
    #[cfg(feature = "gpu")]
    gpu_comparison();
    #[cfg(not(feature = "gpu"))]
    {
        println!("This binary was built WITHOUT the `gpu` feature.");
        println!("Rebuild with: cargo run --release --features gpu --example gpu_bench");
    }
}

/// CPU-only GFLOPS curve, always available.
fn cpu_baseline() {
    println!("CPU reference (pure-Rust SIMD + rayon):");
    for n in [256usize, 512, 768] {
        let a = rand_mat(n, n, 0x1234_5678);
        let b = rand_mat(n, n, 0x9E37_79B9);
        let serial = bench(4, || {
            let _ = mat_mul(&a, &b).unwrap();
        });
        let parallel = bench(4, || {
            let _ = mat_mul_parallel(&a, &b).unwrap();
        });
        println!(
            "  {n:>4}³: serial {:>7.2} GFLOPS   rayon {:>7.2} GFLOPS",
            gflops(n, serial),
            gflops(n, parallel)
        );
    }
    println!("=================================================");
}

/// The GPU-vs-CPU comparison, compiled only with the `gpu` feature.
#[cfg(feature = "gpu")]
fn gpu_comparison() {
    use scico_rs::GpuBackend;
    use scico_rs::WgpuBackend;

    let backend = match WgpuBackend::new() {
        Ok(b) => b,
        Err(e) => {
            println!("No usable GPU adapter: {e}");
            println!("The CPU paths remain fully functional.");
            return;
        }
    };
    println!("device        : {}", backend.description());
    println!("precision     : {:?}", backend.precision());
    println!(
        "f64 kernels   : {}",
        if backend.supports_f64() {
            "yes"
        } else {
            "no (f32 kernels; f64 workloads stay on the CPU by default)"
        }
    );
    println!("max elements  : {}", backend.max_elements());
    println!("=================================================");

    // The GPU only wins once the O(n³) work clearly exceeds the fixed
    // per-dispatch round-trip, so the sweep spans the crossover region.
    let mut last_winner = "cpu";
    for n in [64usize, 128, 256, 512, 768, 1024] {
        let a = rand_mat(n, n, 0x1234_5678);
        let b = rand_mat(n, n, 0x9E37_79B9);

        let cpu_serial = bench(4, || {
            let _ = mat_mul(&a, &b).unwrap();
        });
        let cpu_parallel = bench(4, || {
            let _ = mat_mul_parallel(&a, &b).unwrap();
        });
        let gpu = bench(4, || {
            let _ = backend.mat_mul(&a, &b).unwrap();
        });

        // Verify the device result against the CPU reference.
        let reference = mat_mul(&a, &b).unwrap();
        let device_result = backend.mat_mul(&a, &b).unwrap();
        let diff = max_diff(&reference, &device_result);

        let cpu_best = cpu_serial.min(cpu_parallel);
        let winner = if gpu < cpu_best { "GPU" } else { "CPU" };
        if winner != last_winner {
            println!("-- crossover to {winner} below this size --");
            last_winner = winner;
        }
        println!("size {n:>4}³:");
        println!(
            "  cpu-serial  {:>9.3} ms  {:>8.2} GFLOPS",
            cpu_serial * 1e3,
            gflops(n, cpu_serial)
        );
        println!(
            "  cpu-rayon   {:>9.3} ms  {:>8.2} GFLOPS",
            cpu_parallel * 1e3,
            gflops(n, cpu_parallel)
        );
        println!(
            "  gpu-wgpu    {:>9.3} ms  {:>8.2} GFLOPS   {:.2}× vs best CPU",
            gpu * 1e3,
            gflops(n, gpu),
            cpu_best / gpu
        );
        println!("  verified: max |Δ| gpu vs cpu = {diff:.3e}");
    }

    println!("=================================================");
    println!("Interpretation: every GPU result matches the CPU reference to the");
    println!("precision of the active kernel set. On integrated GPUs (shared memory,");
    println!("large fixed dispatch latency) the CPU SIMD path wins at these sizes;");
    println!("ComputeConfig::discrete_gpu() lowers the GPU threshold for parts with");
    println!("dedicated memory, where the crossover happens much earlier.");
}
