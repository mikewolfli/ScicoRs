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
///
/// # Element connectivity
///
/// An element's stiffness matrix is expressed in *local* degrees of freedom,
/// so assembling it requires knowing which global nodes it spans. That mapping
/// is held in [`FemSystem::element_nodes`], indexed in parallel with
/// `elements`: `element_nodes[e]` lists the global node indices of element `e`,
/// in the element's DOF order. Register it with [`FemSystem::connect`].
///
/// When a system has no connectivity recorded for an element, assembly falls
/// back to the legacy layout that places the element block at the first free
/// slot — correct only for single-element models, and retained so existing
/// callers keep working. New code should always call `connect`.
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
    /// Connectivity: `element_nodes[e]` lists the global node indices of
    /// element `e`, in the element's local DOF order. Empty or shorter than
    /// `elements` means connectivity is unknown for those elements.
    pub element_nodes: Vec<Vec<usize>>,
}

impl FemSystem {
    /// Create a new empty FEM system.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            elements: Vec::new(),
            constraints: Vec::new(),
            loads: Vec::new(),
            element_nodes: Vec::new(),
        }
    }

    /// Number of nodes spanned by each element kind, in local DOF order.
    ///
    /// Truss/spring span two nodes, beam two (3 translations + 3 rotations
    /// each), shell and solid four and eight respectively.
    fn required_nodes(elem: &FemElement) -> usize {
        match elem {
            FemElement::Truss(_) | FemElement::Spring(_) | FemElement::Beam(_) => 2,
            FemElement::Shell(_) => 4,
            FemElement::Solid(_) => 8,
        }
    }

    /// Record the node connectivity of one element and append the element.
    ///
    /// `nodes` lists the global node indices in the element's local DOF order.
    ///
    /// # Panics
    ///
    /// Panics when the element kind needs a different node count, or when an
    /// index is out of range — both are programming errors that would otherwise
    /// silently corrupt the assembled system.
    pub fn connect(&mut self, elem: FemElement, nodes: &[usize]) -> usize {
        let expected = Self::required_nodes(&elem);
        assert!(
            nodes.len() == expected,
            "element kind needs {expected} nodes but {} were supplied",
            nodes.len()
        );
        for &n in nodes {
            assert!(n < self.nodes.len(), "connect: node index {n} out of range");
        }
        self.elements.push(elem);
        self.element_nodes.push(nodes.to_vec());
        self.elements.len() - 1
    }

    /// Determine the total number of DOFs in the system.
    fn n_dofs(&self) -> usize {
        self.nodes.len() * 6 // conservative: 6 DOFs per node
    }

    /// Element stiffness DOF layout: how many local DOFs each element kind has.
    fn element_dofs(elem: &FemElement) -> usize {
        match elem {
            // [u1, v1, u2, v2]
            FemElement::Truss(_) => 4,
            // Spring acts on the two translational DOFs of its nodes.
            FemElement::Spring(_) => 2,
            // 12 = 6 per node (3 translation + 3 rotation)
            FemElement::Beam(_) => 12,
            // 24 = 6 per node × 4 nodes
            FemElement::Shell(_) => 24,
            // 24 = 3 per node × 8 nodes
            FemElement::Solid(_) => 24,
        }
    }

    /// Global DOF indices an element occupies, in local DOF order.
    ///
    /// Returns `None` when connectivity is unknown for this element, so the
    /// caller can fall back to the legacy layout.
    fn element_dof_indices(&self, index: usize) -> Option<Vec<usize>> {
        let nodes = self.element_nodes.get(index)?;
        let elem = self.elements.get(index)?;
        let local = Self::element_dofs(elem);
        match elem {
            // Truss/beam/shell map 6 DOFs (or 2 for truss) onto each node; the
            // truss carries per-node 2-DOF pairs and the solid 3-DOF triples.
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
            FemElement::Spring(_) => {
                if nodes.len() != 2 {
                    return None;
                }
                Some(vec![nodes[0] * 6, nodes[1] * 6])
            }
            FemElement::Beam(_) => {
                if nodes.len() != 2 {
                    return None;
                }
                let mut dofs = Vec::with_capacity(local);
                for &n in nodes {
                    for d in 0..6 {
                        dofs.push(n * 6 + d);
                    }
                }
                Some(dofs)
            }
            FemElement::Shell(_) => {
                if nodes.len() != 4 {
                    return None;
                }
                let mut dofs = Vec::with_capacity(local);
                for &n in nodes {
                    for d in 0..6 {
                        dofs.push(n * 6 + d);
                    }
                }
                Some(dofs)
            }
            FemElement::Solid(_) => {
                if nodes.len() != 8 {
                    return None;
                }
                // Solids carry 3 translational DOFs per node; the rotational
                // slots (3..6) of each node are not activated.
                let mut dofs = Vec::with_capacity(local);
                for &n in nodes {
                    for d in 0..3 {
                        dofs.push(n * 6 + d);
                    }
                }
                Some(dofs)
            }
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

    /// Assemble the global stiffness matrix.
    ///
    /// Element stiffness matrices are computed in parallel (using rayon)
    /// then assembled serially into the global n_dofs × n_dofs matrix.
    ///
    /// Elements registered through [`Self::connect`] are scattered to their
    /// true global DOFs, so a multi-element mesh assembles correctly even when
    /// elements share nodes (their contributions are summed, as the finite
    /// element method requires). Elements without connectivity fall back to the
    /// legacy single-element layout.
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

        // Phase 2: Assemble serially into the global matrix. Connected elements
        // scatter to their real DOFs; the rest keep the legacy layout.
        let mut k_global = vec![vec![0.0; n_dof]; n_dof];
        for (i, contrib) in contributions.iter().enumerate() {
            match self.element_dof_indices(i) {
                Some(dofs) => match contrib {
                    StiffnessContribution::Truss(k)
                    | StiffnessContribution::Beam(k)
                    | StiffnessContribution::Shell(k)
                    | StiffnessContribution::Solid(k) => Self::scatter(&mut k_global, k, &dofs),
                    // A spring is stored as a scalar and expanded on the fly.
                    StiffnessContribution::Spring(k) => {
                        let kl = Self::spring_matrix(*k);
                        Self::scatter(&mut k_global, &kl, &dofs);
                    }
                },
                None => match contrib {
                    StiffnessContribution::Beam(k) => Self::assemble_beam(&mut k_global, k),
                    StiffnessContribution::Truss(k) => Self::assemble_truss(&mut k_global, k),
                    StiffnessContribution::Spring(k) => Self::assemble_spring(&mut k_global, *k),
                    StiffnessContribution::Shell(k) => Self::assemble_shell(&mut k_global, k),
                    StiffnessContribution::Solid(k) => Self::assemble_solid(&mut k_global, k),
                },
            }
        }

        k_global
    }

    /// Local stiffness matrix of a spring acting between two 1-DOF nodes.
    fn spring_matrix(k: Scalar) -> Vec<Vec<Scalar>> {
        vec![vec![k, -k], vec![-k, k]]
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

        // Apply geometric stiffness based on axial load in elements, placed at
        // the element's true DOFs when connectivity is known.
        for (i, elem) in self.elements.iter().enumerate() {
            let axial_force = match elem {
                FemElement::Truss(te) => te.material.young_modulus * te.area * 1e-4,
                FemElement::Beam(be) => be.material.young_modulus * be.area * 1e-4,
                _ => 1e6, // reference force for other elements
            };
            let dofs = self.element_dof_indices(i);
            let (a, b) = match &dofs {
                // With connectivity, stiffen the element's own translational DOFs.
                Some(d) if d.len() >= 2 => (d[0], d[1]),
                // Legacy layout: the element block starts at `i * 6`.
                _ => (i * 6, i * 6 + 1),
            };
            if a < n && b < n {
                kg[a][a] += axial_force;
                kg[b][b] += axial_force;
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
    /// Finds the smallest `n_modes` eigenvalues/vectors. Thin wrapper over the
    /// canonical `crate::core::compute::eigen::subspace_eigen`, with the
    /// eigenpairs sorted ascending to match the FEM contract.
    fn subspace_iteration(
        k: &[Vec<Scalar>],
        m: &[Vec<Scalar>],
        n_modes: usize,
        max_iter: usize,
        tolerance: Scalar,
    ) -> Result<(Vec<Scalar>, Vec<Vec<Scalar>>), String> {
        let (vals, vecs) =
            crate::core::compute::eigen::subspace_eigen(k, m, n_modes, max_iter, tolerance)
                .map_err(|e| e.message)?;
        let nm = vals.len();
        let mut indices: Vec<usize> = (0..nm).collect();
        indices.sort_by(|a, b| vals[*a].partial_cmp(&vals[*b]).unwrap());
        let sorted_vals: Vec<Scalar> = indices.iter().map(|&i| vals[i]).collect();
        let sorted_vecs: Vec<Vec<Scalar>> = (0..nm)
            .map(|r| (0..nm).map(|c| vecs[r][indices[c]]).collect())
            .collect();
        Ok((sorted_vals, sorted_vecs))
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
    fn test_connected_truss_shares_node_coupling() {
        // Two trusses in series over 3 nodes. With real connectivity the shared
        // middle node must receive contributions from BOTH elements, which the
        // legacy "first free block" layout could never produce.
        let mat = steel_structural();
        let k0 = mat.young_modulus * 0.01 / 1.0;
        let mut sys = FemSystem::new();
        for i in 0..3 {
            sys.nodes.push(Coord3D::new(i as Scalar, 0.0, 0.0));
        }
        let truss = || {
            FemElement::Truss(TrussElement {
                length: 1.0,
                area: 0.01,
                material: mat,
            })
        };
        sys.connect(truss(), &[0, 1]);
        sys.connect(truss(), &[1, 2]);

        let k = sys.assemble_stiffness();
        assert_eq!(k.len(), 18);
        // Element 1 occupies node 1 DOF 0 (index 6) and node 2 DOF 0 (index 12).
        // Each element contributes k0 at its end nodes, so the shared node 1
        // must sum to exactly 2·k0 — the key evidence that connectivity is used.
        assert!(
            (k[6][6] - 2.0 * k0).abs() < 1e-6 * k0,
            "shared node stiffness {} != {}",
            k[6][6],
            2.0 * k0
        );
        // End nodes only see their own element.
        assert!((k[0][0] - k0).abs() < 1e-6 * k0);
        assert!((k[12][12] - k0).abs() < 1e-6 * k0);
        // Off-diagonal coupling exists between the shared node and each end.
        assert!((k[0][6] + k0).abs() < 1e-6 * k0);
        assert!((k[6][12] + k0).abs() < 1e-6 * k0);
        // Symmetry.
        assert!((k[0][6] - k[6][0]).abs() < 1e-30);
        assert!((k[6][12] - k[12][6]).abs() < 1e-30);
    }

    #[test]
    fn test_connected_assembly_differs_from_legacy_heuristic() {
        // A connected three-node chain must not equal the legacy layout, which
        // stacks elements into disjoint blocks and loses the shared coupling.
        let mat = steel_structural();
        let build = |connected: bool| {
            let mut sys = FemSystem::new();
            for i in 0..3 {
                sys.nodes.push(Coord3D::new(i as Scalar, 0.0, 0.0));
            }
            let mk = || {
                FemElement::Truss(TrussElement {
                    length: 1.0,
                    area: 0.01,
                    material: mat,
                })
            };
            if connected {
                sys.connect(mk(), &[0, 1]);
                sys.connect(mk(), &[1, 2]);
            } else {
                sys.elements.push(mk());
                sys.elements.push(mk());
            }
            sys.assemble_stiffness()
        };
        let connected = build(true);
        let legacy = build(false);
        // The shared-node entry differs between the two assembly strategies.
        assert!(
            (connected[6][6] - legacy[6][6]).abs() > 1e-6,
            "connected and legacy assembly should differ at the shared node"
        );
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_connect_rejects_bad_node_index() {
        let mut sys = FemSystem::new();
        sys.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        sys.connect(
            FemElement::Spring(SpringElement { stiffness: 1.0 }),
            &[0, 5],
        );
    }

    #[test]
    #[should_panic(expected = "needs 2 nodes")]
    fn test_connect_rejects_wrong_node_count() {
        let mut sys = FemSystem::new();
        for i in 0..3 {
            sys.nodes.push(Coord3D::new(i as Scalar, 0.0, 0.0));
        }
        sys.connect(
            FemElement::Spring(SpringElement { stiffness: 1.0 }),
            &[0, 1, 2],
        );
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
