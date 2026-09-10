//! wgpu 30.0.1 compute backend — the real GPU acceleration path.
//!
//! This module implements [`GpuBackend`] on top of wgpu's WebGPU-style compute
//! pipeline. It is an optional dependency: enable the `gpu` cargo feature to
//! compile it in. When the feature is off (or no GPU adapter is present, or
//! device creation fails) nothing is registered and the adaptive dispatcher
//! transparently keeps using the CPU paths — results stay identical.
//!
//! # Layout
//!
//! - [`precision32`] / [`precision64`] — WGSL kernels for `f32` and `f64`
//!   storage buffers. Which one is used is decided **once** at device-creation
//!   time from `Adapter::features()`, never per call.
//! - [`kernels`] — pipeline construction and the typed launch helpers.
//! - [`device`] — adapter/device acquisition and the capability report.
//! - this file — the [`GpuBackend`] implementation plus the process-wide
//!   `install_global_gpu_backend()` entry point.
//!
//! # Precision portability (verified constraint)
//!
//! WGSL `f64` storage buffers require [`wgpu::Features::SHADER_F64`], which is
//! **not** universal: Apple Silicon Metal reports `false` (verified on an M4
//! adapter). The backend therefore probes the feature and picks the matching
//! shader set, so GPU acceleration works on every WebGPU-capable adapter
//! instead of only on hardware with 64-bit shader support. When the `f32`
//! kernels are in use the backend reports
//! [`GpuPrecision::F32`] and the dispatcher honours
//! [`GpuBackend::supports_f64`] to avoid silently downgrading
//! double-precision workloads.

pub mod device;
pub mod kernels;
pub mod pool;

use std::sync::Arc;

use crate::core::compute::backend::GpuBackend;
use crate::core::error::SimError;
use crate::core::types::Scalar;

pub use device::GpuPrecision;
pub use kernels::GpuKernels;

use device::GpuContext;
use kernels::BinaryOp;

// Re-export the GPU surface at the module root so callers can reach the device
// context, precision enum, kernel set and buffer pool without knowing the
// internal module split.
pub use pool::BufferPool;

/// The device context type, usable to build a backend from an
/// already-initialised context via [`WgpuBackend::from_context`].
pub type GpuDevice = GpuContext;

/// A live wgpu compute backend bound to one adapter/device.
///
/// Holds the device, its queue and the pre-built compute pipelines. All public
/// methods are synchronous: they submit work and block on the result, which
/// matches the synchronous [`GpuBackend`] contract used by the simulation
/// kernel's hot paths.
pub struct WgpuBackend {
    ctx: GpuContext,
    kernels: GpuKernels,
}

impl WgpuBackend {
    /// Create a backend from an already-initialised [`GpuContext`].
    pub fn from_context(ctx: GpuContext) -> Result<Self, SimError> {
        let kernels = GpuKernels::new(&ctx)?;
        Ok(Self { ctx, kernels })
    }

    /// Acquire a GPU adapter/device and build the backend.
    ///
    /// Returns `Err` (rather than panicking) when no usable adapter exists or
    /// device creation fails, so callers can fall back to the CPU path.
    pub fn new() -> Result<Self, SimError> {
        Self::from_context(GpuContext::new()?)
    }

    /// Adapter/device description, e.g. `"wgpu/Metal/Apple M4"`.
    pub fn description(&self) -> &str {
        self.ctx.description()
    }

    /// Which storage precision the device kernels use.
    pub fn precision(&self) -> GpuPrecision {
        self.ctx.precision()
    }

    /// The underlying device context (for diagnostics and benchmarks).
    pub fn context(&self) -> &GpuContext {
        &self.ctx
    }

    /// The compiled kernel set (for diagnostics and benchmarks).
    pub fn kernels(&self) -> &GpuKernels {
        &self.kernels
    }

    /// Maximum number of `f32` elements addressable by one storage buffer on
    /// this device (derived from the adapter's binding limits).
    pub fn max_elements(&self) -> usize {
        self.ctx.max_elements()
    }

    fn check_len(&self, n: usize) -> Result<(), SimError> {
        if n > self.max_elements() {
            return Err(SimError::numerical(format!(
                "wgpu backend: {n} elements exceeds this adapter's limit of {}",
                self.max_elements()
            )));
        }
        Ok(())
    }

