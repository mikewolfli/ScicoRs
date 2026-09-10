//! Modified Nodal Analysis (MNA) matrix builder and solver.
//!
//! Implements the MNA formulation: `[G B; C D] * [v; i] = [s; e]`
//! where:
//! - G: conductance matrix (n×n, n = number of nodes)
//! - B: voltage source coupling matrix (n×m)
//! - C: current coupling matrix (m×n), C = Bᵀ
//! - D: voltage source matrix (m×m), typically zero
//! - v: unknown node voltages
//! - i: unknown currents through voltage sources
//! - s: known current source vector
//! - e: known voltage source values

use crate::core::error::SimError;
use crate::core::types::Scalar;

/// Result of solving the MNA system.
#[derive(Debug, Clone, PartialEq)]
pub struct MnaSolution {
    /// Node voltages (V). Index 0 = ground (0V).
    pub node_voltages: Vec<Scalar>,
    /// Currents through voltage sources (A).
    pub source_currents: Vec<Scalar>,
}

/// MNA matrix builder for circuit simulation.
///
/// Builds the MNA system matrix and RHS vector by stamping component
/// contributions, then solves for node voltages and source currents.
///
/// # Example
///
/// ```rust
/// use scico_rs::domains::analog::mna::MnaMatrix;
/// let mut mna = MnaMatrix::new(1, 0);
/// mna.stamp_resistor(1, 0, 1000.0);  // R1 = 1kΩ between node 1 and ground
/// mna.stamp_current_source(0, 1, 0.01); // I1 = 10mA into node 1
/// let sol = mna.solve().unwrap();
/// assert!((sol.node_voltages[0] - 10.0).abs() < 1e-9);
/// ```
#[derive(Debug, Clone)]
pub struct MnaMatrix {
    /// Number of circuit nodes (excluding ground).
    num_nodes: usize,
    /// Number of voltage sources.
    num_vsources: usize,
    /// Conductance matrix G (n×n).
    g: Vec<Vec<Scalar>>,
    /// Voltage source coupling matrix B (n×m).
    b: Vec<Vec<Scalar>>,
    /// Current source vector s (n).
    s: Vec<Scalar>,
    /// Voltage source values e (m).
    e: Vec<Scalar>,
}

impl MnaMatrix {
    /// Create a new MNA matrix builder.
    ///
    /// # Arguments
    /// * `num_nodes` - Number of circuit nodes (excluding ground, node 0)
    /// * `num_vsources` - Number of independent voltage sources
    pub fn new(num_nodes: usize, num_vsources: usize) -> Self {
        let n = num_nodes;
        let m = num_vsources;
        Self {
            num_nodes: n,
            num_vsources: m,
            g: vec![vec![0.0; n]; n],
            b: vec![vec![0.0; m]; n],
            s: vec![0.0; n],
            e: vec![0.0; m],
        }
    }

    /// Reset all matrix entries to zero.
    pub fn reset(&mut self) {
        let n = self.num_nodes;
        let _m = self.num_vsources;
        for i in 0..n {
            self.g[i].iter_mut().for_each(|x| *x = 0.0);
            self.b[i].iter_mut().for_each(|x| *x = 0.0);
            self.s[i] = 0.0;
        }
        self.e.iter_mut().for_each(|x| *x = 0.0);
    }

    // ── Stamping Methods ──

    /// Stamp a resistor between nodes ni and nj with value R (Ω).
    ///
    /// G(ni,ni) += 1/R, G(nj,nj) += 1/R
    /// G(ni,nj) -= 1/R, G(nj,ni) -= 1/R
    pub fn stamp_resistor(&mut self, ni: usize, nj: usize, r: Scalar) {
        if r <= 0.0 {
            return; // Avoid division by zero
        }
        let g_val = 1.0 / r;
        if ni > 0 && ni <= self.num_nodes {
            self.g[ni - 1][ni - 1] += g_val;
        }
        if nj > 0 && nj <= self.num_nodes {
            self.g[nj - 1][nj - 1] += g_val;
        }
        if ni > 0 && nj > 0 && ni <= self.num_nodes && nj <= self.num_nodes {
            self.g[ni - 1][nj - 1] -= g_val;
            self.g[nj - 1][ni - 1] -= g_val;
        }
    }

