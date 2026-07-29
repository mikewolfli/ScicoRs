//! Eigenvalue solvers for dense matrices.
//!
//! Provides subspace iteration for the generalized eigenvalue problem
//! `K·φ = λ·M·φ`, extracting the smallest eigenvalues/vectors.
//!
//! This is the single source of truth for eigenvalue computations.
//! Domain modules (structural FEA, quantum chemistry, optics) should
//! use these rather than re-implementing their own.

use super::matrix::solve_linear;
use crate::core::error::SimError;
use crate::core::types::Scalar;

/// Solve the generalized eigenvalue problem `K·φ = λ·M·φ` using subspace iteration.
///
/// Finds the `n_modes` smallest eigenvalues and corresponding eigenvectors.
///
/// # Arguments
/// * `k` — stiffness-like matrix (n×n)
/// * `m` — mass-like matrix (n×n, symmetric positive semi-definite)
/// * `n_modes` — number of eigenpairs to compute
/// * `max_iter` — maximum subspace iteration count
/// * `tolerance` — convergence threshold for eigenvalue change
///
/// # Returns
/// `(eigenvalues, eigenvectors)` where eigenvectors is n×n_modes.
pub fn subspace_eigen(
    k: &[Vec<Scalar>],
    m: &[Vec<Scalar>],
    n_modes: usize,
    max_iter: usize,
    tolerance: Scalar,
) -> Result<(Vec<Scalar>, Vec<Vec<Scalar>>), SimError> {
    let n = k.len();
    if n == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    if k[0].len() != n || m.len() != n || m[0].len() != n {
        return Err(SimError::numerical(
            "subspace_eigen: K and M must be square with matching dimensions",
        ));
    }

    let n_modes = n_modes.min(n).max(1);

    // Phase 1: initial subspace with random vectors + orthonormalization
    let mut phi = vec![vec![0.0; n_modes]; n];
    for j in 0..n_modes {
        for i in 0..n {
            phi[i][j] = (i * 31 + j * 17) as Scalar / n as Scalar;
        }
        orthonormalize_m(&mut phi, m, j)?;
    }

    let mut prev_eigenvalues = vec![0.0; n_modes];

    // Phase 2: subspace iteration
    for _iter in 0..max_iter {
        // Solve K·ψ_j = M·φ_j for each j
        let mut psi = vec![vec![0.0; n_modes]; n];
        for j in 0..n_modes {
            let rhs: Vec<Scalar> = (0..n)
                .map(|i| {
                    let mut s = 0.0;
                    for k_idx in 0..n {
                        s += m[i][k_idx] * phi[k_idx][j];
                    }
                    s
                })
                .collect();
            let k_copy = k.to_vec();
            match solve_linear(&k_copy, &rhs) {
                Ok(x) => {
                    for i in 0..n {
                        psi[i][j] = x[i];
                    }
                }
                Err(_) => {
                    // Fallback: random vector
                    for i in 0..n {
                        psi[i][j] = (i * 7 + j * 13) as Scalar / (n + 1) as Scalar;
                    }
                }
            }
        }

        // Project K and M into the subspace: K_proj = ψ^T·K·ψ, M_proj = ψ^T·M·ψ
        let k_proj = project_matrix(k, &psi, n_modes);
        let m_proj = project_matrix(m, &psi, n_modes);

        // Solve dense n_modes×n_modes eigenproblem via Jacobi iteration
        let (eigenvalues, eigenvectors) = solve_reduced_eigen(&k_proj, &m_proj, n_modes);

        // Reconstruct full-space vectors: φ_new = ψ · eigenvectors
        let mut new_phi = vec![vec![0.0; n_modes]; n];
        for i in 0..n {
            for j in 0..n_modes {
                for k_idx in 0..n_modes {
                    new_phi[i][j] += psi[i][k_idx] * eigenvectors[k_idx][j];
                }
            }
        }

        // Orthonormalize w.r.t. M
        for j in 0..n_modes {
            orthonormalize_m(&mut new_phi, m, j)?;
        }

        phi = new_phi;

        // Check convergence
        let mut converged = true;
        for j in 0..n_modes {
            if (eigenvalues[j] - prev_eigenvalues[j]).abs()
                > tolerance * prev_eigenvalues[j].max(1.0)
            {
                converged = false;
            }
            prev_eigenvalues[j] = eigenvalues[j];
        }
        if converged {
            break;
        }
    }

    Ok((prev_eigenvalues, phi))
}

