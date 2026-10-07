// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Krylov iterative linear solvers: CG, MINRES, GMRES (restarted) and BiCGSTAB.
//!
//! Each solver matches a problem class:
//!
//! * [`cg`] — symmetric positive-definite systems. Breaks down on indefinite
//!   matrices; this is reported as a [`StopReason::Breakdown`].
//! * [`minres`] — symmetric indefinite systems.
//! * [`gmres`] — general non-symmetric systems, with a restarted variant.
//! * [`bicgstab`] — general non-symmetric systems, low fixed memory per step.
//!
//! Wrong-algorithm/input combinations (e.g. CG on an asymmetric matrix) return a
//! specific [`SparseError::Unsupported`] instead of silently switching methods.
//! Reaching the iteration cap always reports [`StopReason::MaxIterationsReached`]
//! together with the (non-converged) iterate; it is never labelled success.

use super::diagnostics::{IterationStats, StopReason, summarize};
use super::matrix::{CsrMatrix, SparseError};
use super::operations::{dot, norm2, residual};
use super::preconditioner::{IdentityPreconditioner, Preconditioner};
use crate::core::types::Scalar;

/// Configuration shared by all Krylov solvers.
#[derive(Debug, Clone, PartialEq)]
pub struct KrylovConfig {
    /// Relative residual tolerance on `‖b − A·x‖ / ‖b‖`.
    pub rtol: Scalar,
    /// Absolute residual tolerance (guards the `b = 0` case).
    pub atol: Scalar,
    /// Maximum number of iterations (outer iterations for GMRES).
    pub max_iter: usize,
    /// Restart length for GMRES (`m`); ignored by other solvers.
    pub restart: usize,
    /// Optional stagnation window: stop if the residual does not improve by
    /// `1e-12` relative over this many iterations. `0` disables the check.
    pub stagnation_window: usize,
    /// Optional initial guess. Must have length `n` when provided.
    pub initial_guess: Option<Vec<Scalar>>,
}

impl Default for KrylovConfig {
    fn default() -> Self {
        Self {
            rtol: 1e-10,
            atol: 1e-14,
            max_iter: 1000,
            restart: 30,
            stagnation_window: 0,
            initial_guess: None,
        }
    }
}

impl KrylovConfig {
    /// Validate tolerances and iteration caps. Rejects zero/negative tolerances
    /// and zero iteration limits.
    pub fn validate(&self) -> Result<(), SparseError> {
        if !(self.rtol.is_finite() && self.rtol > 0.0) {
            return Err(SparseError::Unsupported {
                detail: format!("invalid rtol: {}", self.rtol),
            });
        }
        if !(self.atol.is_finite() && self.atol >= 0.0) {
            return Err(SparseError::Unsupported {
                detail: format!("invalid atol: {}", self.atol),
            });
        }
        if self.max_iter == 0 {
            return Err(SparseError::Unsupported {
                detail: "max_iter must be >= 1".to_string(),
            });
        }
        if self.restart == 0 {
            return Err(SparseError::Unsupported {
                detail: "restart must be >= 1".to_string(),
            });
        }
        Ok(())
    }
}

/// The outcome of a Krylov solve: the solution vector plus convergence stats.
#[derive(Debug, Clone)]
pub struct KrylovSolution {
    /// The computed solution (valid even when not converged, but then it is only
    /// the best iterate, not a certified answer).
    pub x: Vec<Scalar>,
    /// Convergence statistics and stop reason.
    pub stats: IterationStats,
}

impl KrylovSolution {
    /// Whether the solve met its tolerance.
    pub fn converged(&self) -> bool {
        self.stats.converged()
    }
}

fn scaled_tolerance(cfg: &KrylovConfig, bnorm: Scalar) -> Scalar {
    cfg.atol.max(cfg.rtol * bnorm)
}

fn check_system(a: &CsrMatrix, b: &[Scalar]) -> Result<usize, SparseError> {
    if !a.is_square() {
        return Err(SparseError::Unsupported {
            detail: format!(
                "Krylov solvers require a square matrix, got {}x{}",
                a.nrows(),
                a.ncols()
            ),
        });
    }
    if b.len() != a.nrows() {
        return Err(SparseError::ShapeMismatch {
            detail: format!("rhs length {} != nrows {}", b.len(), a.nrows()),
        });
    }
    Ok(a.nrows())
}

fn initial_solution(cfg: &KrylovConfig, n: usize) -> Result<Vec<Scalar>, SparseError> {
    match &cfg.initial_guess {
        Some(x0) => {
            if x0.len() != n {
                return Err(SparseError::ShapeMismatch {
                    detail: format!("initial_guess length {} != n {}", x0.len(), n),
                });
            }
            Ok(x0.clone())
        }
        None => Ok(vec![0.0; n]),
    }
}