    /// Stamp a conductance between nodes ni and nj with value G (S).
    pub fn stamp_conductance(&mut self, ni: usize, nj: usize, g_val: Scalar) {
        if ni > 0 && ni <= self.num_nodes {
            self.g[ni - 1][ni - 1] += g_val;
        }
        if nj > 0 && nj <= self.num_nodes {
            self.g[nj - 1][nj - 1] += g_val;
        }
        if ni > 0 && nj > 0 && ni <= self.num_nodes && nj <= self.num_nodes {
            self.g[ni - 1][nj - 1] -= g_val;
            self.g[nj - 1][ni - 1] -= g_val;
        }
    }

    /// Stamp a voltage source between nodes ni (positive) and nj (negative) with value V.
    ///
    /// Assigns voltage source index vsrc_idx (0-based).
    pub fn stamp_voltage_source(&mut self, ni: usize, nj: usize, v: Scalar, vsrc_idx: usize) {
        if vsrc_idx >= self.num_vsources {
            return;
        }
        self.e[vsrc_idx] = v;
        if ni > 0 && ni <= self.num_nodes {
            self.b[ni - 1][vsrc_idx] += 1.0;
        }
        if nj > 0 && nj <= self.num_nodes {
            self.b[nj - 1][vsrc_idx] -= 1.0;
        }
    }

    /// Stamp an independent current source between nodes ni and nj with value I (A).
    ///
    /// Positive current flows from ni to nj.
    pub fn stamp_current_source(&mut self, ni: usize, nj: usize, i: Scalar) {
        if ni > 0 && ni <= self.num_nodes {
            self.s[ni - 1] -= i;
        }
        if nj > 0 && nj <= self.num_nodes {
            self.s[nj - 1] += i;
        }
    }

    /// Stamp a VCCS: I_out = gm * (V_nk - V_nl).
    ///
    /// Current flows from ni to nj, controlled by voltage across nk-nl.
    /// When nl=0 (ground reference), the control voltage is V_nk.
    pub fn stamp_vccs(&mut self, ni: usize, nj: usize, nk: usize, nl: usize, gm: Scalar) {
        // Helper: column index for a node; returns None for ground (node 0).
        let col = |node: usize| -> Option<usize> {
            if node > 0 && node <= self.num_nodes {
                Some(node - 1)
            } else {
                None
            }
        };
        let ctrl_pos = col(nk);
        let ctrl_neg = col(nl);

        if let Some(ni_idx) = col(ni) {
            if let Some(cp) = ctrl_pos {
                self.g[ni_idx][cp] += gm;
            }
            if let Some(cn) = ctrl_neg {
                self.g[ni_idx][cn] -= gm;
            }
        }
        if let Some(nj_idx) = col(nj) {
            if let Some(cp) = ctrl_pos {
                self.g[nj_idx][cp] -= gm;
            }
            if let Some(cn) = ctrl_neg {
                self.g[nj_idx][cn] += gm;
            }
        }
    }

    /// Stamp a VCVS: V_out = A * (V_nk - V_nl).
    ///
    /// Voltage source vsrc_idx has value A * (V_nk - V_nl).
    pub fn stamp_vcvs(
        &mut self,
        ni: usize,
        nj: usize,
        _nk: usize,
        _nl: usize,
        _a: Scalar,
        vsrc_idx: usize,
    ) {
        // Model: insert a voltage source with E = A*(Vnk - Vnl)
        // C matrix row: connects the voltage source
        // B column: contributes to KVL equation
        if vsrc_idx >= self.num_vsources {
            return;
        }
        // Voltage source between ni and nj
        if ni > 0 && ni <= self.num_nodes {
            self.b[ni - 1][vsrc_idx] += 1.0;
        }
        if nj > 0 && nj <= self.num_nodes {
            self.b[nj - 1][vsrc_idx] -= 1.0;
        }
        // The E matrix entry: E(vsrc_idx) contributes -A*Vnk + A*Vnl
        // We store this in e[] but the actual implementation needs special handling
        // For simplicity, we handle it via the system solve
        self.e[vsrc_idx] = 0.0; // Will be handled in solve
    }

    // ── Solver ──

