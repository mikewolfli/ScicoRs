//! Vendor BLAS/LAPACK backends (MKL / ACML / ESSL / cuBLAS) with adaptive selection.
//!
//! The kernel can accelerate its dense linear algebra with any of the major
//! vendor BLAS/LAPACK libraries:
//!
//! - **Intel MKL** (now oneMKL) — CPU: `cblas_*` + LAPACK `*gesv`/`*potrf`/`*geqrf`
//! - **AMD ACML** — CPU: same CBLAS/LAPACK interface
//! - **IBM ESSL** — CPU: CBLAS-compatible entry points
//! - **NVIDIA cuBLAS** — GPU: `cublas*_v2` kernels
//!
//! Selection among these libraries is **adaptive**:
//!
//! - All registered backends are kept in a runtime registry; the dispatcher
//!   picks the *best available* backend for each workload automatically.
//! - For GPU-sized workloads a GPU vendor (cuBLAS) / GPU backend is chosen.
//! - For large CPU workloads the highest-priority available CPU vendor
//!   (MKL > ACML > ESSL) is chosen; otherwise rayon parallel; else serial.
//! - Every vendor op falls back to the internal CPU implementation if the
//!   vendor returns an error (e.g. an op it does not accelerate), so results
//!   are always identical to the CPU path.
//!
//! A production binding implements [`VendorBlasBackend`] (or registers a
//! `libloading`-based loader built from [`BlasVendor::symbols`]). `MockVendor`
//! in the tests exercises the full dispatch/fallback path without a real
//! library present.

use crate::core::error::SimError;
use crate::core::types::Scalar;

/// QR factorisation result returned by a vendor backend: `(q, r)`.
pub type VendorQrResult = (Vec<Vec<Scalar>>, Vec<Vec<Scalar>>);

/// The vendor BLAS/LAPACK libraries supported by the adaptive dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlasVendor {
    /// Intel Math Kernel Library / oneMKL (CPU).
    Mkl,
    /// AMD Core Math Library (CPU).
    Acml,
    /// IBM Engineering and Scientific Subroutine Library (CPU).
    Essl,
    /// NVIDIA cuBLAS (GPU).
    Cublas,
}

impl BlasVendor {
    /// Human-readable vendor name.
    pub fn name(self) -> &'static str {
        match self {
            BlasVendor::Mkl => "mkl",
            BlasVendor::Acml => "acml",
            BlasVendor::Essl => "essl",
            BlasVendor::Cublas => "cublas",
        }
    }

    /// Whether the library runs on a GPU (only cuBLAS).
    pub fn is_gpu(self) -> bool {
        matches!(self, BlasVendor::Cublas)
    }

    /// Adaptive ordering among CPU vendors (lower number = tried first).
    /// MKL is typically the fastest general-purpose CPU BLAS; ACML and ESSL
    /// are tied at the same tier so the first *available* one wins.
    pub fn cpu_priority(self) -> u8 {
        match self {
            BlasVendor::Mkl => 0,
            BlasVendor::Acml => 1,
            BlasVendor::Essl => 1,
            BlasVendor::Cublas => u8::MAX, // not a CPU vendor
        }
    }

    /// Shared-library names a dlopen-based loader should probe for this
    /// vendor (Linux `.so`, then Windows `.dll`, then macOS `.dylib`).
    pub fn known_library_names(self) -> &'static [&'static str] {
        match self {
            BlasVendor::Mkl => &["libmkl_rt.so", "mkl_rt.dll", "libmkl_rt.dylib"],
            BlasVendor::Acml => &["libacml.so", "libacml_mp.so", "acml.dll"],
            BlasVendor::Essl => &["libessl.so", "libesslsmp.so", "essl.dll"],
            BlasVendor::Cublas => &["libcublas.so", "cublas64_12.dll", "libcublas.dylib"],
        }
    }

    /// Exact symbol names a binding must resolve for this vendor.
    ///
    /// BLAS-1/2/3 use the CBLAS interface (except cuBLAS which uses the CUDA
    /// `cublas*_v2` C API); LAPACK routines use the Fortran trailing-underscore
    /// ABI. A real loader resolves these and implements [`VendorBlasBackend`].
    pub fn symbols(self) -> &'static [&'static str] {
        match self {
            BlasVendor::Mkl | BlasVendor::Acml | BlasVendor::Essl => &[
                // CBLAS level-1/2/3 (double)
                "cblas_dgemm",
                "cblas_dgemv",
                "cblas_ddot",
                "cblas_dnrm2",
                "cblas_dasum",
                "cblas_dscal",
                "cblas_daxpy",
                "cblas_idamax",
                // LAPACK (Fortran ABI)
                "dgesv_",
                "dpotrf_",
                "dgeqrf_",
            ],
            BlasVendor::Cublas => &[
                "cublasDgemm_v2",
                "cublasDgemv_v2",
                "cublasDdot_v2",
                "cublasDnrm2_v2",
                "cublasDasum_v2",
                "cublasDscal_v2",
                "cublasDaxpy_v2",
                "cublasIdamax_v2",
            ],
        }
    }
}