/// Solve `A·x = b` for symmetric positive-definite `A` using conjugate gradient.
///
/// Transpositionally-based: uses `A` directly and requires symmetry. An
/// asymmetric matrix is rejected before iterating.
pub fn cg<P: Preconditioner>(
    a: &CsrMatrix,
    b: &[Scalar],
    precond: &P,
    cfg: &KrylovConfig,
) -> Result<KrylovSolution, SparseError> {
    let n = check_system(a, b)?;
    cfg.validate()?;
    if !a.is_symmetric(1e-10) {
        return Err(SparseError::Unsupported {
            detail: "cg requires a symmetric matrix; use gmres or bicgstab for asymmetric systems"
                .to_string(),
        });
    }
    let mut x = initial_solution(cfg, n)?;
    let bnorm = norm2(b);
    let tol = scaled_tolerance(cfg, bnorm);

    let mut r = residual(a, &x, b)?;
    let mut history = vec![norm2(&r)];
    if history[0] <= tol {
        return Ok(KrylovSolution {
            x,
            stats: summarize(history, tol, StopReason::Converged),
        });
    }
    let mut z = vec![0.0; n];
    precond.apply(&r, &mut z)?;
    let mut p = z.clone();
    let mut rz = dot(&r, &z)?;
    let mut best_rel = history[0];
    let mut stagnant = 0usize;

    for _ in 0..cfg.max_iter {
        let ap = a.matvec(&p)?;
        let pap = dot(&p, &ap)?;
        if !pap.is_finite() || pap <= 0.0 {
            // Loss of positive-definiteness or breakdown.
            let mut st = summarize(history, tol, StopReason::Breakdown);
            st.final_residual = best_rel;
            return Ok(KrylovSolution { x, stats: st });
        }
        let alpha = rz / pap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rnorm = norm2(&r);
        history.push(rnorm);
        if rnorm <= tol {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Converged),
            });
        }
        // Stagnation tracking.
        if rnorm < best_rel * (1.0 - 1e-12) {
            best_rel = rnorm;
            stagnant = 0;
        } else {
            stagnant += 1;
            if cfg.stagnation_window > 0 && stagnant >= cfg.stagnation_window {
                return Ok(KrylovSolution {
                    x,
                    stats: summarize(history, tol, StopReason::Stagnation),
                });
            }
        }
        precond.apply(&r, &mut z)?;
        let rz_new = dot(&r, &z)?;
        let beta = rz_new / rz;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
        rz = rz_new;
    }
    Ok(KrylovSolution {
        x,
        stats: summarize(history, tol, StopReason::MaxIterationsReached),
    })
}