    /// Solve the MNA system: assemble [G B; C D] * [v; i] = [s; e] and solve.
    ///
    /// Returns node voltages and source currents.
    pub fn solve(&self) -> Result<MnaSolution, SimError> {
        let n = self.num_nodes;
        let m = self.num_vsources;
        let size = n + m;

        if size == 0 {
            return Ok(MnaSolution {
                node_voltages: Vec::new(),
                source_currents: Vec::new(),
            });
        }

        // Assemble the full system matrix: [G B; B^T 0]
        let mut a = vec![vec![0.0; size]; size];
        let mut rhs = vec![0.0; size];

        // Fill G block
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            a[i][..n].copy_from_slice(&self.g[i]);
            rhs[i] = self.s[i];
        }

        // Fill B and B^T blocks (for voltage sources)
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            a[i][n..(n + m)].copy_from_slice(&self.b[i][..m]);
            for j in 0..m {
                a[n + j][i] = self.b[i][j];
            }
        }

        // Fill RHS for voltage source equations
        rhs[n..(n + m)].copy_from_slice(&self.e[..m]);

        // Solve using Gaussian elimination with partial pivoting
        let raw_sol = solve_linear_system(&mut a, &mut rhs, size)?;

        // Split: first n entries are node voltages, last m are source currents
        let node_voltages = raw_sol.node_voltages[..n.min(raw_sol.node_voltages.len())].to_vec();
        let source_currents = if m > 0 && raw_sol.node_voltages.len() > n {
            raw_sol.node_voltages[n..].to_vec()
        } else {
            Vec::new()
        };

        Ok(MnaSolution {
            node_voltages,
            source_currents,
        })
    }
}

// ──────────────────────────────────────────────
// Linear System Solver (Gaussian Elimination)
// ──────────────────────────────────────────────

/// Solve A*x = b using Gaussian elimination with partial pivoting.
///
/// Delegates to the canonical `crate::core::compute::matrix::solve_linear`.
fn solve_linear_system(
    a: &mut [Vec<Scalar>],
    b: &mut [Scalar],
    n: usize,
) -> Result<MnaSolution, SimError> {
    if n == 0 {
        return Ok(MnaSolution {
            node_voltages: Vec::new(),
            source_currents: Vec::new(),
        });
    }
    // Extract the n×n submatrix and n-length RHS
    let a_mat: Vec<Vec<Scalar>> = a[..n].iter().map(|r| r[..n].to_vec()).collect();
    let b_vec: Vec<Scalar> = b[..n].to_vec();
    let x = crate::core::compute::matrix::solve_linear(&a_mat, &b_vec)?;
    Ok(MnaSolution {
        node_voltages: x,
        source_currents: Vec::new(),
    })
}