/// Interface implemented by vendor BLAS/LAPACK bindings.
///
/// Implement only the operations the vendor accelerates; the default
/// implementations return an error and the adaptive dispatcher falls back to
/// the internal CPU path (results stay identical). `mock` backends in the
/// tests implement the full surface.
pub trait VendorBlasBackend: Send + Sync {
    /// Which vendor library this backend wraps.
    fn vendor(&self) -> BlasVendor;
    /// Display name (e.g. `"mkl/libmkl_rt.so"`).
    fn name(&self) -> &str;
    /// Whether the library is currently usable.
    fn is_available(&self) -> bool;

    /// BLAS-3 `gemm`: `C = A·B`.
    fn gemm(&self, _a: &[Vec<Scalar>], _b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
        Err(SimError::runtime("vendor backend does not accelerate gemm"))
    }
    /// BLAS-2 `gemv`: `y = A·x`.
    fn gemv(&self, _a: &[Vec<Scalar>], _x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        Err(SimError::runtime("vendor backend does not accelerate gemv"))
    }
    /// BLAS-1 `axpy`: `y = α·x + y`.
    fn axpy(&self, _alpha: Scalar, _x: &[Scalar], _y: &mut [Scalar]) -> Result<(), SimError> {
        Err(SimError::runtime("vendor backend does not accelerate axpy"))
    }
    /// BLAS-1 `dot`: `Σ xᵢ·yᵢ`.
    fn dot(&self, _a: &[Scalar], _b: &[Scalar]) -> Result<Scalar, SimError> {
        Err(SimError::runtime("vendor backend does not accelerate dot"))
    }
    /// BLAS-1 `sum`: `Σ xᵢ`.
    fn sum(&self, _a: &[Scalar]) -> Result<Scalar, SimError> {
        Err(SimError::runtime("vendor backend does not accelerate sum"))
    }
    /// BLAS-1 `scal`: `y = α·x`.
    fn scal(&self, _alpha: Scalar, _x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        Err(SimError::runtime("vendor backend does not accelerate scal"))
    }
    /// BLAS-1 `nrm2`: `‖x‖₂`.
    fn nrm2(&self, _x: &[Scalar]) -> Result<Scalar, SimError> {
        Err(SimError::runtime("vendor backend does not accelerate nrm2"))
    }
    /// BLAS-1 `asum`: `Σ|xᵢ|`.
    fn asum(&self, _x: &[Scalar]) -> Result<Scalar, SimError> {
        Err(SimError::runtime("vendor backend does not accelerate asum"))
    }
    /// BLAS-1 `iamax`: index of max `|xᵢ|`.
    fn iamax(&self, _x: &[Scalar]) -> Result<usize, SimError> {
        Err(SimError::runtime(
            "vendor backend does not accelerate iamax",
        ))
    }
    /// LAPACK `dgesv`: solve `A·x = b`.
    fn lu_solve(&self, _a: &[Vec<Scalar>], _b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        Err(SimError::runtime(
            "vendor backend does not accelerate lu_solve",
        ))
    }
    /// LAPACK `dpotrf`: Cholesky factor `A = L·Lᵀ`.
    fn cholesky(&self, _a: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
        Err(SimError::runtime(
            "vendor backend does not accelerate cholesky",
        ))
    }
    /// LAPACK `dgeqrf`: QR `A = Q·R`.
    fn qr(&self, _a: &[Vec<Scalar>]) -> Result<VendorQrResult, SimError> {
        Err(SimError::runtime("vendor backend does not accelerate qr"))
    }
}

/// Probe the filesystem for installed vendor BLAS libraries.
///
/// Searches `LD_LIBRARY_PATH` / `PATH` (and common install roots such as
/// `MKLROOT`, `CUBLAS_HOME`) for each vendor's shared library. This detects
/// *availability* without linking; a real binding then resolves the symbols
/// from [`BlasVendor::symbols`] and registers a [`VendorBlasBackend`].
pub fn detect_installed_vendors() -> Vec<BlasVendor> {
    let mut found = Vec::new();
    for vendor in [
        BlasVendor::Mkl,
        BlasVendor::Acml,
        BlasVendor::Essl,
        BlasVendor::Cublas,
    ] {
        if vendor.is_installed() {
            found.push(vendor);
        }
    }
    found
}

impl BlasVendor {
    /// Whether any of this vendor's shared libraries can be located on disk.
    pub fn is_installed(self) -> bool {
        self.known_library_names()
            .iter()
            .any(|lib| locate_library(lib).is_some())
    }
}

