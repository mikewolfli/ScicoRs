// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Sparse linear algebra (Phase 35).
//!
//! Provides validated COO/CSR/CSC storage, sparse arithmetic, Krylov iterative
//! solvers with explicit stopping reasons, and preconditioners. This module is
//! the CPU correctness baseline for large discretized systems; a domain module
//! provides its own assembled matrix and consumes these routines through the
//! [`iterative::LinearOperator`] interface.
//!
//! # Structure
//!
//! * [`matrix`] — COO/CSR/CSC types, validated constructors, conversions.
//! * [`operations`] — SpMV, sparse add/sub/mul, transpose, submatrix selection.
//! * [`iterative`] — CG, MINRES, GMRES, BiCGSTAB + matrix-free operators.
//! * [`preconditioner`] — Jacobi, diagonal scaling, ILU(0).
//! * [`diagnostics`] — iteration statistics, stop reasons, condition estimation.

pub mod diagnostics;
pub mod iterative;
pub mod matrix;
pub mod operations;
pub mod preconditioner;

pub use diagnostics::{IterationStats, StopReason, condition_number_1norm, relative_residual_norm};
pub use iterative::{
    KrylovConfig, KrylovSolution, LinearOperator, bicgstab, bicgstab_unpreconditioned, cg,
    cg_unpreconditioned, cg_with_scaling, gmres, gmres_unpreconditioned, minres,
};
pub use matrix::{CooMatrix, CscMatrix, CsrMatrix, SparseError};
pub use operations::transpose as sparse_transpose;
pub use operations::{add, axpy, dot, mat_mul, norm2, residual, scale, select, sub};
pub use preconditioner::{
    DiagonalScaling, IdentityPreconditioner, Ilu0Preconditioner, JacobiPreconditioner,
    Preconditioner,
};