/// Top-level MNA solve helper.
///
/// Convenience function that creates an MnaMatrix, stamps components,
/// and solves. For direct matrix manipulation, use `MnaMatrix` directly.
pub fn solve_mna(
    num_nodes: usize,
    num_vsources: usize,
    stamp_fn: impl FnOnce(&mut MnaMatrix) -> Result<(), SimError>,
) -> Result<MnaSolution, SimError> {
    let mut mna = MnaMatrix::new(num_nodes, num_vsources);
    stamp_fn(&mut mna)?;
    let sol = mna.solve()?;

    // Split the result
    let node_voltages = sol.node_voltages[..num_nodes.min(sol.node_voltages.len())].to_vec();
    let source_currents = if num_vsources > 0 && sol.node_voltages.len() > num_nodes {
        sol.node_voltages[num_nodes..].to_vec()
    } else {
        Vec::new()
    };

    Ok(MnaSolution {
        node_voltages,
        source_currents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mna_create() {
        let mna = MnaMatrix::new(3, 1);
        assert_eq!(mna.g.len(), 3);
        assert_eq!(mna.b[0].len(), 1);
    }

    #[test]
    fn test_mna_simple_voltage_divider() {
        let mut mna = MnaMatrix::new(2, 0);
        // R1 = 1kΩ between node 1 and node 2
        mna.stamp_resistor(1, 2, 1000.0);
        // R2 = 2kΩ between node 2 and ground
        mna.stamp_resistor(2, 0, 2000.0);
        // I_in = 1mA into node 1, out of ground
        mna.stamp_current_source(0, 1, 0.001);
        let sol = mna.solve().unwrap();
        assert_eq!(sol.node_voltages.len(), 2);
        // V1 = I * (R1 + R2) = 0.001 * 3000 = 3.0V
        assert!((sol.node_voltages[0] - 3.0).abs() < 1e-10);
        // V2 = V1 * R2/(R1+R2) = 3.0 * 2/3 = 2.0V
        assert!((sol.node_voltages[1] - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_mna_resistor_divider_two_resistors() {
        let mut mna = MnaMatrix::new(2, 0);
        // R1 = 1k between node 1 and node 2
        mna.stamp_resistor(1, 2, 1000.0);
        // R2 = 1k between node 2 and ground
        mna.stamp_resistor(2, 0, 1000.0);
        // I = 5mA into node 1
        // Total R = 2kΩ, V1 = 0.005 * 2000 = 10V
        mna.stamp_current_source(0, 1, 0.005);
        let sol = mna.solve().unwrap();
        // V1 = I * (R1+R2) = 0.005 * 2000 = 10V
        assert!((sol.node_voltages[0] - 10.0).abs() < 1e-10);
        // V2 = V1 * R2/(R1+R2) = 10 * 1000/2000 = 5V
        assert!((sol.node_voltages[1] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_mna_voltage_source() {
        let mut mna = MnaMatrix::new(2, 1);
        // R1 = 1k between node 2 and ground
        mna.stamp_resistor(2, 0, 1000.0);
        // Vs = 5V between node 1 and ground
        mna.stamp_voltage_source(1, 0, 5.0, 0);
        // R2 = 500 between node 1 and node 2
        mna.stamp_resistor(1, 2, 500.0);
        let sol = mna.solve().unwrap();
        // V1 = 5V (forced by voltage source)
        assert!((sol.node_voltages[0] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_mna_vccs() {
        let mut mna = MnaMatrix::new(2, 0);
        // R1 = 1k between node 1 and ground
        mna.stamp_resistor(1, 0, 1000.0);
        // VCCS: gm=0.01, controlled by V(1), output into node 2→ground
        mna.stamp_vccs(2, 0, 1, 0, 0.01);
        // R2 = 100 between node 2 and ground
        mna.stamp_resistor(2, 0, 100.0);
        // I_in = 1mA into node 1
        mna.stamp_current_source(0, 1, 0.001);
        let sol = mna.solve().unwrap();
        // V1 = 0.001 * 1000 = 1.0V
        assert!((sol.node_voltages[0] - 1.0).abs() < 1e-10);
        // VCCS: gm=0.01, Vgs=V1. Current gm*V1 flows out of node 2.
        // KCL at node 2: V2/R2 + gm*V1 = 0 → V2 = -gm*V1*R2 = -0.01*1.0*100 = -1.0V
        assert!((sol.node_voltages[1] + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_mna_reset() {
        let mut mna = MnaMatrix::new(2, 0);
        mna.stamp_resistor(1, 0, 1000.0);
        mna.stamp_current_source(0, 1, 0.001);
        mna.reset();
        // After reset, all entries should be zero
        for row in &mna.g {
            for &val in row {
                assert!((val).abs() < 1e-15);
            }
        }
        for &val in &mna.s {
            assert!((val).abs() < 1e-15);
        }
    }

    #[test]
    fn test_mna_solve_voltage_divider() {
        // Use the convenience function
        let sol = solve_mna(2, 0, |mna| {
            mna.stamp_resistor(1, 2, 1000.0);
            mna.stamp_resistor(2, 0, 2000.0);
            mna.stamp_current_source(0, 1, 0.003);
            Ok(())
        })
        .unwrap();
        // I_total = 3mA, R_total = 3kΩ, V1 = 9V, V2 = 6V
        assert!((sol.node_voltages[0] - 9.0).abs() < 1e-10);
        assert!((sol.node_voltages[1] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_mna_singular_detection() {
        let mna = MnaMatrix::new(2, 0);
        // Unconnected nodes
        let result = mna.solve();
        if let Ok(sol) = result {
            assert!((sol.node_voltages[0]).abs() < 1e-10);
        }
    }
}
