# BLUE3 — Phase 3: General Numerical Solver System

## 1. Overview

Phase 3 builds the **comprehensive numerical solver system** for the simulation kernel,
providing a unified solver trait and multiple algorithm implementations covering
fixed-step, adaptive, stiff, DAE, nonlinear, and linear system solvers.

**Status target:** 100% — compile, zero clippy warnings, all tests pass, no placeholder code,
fully integrated with the Phase 2 simulation engine.

---

## 2. Component Architecture

```
runtime/
  mod.rs              — Module interface (update: expose solver submodules)
  engine.rs           — Update: accept OdeSolver trait, replace hardcoded Euler
  solver/
    mod.rs            — Module interface + re-exports
    traits.rs         — [NEW] OdeSolver trait, OdeSolverConfig, SolverStats
    fixed_step.rs     — [NEW] Euler, RK4, Heun, Midpoint
    adaptive.rs       — [NEW] RK45, RK23, Cash-Karp
    stiff.rs          — [NEW] BDF (order 1-2), Trapezoidal, Backward Euler
    nonlinear.rs      — [NEW] Newton-Raphson solver
    dae.rs            — [NEW] Basic DAE solver (index-1)
    linear.rs         — [NEW] Linear system, basic sparse helpers
```

---

## 3. Detailed Specifications

### 3.1 `solver/traits.rs` — Core Solver Trait & Config

**Trait `OdeSolver`** — common interface for all ODE solvers:
```rust
pub trait OdeSolver: Send + Sync {
    /// Name of this solver method.
    fn name(&self) -> &str;

    /// Advance the system state `x` by one step of size `dt`.
    /// `f` is the ODE right-hand side: computes dx/dt given current state and time.
    fn step(
        &self,
        f: &mut dyn FnMut(&[Scalar], Time, &mut [Scalar]) -> Result<(), SimError>,
        x: &mut [Scalar],
        t: Time,
        dt: Scalar,
    ) -> Result<SolverStepResult, SimError>;

    /// Return the order of accuracy of this method.
    fn order(&self) -> u8;

    /// Return the number of internal stages per step.
    fn stages(&self) -> u8;

    /// Whether this solver supports adaptive step size control.
    fn is_adaptive(&self) -> bool;

    /// Estimate the error for the current step (only for adaptive methods).
    fn estimate_error(&self) -> Option<Scalar> {
        None
    }
}
```

**Struct `SolverConfig`** — configuration for adaptive solvers:
```rust
pub struct SolverConfig {
    pub rtol: Scalar,          // Relative tolerance (default 1e-6)
    pub atol: Scalar,          // Absolute tolerance (default 1e-12)
    pub max_step: Scalar,      // Maximum step size
    pub min_step: Scalar,      // Minimum step size
    pub max_iter: usize,       // Maximum iterations (for Newton, DAE)
    pub safety_factor: Scalar, // Step size safety factor (default 0.9)
}
```

**Enum `SolverStepResult`**:
```rust
pub enum SolverStepResult {
    Accepted,
    Rejected { suggested_dt: Scalar },
    Converged,
    NotConverged,
    Singular,
}
```

**Struct `SolverStats`** — solver statistics:
```rust
pub struct SolverStats {
    pub steps_accepted: u64,
    pub steps_rejected: u64,
    pub function_evals: u64,
    pub jacobian_evals: u64,
}
```

### 3.2 `solver/fixed_step.rs` — Fixed-Step Solvers

**Euler Method** (1st order, 1 stage):
```
x_{n+1} = x_n + dt * f(t_n, x_n)
```

**RK4 (Classical Runge-Kutta)** (4th order, 4 stages):
```
k1 = f(t_n, x_n)
k2 = f(t_n + dt/2, x_n + dt/2 * k1)
k3 = f(t_n + dt/2, x_n + dt/2 * k2)
k4 = f(t_n + dt, x_n + dt * k3)
x_{n+1} = x_n + dt/6 * (k1 + 2*k2 + 2*k3 + k4)
```

**Heun's Method** (2nd order, 2 stages):
```
k1 = f(t_n, x_n)
k2 = f(t_n + dt, x_n + dt * k1)
x_{n+1} = x_n + dt/2 * (k1 + k2)
```

