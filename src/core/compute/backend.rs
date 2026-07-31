//! Adaptive compute backend with CPU (serial/parallel) and GPU dispatch.
//!
//! This module provides a unified execution abstraction for the heavy
//! numerical primitives used across the kernel. It automatically selects
//! the most appropriate backend for a given workload:
//!
//! - [`BackendKind::Serial`] — for small problems where thread/launch
//!   overhead dominates the actual computation.
//! - [`BackendKind::CpuParallel`] — rayons work-stealing thread pool for
//!   medium/large problems (real multi-core speed-up on the CPU).
//! - [`BackendKind::Gpu`] — a registered GPU backend, once one is installed
//!   at runtime via [`AdaptiveCompute::set_gpu_backend`].
//!
//! GPU acceleration is exposed through the [`GpuBackend`] trait so that a
//! real runtime (wgpu / OpenCL / CUDA / oneAPI) can be plugged in without
//! changing any call sites. When no GPU backend is registered (or the
//! workload is too small to justify the transfer overhead), execution
//! transparently falls back to the CPU path — results are identical.

use crate::core::compute::vendor_blas::VendorBlasBackend;
use crate::core::error::SimError;
use crate::core::types::Scalar;

/// Execution backend kinds supported by the adaptive dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Single-threaded execution (lowest overhead).
    Serial,
    /// Rayon work-stealing parallel execution on the CPU.
    CpuParallel,
    /// A registered vendor CPU BLAS library (MKL / ACML / ESSL), chosen
    /// adaptively among those available.
    VendorCpu,
    /// A dedicated GPU backend registered at runtime (wgpu/OpenCL/CUDA).
    Gpu,
}

impl BackendKind {
    /// Human-readable backend name.
    pub fn name(self) -> &'static str {
        match self {
            BackendKind::Serial => "serial",
            BackendKind::CpuParallel => "cpu-parallel",
            BackendKind::VendorCpu => "vendor-cpu",
            BackendKind::Gpu => "gpu",
        }
    }
}