/// Solve `A·x = b` for symmetric indefinite `A` using MINRES.
///
/// MINRES minimizes the residual 2-norm over the Krylov subspace and, unlike CG,
/// tolerates negative eigenvalues. The preconditioner `M` is applied as `M⁻¹A`
/// and should be symmetric positive-definite for the Lanczos recurrence to stay
/// well-defined; the identity preconditioner gives standard MINRES.
pub fn minres<P: Preconditioner>(
    a: &CsrMatrix,
    b: &[Scalar],
    precond: &P,
    cfg: &KrylovConfig,
) -> Result<KrylovSolution, SparseError> {
    let n = check_system(a, b)?;
    cfg.validate()?;
    if !a.is_symmetric(1e-10) {
        return Err(SparseError::Unsupported {
            detail:
                "minres requires a symmetric matrix; use gmres or bicgstab for asymmetric systems"
                    .to_string(),
        });
    }

    // Standard MINRES on the preconditioned system (A' = M⁻¹A, b' = M⁻¹b),
    // following the Paige–Saunders recurrence: each Lanczos step is
    //   r = A' v_k − (β_k/β_{k-1}) r_{k-1} − α_k v_k,  β_{k+1} = ‖r‖.
    // With the identity preconditioner, A' = A and b' = b.
    let mut x = initial_solution(cfg, n)?;
    let bnorm = norm2(b);
    let tol = scaled_tolerance(cfg, bnorm);
    let mut history = vec![norm2(&residual(a, &x, b)?)];
    if history[0] <= tol {
        return Ok(KrylovSolution {
            x,
            stats: summarize(history, tol, StopReason::Converged),
        });
    }
    let mut rhs = vec![0.0; n];
    precond.apply(&residual(a, &x, b)?, &mut rhs)?;
    let beta1 = norm2(&rhs);
    if beta1 == 0.0 {
        return Ok(KrylovSolution {
            x,
            stats: summarize(history, tol, StopReason::Converged),
        });
    }

    let mut r = rhs; // current Lanczos residual
    let mut r_old = vec![0.0; n]; // r_{k-1}
    let mut oldb = 0.0 as Scalar; // β_{k-1}
    let mut beta = beta1; // β_k
    let mut dbar = 0.0 as Scalar;
    let mut epsln = 0.0 as Scalar;
    let mut phibar = beta1;
    let mut cs = -1.0 as Scalar;
    let mut sn = 0.0 as Scalar;
    let mut w = vec![0.0; n];
    let mut w2 = vec![0.0; n];

    let mut best_rel = history[0];
    let mut stagnant = 0usize;

    for _ in 0..cfg.max_iter {
        // v_k = r_k / β_k.
        let v: Vec<Scalar> = r.iter().map(|&ri| ri / beta).collect();
        // y = A' v_k.
        let av = a.matvec(&v)?;
        let mut y = vec![0.0; n];
        precond.apply(&av, &mut y)?;
        // y -= (β_k/β_{k-1}) r_{k-1}.
        if oldb != 0.0 {
            let f = beta / oldb;
            for i in 0..n {
                y[i] -= f * r_old[i];
            }
        }
        let alfa = dot(&v, &y)?;
        // y = A' v_k − (β/β_old) r_old − α_k v_k   (= r_{k+1} raw).
        for i in 0..n {
            y[i] -= (alfa / beta) * r[i];
        }
        r_old = std::mem::take(&mut r);
        r = y;
        oldb = beta;
        beta = norm2(&r);

        // QR (Givens) recurrence from Paige–Saunders.
        let oldeps = epsln;
        let delta = cs * dbar + sn * alfa;
        let gbar = sn * dbar - cs * alfa;
        epsln = sn * beta;
        dbar = -cs * beta;
        let gamma = (gbar * gbar + beta * beta).sqrt();
        if gamma == 0.0 {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Breakdown),
            });
        }
        cs = gbar / gamma;
        sn = beta / gamma;
        let phi = cs * phibar;
        phibar *= sn;
        let res_norm = phibar.abs();

        // Direction update: w_k = (v_k − oldeps·w_{k-2} − delta·w_{k-1}) / γ.
        // At loop entry `w2` holds w_{k-2} and `w` holds w_{k-1}.
        let w_new: Vec<Scalar> = (0..n)
            .map(|i| (v[i] - oldeps * w2[i] - delta * w[i]) / gamma)
            .collect();
        for i in 0..n {
            x[i] += phi * w_new[i];
        }

        history.push(res_norm);
        if res_norm <= tol {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Converged),
            });
        }
        if res_norm < best_rel * (1.0 - 1e-12) {
            best_rel = res_norm;
            stagnant = 0;
        } else {
            stagnant += 1;
            if cfg.stagnation_window > 0 && stagnant >= cfg.stagnation_window {
                return Ok(KrylovSolution {
                    x,
                    stats: summarize(history, tol, StopReason::Stagnation),
                });
            }
        }

        if beta == 0.0 {
            // Invariant subspace reached; the (preconditioned) solve is exact.
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Converged),
            });
        }
        // Rotate the direction history: w2 ← w (becomes w_{k-1}), w ← w_new (w_k).
        w2 = std::mem::replace(&mut w, w_new);
    }
    Ok(KrylovSolution {
        x,
        stats: summarize(history, tol, StopReason::MaxIterationsReached),
    })
}