/// Search the dynamic-library search path and common roots for a library file.
fn locate_library(name: &str) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    for var in ["LD_LIBRARY_PATH", "PATH"] {
        if let Ok(paths) = std::env::var(var) {
            for dir in std::env::split_paths(&paths) {
                candidates.push(dir.join(name));
            }
        }
    }
    for root in ["MKLROOT", "ACML_DIR", "ESSL_HOME", "CUBLAS_HOME"] {
        if let Ok(dir) = std::env::var(root) {
            candidates.push(std::path::PathBuf::from(&dir).join("lib").join(name));
            candidates.push(std::path::PathBuf::from(&dir).join("lib64").join(name));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A deterministic vendor backend replicating the CPU math, used to
    /// exercise the adaptive vendor dispatch / fallback path in tests.
    pub(crate) struct MockVendor {
        pub vendor: BlasVendor,
        pub available: bool,
        /// Ops this backend does NOT accelerate (falls back to CPU).
        pub unsupported: Vec<&'static str>,
    }

    impl MockVendor {
        pub fn new(vendor: BlasVendor) -> Self {
            Self {
                vendor,
                available: true,
                unsupported: Vec::new(),
            }
        }
        pub fn with_unsupported(mut self, ops: &[&'static str]) -> Self {
            self.unsupported.extend_from_slice(ops);
            self
        }
        fn supports(&self, op: &str) -> bool {
            !self.unsupported.contains(&op)
        }
    }

    impl VendorBlasBackend for MockVendor {
        fn vendor(&self) -> BlasVendor {
            self.vendor
        }
        fn name(&self) -> &str {
            self.vendor.name()
        }
        fn is_available(&self) -> bool {
            self.available
        }
        fn gemm(&self, a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
            if self.supports("gemm") {
                crate::core::compute::matrix::mat_mul(a, b)
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn gemv(&self, a: &[Vec<Scalar>], x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
            if self.supports("gemv") {
                super::super::linalg::gemv(a, x)
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn axpy(&self, alpha: Scalar, x: &[Scalar], y: &mut [Scalar]) -> Result<(), SimError> {
            if self.supports("axpy") {
                for (yi, xi) in y.iter_mut().zip(x.iter()) {
                    *yi += alpha * xi;
                }
                Ok(())
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn dot(&self, a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SimError> {
            if self.supports("dot") {
                Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn sum(&self, a: &[Scalar]) -> Result<Scalar, SimError> {
            if self.supports("sum") {
                Ok(a.iter().sum())
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn scal(&self, alpha: Scalar, x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
            if self.supports("scal") {
                Ok(x.iter().map(|&v| alpha * v).collect())
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn nrm2(&self, x: &[Scalar]) -> Result<Scalar, SimError> {
            if self.supports("nrm2") {
                Ok(x.iter().map(|&v| v * v).sum::<Scalar>().sqrt())
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn asum(&self, x: &[Scalar]) -> Result<Scalar, SimError> {
            if self.supports("asum") {
                Ok(x.iter().map(|&v| v.abs()).sum())
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn iamax(&self, x: &[Scalar]) -> Result<usize, SimError> {
            if self.supports("iamax") {
                Ok(super::super::linalg::iamax(x))
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn lu_solve(&self, a: &[Vec<Scalar>], b: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
            if self.supports("lu_solve") {
                super::super::linalg::lu_solve_cpu(a, b)
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn cholesky(&self, a: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
            if self.supports("cholesky") {
                super::super::linalg::cholesky_cpu(a)
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
        fn qr(&self, a: &[Vec<Scalar>]) -> Result<VendorQrResult, SimError> {
            if self.supports("qr") {
                super::super::linalg::qr_cpu(a)
            } else {
                Err(SimError::runtime("unsupported"))
            }
        }
    }

    #[test]
    fn test_vendor_metadata() {
        assert_eq!(BlasVendor::Mkl.name(), "mkl");
        assert_eq!(BlasVendor::Acml.name(), "acml");
        assert_eq!(BlasVendor::Essl.name(), "essl");
        assert_eq!(BlasVendor::Cublas.name(), "cublas");
        assert!(!BlasVendor::Mkl.is_gpu());
        assert!(BlasVendor::Cublas.is_gpu());
        // Every vendor exposes at least its key symbols.
        for v in [
            BlasVendor::Mkl,
            BlasVendor::Acml,
            BlasVendor::Essl,
            BlasVendor::Cublas,
        ] {
            assert!(!v.symbols().is_empty());
            assert!(!v.known_library_names().is_empty());
        }
        // Adaptive ordering: MKL beats ACML/ESSL among CPU vendors.
        assert!(BlasVendor::Mkl.cpu_priority() < BlasVendor::Acml.cpu_priority());
        assert!(BlasVendor::Mkl.cpu_priority() < BlasVendor::Essl.cpu_priority());
    }

    #[test]
    fn test_vendor_symbols_known() {
        let mkl = BlasVendor::Mkl.symbols();
        assert!(mkl.contains(&"cblas_dgemm"));
        assert!(mkl.contains(&"dgesv_"));
        let cublas = BlasVendor::Cublas.symbols();
        assert!(cublas.contains(&"cublasDgemm_v2"));
    }

    #[test]
    fn test_detect_installed_is_stable() {
        // Runs without panicking regardless of what is installed.
        let found = detect_installed_vendors();
        assert!(found.len() <= 4);
    }
}
