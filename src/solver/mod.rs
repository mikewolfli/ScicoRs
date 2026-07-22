//! General Numerical Solver System
//!
//! A comprehensive suite of numerical solvers for simulation:
//!
//! - **ODE**: Fixed-step (Euler, RK4) and adaptive (RK45) solvers
//! - **Stiff**: Implicit methods (Backward Euler, Trapezoidal) for stiff systems
//! - **DAE**: Differential-algebraic equation solvers
//! - **Nonlinear**: Newton-Raphson and related nonlinear system solvers
//! - **Sparse**: Sparse matrix storage (CSR), CG iterative solver, power iteration

pub mod dae;
pub mod nonlinear;
pub mod ode;
pub mod sparse;
pub mod stiff;

pub use dae::{DaeBdf1Solver, DaeFunction, DaeSolver, DaeSolverConfig, DaeStepResult};
pub use nonlinear::{NewtonRaphsonSolver, NonlinearFunction, NonlinearSolverConfig, NonlinearSolveResult};
pub use ode::{EulerSolver, OdeFunction, OdeSolver, OdeSolverConfig, OdeStepResult, RK45Solver, RK4Solver};
pub use sparse::{ConjugateGradientSolver, PowerIterationSolver, SparseMatrix};
pub use stiff::{BackwardEulerSolver, TrapezoidalSolver};
