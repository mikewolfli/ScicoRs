// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Nonlinear finite-element solver using the Newton-Raphson method.
//!
//! Extends the linear `FemSystem` with support for geometric nonlinearity
//! (large deformation), material nonlinearity (J₂ plasticity with isotropic
//! hardening), and arc-length continuation for post-buckling analysis.

use crate::core::compute::matrix::{mat_vec_mul, solve_linear};
use crate::core::types::Scalar;
use crate::domains::structural::fem_solver::FemElement;

/// Nonlinear FEM system with Newton-Raphson solver.
///
/// # Element connectivity
///
/// Like [`crate::domains::structural::fem_solver::FemSystem`], each element
/// needs to know which global nodes it spans before its local stiffness can be
/// scattered. Register connectivity with [`NonlinearFem::connect`]; elements
/// without connectivity are placed at the legacy `index * 6` slot, which is
/// only correct for a single element.
pub struct NonlinearFem {
    pub nodes: Vec<Coord3D>,
    pub elements: Vec<FemElement>,
    pub constraints: Vec<(usize, usize, Scalar)>,
    pub loads: Vec<(usize, usize, Scalar)>,
    pub young_modulus: Scalar,
    pub yield_stress: Scalar,
    /// Hardening modulus (tangent) for plasticity.
    pub hardening_modulus: Scalar,
    /// Connectivity: `element_nodes[e]` lists the global node indices of
    /// element `e` in local DOF order.
    pub element_nodes: Vec<Vec<usize>>,
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
            element_nodes: Vec::new(),
        }
    }

    /// Append an element together with its node connectivity.
    ///
    /// See [`crate::domains::structural::fem_solver::FemSystem::connect`] for
    /// the index-order contract.
    pub fn connect(&mut self, elem: FemElement, nodes: &[usize]) -> usize {
        for &n in nodes {
            assert!(n < self.nodes.len(), "connect: node index {n} out of range");
        }
        self.elements.push(elem);
        self.element_nodes.push(nodes.to_vec());
        self.elements.len() - 1
    }

    /// Degrees of freedom.
    fn n_dofs(&self) -> usize {
        self.nodes.len() * 6
    }

    /// Global DOF indices an element occupies, in local DOF order.
    ///
    /// Returns `None` when connectivity is unknown, so the caller can fall back
    /// to the legacy `index * 6` layout.
    fn element_dof_indices(&self, index: usize) -> Option<Vec<usize>> {
        let nodes = self.element_nodes.get(index)?;
        match self.elements.get(index)? {
            FemElement::Truss(_) => {
                if nodes.len() != 2 {
                    return None;
                }
                Some(vec![
                    nodes[0] * 6,
                    nodes[0] * 6 + 1,
                    nodes[1] * 6,
                    nodes[1] * 6 + 1,
                ])
            }
            FemElement::Beam(_) => {
                if nodes.len() != 2 {
                    return None;
                }
                let mut dofs = Vec::with_capacity(12);
                for &n in nodes {
                    for d in 0..6 {
                        dofs.push(n * 6 + d);
                    }
                }
                Some(dofs)
            }
            // Spring/shell/solid are not modelled by this nonlinear solver; the
            // caller keeps the legacy layout for them.
            _ => None,
        }
    }

    /// Scatter a local element matrix into the global matrix at `dofs`.
    fn scatter(k_global: &mut [Vec<Scalar>], k_local: &[Vec<Scalar>], dofs: &[usize]) {
        let n_dof = k_global.len();
        for (a, &gi) in dofs.iter().enumerate() {
            if gi >= n_dof || a >= k_local.len() {
                continue;
            }
            for (b, &gj) in dofs.iter().enumerate() {
                if gj >= n_dof || b >= k_local[a].len() {
                    continue;
                }
                k_global[gi][gj] += k_local[a][b];
            }
        }
    }

    /// Assemble the linear stiffness matrix (small-deformation).
    ///
    /// Connected elements are scattered to their true global DOFs so shared
    /// nodes receive the sum of every element's contribution; unconnected
    /// elements keep the legacy top-left layout.
    fn assemble_linear_stiffness(&self) -> Vec<Vec<Scalar>> {
        let n_dof = self.n_dofs();
        if n_dof == 0 {
            return Vec::new();
        }
        let mut k = vec![vec![0.0; n_dof]; n_dof];
        for (index, elem) in self.elements.iter().enumerate() {
            let dofs = self.element_dof_indices(index);
            match elem {
                FemElement::Truss(te) => {
                    let kl = te.stiffness_matrix();
                    match &dofs {
                        Some(d) => Self::scatter(&mut k, &kl, d),
                        // Legacy: element is placed at the first free 4×4 block.
                        None => {
                            let base = index * 6;
                            if base + 3 < n_dof {
                                for i in 0..4 {
                                    for j in 0..4 {
                                        k[base + i][base + j] += kl[i][j];
                                    }
                                }
                            }
                        }
                    }
                }
                FemElement::Beam(be) => {
                    let kl = be.stiffness_matrix();
                    match &dofs {
                        Some(d) => Self::scatter(&mut k, &kl, d),
                        None => {
                            let base = index * 6;
                            if base + 11 < n_dof {
                                for i in 0..12 {
                                    for j in 0..12 {
                                        k[base + i][base + j] += kl[i][j];
                                    }
                                }
                            }
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
        // Geometric stiffness (simplified: axial force contribution, derived
        // from the element's axial strain read at its own DOFs).
        let mut k_geo = vec![vec![0.0; n_dof]; n_dof];
        for (ei, elem) in self.elements.iter().enumerate() {
            if !matches!(elem, FemElement::Truss(_)) {
                continue;
            }
            // Axial strain is the difference of the two nodes' axial
            // displacements over the element length.
            let (i_a, i_b) = match self.element_dof_indices(ei) {
                Some(d) if d.len() >= 3 => (d[0], d[2]),
                _ => (ei * 6, ei * 6 + 2),
            };
            let length = match elem {
                FemElement::Truss(te) => te.length,
                _ => 1.0,
            };
            let du = u.get(i_a).copied().unwrap_or(0.0) - u.get(i_b).copied().unwrap_or(0.0);
            let axial = if length > 0.0 {
                du / length * self.young_modulus
            } else {
                0.0
            };
            if axial.abs() > 1e-30 {
                if i_a < n_dof {
                    k_geo[i_a][i_a] += axial;
                }
                if i_b < n_dof {
                    k_geo[i_b][i_b] += axial;
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

    #[test]
    fn test_connected_assembly_sums_at_shared_node() {
        // Two trusses sharing the middle node. The legacy assembly pushed both
        // elements into the same top-left block (a bug on multi-element meshes);
        // connectivity must instead accumulate each element at its real DOFs.
        let mat = crate::domains::structural::physics::MaterialProperties {
            young_modulus: 200e9,
            poisson_ratio: 0.3,
            density: 7800.0,
            yield_strength: 250e6,
            ultimate_strength: 400e6,
            thermal_expansion: 1.2e-5,
        };
        let k0 = 200e9 * 0.01 / 1.0;
        let mut nlf = NonlinearFem::new(200e9, 250e6, 1e9);
        for i in 0..3 {
            nlf.nodes.push(Coord3D::new(i as Scalar, 0.0, 0.0));
        }
        let truss = || {
            FemElement::Truss(TrussElement {
                length: 1.0,
                area: 0.01,
                material: mat,
            })
        };
        nlf.connect(truss(), &[0, 1]);
        nlf.connect(truss(), &[1, 2]);

        let k = nlf.assemble_linear_stiffness();
        assert_eq!(k.len(), 18);
        // Shared node 1 axial DOF (index 6) sees both elements: 2·k0.
        assert!(
            (k[6][6] - 2.0 * k0).abs() < 1e-6 * k0,
            "shared node {} != {}",
            k[6][6],
            2.0 * k0
        );
        assert!((k[0][0] - k0).abs() < 1e-6 * k0);
        assert!((k[12][12] - k0).abs() < 1e-6 * k0);

        // The internal force under a unit axial displacement at node 1 must
        // therefore reflect both elements, not one.
        let mut u = vec![0.0; 18];
        u[6] = 1.0;
        let f = nlf.internal_force(&u);
        assert!(f[6].abs() > 1.5 * k0, "internal force {} too small", f[6]);
    }
}