    /// BLAS-2 `gemv`: `y = A·x`, delegated to the matrix-multiply kernel with a
    /// one-column right-hand side so there is a single canonical GPU code path.
    pub fn gemv(&self, a: &[Vec<Scalar>], x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        if a.is_empty() {
            return Ok(Vec::new());
        }
        let cols = a[0].len();
        if x.len() != cols {
            return Err(SimError::numerical(format!(
                "wgpu gemv: matrix cols={cols}, vector len={}",
                x.len()
            )));
        }
        let rhs: Vec<Vec<Scalar>> = x.iter().map(|&v| vec![v]).collect();
        let prod = self.mat_mul(a, &rhs)?;
        Ok(prod.into_iter().map(|row| row[0]).collect())
    }
}

impl GpuBackend for WgpuBackend {
    fn name(&self) -> &str {
        self.ctx.description()
    }

    fn is_available(&self) -> bool {
        self.ctx.device_ok()
    }

    /// Whether this device executes that kernels in `f64`.
    ///
    /// `false` means the `f32` kernel set is active; the dispatcher must then
    /// keep `f64`-critical workloads on the CPU rather than silently losing
    /// precision.
    fn supports_f64(&self) -> bool {
        self.precision() == GpuPrecision::F64
    }

    fn mat_mul(&self, a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
        if a.is_empty() || b.is_empty() {
            return Ok(Vec::new());
        }
        let m = a.len();
        let k = a[0].len();
        if b.len() != k {
            return Err(SimError::numerical(format!(
                "wgpu mat_mul: A cols={k}, B rows={}",
                b.len()
            )));
        }
        let n = b[0].len();
        self.check_len(m.saturating_mul(k))?;
        self.check_len(k.saturating_mul(n))?;
        self.check_len(m.saturating_mul(n))?;
        let flat_a = flatten(a);
        let flat_b = flatten(b);
        let flat_c = self.kernels.mat_mul(&self.ctx, &flat_a, &flat_b, m, k, n)?;
        Ok(unflatten(&flat_c, m, n))
    }

    fn elementwise_add(&self, a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        self.binary(BinaryOp::Add, a, b)
    }

    fn axpy(&self, alpha: Scalar, x: &[Scalar], y: &mut [Scalar]) -> Result<(), SimError> {
        if x.len() != y.len() {
            return Err(SimError::numerical(format!(
                "wgpu axpy: length mismatch {} vs {}",
                x.len(),
                y.len()
            )));
        }
        if x.is_empty() {
            return Ok(());
        }
        self.check_len(x.len())?;
        let out = self.kernels.axpy(&self.ctx, alpha, x, y)?;
        y.copy_from_slice(&out);
        Ok(())
    }

    fn dot(&self, a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SimError> {
        self.reduce(BinaryOp::Dot, a, b)
    }

    fn sum(&self, a: &[Scalar]) -> Result<Scalar, SimError> {
        self.reduce(BinaryOp::Sum, a, a)
    }

    fn vec_sub(&self, a: &[Scalar], b: &[Scalar]) -> Option<Result<Vec<Scalar>, SimError>> {
        Some(self.dispatch_sub(a, b))
    }

    fn vec_hadamard(&self, a: &[Scalar], b: &[Scalar]) -> Option<Result<Vec<Scalar>, SimError>> {
        Some(self.dispatch_hadamard(a, b))
    }

    fn vec_scale(&self, a: &[Scalar], scale: Scalar) -> Option<Result<Vec<Scalar>, SimError>> {
        Some(self.dispatch_scale(a, scale))
    }

    fn vec_abs(&self, a: &[Scalar]) -> Option<Result<Vec<Scalar>, SimError>> {
        Some(self.binary(BinaryOp::Abs, a, a))
    }

    fn asum(&self, a: &[Scalar]) -> Option<Result<Scalar, SimError>> {
        Some(self.reduce(BinaryOp::Asum, a, a))
    }

    fn max_abs(&self, a: &[Scalar]) -> Option<Result<Scalar, SimError>> {
        Some(self.reduce(BinaryOp::MaxAbs, a, a))
    }

    fn gemv(&self, a: &[Vec<Scalar>], x: &[Scalar]) -> Option<Result<Vec<Scalar>, SimError>> {
        Some(WgpuBackend::gemv(self, a, x))
    }

    fn transpose(&self, a: &[Vec<Scalar>]) -> Option<Result<Vec<Vec<Scalar>>, SimError>> {
        Some(self.dispatch_transpose(a))
    }
}

/// Inherent implementations of the same primitives, used by the [`GpuBackend`]
/// trait impl above so the two surfaces cannot drift apart.
impl WgpuBackend {
    fn dispatch_sub(&self, a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        self.binary(BinaryOp::Sub, a, b)
    }

