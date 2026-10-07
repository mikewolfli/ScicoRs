// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
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
/// Real GPU acceleration on wgpu 30.0.1 (optional `gpu` feature).
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod integration;
pub mod least_squares;
pub mod linalg;
pub mod matrix;
pub mod simd;
pub mod sparse;
pub mod vector;
pub mod vendor_blas;
pub mod vendor_ffi;

pub use backend::*;
pub use eigen::*;
pub use fft::*;
pub use integration::*;
pub use least_squares::*;
pub use linalg::*;
pub use matrix::*;
pub use vector::*;
pub use vendor_blas::*;

// `sparse` exports `dot`/`mat_mul`/`norm2`/`add`/`sub`/`scale`, which overlap with
// the dense `matrix`/`vector` globs. The ambiguous names are re-exported here
// under a `sparse_` prefix so both APIs stay reachable without a glob clash.
pub use sparse::{
    CooMatrix, CscMatrix, CsrMatrix, DiagonalScaling, IdentityPreconditioner, Ilu0Preconditioner,
    IterationStats, JacobiPreconditioner, KrylovConfig, KrylovSolution, LinearOperator,
    Preconditioner, SparseError, StopReason, axpy as sparse_axpy, bicgstab,
    bicgstab_unpreconditioned, cg, cg_unpreconditioned, condition_number_1norm, gmres,
    gmres_unpreconditioned, minres, relative_residual_norm, select, sparse_transpose,
};
pub use sparse::{
    add as sparse_add, dot as sparse_dot, mat_mul as sparse_mat_mul, norm2 as sparse_norm2,
    residual as sparse_residual, scale as sparse_scale, sub as sparse_sub,
};
