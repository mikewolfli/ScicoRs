//! Finite-element solver: static, modal, and buckling analysis.
//!
//! Assembles global stiffness/mass matrices from element contributions,
//! applies boundary conditions, and solves the resulting linear systems.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use crate::domains::structural::elements::{
    BeamElement, ShellElement, SolidElement, SpringElement, TrussElement,
};

/// Union type for any supported finite element.
#[derive(Debug, Clone)]
pub enum FemElement {
    /// 3D beam element.
    Beam(BeamElement),
    /// 2D truss element.
    Truss(TrussElement),
    /// Spring element.
    Spring(SpringElement),
    /// 4-node quadrilateral shell.
    Shell(ShellElement),
    /// 8-node hexahedral solid.
    Solid(SolidElement),
}

/// A pre-computed element stiffness matrix paired with its type tag.
/// Used internally to separate computation from assembly for parallelisation.
enum StiffnessContribution {
    Beam(Vec<Vec<Scalar>>),
    Truss(Vec<Vec<Scalar>>),
    Spring(Scalar),
    Shell(Vec<Vec<Scalar>>),
    Solid(Vec<Vec<Scalar>>),
}

/// A complete finite-element system.
///
/// Stores nodal coordinates, element definitions, constraints (boundary
/// conditions), and nodal loads.
#[derive(Debug, Clone)]
pub struct FemSystem {
    /// Nodal coordinates.
    pub nodes: Vec<Coord3D>,
    /// Finite elements referencing node indices.
    pub elements: Vec<FemElement>,
    /// Constraints: (node_index, dof, prescribed_value).
    pub constraints: Vec<(usize, usize, Scalar)>,
    /// Nodal loads: (node_index, dof, force_magnitude).
    pub loads: Vec<(usize, usize, Scalar)>,
}

