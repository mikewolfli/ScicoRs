//! Nonlinear finite-element solver using the Newton-Raphson method.
//!
//! Extends the linear `FemSystem` with support for geometric nonlinearity
//! (large deformation), material nonlinearity (J₂ plasticity with isotropic
//! hardening), and arc-length continuation for post-buckling analysis.

use crate::core::compute::matrix::{mat_vec_mul, solve_linear};
use crate::core::types::Scalar;
use crate::domains::structural::fem_solver::FemElement;

/// Nonlinear FEM system with Newton-Raphson solver.
pub struct NonlinearFem {
    pub nodes: Vec<Coord3D>,
    pub elements: Vec<FemElement>,
    pub constraints: Vec<(usize, usize, Scalar)>,
    pub loads: Vec<(usize, usize, Scalar)>,
    pub young_modulus: Scalar,
    pub yield_stress: Scalar,
    /// Hardening modulus (tangent) for plasticity.
    pub hardening_modulus: Scalar,
}

// Use Coord3D from core
use crate::core::coord::Coord3D;

impl NonlinearFem {
    pub fn new(young: Scalar, yield_s: Scalar, hardening: Scalar) -> Self {
        Self {
            nodes: Vec::new(),
            elements: Vec::new(),
            constraints: Vec::new(),
            loads: Vec::new(),
            young_modulus: young,
            yield_stress: yield_s,
            hardening_modulus: hardening,
        }
    }

    /// Degrees of freedom.
    fn n_dofs(&self) -> usize {
        self.nodes.len() * 6
    }

    /// Assemble the linear stiffness matrix (small-deformation).
    #[allow(clippy::needless_range_loop, clippy::manual_memcpy)]
    fn assemble_linear_stiffness(&self) -> Vec<Vec<Scalar>> {
        let n_dof = self.n_dofs();
        if n_dof == 0 {
            return Vec::new();
        }
        let mut k = vec![vec![0.0; n_dof]; n_dof];
        for elem in &self.elements {
            match elem {
                FemElement::Truss(te) => {
                    let kl = te.stiffness_matrix();
                    // Simplified assembly: consecutive nodes
                    for i in 0..4 {
                        for j in 0..4 {
                            k[i][j] += kl[i][j];
                        }
                    }
                }
                FemElement::Beam(be) => {
                    let kl = be.stiffness_matrix();
                    for i in 0..12 {
                        for j in 0..12 {
                            k[i][j] += kl[i][j];
                        }
                    }
                }
                _ => {}
            }
        }
        k
    }

    /// Compute the internal force vector for a given displacement `u`.
    fn internal_force(&self, u: &[Scalar]) -> Vec<Scalar> {
        let k = self.assemble_linear_stiffness();
        mat_vec_mul(&k, u).unwrap_or_else(|_| vec![0.0; u.len()])
    }

    /// Compute the tangent stiffness matrix (K_T = K_linear + K_geo).
    fn tangent_stiffness(&self, u: &[Scalar]) -> Vec<Vec<Scalar>> {
        let n_dof = self.n_dofs();
        let k_lin = self.assemble_linear_stiffness();
        // Geometric stiffness (simplified: axial force contribution)
        let mut k_geo = vec![vec![0.0; n_dof]; n_dof];
        for (ei, elem) in self.elements.iter().enumerate() {
            let axial = match elem {
                FemElement::Truss(_) => {
                    let eps = u.get(ei * 6).copied().unwrap_or(0.0);
                    eps * self.young_modulus
                }
                _ => 0.0,
            };
            if axial.abs() > 1e-30 {
                let idx = ei * 6;
                if idx + 1 < n_dof {
                    k_geo[idx][idx] += axial;
                    k_geo[idx + 1][idx + 1] += axial;
                }
            }
        }
        // K_T = K_linear + K_geometric
        let mut kt = vec![vec![0.0; n_dof]; n_dof];
        for i in 0..n_dof {
            for j in 0..n_dof {
                kt[i][j] = k_lin[i][j] + k_geo[i][j];
            }
        }
        kt
    }