/// Solve `A·x = b` for general (non-symmetric) `A` using restarted GMRES(m).
pub fn gmres<P: Preconditioner>(
    a: &CsrMatrix,
    b: &[Scalar],
    precond: &P,
    cfg: &KrylovConfig,
) -> Result<KrylovSolution, SparseError> {
    let n = check_system(a, b)?;
    cfg.validate()?;
    let m = cfg.restart.min(n);
    let mut x = initial_solution(cfg, n)?;
    let bnorm = norm2(b);
    let tol = scaled_tolerance(cfg, bnorm);

    let mut r = residual(a, &x, b)?;
    let mut history = vec![norm2(&r)];
    if history[0] <= tol {
        return Ok(KrylovSolution {
            x,
            stats: summarize(history, tol, StopReason::Converged),
        });
    }

    let mut total_iters = 0usize;
    let mut best_rel = history[0];
    let mut stagnant = 0usize;

    while total_iters < cfg.max_iter {
        let budget = (cfg.max_iter - total_iters).min(m);
        // Arnoldi basis V (n × (budget+1)), stored column-major as Vec<Vec>.
        let mut basis: Vec<Vec<Scalar>> = Vec::with_capacity(budget + 1);
        // Hessenberg H stored as (budget+1) × budget, row-major.
        let mut h = vec![vec![0.0; budget]; budget + 1];
        let mut g = vec![0.0; budget + 1];
        let mut cs = vec![0.0; budget];
        let mut sn = vec![0.0; budget];

        let mut w = vec![0.0; n];
        precond.apply(&r, &mut w)?;
        let beta = norm2(&w);
        if beta == 0.0 {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Converged),
            });
        }
        basis.push(w.iter().map(|&wi| wi / beta).collect());
        g[0] = beta;

        let mut j = 0;
        while j < budget {
            // w = M⁻¹ A v_j
            let av = a.matvec(&basis[j])?;
            let mut w = vec![0.0; n];
            precond.apply(&av, &mut w)?;
            // Modified Gram–Schmidt with reorthogonalization for stability.
            for _ in 0..2 {
                for (i, vi) in basis.iter().take(j + 1).enumerate() {
                    let hij = dot(&w, vi)?;
                    h[i][j] += hij;
                    for k in 0..n {
                        w[k] -= hij * vi[k];
                    }
                }
            }
            let h_next = norm2(&w);
            h[j + 1][j] = h_next;
            basis.push(if h_next > 0.0 {
                w.iter().map(|&wi| wi / h_next).collect()
            } else {
                vec![0.0; n]
            });

            // Apply Givens rotations to the new column.
            for i in 0..j {
                let temp = cs[i] * h[i][j] + sn[i] * h[i + 1][j];
                h[i + 1][j] = -sn[i] * h[i][j] + cs[i] * h[i + 1][j];
                h[i][j] = temp;
            }
            let denom = (h[j][j] * h[j][j] + h[j + 1][j] * h[j + 1][j]).sqrt();
            if denom == 0.0 {
                return Ok(KrylovSolution {
                    x,
                    stats: summarize(history, tol, StopReason::Breakdown),
                });
            }
            cs[j] = h[j][j] / denom;
            sn[j] = h[j + 1][j] / denom;
            h[j][j] = cs[j] * h[j][j] + sn[j] * h[j + 1][j];
            h[j + 1][j] = 0.0;
            let gj = g[j];
            g[j] = cs[j] * gj;
            g[j + 1] = -sn[j] * gj;

            j += 1;
            total_iters += 1;
            let rnorm = g[j].abs();
            history.push(rnorm);
            if rnorm <= tol {
                // Solve the triangular system and update x.
                let y = back_substitution(&h, &g, j);
                for k in 0..j {
                    for i in 0..n {
                        x[i] += y[k] * basis[k][i];
                    }
                }
                return Ok(KrylovSolution {
                    x,
                    stats: summarize(history, tol, StopReason::Converged),
                });
            }
            if rnorm < best_rel * (1.0 - 1e-12) {
                best_rel = rnorm;
                stagnant = 0;
            } else {
                stagnant += 1;
                if cfg.stagnation_window > 0 && stagnant >= cfg.stagnation_window {
                    let y = back_substitution(&h, &g, j);
                    for k in 0..j {
                        for i in 0..n {
                            x[i] += y[k] * basis[k][i];
                        }
                    }
                    return Ok(KrylovSolution {
                        x,
                        stats: summarize(history, tol, StopReason::Stagnation),
                    });
                }
            }
            if total_iters >= cfg.max_iter {
                break;
            }
        }
        // Restart: apply the current cycle's update and recompute the residual.
        let y = back_substitution(&h, &g, j);
        for k in 0..j {
            for i in 0..n {
                x[i] += y[k] * basis[k][i];
            }
        }
        r = residual(a, &x, b)?;
        let rnorm = norm2(&r);
        let last = history.last().copied().unwrap_or(rnorm);
        if (rnorm - last).abs() > 1e-14 * rnorm.max(1.0) {
            history.push(rnorm);
        }
        if rnorm <= tol {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Converged),
            });
        }
    }
    Ok(KrylovSolution {
        x,
        stats: summarize(history, tol, StopReason::MaxIterationsReached),
    })
}

/// Solve the upper-triangular system `H[0..k][0..k] · y = g[0..k]`.
fn back_substitution(h: &[Vec<Scalar>], g: &[Scalar], k: usize) -> Vec<Scalar> {
    let mut y = vec![0.0; k];
    for i in (0..k).rev() {
        let mut s = g[i];
        for j in (i + 1)..k {
            s -= h[i][j] * y[j];
        }
        y[i] = if h[i][i] != 0.0 { s / h[i][i] } else { 0.0 };
    }
    y
}