**Midpoint Method** (2nd order, 2 stages):
```
k1 = f(t_n, x_n)
k2 = f(t_n + dt/2, x_n + dt/2 * k1)
x_{n+1} = x_n + dt * k2
```

### 3.3 `solver/adaptive.rs` — Adaptive Step Solvers

**Embedded Runge-Kutta Methods** use two orders of accuracy per step.
The difference gives an error estimate for step size control.

**RK45 (Dormand-Prince)** — 5th order with 4th order embedded estimate.
Butcher tableau for DOPRI5:
```
  0    |
  1/5  | 1/5
  3/10 | 3/40      9/40
  4/5  | 44/45     -56/15    32/9
  8/9  | 19372/6561 -25360/2187 64448/6561 -212/729
  1    | 9017/3168  -355/33   46732/5247 49/176   -5103/18656
  1    | 35/384     0         500/1113   125/192  -2187/6784 11/84
  -----------------------------------------------------------------
       | 35/384     0         500/1113   125/192  -2187/6784 11/84      (5th order)
       | 5179/57600 0         7571/16695 393/640  -92097/339200 187/2100 1/40  (4th order)
```

**RK23 (Bogacki-Shampine)** — 3rd order with 2nd order embedded.
```rust
pub struct RK23 { /* 4 stages, order 3(2) */ }
```

**Cash-Karp** — 5th order with 4th order embedded, 6 stages.
```rust
pub struct CashKarp { /* 6 stages, order 5(4) */ }
```

**Adaptive Step Control Algorithm:**
```rust
fn adapt_step(error: Scalar, dt: Scalar, rtol: Scalar, atol: Scalar,
              order: u8, safety: Scalar) -> Scalar {
    let scale = (error / (rtol + atol)).max(1e-10);
    let new_dt = dt * safety * scale.powf(-1.0 / (order as Scalar + 1.0));
    new_dt.clamp(min_step, max_step)
}
```

### 3.4 `solver/stiff.rs` — Stiff System Solvers

**Backward Euler (BDF1)** — implicit first order:
```
x_{n+1} = x_n + dt * f(t_{n+1}, x_{n+1})
```
Solved via Newton iteration at each step.

**Trapezoidal Rule** — implicit second order:
```
x_{n+1} = x_n + dt/2 * (f(t_n, x_n) + f(t_{n+1}, x_{n+1}))
```

**BDF2** — implicit second order backward differentiation:
```
x_{n+1} = 4/3*x_n - 1/3*x_{n-1} + 2/3*dt*f(t_{n+1}, x_{n+1})
```

### 3.5 `solver/nonlinear.rs` — Newton-Raphson Solver

Solves F(x) = 0 via Newton iteration:
```
J(x_k) * dx = -F(x_k)
x_{k+1} = x_k + dx
```

```rust
pub struct NewtonRaphson {
    config: SolverConfig,
    stats: SolverStats,
}

impl NewtonRaphson {
    pub fn new(config: SolverConfig) -> Self;
    pub fn solve(
        &mut self,
        f: &mut dyn FnMut(&[Scalar], &mut [Scalar]) -> Result<(), SimError>,
        jacobian: Option<&mut dyn FnMut(&[Scalar], &mut [Vec<Scalar>]) -> Result<(), SimError>>,
        x: &mut [Scalar],
    ) -> Result<SolverStepResult, SimError>;
}
```

### 3.6 `solver/dae.rs` — DAE Solver

Basic index-1 DAE solver using backward Euler discretization:
```
F(t, x, dx/dt) = 0
Backward Euler: dx/dt ≈ (x_{n+1} - x_n) / dt
F(t_{n+1}, x_{n+1}, (x_{n+1} - x_n)/dt) = 0 → solve via Newton
```

### 3.7 `solver/linear.rs` — Linear System Helpers

