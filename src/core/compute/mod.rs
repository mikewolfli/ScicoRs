//! Unified Computation Platform (Phase 30+).
//!
//! Provides a centralized set of high-performance numerical computing primitives
//! for the entire simulation kernel. All domain modules should use these
//! instead of implementing their own matrix/vector/integration operations.
//!
//! # Sub-modules
//!
//! - **`matrix`** — matrix multiply, inverse, determinant, transpose, LU/Cholesky decomposition
//! - **`linalg`** — numpy/MKL-style BLAS-1/2 + LAPACK (scal, nrm2, asum, iamax, gemv, LU, Cholesky, QR) with adaptive dispatch
//! - **`vendor_blas`** — vendor BLAS/LAPACK backends (MKL / ACML / ESSL / cuBLAS) with adaptive selection and CPU fallback
//! - **`vector`** — dot product, cross product, norm, normalization, linear/spline interpolation
//! - **`fft`** — base-2 Cooley-Tukey FFT for spectral analysis
//! - **`integration`** — numerical quadrature (trapezoidal, Simpson, Gauss-Legendre)
//! - **`backend`** — adaptive CPU (serial/parallel/vendor) + GPU dispatch for heavy primitives

#![allow(clippy::excessive_precision)]

pub mod backend;
pub mod eigen;
pub mod fft;
pub mod integration;
pub mod linalg;
pub mod matrix;
pub mod simd;
pub mod vector;
pub mod vendor_blas;
pub mod vendor_ffi;

pub use backend::*;
pub use eigen::*;
pub use fft::*;
pub use integration::*;
pub use linalg::*;
pub use matrix::*;
pub use vector::*;
pub use vendor_blas::*;