/// Orthonormalize column `j` of `phi` against columns `0..j` w.r.t. matrix `m`.
///
/// Standard Gram-Schmidt: for each k < j, subtract the M-inner product
/// projection, then normalize so that φ_j^T · M · φ_j = 1.
fn orthonormalize_m(phi: &mut [Vec<Scalar>], m: &[Vec<Scalar>], j: usize) -> Result<(), SimError> {
    let n = phi.len();
    // Gram-Schmidt against previous vectors
    for k in 0..j {
        let mut inner = 0.0;
        for i in 0..n {
            let mut m_phi_k = 0.0;
            for l in 0..n {
                m_phi_k += m[i][l] * phi[l][k];
            }
            inner += m_phi_k * phi[i][j];
        }
        for i in 0..n {
            phi[i][j] -= inner * phi[i][k];
        }
    }
    // Normalize: φ_j^T · M · φ_j = 1
    let mut norm_sq = 0.0;
    for i in 0..n {
        let mut m_phi_j = 0.0;
        for l in 0..n {
            m_phi_j += m[i][l] * phi[l][j];
        }
        norm_sq += m_phi_j * phi[i][j];
    }
    if norm_sq <= 0.0 {
        return Err(SimError::numerical(
            "Zero norm in subspace orthonormalization",
        ));
    }
    let inv_norm = 1.0 / norm_sq.sqrt();
    for i in 0..n {
        phi[i][j] *= inv_norm;
    }
    Ok(())
}

/// Project a matrix `a` into the subspace spanned by `psi`:
/// `A_proj = ψ^T · a · ψ`  (n_modes × n_modes).
fn project_matrix(a: &[Vec<Scalar>], psi: &[Vec<Scalar>], n_modes: usize) -> Vec<Vec<Scalar>> {
    let n = a.len();
    let mut proj = vec![vec![0.0; n_modes]; n_modes];
    for i in 0..n_modes {
        for j in 0..n_modes {
            let mut s = 0.0;
            for p in 0..n {
                let mut a_psi = 0.0;
                for q in 0..n {
                    a_psi += a[p][q] * psi[q][j];
                }
                s += psi[p][i] * a_psi;
            }
            proj[i][j] = s;
        }
    }
    proj
}

/// Solve the dense generalized eigenvalue problem `K_proj · v = λ · M_proj · v`
/// using Jacobi iteration on the reduced n_modes×n_modes system.
///
/// Returns `(eigenvalues, eigenvectors)` where eigenvectors is n_modes×n_modes
/// and each column is a unit eigenvector.
fn solve_reduced_eigen(
    k_proj: &[Vec<Scalar>],
    m_proj: &[Vec<Scalar>],
    n_modes: usize,
) -> (Vec<Scalar>, Vec<Vec<Scalar>>) {
    if n_modes == 0 {
        return (Vec::new(), Vec::new());
    }

    // Cholesky decomposition of M_proj = L·L^T
    let mut l = vec![vec![0.0; n_modes]; n_modes];
    for i in 0..n_modes {
        let mut s = 0.0;
        for k in 0..i {
            s += l[i][k] * l[i][k];
        }
        let val = m_proj[i][i] - s;
        if val <= 0.0 {
            // Fallback: use identity
            l = vec![vec![0.0; n_modes]; n_modes];
            for k in 0..n_modes {
                l[k][k] = 1.0;
            }
            break;
        }
        l[i][i] = val.sqrt();
        for j in (i + 1)..n_modes {
            let mut s2 = 0.0;
            for k in 0..i {
                s2 += l[j][k] * l[i][k];
            }
            l[j][i] = (m_proj[j][i] - s2) / l[i][i];
        }
    }

    // Transform to standard eigenproblem: A = L^{-1} · K_proj · L^{-T}
    // First compute L^{-1} (forward substitution)
    let mut l_inv = vec![vec![0.0; n_modes]; n_modes];
    for i in 0..n_modes {
        for j in 0..=i {
            if i == j {
                l_inv[i][j] = 1.0 / l[i][j];
            } else {
                let mut s = 0.0;
                for k in j..i {
                    s += l[i][k] * l_inv[k][j];
                }
                l_inv[i][j] = -s / l[i][i];
            }
        }
    }

    // A_std = L^{-1} · K_proj · L^{-T}
    let mut a_std = vec![vec![0.0; n_modes]; n_modes];
    let mut temp = vec![vec![0.0; n_modes]; n_modes];
    // temp = L^{-1} · K_proj
    for i in 0..n_modes {
        for j in 0..n_modes {
            let mut s = 0.0;
            for k in 0..n_modes {
                s += l_inv[i][k] * k_proj[k][j];
            }
            temp[i][j] = s;
        }
    }
    // A_std = temp · L^{-T}
    for i in 0..n_modes {
        for j in 0..n_modes {
            let mut s = 0.0;
            for k in 0..n_modes {
                s += temp[i][k] * l_inv[j][k];
            }
            a_std[i][j] = s;
        }
    }

    // Jacobi iteration on the standard eigenproblem
    jacobi_eigen(&a_std, n_modes)
}