    fn dispatch_hadamard(&self, a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        self.binary(BinaryOp::Mul, a, b)
    }

    fn dispatch_scale(&self, a: &[Scalar], scale: Scalar) -> Result<Vec<Scalar>, SimError> {
        if a.is_empty() {
            return Ok(Vec::new());
        }
        self.check_len(a.len())?;
        self.kernels.scale(&self.ctx, scale, a)
    }

    fn dispatch_transpose(&self, a: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
        if a.is_empty() {
            return Ok(Vec::new());
        }
        let rows = a.len();
        let cols = a[0].len();
        if a.iter().any(|row| row.len() != cols) {
            return Err(SimError::numerical(
                "wgpu transpose: ragged matrix (rows differ in length)",
            ));
        }
        if cols == 0 {
            return Ok(vec![Vec::new(); cols]);
        }
        self.check_len(rows.saturating_mul(cols))?;
        let flat = flatten(a);
        let out = self.kernels.transpose(&self.ctx, &flat, rows, cols)?;
        Ok(unflatten(&out, cols, rows))
    }
}

/// Additional primitives beyond the base [`GpuBackend`] surface.
impl WgpuBackend {
    /// Element-wise subtraction `c = a - b`.
    pub fn vec_sub(&self, a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        self.binary(BinaryOp::Sub, a, b)
    }

    /// Element-wise product `c[i] = a[i] * b[i]`.
    pub fn vec_hadamard(&self, a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        self.binary(BinaryOp::Mul, a, b)
    }

    /// Scalar-vector multiply `c = scale * a`.
    pub fn vec_scale(&self, a: &[Scalar], scale: Scalar) -> Result<Vec<Scalar>, SimError> {
        if a.is_empty() {
            return Ok(Vec::new());
        }
        self.check_len(a.len())?;
        self.kernels.scale(&self.ctx, scale, a)
    }

    /// Squared Euclidean norm `‖a‖²`, computed on-device as `dot(a, a)`.
    pub fn norm_squared(&self, a: &[Scalar]) -> Result<Scalar, SimError> {
        self.reduce(BinaryOp::Dot, a, a)
    }

    fn binary(&self, op: BinaryOp, a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        if a.len() != b.len() {
            return Err(SimError::numerical(format!(
                "wgpu {}: length mismatch {} vs {}",
                op.name(),
                a.len(),
                b.len()
            )));
        }
        if a.is_empty() {
            return Ok(Vec::new());
        }
        self.check_len(a.len())?;
        self.kernels.binary(&self.ctx, op, a, b)
    }

    fn reduce(&self, op: BinaryOp, a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SimError> {
        if a.len() != b.len() {
            return Err(SimError::numerical(format!(
                "wgpu {}: length mismatch {} vs {}",
                op.name(),
                a.len(),
                b.len()
            )));
        }
        if a.is_empty() {
            return Ok(0.0);
        }
        self.check_len(a.len())?;
        self.kernels.reduce(&self.ctx, op, a, b)
    }
}

impl std::fmt::Debug for WgpuBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuBackend")
            .field("description", &self.description())
            .field("precision", &self.precision())
            .finish()
    }
}

/// Attempt to acquire a wgpu backend and register it as the process-wide GPU
/// backend used by [`crate::core::compute::backend::global`].
///
/// This is the **single, idempotent** entry point that turns GPU acceleration
/// on. Call it once during application start-up (or from the platform
/// bindings); every subsequent `adaptive_*` call in the crate then dispatches
/// large workloads to the GPU automatically.
///
/// Returns the backend description on success. Returns `Err` when no adapter
/// or device is available — the caller keeps running on the CPU and the global
/// dispatcher stays in its CPU-only state, so this is always safe to attempt.
///
/// Idempotent: if a GPU backend is already registered, no device is created and
/// the existing description is returned unchanged.
pub fn install_global_gpu_backend() -> Result<String, SimError> {
    let backend = WgpuBackend::new()?;
    let description = backend.description().to_string();
    crate::core::compute::backend::set_global_gpu_backend(Arc::new(backend))?;
    Ok(description)
}