```rust
/// Solve A * x = b for a dense matrix using Gaussian elimination (LU).
pub fn solve_linear_dense(a: &[Vec<Scalar>], b: &[Scalar]) -> Result<Vec<Scalar>, SimError>

/// Check if a matrix is singular (for Newton convergence).
pub fn is_singular(a: &[Vec<Scalar>], tol: Scalar) -> bool

/// Compute the infinity norm of a matrix.
pub fn matrix_inf_norm(a: &[Vec<Scalar>]) -> Scalar

/// Simple sparse CSR matrix structure for future expansion.
pub struct SparseMatrix {
    pub rows: usize,
    pub cols: usize,
    pub row_ptr: Vec<usize>,
    pub col_ind: Vec<usize>,
    pub values: Vec<Scalar>,
}
```

---

## 4. Implementation Order

1. `solver/traits.rs` — Core trait, config, stats, step result
2. `solver/fixed_step.rs` — Euler, RK4, Heun, Midpoint (with tests)
3. `solver/adaptive.rs` — RK45, RK23, Cash-Karp (with tests)
4. `solver/stiff.rs` — Backward Euler, Trapezoidal, BDF2 (with tests)
5. `solver/nonlinear.rs` — Newton-Raphson (with tests)
6. `solver/linear.rs` — Linear system helpers (with tests)
7. `solver/dae.rs` — Basic DAE solver (with tests)
8. Update `solver/mod.rs` — Expose all public types
9. Update `runtime/mod.rs` — Expose solver module publicly
10. Update `engine.rs` — Accept `Box<dyn OdeSolver>`, use solver trait instead of hardcoded Euler
11. Comprehensive integration tests

---

## 5. Testing Requirements

### Fixed-step solver tests:
- Each solver method: step a simple ODE (dx/dt = -x, x(0)=1) and verify accuracy
- Compare RK4 result against analytical at t=1.0 (error < 1e-4 for dt=0.01)
- Euler accuracy is O(dt) — verify first-order convergence
- Multiple steps maintain stability

### Adaptive solver tests:
- RK45 can solve dx/dt = -x with automatic step size
- Error estimation returns reasonable values
- Step rejection works correctly
- Adaptive step handles stiff-ish problems

### Stiff solver tests:
- Backward Euler can solve a stiff problem (e.g., dx/dt = -1000*x)
- BDF2 achieves 2nd order convergence
- Newton iteration converges for linear problems

### Nonlinear solver tests:
- Newton-Raphson solves a simple quadratic
- Convergence failure returns NotConverged
- Jacobian-free mode works (finite difference approximation)

### DAE solver tests:
- Index-1 linear DAE: y' = z, 0 = y - sin(t)
- Solver produces consistent solution

### Linear solver tests:
- 2x2 system solves correctly
- 3x3 system produces correct inverse-equivalent
- Singular matrix detection works

### Integration tests:
- Engine uses OdeSolver trait with RK4
- Engine uses OdeSolver trait with Euler
- Solver swap at runtime produces different accuracy results

---

## 6. Acceptance Criteria

- [ ] `cargo build` succeeds
- [ ] `cargo test` — all tests pass (no failures, no ignored)
- [ ] `cargo clippy --all-targets -- -D warnings` — zero warnings
- [ ] No `todo!()`, `unimplemented!()`, or empty function bodies
- [ ] All new code has English comments
- [ ] `solver/mod.rs` only exposes interfaces
- [ ] Every solver has at least 2 tests (creation + accuracy)
- [ ] Solver trait properly integrated with engine (replace hardcoded Euler)
- [ ] Fixed-step solvers achieve rated order of accuracy on test problems
- [ ] Adaptive solvers can control step size based on error estimate
- [ ] Newton-Raphson converges quadratically on smooth problems
- [ ] Linear system solver handles dense matrices correctly
- [ ] DAE solver handles index-1 problems

---

## 7. Integration with Phase 2

Phase 3 builds on Phase 2 infrastructure:
- `ContinuousState` — state vector managed by solver
- `SimEngine` — orchestrator that drives solver
- `TimeConfig` — step size bounds for adaptive control
- `SimError` — error propagation
- `Scalar` — `f64` uniform type

The engine will gain a new field:
```rust
pub struct SimEngine {
    // ... existing fields ...
    pub solver: Box<dyn OdeSolver>,
}
```

The `step()` method will call `self.solver.step(...)` instead of
`self.state.continuous.integrate_euler(self.context.dt)`.