/// Solve `A·x = b` for general `A` using BiCGSTAB (stabilized bi-conjugate
/// gradient), which uses fixed memory per iteration.
pub fn bicgstab<P: Preconditioner>(
    a: &CsrMatrix,
    b: &[Scalar],
    precond: &P,
    cfg: &KrylovConfig,
) -> Result<KrylovSolution, SparseError> {
    let n = check_system(a, b)?;
    cfg.validate()?;
    let mut x = initial_solution(cfg, n)?;
    let bnorm = norm2(b);
    let tol = scaled_tolerance(cfg, bnorm);

    let mut r = residual(a, &x, b)?;
    let mut history = vec![norm2(&r)];
    if history[0] <= tol {
        return Ok(KrylovSolution {
            x,
            stats: summarize(history, tol, StopReason::Converged),
        });
    }
    let r_hat = r.clone();
    let mut rho_old = 1.0;
    let mut alpha = 1.0;
    let mut omega = 1.0;
    let mut v = vec![0.0; n];
    let mut p = vec![0.0; n];

    let mut best_rel = history[0];
    let mut stagnant = 0usize;

    for _ in 0..cfg.max_iter {
        let rho = dot(&r_hat, &r)?;
        if rho.abs() < 1e-300 {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Breakdown),
            });
        }
        let beta = (rho / rho_old) * (alpha / omega);
        for i in 0..n {
            p[i] = r[i] + beta * (p[i] - omega * v[i]);
        }
        let mut phat = vec![0.0; n];
        precond.apply(&p, &mut phat)?;
        let v_new = a.matvec(&phat)?;
        let denom = dot(&r_hat, &v_new)?;
        if denom.abs() < 1e-300 {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Breakdown),
            });
        }
        alpha = rho / denom;
        let mut s = vec![0.0; n];
        for i in 0..n {
            s[i] = r[i] - alpha * v_new[i];
        }
        if norm2(&s) <= tol {
            for i in 0..n {
                x[i] += alpha * phat[i];
            }
            history.push(norm2(&s));
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Converged),
            });
        }
        let mut shat = vec![0.0; n];
        precond.apply(&s, &mut shat)?;
        let t = a.matvec(&shat)?;
        let tt = dot(&t, &t)?;
        if tt.abs() < 1e-300 {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Breakdown),
            });
        }
        omega = dot(&t, &s)? / tt;
        if omega.abs() < 1e-300 {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Breakdown),
            });
        }
        for i in 0..n {
            x[i] += alpha * phat[i] + omega * shat[i];
            r[i] = s[i] - omega * t[i];
        }
        let rnorm = norm2(&r);
        history.push(rnorm);
        if rnorm <= tol {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Converged),
            });
        }
        if rnorm < best_rel * (1.0 - 1e-12) {
            best_rel = rnorm;
            stagnant = 0;
        } else {
            stagnant += 1;
            if cfg.stagnation_window > 0 && stagnant >= cfg.stagnation_window {
                return Ok(KrylovSolution {
                    x,
                    stats: summarize(history, tol, StopReason::Stagnation),
                });
            }
        }
        if omega.abs() < 1e-300 {
            return Ok(KrylovSolution {
                x,
                stats: summarize(history, tol, StopReason::Breakdown),
            });
        }
        rho_old = rho;
        v = t;
    }
    Ok(KrylovSolution {
        x,
        stats: summarize(history, tol, StopReason::MaxIterationsReached),
    })
}

/// Convenience wrapper: CG without preconditioning.
pub fn cg_unpreconditioned(
    a: &CsrMatrix,
    b: &[Scalar],
    cfg: &KrylovConfig,
) -> Result<KrylovSolution, SparseError> {
    cg(a, b, &IdentityPreconditioner::new(a.nrows()), cfg)
}

/// Convenience wrapper: GMRES without preconditioning.
pub fn gmres_unpreconditioned(
    a: &CsrMatrix,
    b: &[Scalar],
    cfg: &KrylovConfig,
) -> Result<KrylovSolution, SparseError> {
    gmres(a, b, &IdentityPreconditioner::new(a.nrows()), cfg)
}

/// Convenience wrapper: BiCGSTAB without preconditioning.
pub fn bicgstab_unpreconditioned(
    a: &CsrMatrix,
    b: &[Scalar],
    cfg: &KrylovConfig,
) -> Result<KrylovSolution, SparseError> {
    bicgstab(a, b, &IdentityPreconditioner::new(a.nrows()), cfg)
}