/// Whether the process-wide dispatcher currently has a GPU backend installed.
pub fn global_gpu_installed() -> bool {
    crate::core::compute::backend::global_gpu_description().is_some()
}

/// Human-readable summary of the active GPU configuration, or `None` when the
/// dispatcher is running CPU-only.
pub fn global_gpu_description() -> Option<String> {
    crate::core::compute::backend::global_gpu_description()
}

/// Flatten a row-major `Vec<Vec<Scalar>>` into a contiguous buffer.
///
/// Rows shorter than the first row are zero-padded so the flat layout always
/// matches the declared `cols`; matrices produced by this crate are rectangular.
fn flatten(m: &[Vec<Scalar>]) -> Vec<Scalar> {
    let cols = m.first().map_or(0, Vec::len);
    let mut out = vec![0.0; m.len() * cols];
    for (i, row) in m.iter().enumerate() {
        let n = row.len().min(cols);
        out[i * cols..i * cols + n].copy_from_slice(&row[..n]);
    }
    out
}

/// Rebuild a row-major `Vec<Vec<Scalar>>` from a contiguous buffer.
fn unflatten(flat: &[Scalar], rows: usize, cols: usize) -> Vec<Vec<Scalar>> {
    (0..rows)
        .map(|i| flat[i * cols..(i + 1) * cols].to_vec())
        .collect()
}

#[cfg(feature = "gpu")]
#[cfg(test)]
mod tests {
    use super::*;

    /// Build a backend, or `None` when the machine has no usable GPU. Tests
    /// that need a device skip cleanly instead of failing on headless CI.
    fn backend() -> Option<WgpuBackend> {
        WgpuBackend::new().ok()
    }

