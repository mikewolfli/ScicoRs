//! Unified Computation Platform (Phase 30+).
//!
//! Provides a centralized set of high-performance numerical computing primitives
//! for the entire simulation kernel. All domain modules should use these
//! instead of implementing their own matrix/vector/integration operations.
//!
//! # Sub-modules
//!
//! - **`matrix`** — matrix multiply, inverse, determinant, transpose, LU/Cholesky decomposition
//! - **`vector`** — dot product, cross product, norm, normalization, linear/spline interpolation
//! - **`fft`** — base-2 Cooley-Tukey FFT for spectral analysis
//! - **`integration`** — numerical quadrature (trapezoidal, Simpson, Gauss-Legendre)

#![allow(clippy::excessive_precision)]

pub mod eigen;
pub mod fft;
pub mod integration;
pub mod matrix;
pub mod vector;

pub use eigen::*;
pub use fft::*;
pub use integration::*;
pub use matrix::*;
pub use vector::*;