/// A matrix-free linear operator interface, letting Krylov solvers consume a
/// user-provided `apply`/`apply_transpose` pair without a stored matrix.
///
/// This mirrors the [`CsrMatrix`] operations so that finite element / finite
/// volume codes can avoid assembling a global matrix.
pub trait LinearOperator {
    /// Dimension of the (square) operator.
    fn dim(&self) -> usize;
    /// Apply the operator: `y = A · x`.
    fn apply(&self, x: &[Scalar]) -> Result<Vec<Scalar>, SparseError>;
    /// Apply the transpose: `y = Aᵀ · x`. Defaults to an error when unsupported.
    fn apply_transpose(&self, _x: &[Scalar]) -> Result<Vec<Scalar>, SparseError> {
        Err(SparseError::Unsupported {
            detail: "transpose apply not implemented for this operator".to_string(),
        })
    }
    /// Optional diagonal (or block-diagonal) approximation for preconditioning.
    fn diagonal(&self) -> Option<Vec<Scalar>> {
        None
    }
}

impl LinearOperator for CsrMatrix {
    fn dim(&self) -> usize {
        self.nrows()
    }

    fn apply(&self, x: &[Scalar]) -> Result<Vec<Scalar>, SparseError> {
        self.matvec(x)
    }

    fn apply_transpose(&self, x: &[Scalar]) -> Result<Vec<Scalar>, SparseError> {
        self.matvec_transpose(x)
    }

    fn diagonal(&self) -> Option<Vec<Scalar>> {
        Some(self.diagonal())
    }
}