    fn rand_vec(n: usize, seed: u64) -> Vec<Scalar> {
        let mut x = seed | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x as f64 / u64::MAX as f64) * 2.0 - 1.0
            })
            .collect()
    }

    fn rand_mat(r: usize, c: usize, seed: u64) -> Vec<Vec<Scalar>> {
        let flat = rand_vec(r * c, seed);
        flat.chunks(c).map(|s| s.to_vec()).collect()
    }

    /// f32 kernels hold ~7 significant digits; scale the tolerance by the
    /// magnitude of the accumulated sum so large problems stay comparable.
    fn tol(scale: f64) -> f64 {
        1e-3 * scale.abs().max(1.0)
    }

    fn max_abs_diff(a: &[Scalar], b: &[Scalar]) -> f64 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
    }

    #[test]
    fn backend_reports_device_metadata() {
        let Some(be) = backend() else { return };
        assert!(be.is_available());
        assert!(be.name().starts_with("wgpu/"), "name = {}", be.name());
        assert!(be.max_elements() > 0);
        println!(
            "wgpu adapter: {} (precision {:?})",
            be.description(),
            be.precision()
        );
    }

    #[test]
    fn gpu_mat_mul_matches_cpu() {
        let Some(be) = backend() else { return };
        let (m, k, n) = (37, 41, 29);
        let a = rand_mat(m, k, 0x1234_5678);
        let b = rand_mat(k, n, 0x9E37_79B9);
        let cpu = crate::core::compute::matrix::mat_mul(&a, &b).unwrap();
        let gpu = be.mat_mul(&a, &b).unwrap();
        assert_eq!(gpu.len(), m);
        assert_eq!(gpu[0].len(), n);
        for (r, row) in cpu.iter().enumerate() {
            let d = max_abs_diff(row, &gpu[r]);
            // 41 multiply-adds per entry.
            assert!(d <= tol(41.0), "row {r}: max|Δ| = {d:e}");
        }
    }

    #[test]
    fn gpu_mat_mul_large_matches_cpu_within_f32_precision() {
        let Some(be) = backend() else { return };
        let n = 96;
        let a = rand_mat(n, n, 0xDEAD_BEEF);
        let b = rand_mat(n, n, 0x0BAD_F00D);
        let cpu = crate::core::compute::matrix::mat_mul(&a, &b).unwrap();
        let gpu = be.mat_mul(&a, &b).unwrap();
        for (r, row) in cpu.iter().enumerate() {
            let d = max_abs_diff(row, &gpu[r]);
            assert!(d <= tol(n as f64), "row {r}: max|Δ| = {d:e}");
        }
    }

    #[test]
    fn gpu_mat_mul_shape_errors() {
        let Some(be) = backend() else { return };
        let a = vec![vec![1.0, 2.0, 3.0]];
        let b = vec![vec![1.0, 2.0]];
        assert!(be.mat_mul(&a, &b).is_err());
    }

    #[test]
    fn gpu_elementwise_and_axpy_match_cpu() {
        let Some(be) = backend() else { return };
        let a = rand_vec(5000, 0xABCD_1234);
        let b = rand_vec(5000, 0x5678_9ABC);

        let add_cpu: Vec<Scalar> = a.iter().zip(&b).map(|(x, y)| x + y).collect();
        let add_gpu = be.elementwise_add(&a, &b).unwrap();
        assert!(max_abs_diff(&add_cpu, &add_gpu) <= tol(1.0));

        let sub_cpu: Vec<Scalar> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
        let sub_gpu = be.vec_sub(&a, &b).unwrap();
        assert!(max_abs_diff(&sub_cpu, &sub_gpu) <= tol(1.0));

        let mul_cpu: Vec<Scalar> = a.iter().zip(&b).map(|(x, y)| x * y).collect();
        let mul_gpu = be.vec_hadamard(&a, &b).unwrap();
        assert!(max_abs_diff(&mul_cpu, &mul_gpu) <= tol(1.0));

        let scale_cpu: Vec<Scalar> = a.iter().map(|x| 2.5 * x).collect();
        let scale_gpu = be.vec_scale(&a, 2.5).unwrap();
        assert!(max_abs_diff(&scale_cpu, &scale_gpu) <= tol(1.0));

        let alpha = -0.75;
        let mut y_cpu = b.clone();
        for (yi, xi) in y_cpu.iter_mut().zip(&a) {
            *yi += alpha * xi;
        }
        let mut y_gpu = b.clone();
        be.axpy(alpha, &a, &mut y_gpu).unwrap();
        assert!(max_abs_diff(&y_cpu, &y_gpu) <= tol(1.0));
    }

    #[test]
    fn gpu_reductions_match_cpu() {
        let Some(be) = backend() else { return };
        // Cross several workgroup boundaries (256 * 64 = 16384 elements each).
        let n = 40_000;
        let a = rand_vec(n, 0x1111_2222);
        let b = rand_vec(n, 0x3333_4444);

        let dot_cpu: Scalar = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        let dot_gpu = be.dot(&a, &b).unwrap();
        assert!(
            (dot_cpu - dot_gpu).abs() <= tol(dot_cpu),
            "dot: cpu={dot_cpu:e} gpu={dot_gpu:e}"
        );

        let sum_cpu: Scalar = a.iter().sum();
        let sum_gpu = be.sum(&a).unwrap();
        assert!(
            (sum_cpu - sum_gpu).abs() <= tol(sum_cpu),
            "sum: cpu={sum_cpu:e} gpu={sum_gpu:e}"
        );

        let ns_cpu: Scalar = a.iter().map(|x| x * x).sum();
        let ns_gpu = be.norm_squared(&a).unwrap();
        assert!((ns_cpu - ns_gpu).abs() <= tol(ns_cpu));
    }

    #[test]
    fn gpu_gemv_matches_cpu() {
        let Some(be) = backend() else { return };
        let (m, k) = (64, 48);
        let a = rand_mat(m, k, 0xFACE_CAFE);
        let x = rand_vec(k, 0x0F0F_0F0F);
        let cpu = crate::core::compute::linalg::gemv(&a, &x).unwrap();
        let gpu = be.gemv(&a, &x).unwrap();
        assert!(max_abs_diff(&cpu, &gpu) <= tol(k as f64));
    }

    #[test]
    fn gpu_abs_asum_max_abs_and_transpose_match_cpu() {
        let Some(be) = backend() else { return };
        let n = 20_000;
        let a = rand_vec(n, 0x2468_ACE0);

        let abs_cpu: Vec<Scalar> = a.iter().map(|v| v.abs()).collect();
        let abs_gpu = be.vec_abs(&a).unwrap().expect("wgpu implements vec_abs");
        assert!(max_abs_diff(&abs_cpu, &abs_gpu) <= tol(1.0));

        let asum_cpu: Scalar = a.iter().map(|v| v.abs()).sum();
        let asum_gpu = be.asum(&a).unwrap().expect("wgpu implements asum");
        assert!(
            (asum_cpu - asum_gpu).abs() <= tol(asum_cpu),
            "asum: cpu={asum_cpu:e} gpu={asum_gpu:e}"
        );

        let max_cpu: Scalar = a.iter().map(|v| v.abs()).fold(0.0, Scalar::max);
        let max_gpu = be.max_abs(&a).unwrap().expect("wgpu implements max_abs");
        assert!(
            (max_cpu - max_gpu).abs() <= tol(max_cpu),
            "max_abs: cpu={max_cpu:e} gpu={max_gpu:e}"
        );

        let (rows, cols) = (37, 53);
        let m = rand_mat(rows, cols, 0x1357_9BDF);
        let t_gpu = be
            .transpose(&m)
            .unwrap()
            .expect("wgpu implements transpose");
        assert_eq!(t_gpu.len(), cols);
        assert_eq!(t_gpu[0].len(), rows);
        for r in 0..rows {
            for c in 0..cols {
                assert!(
                    (t_gpu[c][r] - m[r][c]).abs() <= tol(1.0),
                    "({r},{c}) {} vs {}",
                    t_gpu[c][r],
                    m[r][c]
                );
            }
        }
    }

    #[test]
    fn gpu_transpose_rejects_ragged_matrix() {
        let Some(be) = backend() else { return };
        let ragged = vec![vec![1.0, 2.0, 3.0], vec![4.0]];
        let result = be.transpose(&ragged).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn gpu_length_mismatch_errors() {
        let Some(be) = backend() else { return };
        assert!(be.elementwise_add(&[1.0, 2.0], &[1.0]).is_err());
        assert!(be.dot(&[1.0, 2.0], &[1.0]).is_err());
        assert!(be.sum(&[]).unwrap() == 0.0);
    }

    #[test]
    fn gpu_backend_drives_adaptive_dispatcher() {
        let Some(be) = backend() else { return };
        use crate::core::compute::backend::{
            AdaptiveCompute, BackendKind, ComputeConfig, GpuBackend,
        };
        // Capture the precision before moving the backend into the dispatcher.
        let supports_f64 = be.supports_f64();
        let mut comp = AdaptiveCompute::new(ComputeConfig::new(16, 1024));
        comp.set_gpu_backend(std::sync::Arc::new(be) as std::sync::Arc<dyn GpuBackend>);

        // A single-precision device (e.g. Apple Silicon Metal, where
        // `SHADER_F64` is absent) must not take over the crate's f64 workload:
        // the dispatcher falls back to the CPU instead of silently returning a
        // lower-precision result. A double-precision device selects the GPU.
        let large = 64 * 64 * 64;
        let expected = if supports_f64 {
            BackendKind::Gpu
        } else {
            BackendKind::CpuParallel
        };
        assert_eq!(comp.kind_for(large), expected);
        assert_eq!(comp.kind_for(2 * 2 * 2), BackendKind::Serial);

        // Both dispatch decisions must still produce a correct product, whether
        // the GPU handled it or the CPU fallback did.
        let a = rand_mat(8, 8, 1);
        let b = rand_mat(8, 8, 2);
        let cpu = crate::core::compute::matrix::mat_mul(&a, &b).unwrap();
        let routed = comp.mat_mul(&a, &b).unwrap();
        assert!(max_abs_diff(&cpu[0], &routed[0]) <= tol(8.0));
    }

    #[test]
    fn gpu_forced_backend_bypasses_precision_guard() {
        let Some(be) = backend() else { return };
        use crate::core::compute::backend::{
            AdaptiveCompute, BackendKind, ComputeConfig, GpuBackend,
        };
        // An explicit `force` is a deliberate request (benchmarks, determinism)
        // and overrides the precision guard, so the device is always reachable
        // for measurement even on a single-precision adapter.
        let comp = AdaptiveCompute::new(ComputeConfig::forced(BackendKind::Gpu))
            .with_gpu_backend(std::sync::Arc::new(be) as std::sync::Arc<dyn GpuBackend>);
        assert_eq!(comp.kind_for(1), BackendKind::Gpu);
    }
}