/// Solve the standard eigenvalue problem `A·v = λ·v` using Jacobi iteration.
///
/// A is symmetric n×n. Returns `(eigenvalues, eigenvectors)`.
pub fn jacobi_eigen(a: &[Vec<Scalar>], n: usize) -> (Vec<Scalar>, Vec<Vec<Scalar>>) {
    if n == 0 {
        return (Vec::new(), Vec::new());
    }

    let max_iter = 100;
    let tolerance = 1e-12;

    let mut eigenvalues = vec![0.0; n];
    let mut eigenvectors = vec![vec![0.0; n]; n];
    for i in 0..n {
        eigenvectors[i][i] = 1.0;
    }

    // Copy A
    let mut a_copy = a.to_vec();

    for _iter in 0..max_iter {
        // Find the largest off-diagonal element
        let mut max_val: Scalar = 0.0;
        let mut p = 0;
        let mut q = 0;
        for i in 0..n {
            for j in (i + 1)..n {
                let val = a_copy[i][j].abs();
                if val > max_val {
                    max_val = val;
                    p = i;
                    q = j;
                }
            }
        }
        if max_val < tolerance {
            break;
        }

        // Compute rotation angle
        let beta = (a_copy[q][q] - a_copy[p][p]) / (2.0 * a_copy[p][q]);
        let t = if beta >= 0.0 {
            1.0 / (beta + (1.0 + beta * beta).sqrt())
        } else {
            -1.0 / (-beta + (1.0 + beta * beta).sqrt())
        };
        let c = 1.0 / (1.0 + t * t).sqrt();
        let s = t * c;

        // Apply Jacobi rotation to A
        let a_pp = a_copy[p][p];
        let a_qq = a_copy[q][q];
        let a_pq = a_copy[p][q];
        a_copy[p][p] = a_pp - t * a_pq;
        a_copy[q][q] = a_qq + t * a_pq;
        a_copy[p][q] = 0.0;
        a_copy[q][p] = 0.0;

        for i in 0..n {
            if i != p && i != q {
                let a_ip = a_copy[i][p];
                let a_iq = a_copy[i][q];
                a_copy[i][p] = c * a_ip - s * a_iq;
                a_copy[p][i] = a_copy[i][p];
                a_copy[i][q] = s * a_ip + c * a_iq;
                a_copy[q][i] = a_copy[i][q];
            }
        }

        // Update eigenvectors
        for i in 0..n {
            let e_ip = eigenvectors[i][p];
            let e_iq = eigenvectors[i][q];
            eigenvectors[i][p] = c * e_ip - s * e_iq;
            eigenvectors[i][q] = s * e_ip + c * e_iq;
        }
    }

    // Extract eigenvalues from diagonal
    for i in 0..n {
        eigenvalues[i] = a_copy[i][i];
    }

    (eigenvalues, eigenvectors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jacobi_2x2() {
        let a = vec![vec![2.0, 1.0], vec![1.0, 2.0]];
        let (vals, vecs) = jacobi_eigen(&a, 2);
        assert!((vals[0] - 1.0).abs() < 1e-10 || (vals[0] - 3.0).abs() < 1e-10);
        assert!((vals[1] - 1.0).abs() < 1e-10 || (vals[1] - 3.0).abs() < 1e-10);
        assert!((vals[0] - vals[1]).abs() > 1e-6);
        // Eigenvectors should be orthogonal
        let dot = vecs[0][0] * vecs[0][1] + vecs[1][0] * vecs[1][1];
        assert!(dot.abs() < 1e-10);
    }

    #[test]
    fn test_jacobi_identity() {
        let a = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let (vals, _vecs) = jacobi_eigen(&a, 2);
        assert!((vals[0] - 1.0).abs() < 1e-10);
        assert!((vals[1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_subspace_eigen_small() {
        // Simple 2×2: K = [[2,0],[0,8]], M = I → eigenvalues 2, 8
        let k = vec![vec![2.0, 0.0], vec![0.0, 8.0]];
        let m = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let (vals, _vecs) = subspace_eigen(&k, &m, 2, 20, 1e-8).unwrap();
        assert!((vals[0] - 2.0).abs() < 1e-6 || (vals[0] - 8.0).abs() < 1e-6);
        assert!((vals[1] - 2.0).abs() < 1e-6 || (vals[1] - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_empty_system() {
        let k: Vec<Vec<Scalar>> = Vec::new();
        let m: Vec<Vec<Scalar>> = Vec::new();
        let (vals, vecs) = subspace_eigen(&k, &m, 0, 10, 1e-8).unwrap();
        assert!(vals.is_empty());
        assert!(vecs.is_empty());
    }
}