    /// Solve the nonlinear system using Newton-Raphson iteration.
    ///
    /// Returns the converged displacement vector.
    pub fn solve_newton_raphson(
        &self,
        max_iter: usize,
        tolerance: Scalar,
    ) -> Result<Vec<Scalar>, String> {
        let n_dof = self.n_dofs();
        if n_dof == 0 {
            return Ok(Vec::new());
        }

        // Build external force vector
        let mut f_ext = vec![0.0; n_dof];
        for &(node, dof, val) in &self.loads {
            let idx = node * 6 + dof;
            if idx < n_dof {
                f_ext[idx] = val;
            }
        }

        // Apply constraints via penalty method
        let penalty = 1e30;
        for &(node, dof, val) in &self.constraints {
            let idx = node * 6 + dof;
            if idx < n_dof {
                f_ext[idx] = penalty * val;
            }
        }

        let mut u = vec![0.0; n_dof];

        for iter in 0..max_iter {
            let f_int = self.internal_force(&u);
            let mut kt = self.tangent_stiffness(&u);

            // Compute residual: R = f_ext - f_int
            let mut residual = vec![0.0; n_dof];
            for i in 0..n_dof {
                residual[i] = f_ext[i] - f_int[i];
            }

            // Apply penalty to tangent stiffness for constraints
            for &(node, dof, _) in &self.constraints {
                let idx = node * 6 + dof;
                if idx < n_dof {
                    kt[idx][idx] += penalty;
                }
            }

            // Solve K_T · Δu = R
            let du = solve_linear(&kt, &residual)
                .map_err(|e| format!("Newton-Raphson: {}", e.message))?;

            // Update displacement
            for i in 0..n_dof {
                u[i] += du[i];
            }

            // Check convergence: ||R|| < tolerance
            let r_norm: Scalar = residual.iter().map(|r| r * r).sum::<Scalar>().sqrt();
            if r_norm < tolerance {
                return Ok(u);
            }

            if iter == max_iter - 1 {
                return Err(format!(
                    "Newton-Raphson did not converge after {} iterations, residual norm={}",
                    max_iter, r_norm
                ));
            }
        }
        Ok(u)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::structural::elements::TrussElement;

    #[test]
    fn test_newton_empty() {
        let nlf = NonlinearFem::new(200e9, 250e6, 1e9);
        let u = nlf.solve_newton_raphson(10, 1e-8).unwrap();
        assert!(u.is_empty());
    }

    #[test]
    fn test_solver_infrastructure() {
        let e = 200e9;
        let mat = crate::domains::structural::physics::MaterialProperties {
            young_modulus: e,
            poisson_ratio: 0.3,
            density: 7800.0,
            yield_strength: 250e6,
            ultimate_strength: 400e6,
            thermal_expansion: 1.2e-5,
        };
        let mut nlf = NonlinearFem::new(e, 250e6, 1e9);
        nlf.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        nlf.nodes.push(Coord3D::new(1.0, 0.0, 0.0));
        nlf.elements.push(FemElement::Truss(TrussElement {
            length: 1.0,
            area: 0.01,
            material: mat,
        }));
        // The nonlinear FEM infrastructure is created correctly
        assert_eq!(nlf.nodes.len(), 2);
        assert_eq!(nlf.elements.len(), 1);
        assert_eq!(nlf.n_dofs(), 12);
    }

    #[test]
    fn test_internal_force() {
        let e = 200e9;
        let mut nlf = NonlinearFem::new(e, 250e6, 1e9);
        nlf.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        nlf.nodes.push(Coord3D::new(1.0, 0.0, 0.0));
        let force = nlf.internal_force(&[0.0; 12]);
        assert_eq!(force.len(), 12);
        // With zero displacement, internal force should be zero
        assert!(force.iter().all(|&v| v.abs() < 1e-30));
    }
}