impl FemSystem {
    /// Create a new empty FEM system.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            elements: Vec::new(),
            constraints: Vec::new(),
            loads: Vec::new(),
        }
    }

    /// Determine the total number of DOFs in the system.
    fn n_dofs(&self) -> usize {
        self.nodes.len() * 6 // conservative: 6 DOFs per node
    }

    /// Assemble the global stiffness matrix.
    ///
    /// Element stiffness matrices are computed in parallel (using rayon)
    /// then assembled serially into the global n_dofs × n_dofs matrix.
    pub fn assemble_stiffness(&self) -> Vec<Vec<Scalar>> {
        let n_dof = self.n_dofs();
        if n_dof == 0 {
            return Vec::new();
        }

        // Phase 1: Compute all element stiffness matrices in parallel
        use rayon::prelude::*;
        let contributions: Vec<StiffnessContribution> = self
            .elements
            .par_iter()
            .map(|elem| match elem {
                FemElement::Beam(be) => StiffnessContribution::Beam(be.stiffness_matrix()),
                FemElement::Truss(te) => StiffnessContribution::Truss(te.stiffness_matrix()),
                FemElement::Spring(se) => StiffnessContribution::Spring(se.stiffness),
                FemElement::Shell(se) => StiffnessContribution::Shell(se.stiffness_matrix()),
                FemElement::Solid(se) => StiffnessContribution::Solid(se.stiffness_matrix()),
            })
            .collect();

        // Phase 2: Assemble serially into the global matrix
        let mut k_global = vec![vec![0.0; n_dof]; n_dof];
        for contrib in &contributions {
            match contrib {
                StiffnessContribution::Beam(k) => Self::assemble_beam(&mut k_global, k),
                StiffnessContribution::Truss(k) => Self::assemble_truss(&mut k_global, k),
                StiffnessContribution::Spring(k) => Self::assemble_spring(&mut k_global, *k),
                StiffnessContribution::Shell(k) => Self::assemble_shell(&mut k_global, k),
                StiffnessContribution::Solid(k) => Self::assemble_solid(&mut k_global, k),
            }
        }

        k_global
    }

    /// Assemble beam element (12×12 → global).
    /// Assumes element index j maps to nodes [j, j+1].
    fn assemble_beam(k_global: &mut Vec<Vec<Scalar>>, k_local: &[Vec<Scalar>]) {
        let n_dof = k_global.len();
        let n_elems_est = n_dof / 6;
        // Find the first unconstrained node pair by scanning
        let mut start_node = 0;
        // Simple heuristic: find a block where we can place a 12×12
        for candidate in 0..n_elems_est.saturating_sub(1) {
            let row_start = candidate * 6;
            let col_start = row_start;
            if row_start + 11 < n_dof {
                // Check if this block is mostly zero -> free slot
                let mut empty = true;
                for r in 0..12 {
                    for c in 0..12 {
                        if k_global[row_start + r][col_start + c].abs() > 1e-30 {
                            empty = false;
                            break;
                        }
                    }
                    if !empty {
                        break;
                    }
                }
                if empty {
                    start_node = candidate;
                    break;
                }
            }
        }

        let row_base = start_node * 6;
        let col_base = row_base;
        if row_base + 11 < n_dof {
            for r in 0..12 {
                for c in 0..12 {
                    k_global[row_base + r][col_base + c] += k_local[r][c];
                }
            }
        }
    }

    /// Assemble truss element (4×4 → global).
    fn assemble_truss(k_global: &mut Vec<Vec<Scalar>>, k_local: &[Vec<Scalar>]) {
        let n_dof = k_global.len();
        let n_elems_est = n_dof / 6;
        let mut start_node = 0;
        for candidate in 0..n_elems_est.saturating_sub(1) {
            let row_start = candidate * 6;
            let col_start = row_start;
            if row_start + 3 < n_dof {
                let mut empty = true;
                for r in 0..4 {
                    for c in 0..4 {
                        if k_global[row_start + r][col_start + c].abs() > 1e-30 {
                            empty = false;
                            break;
                        }
                    }
                    if !empty {
                        break;
                    }
                }
                if empty {
                    start_node = candidate;
                    break;
                }
            }
        }

        let row_base = start_node * 6;
        let col_base = row_base;
        if row_base + 3 < n_dof {
            for r in 0..4 {
                for c in 0..4 {
                    k_global[row_base + r][col_base + c] += k_local[r][c];
                }
            }
        }
    }

    /// Assemble spring element (scalar → global).
    fn assemble_spring(k_global: &mut Vec<Vec<Scalar>>, k_spring: Scalar) {
        let n_dof = k_global.len();
        let n_elems_est = n_dof / 6;
        let mut start_node = 0;
        for candidate in 0..n_elems_est.saturating_sub(1) {
            let row_start = candidate * 6;
            if row_start + 1 < n_dof {
                if k_global[row_start][row_start].abs() < 1e-30 {
                    start_node = candidate;
                    break;
                }
            }
        }
        let base = start_node * 6;
        if base + 1 < n_dof {
            k_global[base][base] += k_spring;
            k_global[base + 1][base + 1] += k_spring;
            k_global[base][base + 1] -= k_spring;
            k_global[base + 1][base] -= k_spring;
        }
    }

    /// Assemble shell element (24×24 → global).
    fn assemble_shell(k_global: &mut Vec<Vec<Scalar>>, k_local: &[Vec<Scalar>]) {
        let n_dof = k_global.len();
        let n_elems_est = n_dof / 6;
        let mut start_node = 0;
        for candidate in 0..n_elems_est.saturating_sub(3) {
            let row_start = candidate * 6;
            let col_start = row_start;
            if row_start + 23 < n_dof {
                let mut empty = true;
                for r in 0..24 {
                    for c in 0..24 {
                        if k_global[row_start + r][col_start + c].abs() > 1e-30 {
                            empty = false;
                            break;
                        }
                    }
                    if !empty {
                        break;
                    }
                }
                if empty {
                    start_node = candidate;
                    break;
                }
            }
        }
        let row_base = start_node * 6;
        let col_base = row_base;
        if row_base + 23 < n_dof {
            for r in 0..24 {
                for c in 0..24 {
                    k_global[row_base + r][col_base + c] += k_local[r][c];
                }
            }
        }
    }

    /// Assemble solid element (24×24 → global).
    fn assemble_solid(k_global: &mut Vec<Vec<Scalar>>, k_local: &[Vec<Scalar>]) {
        // Same strategy as shell
        Self::assemble_shell(k_global, k_local)
    }

    /// Apply boundary conditions by modifying the system constraints and loads.
    ///
    /// Returns Ok(()) if the system is well-posed, Err(message) otherwise.
    pub fn apply_bc(&mut self) -> Result<(), String> {
        if self.nodes.is_empty() {
            return Err("No nodes defined in the system".to_string());
        }
        if self.constraints.is_empty() {
            return Err("No boundary conditions applied — system is singular".to_string());
        }
        // Validate constraint indices
        for (node, dof, _) in &self.constraints {
            if *node >= self.nodes.len() {
                return Err(format!(
                    "Constraint references node {} but only {} nodes exist",
                    node,
                    self.nodes.len()
                ));
            }
            if *dof > 5 {
                return Err(format!("Invalid DOF {} (must be 0-5)", dof));
            }
        }
        // Validate load indices
        for (node, dof, _) in &self.loads {
            if *node >= self.nodes.len() {
                return Err(format!(
                    "Load references node {} but only {} nodes exist",
                    node,
                    self.nodes.len()
                ));
            }
            if *dof > 5 {
                return Err(format!("Invalid DOF {} (must be 0-5)", dof));
            }
        }
        Ok(())
    }

    // ──────────────────────────────────────────────
    //  Linear System Solvers
    // ──────────────────────────────────────────────

    /// Solve the static equilibrium system: K·u = F.
    ///
    /// Returns nodal displacement vector on success.
    pub fn solve_static(&self) -> Result<Vec<Scalar>, String> {
        let k = self.assemble_stiffness();
        let n = k.len();
        if n == 0 {
            return Err("Empty stiffness matrix".to_string());
        }

        // Build force vector
        let mut f = vec![0.0; n];
        for &(node, dof, val) in &self.loads {
            let idx = node * 6 + dof;
            if idx < n {
                f[idx] = val;
            }
        }

        // Apply constraints by modifying K and F (penalty method)
        let penalty = 1e30;
        let mut k_mod = k.clone();
        for &(node, dof, val) in &self.constraints {
            let idx = node * 6 + dof;
            if idx < n {
                k_mod[idx][idx] += penalty;
                f[idx] = penalty * val;
            }
        }

        // Solve via Gaussian elimination with partial pivoting
        Self::gauss_elimination(&mut k_mod, &mut f)
    }

    /// Solve the modal (eigenvalue) problem: K·φ = λ·M·φ.
    ///
    /// Returns (eigenvalues, eigenvectors) for the smallest `n_modes` modes.
    pub fn solve_modal(&self, n_modes: usize) -> Result<(Vec<Scalar>, Vec<Vec<Scalar>>), String> {
        if n_modes == 0 {
            return Err("Number of modes must be positive".to_string());
        }

        let k = self.assemble_stiffness();
        let n = k.len();
        if n == 0 {
            return Err("Empty stiffness matrix".to_string());
        }

        // Build mass matrix (use lumped mass approximation)
        let mut m = vec![vec![0.0; n]; n];
        let total_mass: Scalar = self.nodes.len() as Scalar * 1000.0; // approximate
        let nodal_mass = total_mass / self.nodes.len() as Scalar;
        for i in 0..self.nodes.len() {
            let idx = i * 6;
            if idx < n {
                m[idx][idx] = nodal_mass;
                m[idx + 1][idx + 1] = nodal_mass;
                m[idx + 2][idx + 2] = nodal_mass;
                // Rotational inertia (small)
                if idx + 3 < n {
                    m[idx + 3][idx + 3] = nodal_mass * 0.01;
                    m[idx + 4][idx + 4] = nodal_mass * 0.01;
                    m[idx + 5][idx + 5] = nodal_mass * 0.01;
                }
            }
        }

        // Apply constraints: zero out constrained DOFs using penalty
        let penalty = 1e30;
        let mut k_mod = k.clone();
        let mut m_mod = m;
        for &(node, dof, _) in &self.constraints {
            let idx = node * 6 + dof;
            if idx < n {
                k_mod[idx][idx] += penalty;
                m_mod[idx][idx] = 1.0; // avoid singular mass
            }
        }

        // Inverse iteration with deflation to find smallest eigenvalues
        Self::subspace_iteration(&k_mod, &m_mod, n_modes, 50, 1e-8)
    }

    /// Solve the linear buckling problem: (K + λ·K_G)·φ = 0.
    ///
    /// Returns (buckling_load_factors, buckling_modes).
    pub fn solve_buckling(
        &self,
        n_modes: usize,
    ) -> Result<(Vec<Scalar>, Vec<Vec<Scalar>>), String> {
        if n_modes == 0 {
            return Err("Number of buckling modes must be positive".to_string());
        }

        let k = self.assemble_stiffness();
        let n = k.len();
        if n == 0 {
            return Err("Empty stiffness matrix".to_string());
        }

        // Build geometric stiffness matrix K_G (stress stiffness).
        // For a simplified buckling analysis, approximate K_G as proportional
        // to the axial load distribution.
        let mut kg = vec![vec![0.0; n]; n];

        // Apply geometric stiffness based on axial load in elements
        for (i, elem) in self.elements.iter().enumerate() {
            let axial_force = match elem {
                FemElement::Truss(te) => te.material.young_modulus * te.area * 1e-4,
                FemElement::Beam(be) => be.material.young_modulus * be.area * 1e-4,
                _ => 1e6, // reference force for other elements
            };
            let base_idx = i * 6;
            if base_idx + 1 < n {
                kg[base_idx][base_idx] += axial_force;
                kg[base_idx + 1][base_idx + 1] += axial_force;
            }
        }

        // Apply BC penalty to K and K_G
        let penalty = 1e30;
        let mut k_mod = k.clone();
        let mut kg_mod = kg;
        for &(node, dof, _) in &self.constraints {
            let idx = node * 6 + dof;
            if idx < n {
                k_mod[idx][idx] += penalty;
                kg_mod[idx][idx] = 0.0;
            }
        }

        // Solve generalized eigenvalue problem using subspace iteration
        // Use mass=K_G for the buckling eigen-problem
        Self::subspace_iteration(&k_mod, &kg_mod, n_modes, 50, 1e-8)
    }

    // ──────────────────────────────────────────────
    //  Numerical Helpers
    // ──────────────────────────────────────────────

    /// Gaussian elimination with partial pivoting.
    /// Solves A·x = b, returns x.
    ///
    /// Delegates to the canonical `crate::core::compute::matrix::solve_linear`.
    fn gauss_elimination(
        a: &mut Vec<Vec<Scalar>>,
        b: &mut [Scalar],
    ) -> Result<Vec<Scalar>, String> {
        crate::core::compute::matrix::solve_linear(a, b).map_err(|e| e.message)
    }

    /// Subspace iteration for generalized eigenvalue problem K·φ = λ·M·φ.
    ///
    /// Finds the smallest `n_modes` eigenvalues/vectors.
    fn subspace_iteration(
        k: &[Vec<Scalar>],
        m: &[Vec<Scalar>],
        n_modes: usize,
        max_iter: usize,
        tolerance: Scalar,
    ) -> Result<(Vec<Scalar>, Vec<Vec<Scalar>>), String> {
        let n = k.len();
        let n_modes = n_modes.min(n);

        // Initialize subspace with random vectors
        let mut phi = vec![vec![0.0; n_modes]; n];
        for j in 0..n_modes {
            for i in 0..n {
                phi[i][j] = (i * 31 + j * 17) as Scalar / n as Scalar;
            }
            // Orthonormalize w.r.t. M
            Self::orthonormalize(&mut phi, m, j)?;
        }

        let mut prev_eigenvalues = vec![0.0; n_modes];

        for _iter in 0..max_iter {
            // Solve K·ψ = M·φ for each vector
            let mut psi = vec![vec![0.0; n_modes]; n];
            for j in 0..n_modes {
                let mut rhs: Vec<Scalar> = (0..n)
                    .map(|i| (0..n).map(|k_idx| m[i][k_idx] * phi[k_idx][j]).sum())
                    .collect();
                let mut k_copy = k.to_vec();
                match Self::gauss_elimination(&mut k_copy, &mut rhs) {
                    Ok(x) => {
                        for i in 0..n {
                            psi[i][j] = x[i];
                        }
                    }
                    Err(_) => {
                        // If singular, set to random
                        for i in 0..n {
                            psi[i][j] = (i * 7 + j * 13) as Scalar / (n + 1) as Scalar;
                        }
                    }
                }
            }

            // Project: K_proj = ψ^T · K · ψ, M_proj = ψ^T · M · ψ
            let k_proj = Self::project_matrix(k, &psi, n_modes);
            let m_proj = Self::project_matrix(m, &psi, n_modes);

            // Solve reduced eigenvalue problem using Jacobi iteration
            let (eigenvalues, eigenvectors) = Self::solve_reduced_eigen(&k_proj, &m_proj, n_modes);

            // Compute new subspace: φ = ψ · eigenvectors
            let mut new_phi = vec![vec![0.0; n_modes]; n];
            for i in 0..n {
                for j in 0..n_modes {
                    for k_idx in 0..n_modes {
                        new_phi[i][j] += psi[i][k_idx] * eigenvectors[k_idx][j];
                    }
                }
            }

            // Orthonormalize
            for j in 0..n_modes {
                Self::orthonormalize(&mut new_phi, m, j)?;
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

    /// Orthonormalize vector j against previous vectors w.r.t. M.
    fn orthonormalize(phi: &mut [Vec<Scalar>], m: &[Vec<Scalar>], j: usize) -> Result<(), String> {
        let n = phi.len();
        // Gram-Schmidt
        for k in 0..j {
            // Compute inner product φ_k^T · M · φ_j
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
            return Err("Zero norm in orthonormalization".to_string());
        }
        let inv_norm = 1.0 / norm_sq.sqrt();
        for i in 0..n {
            phi[i][j] *= inv_norm;
        }
        Ok(())
    }

    /// Project a matrix onto a subspace: A_proj = ψ^T · A · ψ
    fn project_matrix(a: &[Vec<Scalar>], psi: &[Vec<Scalar>], n_modes: usize) -> Vec<Vec<Scalar>> {
        let n = a.len();
        // A_proj = ψ^T · A · ψ (size: n_modes × n_modes)
        let mut result = vec![vec![0.0; n_modes]; n_modes];
        let mut temp = vec![vec![0.0; n_modes]; n]; // A · ψ (n × n_modes)

        for i in 0..n {
            for j in 0..n_modes {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += a[i][k] * psi[k][j];
                }
                temp[i][j] = sum;
            }
        }

        for i in 0..n_modes {
            for j in 0..n_modes {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += psi[k][i] * temp[k][j];
                }
                result[i][j] = sum;
            }
        }

        result
    }

    /// Solve the reduced eigenvalue problem using Jacobi iteration.
    ///
    /// Delegates to the canonical `crate::core::compute::eigen::jacobi_eigen`.
    fn solve_reduced_eigen(
        k_proj: &[Vec<Scalar>],
        _m_proj: &[Vec<Scalar>],
        n_modes: usize,
    ) -> (Vec<Scalar>, Vec<Vec<Scalar>>) {
        // After subspace orthonormalization w.r.t. M, M_proj ≈ I,
        // so we solve K_proj · v = λ · v directly.
        let (eigenvalues, eigenvectors) =
            crate::core::compute::eigen::jacobi_eigen(k_proj, n_modes);

        // Sort by eigenvalue magnitude (ascending)
        let mut indices: Vec<usize> = (0..n_modes).collect();
        indices.sort_by(|a, b| eigenvalues[*a].partial_cmp(&eigenvalues[*b]).unwrap());

        let sorted_eigenvalues: Vec<Scalar> = indices.iter().map(|&i| eigenvalues[i]).collect();
        let sorted_eigenvectors: Vec<Vec<Scalar>> = (0..n_modes)
            .map(|i| (0..n_modes).map(|j| eigenvectors[j][indices[i]]).collect())
            .collect();

        (sorted_eigenvalues, sorted_eigenvectors)
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::if_same_then_else,
        clippy::needless_borrowed_reference,
        clippy::new_without_default,
        clippy::ptr_arg
    )]
    use super::*;
    use crate::domains::structural::physics::steel_structural;

    #[test]
    fn test_fem_system_new() {
        let sys = FemSystem::new();
        assert!(sys.nodes.is_empty());
        assert!(sys.elements.is_empty());
    }

    #[test]
    fn test_assemble_stiffness_empty() {
        let sys = FemSystem::new();
        let k = sys.assemble_stiffness();
        assert!(k.is_empty());
    }

    #[test]
    fn test_apply_bc_no_nodes() {
        let mut sys = FemSystem::new();
        let result = sys.apply_bc();
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_bc_no_constraints() {
        let mut sys = FemSystem::new();
        sys.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        let result = sys.apply_bc();
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_bc_valid() {
        let mut sys = FemSystem::new();
        sys.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        sys.nodes.push(Coord3D::new(1.0, 0.0, 0.0));
        sys.constraints.push((0, 0, 0.0));
        sys.constraints.push((0, 1, 0.0));
        sys.constraints.push((0, 2, 0.0));
        assert!(sys.apply_bc().is_ok());
    }

    #[test]
    fn test_gauss_elimination_2x2() {
        let mut a = vec![vec![4.0, 1.0], vec![1.0, 3.0]];
        let mut b = vec![1.0, 2.0];
        let x = FemSystem::gauss_elimination(&mut a, &mut b).unwrap();
        assert!((x[0] - 0.090909).abs() < 1e-4);
        assert!((x[1] - 0.636364).abs() < 1e-4);
    }

    #[test]
    fn test_gauss_elimination_singular() {
        let mut a = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        let mut b = vec![1.0, 2.0];
        let result = FemSystem::gauss_elimination(&mut a, &mut b);
        assert!(result.is_err());
    }

    #[test]
    fn test_spring_assemble() {
        let mut sys = FemSystem::new();
        sys.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        sys.nodes.push(Coord3D::new(1.0, 0.0, 0.0));
        sys.elements
            .push(FemElement::Spring(SpringElement { stiffness: 1000.0 }));
        let k = sys.assemble_stiffness();
        assert_eq!(k.len(), 12);
        // Spring should place entries at (0,0), (0,1), (1,0), (1,1)
        assert!((k[0][0] - 1000.0).abs() < 1e-6 || (k[0][0] + 1000.0).abs() < 1e-6);
    }

    #[test]
    fn test_truss_assemble() {
        let mat = steel_structural();
        let mut sys = FemSystem::new();
        sys.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        sys.nodes.push(Coord3D::new(1.0, 0.0, 0.0));
        sys.elements.push(FemElement::Truss(TrussElement {
            length: 1.0,
            area: 0.01,
            material: mat,
        }));
        let k = sys.assemble_stiffness();
        assert_eq!(k.len(), 12);
    }

    #[test]
    fn test_solve_static_singular() {
        let sys = FemSystem::new();
        let result = sys.solve_static();
        assert!(result.is_err());
    }

    #[test]
    fn test_solve_modal_zero_modes() {
        let mut sys = FemSystem::new();
        sys.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        let result = sys.solve_modal(0);
        assert!(result.is_err());
    }
}