/// Interface implemented by GPU runtimes.
///
/// A production GPU backend (e.g. wrapping wgpu, OpenCL or CUDA) implements
/// this trait and registers itself with [`AdaptiveCompute::set_gpu_backend`].
/// The adapter handles device selection, buffer allocation and kernel launch;
/// call sites only see the same [`Scalar`]-based API as the CPU path.
pub trait GpuBackend: Send + Sync {
    /// Backend/device display name (e.g. "wgpu/GeForce RTX 4070").
    fn name(&self) -> &str;
    /// Whether the device is currently available.
    fn is_available(&self) -> bool;
    /// Matrix multiply `C = A * B` on the GPU.
    fn mat_mul(&self, a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError>;
    /// Element-wise `c[i] = a[i] + b[i]`.
    fn elementwise_add(&self, a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError>;
    /// AXPY: `y[i] = alpha * x[i] + y[i]`.
    fn axpy(&self, alpha: Scalar, x: &[Scalar], y: &mut [Scalar]) -> Result<(), SimError>;
    /// Dot product of two vectors.
    fn dot(&self, a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SimError>;
    /// Sum of all elements.
    fn sum(&self, a: &[Scalar]) -> Result<Scalar, SimError>;
}

/// Configuration for the adaptive dispatcher.
#[derive(Debug, Clone)]
pub struct ComputeConfig {
    /// Number of "work units" at which we switch from serial to CPU-parallel.
    /// A work unit is one multiply-add for `mat_mul` / `axpy`, one element
    /// for element-wise ops. Default: 4096.
    pub parallel_threshold: usize,
    /// Number of work units at which we attempt GPU dispatch (if available).
    /// Default: 262_144.
    pub gpu_threshold: usize,
    /// Force a specific backend regardless of size. `None` = adaptive.
    pub force: Option<BackendKind>,
}

impl Default for ComputeConfig {
    fn default() -> Self {
        Self {
            parallel_threshold: 4096,
            gpu_threshold: 262_144,
            force: None,
        }
    }
}

impl ComputeConfig {
    /// An adaptive configuration with explicit thresholds.
    pub fn new(parallel_threshold: usize, gpu_threshold: usize) -> Self {
        Self {
            parallel_threshold,
            gpu_threshold,
            force: None,
        }
    }

    /// Force a single backend (useful for benchmarks and deterministic runs).
    pub fn forced(backend: BackendKind) -> Self {
        Self {
            force: Some(backend),
            ..Self::default()
        }
    }
}

/// Adaptive compute dispatcher.
///
/// Selects [`BackendKind`] based on workload size, registered GPU backend
/// availability and any forced override, then executes the primitive on the
/// chosen backend. All paths produce numerically identical results.
pub struct AdaptiveCompute {
    config: ComputeConfig,
    gpu: Option<std::sync::Arc<dyn GpuBackend>>,
    /// Registered vendor BLAS/LAPACK backends (MKL / ACML / ESSL / cuBLAS).
    /// The dispatcher selects among them adaptively (by availability + vendor
    /// priority + workload size).
    vendor: Vec<std::sync::Arc<dyn VendorBlasBackend>>,
}

impl Default for AdaptiveCompute {
    fn default() -> Self {
        Self::new(ComputeConfig::default())
    }
}

impl AdaptiveCompute {
    /// Create a new adaptive dispatcher with the given configuration.
    pub fn new(config: ComputeConfig) -> Self {
        Self {
            config,
            gpu: None,
            vendor: Vec::new(),
        }
    }

    /// Create a default adaptive dispatcher.
    pub fn default_config() -> Self {
        Self::default()
    }

    /// Install (or replace) the GPU backend used for large workloads.
    pub fn set_gpu_backend(&mut self, backend: std::sync::Arc<dyn GpuBackend>) {
        self.gpu = Some(backend);
    }

    /// Remove the GPU backend, falling back to CPU only.
    pub fn clear_gpu_backend(&mut self) {
        self.gpu = None;
    }

    /// Current GPU backend, if any.
    pub fn gpu_backend(&self) -> Option<&dyn GpuBackend> {
        self.gpu.as_deref()
    }

    /// Register a vendor BLAS/LAPACK backend (MKL / ACML / ESSL / cuBLAS).
    ///
    /// Multiple vendors may be registered; the dispatcher selects adaptively
    /// among the available ones (best CPU vendor for CPU workloads, a GPU
    /// vendor for GPU-sized workloads). Registering the same vendor again
    /// replaces it.
    pub fn register_vendor_blas(&mut self, backend: std::sync::Arc<dyn VendorBlasBackend>) {
        self.vendor.retain(|b| b.vendor() != backend.vendor());
        self.vendor.push(backend);
    }

    /// Remove all registered vendor BLAS backends (CPU fallback only).
    pub fn clear_vendor_blas(&mut self) {
        self.vendor.clear();
    }

    /// All registered vendor backends (in registration order).
    pub fn vendor_blases(&self) -> impl Iterator<Item = &dyn VendorBlasBackend> {
        self.vendor.iter().map(|b| b.as_ref())
    }

    /// The best available CPU vendor backend, or `None` if none is usable.
    ///
    /// Adaptive selection: among available CPU vendors (MKL / ACML / ESSL),
    /// the one with the lowest [`BlasVendor::cpu_priority`] is chosen (ties
    /// broken by registration order).
    pub fn best_vendor_cpu(&self) -> Option<&dyn VendorBlasBackend> {
        self.vendor
            .iter()
            .filter(|b| b.is_available() && !b.vendor().is_gpu())
            .min_by_key(|b| b.vendor().cpu_priority())
            .map(|b| b.as_ref())
    }

    /// The best available GPU vendor backend (cuBLAS), or `None`.
    pub fn best_vendor_gpu(&self) -> Option<&dyn VendorBlasBackend> {
        self.vendor
            .iter()
            .find(|b| b.is_available() && b.vendor().is_gpu())
            .map(|b| b.as_ref())
    }

    /// Run `vendor_op` against the selected vendor CPU backend, falling back
    /// to `cpu` if the vendor is absent or the op is not accelerated.
    pub(crate) fn vendor_or_cpu<T>(
        &self,
        vendor_op: impl FnOnce(&dyn VendorBlasBackend) -> Result<T, SimError>,
        cpu: impl FnOnce() -> Result<T, SimError>,
    ) -> Result<T, SimError> {
        if let Some(v) = self.best_vendor_cpu() {
            if let Ok(r) = vendor_op(v) {
                return Ok(r);
            }
        }
        cpu()
    }

    /// The active configuration.
    pub fn config(&self) -> &ComputeConfig {
        &self.config
    }

    /// Select the backend for a workload of `work_units` operations.
    pub fn kind_for(&self, work_units: usize) -> BackendKind {
        if let Some(forced) = self.config.force {
            return forced;
        }
        // GPU only for large workloads, and only when a GPU backend (or a GPU
        // vendor such as cuBLAS) is registered and available.
        if work_units >= self.config.gpu_threshold
            && (self.gpu.as_ref().is_some_and(|g| g.is_available())
                || self.best_vendor_gpu().is_some())
        {
            return BackendKind::Gpu;
        }
        // Vendor CPU BLAS (MKL/ACML/ESSL) for large CPU workloads, chosen
        // adaptively among those registered and available.
        if work_units >= self.config.parallel_threshold && self.best_vendor_cpu().is_some() {
            return BackendKind::VendorCpu;
        }
        if work_units >= self.config.parallel_threshold {
            return BackendKind::CpuParallel;
        }
        BackendKind::Serial
    }

    /// Describe the current execution strategy (for diagnostics/logging).
    pub fn describe(&self) -> String {
        let gpu = self
            .gpu
            .as_ref()
            .filter(|g| g.is_available())
            .map(|g| g.name().to_string())
            .unwrap_or_else(|| "none".to_string());
        let vendor = self
            .best_vendor_cpu()
            .map(|v| v.name().to_string())
            .unwrap_or_else(|| "none".to_string());
        let gpu_vendor = self
            .best_vendor_gpu()
            .map(|v| v.name().to_string())
            .unwrap_or_else(|| "none".to_string());
        format!(
            "adaptive(p_parallel={}, p_gpu={}, vendor_cpu={}, vendor_gpu={}, gpu={})",
            self.config.parallel_threshold, self.config.gpu_threshold, vendor, gpu_vendor, gpu
        )
    }

    /// Matrix multiply with adaptive backend selection.
    ///
    /// Work units ≈ m · n · p (multiply-adds).
    pub fn mat_mul(
        &self,
        a: &[Vec<Scalar>],
        b: &[Vec<Scalar>],
    ) -> Result<Vec<Vec<Scalar>>, SimError> {
        if a.is_empty() || b.is_empty() {
            return Ok(Vec::new());
        }
        let m = a.len();
        let n = a[0].len();
        if b.len() != n {
            return Err(SimError::numerical(format!(
                "mat_mul: inner dimensions don't match: A cols={}, B rows={}",
                n,
                b.len()
            )));
        }
        let p = b[0].len();
        let work = m.saturating_mul(n).saturating_mul(p);
        match self.kind_for(work) {
            BackendKind::Gpu => {
                let gpu = self.gpu.as_ref().expect("gpu selected but not registered");
                gpu.mat_mul(a, b)
            }
            BackendKind::VendorCpu => {
                self.vendor_or_cpu(|v| v.gemm(a, b), || super::matrix::mat_mul_parallel(a, b))
            }
            BackendKind::CpuParallel => super::matrix::mat_mul_parallel(a, b),
            // Serial path already uses the pure-Rust SIMD kernel internally.
            BackendKind::Serial => super::matrix::mat_mul(a, b),
        }
    }

    /// Element-wise `c[i] = a[i] + b[i]`.
    pub fn elementwise_add(&self, a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        if a.len() != b.len() {
            return Err(SimError::numerical(format!(
                "elementwise_add: length mismatch {} vs {}",
                a.len(),
                b.len()
            )));
        }
        match self.kind_for(a.len()) {
            BackendKind::Gpu => {
                let gpu = self.gpu.as_ref().expect("gpu selected but not registered");
                gpu.elementwise_add(a, b)
            }
            BackendKind::VendorCpu => {
                // No standard BLAS op for element-wise add; use the CPU path.
                Ok(a.iter().zip(b.iter()).map(|(x, y)| x + y).collect())
            }
            BackendKind::CpuParallel => {
                use rayon::prelude::*;
                Ok(a.par_iter().zip(b.par_iter()).map(|(x, y)| x + y).collect())
            }
            BackendKind::Serial => Ok(a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()),
        }
    }

    /// AXPY: `y[i] = alpha * x[i] + y[i]`.
    pub fn axpy(&self, alpha: Scalar, x: &[Scalar], y: &mut [Scalar]) -> Result<(), SimError> {
        if x.len() != y.len() {
            return Err(SimError::numerical(format!(
                "axpy: length mismatch {} vs {}",
                x.len(),
                y.len()
            )));
        }
        match self.kind_for(x.len()) {
            BackendKind::Gpu => {
                let gpu = self.gpu.as_ref().expect("gpu selected but not registered");
                gpu.axpy(alpha, x, y)
            }
            BackendKind::VendorCpu => {
                // Try the vendor; fall back to the serial loop on failure (the
                // vendor and fallback both need `&mut y`, so they cannot live
                // in two closures at once).
                if let Some(v) = self.best_vendor_cpu()
                    && v.axpy(alpha, x, y).is_ok()
                {
                    return Ok(());
                }
                for (yi, xi) in y.iter_mut().zip(x.iter()) {
                    *yi += alpha * xi;
                }
                Ok(())
            }
            BackendKind::CpuParallel => {
                use rayon::prelude::*;
                y.par_iter_mut()
                    .zip(x.par_iter())
                    .for_each(|(yi, xi)| *yi += alpha * xi);
                Ok(())
            }
            BackendKind::Serial => {
                for (yi, xi) in y.iter_mut().zip(x.iter()) {
                    *yi += alpha * xi;
                }
                Ok(())
            }
        }
    }

    /// Dot product of two vectors.
    pub fn dot(&self, a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SimError> {
        if a.len() != b.len() {
            return Err(SimError::numerical(format!(
                "dot: length mismatch {} vs {}",
                a.len(),
                b.len()
            )));
        }
        match self.kind_for(a.len()) {
            BackendKind::Gpu => {
                let gpu = self.gpu.as_ref().expect("gpu selected but not registered");
                gpu.dot(a, b)
            }
            BackendKind::VendorCpu => self.vendor_or_cpu(
                |v| v.dot(a, b),
                || Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()),
            ),
            BackendKind::CpuParallel => {
                use rayon::prelude::*;
                Ok(a.par_iter().zip(b.par_iter()).map(|(x, y)| x * y).sum())
            }
            BackendKind::Serial => Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()),
        }
    }

    /// Sum of all elements.
    pub fn sum(&self, a: &[Scalar]) -> Result<Scalar, SimError> {
        match self.kind_for(a.len()) {
            BackendKind::Gpu => {
                let gpu = self.gpu.as_ref().expect("gpu selected but not registered");
                gpu.sum(a)
            }
            BackendKind::VendorCpu => self.vendor_or_cpu(|v| v.sum(a), || Ok(a.iter().sum())),
            BackendKind::CpuParallel => {
                use rayon::prelude::*;
                Ok(a.par_iter().sum())
            }
            BackendKind::Serial => Ok(a.iter().sum()),
        }
    }

    /// Apply a map function over a slice with adaptive dispatch.
    ///
    /// `f` is `Fn(Scalar) -> Scalar`; the parallel path processes elements
    /// in parallel via rayon.
    pub fn map_vec<F>(&self, a: &[Scalar], f: F) -> Result<Vec<Scalar>, SimError>
    where
        F: Fn(Scalar) -> Scalar + Sync,
    {
        match self.kind_for(a.len()) {
            BackendKind::CpuParallel => {
                use rayon::prelude::*;
                Ok(a.par_iter().map(|&v| f(v)).collect())
            }
            _ => Ok(a.iter().map(|&v| f(v)).collect()),
        }
    }
}

/// A global default adaptive dispatcher, initialised once.
static GLOBAL_ADAPTIVE: std::sync::OnceLock<AdaptiveCompute> = std::sync::OnceLock::new();

/// Return a reference to the global default adaptive dispatcher.
pub fn global() -> &'static AdaptiveCompute {
    GLOBAL_ADAPTIVE.get_or_init(AdaptiveCompute::default)
}

/// Convenience: adaptive matrix multiply via the global dispatcher.
pub fn adaptive_mat_mul(
    a: &[Vec<Scalar>],
    b: &[Vec<Scalar>],
) -> Result<Vec<Vec<Scalar>>, SimError> {
    global().mat_mul(a, b)
}

/// Convenience: adaptive element-wise add via the global dispatcher.
pub fn adaptive_add(a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
    global().elementwise_add(a, b)
}

/// Convenience: adaptive AXPY via the global dispatcher.
pub fn adaptive_axpy(alpha: Scalar, x: &[Scalar], y: &mut [Scalar]) -> Result<(), SimError> {
    global().axpy(alpha, x, y)
}

/// Convenience: adaptive dot product via the global dispatcher.
pub fn adaptive_dot(a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SimError> {
    global().dot(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic GPU backend used to exercise the GPU dispatch path in
    /// tests. Computes the same results as the CPU path; in production a real
    /// GPU runtime implements `GpuBackend`.
    struct MockGpuBackend;

    impl GpuBackend for MockGpuBackend {
        fn name(&self) -> &str {
            "mock-gpu"
        }
        fn is_available(&self) -> bool {
            true
        }
        fn mat_mul(
            &self,
            a: &[Vec<Scalar>],
            b: &[Vec<Scalar>],
        ) -> Result<Vec<Vec<Scalar>>, SimError> {
            super::super::matrix::mat_mul(a, b)
        }
        fn elementwise_add(&self, a: &[Scalar], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
            Ok(a.iter().zip(b.iter()).map(|(x, y)| x + y).collect())
        }
        fn axpy(&self, alpha: Scalar, x: &[Scalar], y: &mut [Scalar]) -> Result<(), SimError> {
            for (yi, xi) in y.iter_mut().zip(x.iter()) {
                *yi += alpha * xi;
            }
            Ok(())
        }
        fn dot(&self, a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SimError> {
            Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
        }
        fn sum(&self, a: &[Scalar]) -> Result<Scalar, SimError> {
            Ok(a.iter().sum())
        }
    }

    #[test]
    fn test_backend_kind_names() {
        assert_eq!(BackendKind::Serial.name(), "serial");
        assert_eq!(BackendKind::CpuParallel.name(), "cpu-parallel");
        assert_eq!(BackendKind::VendorCpu.name(), "vendor-cpu");
        assert_eq!(BackendKind::Gpu.name(), "gpu");
    }

    #[test]
    fn test_kind_for_small_is_serial() {
        let comp = AdaptiveCompute::default();
        assert_eq!(comp.kind_for(10), BackendKind::Serial);
        assert_eq!(comp.kind_for(100), BackendKind::Serial);
    }

    #[test]
    fn test_kind_for_large_is_parallel() {
        let comp = AdaptiveCompute::new(ComputeConfig::new(100, 1_000_000));
        assert_eq!(comp.kind_for(10_000), BackendKind::CpuParallel);
    }

    #[test]
    fn test_kind_for_gpu_only_when_registered() {
        // No GPU registered -> large workloads still go to CPU parallel.
        let comp = AdaptiveCompute::new(ComputeConfig::new(100, 50));
        assert_eq!(comp.kind_for(100_000), BackendKind::CpuParallel);

        // GPU registered -> large workloads go to GPU.
        let mut comp = AdaptiveCompute::new(ComputeConfig::new(100, 50));
        comp.set_gpu_backend(std::sync::Arc::new(MockGpuBackend));
        assert_eq!(comp.kind_for(100_000), BackendKind::Gpu);

        // Small workloads never go to GPU (transfer overhead).
        assert_eq!(comp.kind_for(10), BackendKind::Serial);
    }

    #[test]
    fn test_kind_for_forced_override() {
        let comp = AdaptiveCompute::new(ComputeConfig::forced(BackendKind::Serial));
        assert_eq!(comp.kind_for(1_000_000), BackendKind::Serial);
    }

    #[test]
    fn test_mat_mul_serial_and_parallel_match() {
        let n = 32;
        let a: Vec<Vec<Scalar>> = (0..n)
            .map(|i| (0..n).map(|j| (i * n + j) as Scalar / 10.0).collect())
            .collect();
        let b: Vec<Vec<Scalar>> = (0..n)
            .map(|i| (0..n).map(|j| ((i + j) % 7) as Scalar).collect())
            .collect();

        let serial = AdaptiveCompute::new(ComputeConfig::forced(BackendKind::Serial));
        let parallel = AdaptiveCompute::new(ComputeConfig::forced(BackendKind::CpuParallel));

        let c1 = serial.mat_mul(&a, &b).unwrap();
        let c2 = parallel.mat_mul(&a, &b).unwrap();
        assert_eq!(c1.len(), n);
        for i in 0..n {
            for j in 0..n {
                assert!((c1[i][j] - c2[i][j]).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn test_mat_mul_gpu_dispatch() {
        let n = 24;
        let a: Vec<Vec<Scalar>> = (0..n)
            .map(|i| (0..n).map(|j| (i + 2 * j) as Scalar).collect())
            .collect();
        let b: Vec<Vec<Scalar>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| (3 * i as i32 - j as i32) as Scalar)
                    .collect()
            })
            .collect();

        let mut comp = AdaptiveCompute::new(ComputeConfig::new(0, 0));
        comp.set_gpu_backend(std::sync::Arc::new(MockGpuBackend));
        assert_eq!(comp.kind_for(1000), BackendKind::Gpu);

        let c_gpu = comp.mat_mul(&a, &b).unwrap();
        let c_ref = AdaptiveCompute::new(ComputeConfig::forced(BackendKind::Serial))
            .mat_mul(&a, &b)
            .unwrap();
        for i in 0..n {
            for j in 0..n {
                assert!((c_gpu[i][j] - c_ref[i][j]).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn test_elementwise_ops_all_backends_match() {
        let a: Vec<Scalar> = (0..1000).map(|i| i as Scalar).collect();
        let b: Vec<Scalar> = (0..1000).map(|i| (i as Scalar) * 2.0 + 1.0).collect();
        for forced in [
            BackendKind::Serial,
            BackendKind::CpuParallel,
            BackendKind::Gpu,
        ] {
            let mut comp = AdaptiveCompute::new(ComputeConfig::forced(forced));
            if forced == BackendKind::Gpu {
                comp.set_gpu_backend(std::sync::Arc::new(MockGpuBackend));
            }
            let c = comp.elementwise_add(&a, &b).unwrap();
            for (i, v) in c.iter().enumerate() {
                assert!((v - (a[i] + b[i])).abs() < 1e-9);
            }
            let dot = comp.dot(&a, &b).unwrap();
            let expected: Scalar = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            assert!((dot - expected).abs() < 1e-6);

            let mut y = vec![1.0; a.len()];
            comp.axpy(2.0, &a, &mut y).unwrap();
            assert!((y[0] - 1.0).abs() < 1e-12);

            let s = comp.sum(&a).unwrap();
            let exp_sum: Scalar = a.iter().sum();
            assert!((s - exp_sum).abs() < 1e-6);
        }
    }

    #[test]
    fn test_map_vec() {
        let comp = AdaptiveCompute::new(ComputeConfig::forced(BackendKind::CpuParallel));
        let a: Vec<Scalar> = vec![1.0, 2.0, 3.0];
        let out = comp.map_vec(&a, |v| v * v).unwrap();
        assert_eq!(out, vec![1.0, 4.0, 9.0]);
    }

    #[test]
    fn test_length_mismatch_errors() {
        let comp = AdaptiveCompute::default();
        assert!(comp.elementwise_add(&[1.0], &[1.0, 2.0]).is_err());
        assert!(comp.dot(&[1.0], &[1.0, 2.0]).is_err());
        assert!(comp.mat_mul(&[vec![1.0, 2.0]], &[vec![1.0]]).is_err());
        let mut y = vec![1.0];
        assert!(comp.axpy(1.0, &[1.0, 2.0], &mut y).is_err());
    }

    #[test]
    fn test_global_convenience() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let b = vec![vec![5.0, 6.0], vec![7.0, 8.0]];
        let c = adaptive_mat_mul(&a, &b).unwrap();
        assert_eq!(c, vec![vec![19.0, 22.0], vec![43.0, 50.0]]);
        assert_eq!(
            adaptive_add(&[1.0, 2.0], &[3.0, 4.0]).unwrap(),
            vec![4.0, 6.0]
        );
        assert!((adaptive_dot(&[1.0, 2.0], &[3.0, 4.0]).unwrap() - 11.0).abs() < 1e-12);
    }

    // ──────────────────────────────────────────────
    // Vendor BLAS (MKL / ACML / ESSL / cuBLAS) adaptive dispatch
    // ──────────────────────────────────────────────

    use crate::core::compute::vendor_blas::BlasVendor;
    use crate::core::compute::vendor_blas::tests::MockVendor;

    #[test]
    fn test_kind_for_vendor_cpu_selected_for_large() {
        let mut comp = AdaptiveCompute::new(ComputeConfig::new(100, 1_000_000));
        comp.register_vendor_blas(std::sync::Arc::new(MockVendor::new(BlasVendor::Mkl)));
        // Large CPU workload -> vendor CPU BLAS (not rayon).
        assert_eq!(comp.kind_for(10_000), BackendKind::VendorCpu);
        // Small workload still serial (vendor overhead not justified).
        assert_eq!(comp.kind_for(10), BackendKind::Serial);
    }

    #[test]
    fn test_kind_for_vendor_not_selected_when_unavailable() {
        let mut comp = AdaptiveCompute::new(ComputeConfig::new(100, 1_000_000));
        let mut v = MockVendor::new(BlasVendor::Essl);
        v.available = false;
        comp.register_vendor_blas(std::sync::Arc::new(v));
        // Unavailable vendor must not be selected -> rayon fallback.
        assert_eq!(comp.kind_for(10_000), BackendKind::CpuParallel);
    }

    #[test]
    fn test_kind_for_cublas_vendor_goes_to_gpu_tier() {
        let mut comp = AdaptiveCompute::new(ComputeConfig::new(100, 50));
        comp.register_vendor_blas(std::sync::Arc::new(MockVendor::new(BlasVendor::Cublas)));
        // cuBLAS is a GPU vendor -> huge workloads target the GPU tier.
        assert_eq!(comp.kind_for(100_000), BackendKind::Gpu);
    }

    #[test]
    fn test_best_vendor_cpu_adaptive_priority() {
        let mut comp = AdaptiveCompute::new(ComputeConfig::default());
        // Register lower-priority first; MKL must still be chosen.
        comp.register_vendor_blas(std::sync::Arc::new(MockVendor::new(BlasVendor::Essl)));
        comp.register_vendor_blas(std::sync::Arc::new(MockVendor::new(BlasVendor::Mkl)));
        let best = comp.best_vendor_cpu().expect("a CPU vendor is registered");
        assert_eq!(best.vendor(), BlasVendor::Mkl);
        // Re-registering MKL replaces it (no duplicates).
        comp.register_vendor_blas(std::sync::Arc::new(MockVendor::new(BlasVendor::Mkl)));
        assert_eq!(comp.vendor_blases().count(), 2);
    }

    #[test]
    fn test_mat_mul_vendor_matches_serial() {
        let n = 32;
        let a: Vec<Vec<Scalar>> = (0..n)
            .map(|i| (0..n).map(|j| (i * n + j) as Scalar / 10.0).collect())
            .collect();
        let b: Vec<Vec<Scalar>> = (0..n)
            .map(|i| (0..n).map(|j| ((i + j) % 7) as Scalar).collect())
            .collect();

        let mut comp = AdaptiveCompute::new(ComputeConfig::forced(BackendKind::VendorCpu));
        comp.register_vendor_blas(std::sync::Arc::new(MockVendor::new(BlasVendor::Mkl)));
        let c_vendor = comp.mat_mul(&a, &b).unwrap();
        let c_ref = AdaptiveCompute::new(ComputeConfig::forced(BackendKind::Serial))
            .mat_mul(&a, &b)
            .unwrap();
        for i in 0..n {
            for j in 0..n {
                assert!((c_vendor[i][j] - c_ref[i][j]).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn test_vendor_falls_back_when_op_unsupported() {
        // A vendor that does NOT accelerate gemm must transparently fall back
        // to the CPU path with identical results.
        let mut comp = AdaptiveCompute::new(ComputeConfig::forced(BackendKind::VendorCpu));
        comp.register_vendor_blas(std::sync::Arc::new(
            MockVendor::new(BlasVendor::Mkl).with_unsupported(&["gemm"]),
        ));
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let b = vec![vec![5.0, 6.0], vec![7.0, 8.0]];
        let c = comp.mat_mul(&a, &b).unwrap();
        assert_eq!(c, vec![vec![19.0, 22.0], vec![43.0, 50.0]]);

        // Same for dot / axpy.
        let dot = comp.dot(&[1.0, 2.0], &[3.0, 4.0]).unwrap();
        assert!((dot - 11.0).abs() < 1e-12);
        let mut y = vec![0.0, 0.0];
        comp.axpy(2.0, &[1.0, 2.0], &mut y).unwrap();
        assert_eq!(y, vec![2.0, 4.0]);
    }

    #[test]
    fn test_describe_includes_vendor() {
        let mut comp = AdaptiveCompute::new(ComputeConfig::default());
        let desc = comp.describe();
        assert!(desc.contains("vendor_cpu=none"));
        comp.register_vendor_blas(std::sync::Arc::new(MockVendor::new(BlasVendor::Acml)));
        assert!(comp.describe().contains("vendor_cpu=acml"));
    }
}