/// Solve `A·x = b` with optional row diagonal equilibration followed by a CG
/// solve on the scaled system.
///
/// When `equilibrate` is true, the system is row-scaled so each row has unit
/// infinity norm, which improves conditioning for badly scaled problems. The
/// reported residual is always that of the **original** system (`‖b − A·x‖`),
/// never the scaled one — the scaled residual can be much smaller and would
/// otherwise overstate accuracy.
pub fn cg_with_scaling(
    a: &CsrMatrix,
    b: &[Scalar],
    equilibrate: bool,
    cfg: &KrylovConfig,
) -> Result<KrylovSolution, SparseError> {
    if equilibrate {
        // Symmetric equilibration preserves symmetry, so CG remains valid.
        let scaling = super::preconditioner::DiagonalScaling::symmetric_equilibrate(a);
        let b_scaled = scaling.scale_rhs_symmetric(b);
        let inner = cg_unpreconditioned(&scaling.scaled, &b_scaled, cfg)?;
        // Recover the solution of the original system: x = D·x'.
        let x = scaling.unscale_solution_symmetric(&inner.x);
        // Recompute the residual against the original system so convergence is
        // reported honestly in the original metric.
        let r = residual(a, &x, b)?;
        let bnorm = norm2(b);
        let tol = cfg.atol.max(cfg.rtol * bnorm);
        let mut history = inner.stats.residual_history;
        let true_res = norm2(&r);
        history.push(true_res);
        let reason = if true_res <= tol {
            StopReason::Converged
        } else {
            inner.stats.reason
        };
        Ok(KrylovSolution {
            x,
            stats: summarize(history, tol, reason),
        })
    } else {
        cg_unpreconditioned(a, b, cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compute::sparse::matrix::CooMatrix;
    use crate::core::compute::sparse::preconditioner::{Ilu0Preconditioner, JacobiPreconditioner};

    fn tridiag(n: usize, diag: Scalar, off: Scalar) -> CsrMatrix {
        let mut coo = CooMatrix::new(n, n);
        for i in 0..n {
            coo.push(i, i, diag).unwrap();
            if i + 1 < n {
                coo.push(i, i + 1, off).unwrap();
                coo.push(i + 1, i, off).unwrap();
            }
        }
        coo.to_csr()
    }

    fn exact_solution(a: &CsrMatrix, x_true: &[Scalar]) -> Vec<Scalar> {
        a.matvec(x_true).unwrap()
    }

    #[test]
    fn cg_solves_spd_system() {
        let a = tridiag(20, 4.0, -1.0);
        let x_true: Vec<Scalar> = (0..20).map(|i| (i as Scalar) * 0.1).collect();
        let b = exact_solution(&a, &x_true);
        let cfg = KrylovConfig {
            rtol: 1e-10,
            max_iter: 500,
            ..Default::default()
        };
        let sol = cg_unpreconditioned(&a, &b, &cfg).unwrap();
        assert!(sol.converged(), "reason={:?}", sol.stats.reason);
        for (xi, xt) in sol.x.iter().zip(x_true.iter()) {
            assert!((xi - xt).abs() < 1e-7);
        }
    }

    #[test]
    fn cg_rejects_asymmetric() {
        let mut coo = CooMatrix::new(2, 2);
        coo.push(0, 0, 1.0).unwrap();
        coo.push(0, 1, 5.0).unwrap();
        coo.push(1, 0, 0.0).unwrap();
        coo.push(1, 1, 1.0).unwrap();
        let a = coo.to_csr();
        let cfg = KrylovConfig::default();
        assert!(matches!(
            cg_unpreconditioned(&a, &[1.0, 1.0], &cfg),
            Err(SparseError::Unsupported { .. })
        ));
    }

    #[test]
    fn gmres_solves_asymmetric() {
        let mut coo = CooMatrix::new(3, 3);
        // Diagonally dominant but asymmetric.
        coo.push(0, 0, 4.0).unwrap();
        coo.push(0, 1, 1.0).unwrap();
        coo.push(1, 0, -1.0).unwrap();
        coo.push(1, 1, 4.0).unwrap();
        coo.push(1, 2, 1.0).unwrap();
        coo.push(2, 1, -1.0).unwrap();
        coo.push(2, 2, 4.0).unwrap();
        let a = coo.to_csr();
        let x_true = vec![1.0, 2.0, 3.0];
        let b = exact_solution(&a, &x_true);
        let cfg = KrylovConfig {
            rtol: 1e-12,
            max_iter: 100,
            restart: 3,
            ..Default::default()
        };
        let sol = gmres_unpreconditioned(&a, &b, &cfg).unwrap();
        assert!(sol.converged(), "reason={:?}", sol.stats.reason);
        for (xi, xt) in sol.x.iter().zip(x_true.iter()) {
            assert!((xi - xt).abs() < 1e-8, "got {xi} want {xt}");
        }
    }

    #[test]
    fn bicgstab_solves_asymmetric() {
        let mut coo = CooMatrix::new(4, 4);
        for i in 0..4 {
            coo.push(i, i, 5.0).unwrap();
        }
        coo.push(0, 3, 1.0).unwrap();
        coo.push(1, 0, 2.0).unwrap();
        coo.push(2, 1, -1.0).unwrap();
        coo.push(3, 2, 1.5).unwrap();
        let a = coo.to_csr();
        let x_true = vec![1.0, -1.0, 2.0, 0.5];
        let b = exact_solution(&a, &x_true);
        let cfg = KrylovConfig {
            rtol: 1e-12,
            max_iter: 200,
            ..Default::default()
        };
        let sol = bicgstab_unpreconditioned(&a, &b, &cfg).unwrap();
        assert!(sol.converged(), "reason={:?}", sol.stats.reason);
        for (xi, xt) in sol.x.iter().zip(x_true.iter()) {
            assert!((xi - xt).abs() < 1e-8);
        }
    }

    #[test]
    fn minres_solves_symmetric_indefinite() {
        // diag(1, -1, 2, -2) is symmetric indefinite.
        let mut coo = CooMatrix::new(4, 4);
        coo.push(0, 0, 1.0).unwrap();
        coo.push(1, 1, -1.0).unwrap();
        coo.push(2, 2, 2.0).unwrap();
        coo.push(3, 3, -2.0).unwrap();
        let a = coo.to_csr();
        let x_true = vec![1.0, 2.0, 3.0, 4.0];
        let b = exact_solution(&a, &x_true);
        let cfg = KrylovConfig {
            rtol: 1e-12,
            max_iter: 100,
            ..Default::default()
        };
        let sol = minres(&a, &b, &IdentityPreconditioner::new(4), &cfg).unwrap();
        assert!(
            sol.converged(),
            "reason={:?} final={}",
            sol.stats.reason,
            sol.stats.final_residual
        );
        for (xi, xt) in sol.x.iter().zip(x_true.iter()) {
            assert!((xi - xt).abs() < 1e-8, "got {xi} want {xt}");
        }
    }

    #[test]
    fn max_iterations_is_not_convergence() {
        // Tiny iteration budget on a system needing many steps.
        let a = tridiag(50, 4.0, -1.0);
        let b: Vec<Scalar> = (0..50).map(|i| 1.0 / (i as Scalar + 1.0)).collect();
        let cfg = KrylovConfig {
            rtol: 1e-14,
            max_iter: 2,
            ..Default::default()
        };
        let sol = cg_unpreconditioned(&a, &b, &cfg).unwrap();
        assert!(!sol.converged());
        assert_eq!(sol.stats.reason, StopReason::MaxIterationsReached);
    }

    #[test]
    fn precon_does_not_change_solution() {
        let a = tridiag(30, 4.0, -1.0);
        let x_true: Vec<Scalar> = (0..30).map(|i| ((i as Scalar) * 0.2).sin()).collect();
        let b = exact_solution(&a, &x_true);
        let cfg = KrylovConfig {
            rtol: 1e-10,
            max_iter: 500,
            ..Default::default()
        };
        let jac = JacobiPreconditioner::new(&a).unwrap();
        let ilu = Ilu0Preconditioner::new(&a).unwrap();
        let s1 = cg(&a, &b, &jac, &cfg).unwrap();
        let s2 = cg(&a, &b, &ilu, &cfg).unwrap();
        assert!(s1.converged());
        assert!(s2.converged());
        // ILU(0) on a tridiagonal is exact, so it should need ≤ 2 iterations.
        assert!(
            s2.stats.iterations <= 3,
            "ilu iters = {}",
            s2.stats.iterations
        );
        for i in 0..30 {
            assert!((s1.x[i] - x_true[i]).abs() < 1e-6);
            assert!((s2.x[i] - x_true[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn invalid_config_rejected() {
        let a = tridiag(3, 2.0, -1.0);
        let cfg = KrylovConfig {
            rtol: 0.0,
            ..Default::default()
        };
        assert!(matches!(
            cg_unpreconditioned(&a, &[1.0, 1.0, 1.0], &cfg),
            Err(SparseError::Unsupported { .. })
        ));
    }

    #[test]
    fn zero_rhs_converges_immediately() {
        let a = tridiag(5, 2.0, -1.0);
        let b = vec![0.0; 5];
        let cfg = KrylovConfig::default();
        let sol = cg_unpreconditioned(&a, &b, &cfg).unwrap();
        assert!(sol.converged());
        assert_eq!(sol.stats.iterations, 0);
    }

    #[test]
    fn cg_with_scaling_solves_and_reports_original_residual() {
        // A badly scaled but SPD system: diagonal entries span four orders of
        // magnitude, so equilibration is genuinely exercised.
        let n = 8;
        let mut coo = CooMatrix::new(n, n);
        for i in 0..n {
            let d = 10f64.powi(i as i32 - 3);
            coo.push(i, i, d).unwrap();
            if i + 1 < n {
                coo.push(i, i + 1, -0.05 * d).unwrap();
                coo.push(i + 1, i, -0.05 * d).unwrap();
            }
        }
        let a = coo.to_csr();
        let x_true: Vec<Scalar> = (0..n).map(|i| 1.0 + i as Scalar).collect();
        let b = exact_solution(&a, &x_true);
        let cfg = KrylovConfig {
            rtol: 1e-12,
            max_iter: 5000,
            ..Default::default()
        };
        let sol = cg_with_scaling(&a, &b, true, &cfg).unwrap();
        // The scaling must produce a solution whose ORIGINAL-system residual is
        // small (the scaled residual is not what we accept).
        let true_res = norm2(&residual(&a, &sol.x, &b).unwrap());
        let bnorm = norm2(&b);
        assert!(
            true_res <= 1e-6 * bnorm,
            "original residual {true_res} (||b||={bnorm}) reason={:?}",
            sol.stats.reason
        );
        for (i, (xi, xt)) in sol.x.iter().zip(x_true.iter()).enumerate() {
            assert!((xi - xt).abs() < 1e-3, "x[{i}]={xi} want {xt}");
        }
        // The reported final residual must be the ORIGINAL system's residual.
        assert!(
            (sol.stats.final_residual - true_res).abs() < 1e-9 * true_res.max(1.0),
            "reported {} vs true {}",
            sol.stats.final_residual,
            true_res
        );
    }

    #[test]
    fn cg_with_scaling_without_equilibration_matches_plain_cg() {
        let a = tridiag(10, 3.0, -1.0);
        let x_true: Vec<Scalar> = (0..10).map(|i| (i as Scalar).sin()).collect();
        let b = exact_solution(&a, &x_true);
        let cfg = KrylovConfig {
            rtol: 1e-12,
            max_iter: 500,
            ..Default::default()
        };
        let s1 = cg_with_scaling(&a, &b, false, &cfg).unwrap();
        let s2 = cg_unpreconditioned(&a, &b, &cfg).unwrap();
        for i in 0..10 {
            assert!((s1.x[i] - s2.x[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn linear_operator_trait_defaults() {
        let a = tridiag(3, 2.0, -1.0);
        assert_eq!(LinearOperator::dim(&a), 3);
        let y = LinearOperator::apply(&a, &[1.0, 1.0, 1.0]).unwrap();
        assert_eq!(y, vec![1.0, 0.0, 1.0]);
        assert!(LinearOperator::diagonal(&a).is_some());
    }
}
