//! General Numerical Solver System (Phase 3)
//!
//! Provides a comprehensive suite of numerical solvers for the simulation kernel:
//!
//! - **Fixed-step solvers**: Euler (1st), RK4 (4th), Heun (2nd), Midpoint (2nd)
//! - **Adaptive solvers**: RK45 (5(4)), RK23 (3(2)), Cash-Karp (5(4))
//! - **Stiff solvers**: BackwardEuler (BDF1), Trapezoidal, BDF2
//! - **Nonlinear solver**: Newton-Raphson
//! - **DAE solver**: Index-1 backward Euler
//! - **Linear solvers**: Dense Gaussian elimination, sparse CSR helpers
//!
//! All solvers implement the `OdeSolver` trait defined in `traits.rs`.

pub mod adaptive;
pub mod dae;
pub mod fixed_step;
pub mod linear;
pub mod nonlinear;
pub mod stiff;
pub mod traits;

pub use adaptive::{CashKarp, RK23, RK45};
pub use dae::DaeSolver;
pub use fixed_step::{Euler, Heun, Midpoint, RK4, RK4_A, RK4_B, RK4_C};
pub use linear::{SparseMatrix, is_singular, matrix_inf_norm, solve_linear_dense, vector_inf_norm};
pub use nonlinear::NewtonRaphson;
pub use stiff::{BDF2, BackwardEuler, Trapezoidal};
pub use traits::{
    OdeSolver, SolverConfig, SolverStats, SolverStepResult, adapt_step_size, finite_diff_jacobian,
};
