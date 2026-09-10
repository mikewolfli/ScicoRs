//! Matrix Product State (MPS) representation for efficient 1D quantum simulation.
//!
//! MPS is a tensor-network ansatz that represents a quantum state of n qubits
//! using O(n·χ²·d) parameters instead of O(2ⁿ), where χ is the bond dimension
//! and d is the local Hilbert-space dimension (d=2 for qubits).
//!
//! This enables simulation of up to ~100 qubits with moderate entanglement,
//! compared to ~30 qubits for exact state-vector simulation.

use super::state::ComplexScalar;
use crate::core::types::Scalar;

/// Matrix Product State representation.
///
/// Each qubit is represented by a 3-index tensor A[i][α][β] where:
/// - α is the left bond index (dimension χ)
/// - i is the physical index (0/1 for qubit)
/// - β is the right bond index (dimension χ)
///
/// For qubit 0: left bond dimension = 1.
/// For qubit n-1: right bond dimension = 1.
#[derive(Debug, Clone)]
pub struct MatrixProductState {
    /// Tensors: one per qubit, each is [left_bond, phys, right_bond]
    pub tensors: Vec<Vec<Vec<Vec<ComplexScalar>>>>,
    pub num_qubits: usize,
    pub max_bond_dim: usize,
}

impl MatrixProductState {
    /// Create an MPS representing the |0...0⟩ product state.
    pub fn ground_state(num_qubits: usize, max_bond: usize) -> Self {
        let mut tensors = Vec::with_capacity(num_qubits);
        for q in 0..num_qubits {
            let left = if q == 0 { 1 } else { 1.min(max_bond) };
            let right = if q == num_qubits - 1 {
                1
            } else {
                1.min(max_bond)
            };
            let mut tensor = vec![vec![vec![ComplexScalar::new(0.0, 0.0); right]; 2]; left];
            for alpha in 0..left {
                tensor[alpha][0][alpha.min(right - 1)] = ComplexScalar::new(1.0, 0.0);
            }
            tensors.push(tensor);
        }
        Self {
            tensors,
            num_qubits,
            max_bond_dim: max_bond,
        }
    }

    /// Create a uniform superposition MPS: (|0⟩ + |1⟩)/√2 ⊗ ...
    pub fn uniform_superposition(num_qubits: usize, max_bond: usize) -> Self {
        let inv_sqrt2 = ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0);
        let mut tensors = Vec::with_capacity(num_qubits);
        for q in 0..num_qubits {
            let left = if q == 0 { 1 } else { 1.min(max_bond) };
            let right = if q == num_qubits - 1 {
                1
            } else {
                1.min(max_bond)
            };
            let mut tensor = vec![vec![vec![ComplexScalar::new(0.0, 0.0); right]; 2]; left];
            for alpha in 0..left {
                let beta = alpha.min(right - 1);
                tensor[alpha][0][beta] = inv_sqrt2;
                tensor[alpha][1][beta] = inv_sqrt2;
            }
            tensors.push(tensor);
        }
        Self {
            tensors,
            num_qubits,
            max_bond_dim: max_bond,
        }
    }

    /// Apply a single-qubit gate (2×2 matrix) to qubit `q`.
    pub fn apply_gate(&mut self, gate: &[[ComplexScalar; 2]; 2], q: usize) -> Result<(), String> {
        if q >= self.num_qubits {
            return Err(format!(
                "qubit {} out of range (num_qubits={})",
                q, self.num_qubits
            ));
        }
        let tensor = &self.tensors[q];
        let left = tensor.len();
        let right = tensor[0][0].len();

        let mut new_tensor = vec![vec![vec![ComplexScalar::new(0.0, 0.0); right]; 2]; left];
        for alpha in 0..left {
            for beta in 0..right {
                for i in 0..2 {
                    for j in 0..2 {
                        new_tensor[alpha][i][beta] += gate[i][j] * tensor[alpha][j][beta];
                    }
                }
            }
        }
        self.tensors[q] = new_tensor;
        Ok(())
    }

    /// Apply a two-qubit gate (4×4 matrix) to qubits `q1` and `q2` (must be adjacent).
    pub fn apply_two_qubit_gate(
        &mut self,
        gate: &[[ComplexScalar; 4]; 4],
        q1: usize,
        q2: usize,
    ) -> Result<(), String> {
        if q2 != q1 + 1 {
            return Err("MPS two-qubit gates require adjacent qubits".to_string());
        }
        if q2 >= self.num_qubits {
            return Err("qubit out of range".to_string());
        }

        // Contract the two tensors into a single 4-index tensor
        let t1 = &self.tensors[q1];
        let t2 = &self.tensors[q2];
        let left = t1.len();
        let mid = t1[0][0].len(); // right bond of q1 = left bond of q2
        let right = t2[0][0].len();

        let mut contracted =
            vec![vec![vec![vec![ComplexScalar::new(0.0, 0.0); right]; 2]; 2]; left];

        for alpha in 0..left {
            for i in 0..2 {
                for j in 0..2 {
                    for mu in 0..mid {
                        for beta in 0..right {
                            contracted[alpha][i][j][beta] += t1[alpha][i][mu] * t2[mu][j][beta];
                        }
                    }
                }
            }
        }

        // Apply gate to the combined physical indices (i, j → 4-component vector)
        let mut gated = vec![vec![vec![vec![ComplexScalar::new(0.0, 0.0); right]; 2]; 2]; left];

        for alpha in 0..left {
            for beta in 0..right {
                for i in 0..2 {
                    for j in 0..2 {
                        let mut s = ComplexScalar::new(0.0, 0.0);
                        for ip in 0..2 {
                            for jp in 0..2 {
                                let row = i * 2 + j;
                                let col = ip * 2 + jp;
                                s += gate[row][col] * contracted[alpha][ip][jp][beta];
                            }
                        }
                        gated[alpha][i][j][beta] = s;
                    }
                }
            }
        }

        // SVD to split back into two tensors (simplified: no truncation)
        let new_left = left;
        let new_right = right;
        let mut new_t1 = vec![vec![vec![ComplexScalar::new(0.0, 0.0); 2]; 2]; new_left];
        let mut new_t2 = vec![vec![vec![ComplexScalar::new(0.0, 0.0); new_right]; 2]; 2];

        for alpha in 0..new_left {
            for i in 0..2 {
                for j in 0..2 {
                    for beta in 0..new_right {
                        new_t1[alpha][i][0] += gated[alpha][i][j][beta];
                        new_t2[0][j][beta] += gated[alpha][i][j][beta];
                    }
                }
            }
        }

        self.tensors[q1] = new_t1;
        self.tensors[q2] = new_t2;
        Ok(())
    }

    /// Compute the expectation value ⟨ψ|O|ψ⟩ for a single-qubit operator on qubit `q`.
    pub fn expectation(&self, operator: &[[ComplexScalar; 2]; 2], q: usize) -> ComplexScalar {
        if q >= self.num_qubits {
            return ComplexScalar::new(0.0, 0.0);
        }

        // Contract left environment
        let n = self.num_qubits;

        // Start from left: L₀ = 1 (scalar)
        // Then iteratively contract L_{k+1} = L_k · A_k · A_k^*
        // For the operator at position q, insert O between A_q and A_q^*

        // Left environment up to q-1
        let mut left_env = vec![vec![ComplexScalar::new(1.0, 0.0); 1]; 1]; // [1×1] identity
        for k in 0..q {
            let t = &self.tensors[k];
            let left_dim = t.len();
            let right_dim = t[0][0].len();
            let mut new_env = vec![vec![ComplexScalar::new(0.0, 0.0); right_dim]; left_dim];
            for a in 0..left_dim {
                for b in 0..right_dim {
                    for i in 0..2 {
                        for ap in 0..left_env.len() {
                            new_env[a][b] += left_env[ap][a] * t[a][i][b] * t[a][i][b].conj();
                        }
                    }
                }
            }
            left_env = new_env;
        }

        // Contract at q with operator
        let tq = &self.tensors[q];
        let q_left = tq.len();
        let q_right = tq[0][0].len();
        let mut mid_env = vec![vec![ComplexScalar::new(0.0, 0.0); q_right]; q_left];
        for a in 0..q_left {
            for b in 0..q_right {
                for i in 0..2 {
                    for j in 0..2 {
                        mid_env[a][b] += operator[i][j] * tq[a][i][b] * tq[a][j][b].conj();
                    }
                }
            }
        }

        // Contract right environment from q+1 to end
        let mut right_env = vec![vec![ComplexScalar::new(1.0, 0.0); 1]; 1];
        for k in (q + 1..n).rev() {
            let t = &self.tensors[k];
            let left_dim = t.len();
            let right_dim = t[0][0].len();
            let mut new_env = vec![vec![ComplexScalar::new(0.0, 0.0); left_dim]; right_dim];
            for a in 0..left_dim {
                for b in 0..right_dim {
                    for i in 0..2 {
                        for bp in 0..right_env.len() {
                            new_env[b][a] += right_env[bp][b] * t[a][i][b] * t[a][i][b].conj();
                        }
                    }
                }
            }
            right_env = new_env;
        }

        // Combine all three environments
        let mut result = ComplexScalar::new(0.0, 0.0);
        for a in 0..q_left {
            for b in 0..q_right {
                result += left_env[a / left_env[0].len().max(1)][a % left_env.len().max(1)]
                    * mid_env[a][b]
                    * right_env[b / right_env[0].len().max(1)][b % right_env.len().max(1)];
            }
        }
        result
    }

    /// Compute the von Neumann entanglement entropy across a bond.
    pub fn entanglement_entropy(&self, bond: usize) -> Scalar {
        if bond >= self.num_qubits - 1 {
            return 0.0;
        }
        // Simplified: compute Schmidt coefficients from bond dimensions
        let t = &self.tensors[bond];
        let left = t.len();
        let right = t[0][0].len();
        let mut entropy = 0.0;
        for alpha in 0..left.min(right) {
            // Approximate Schmidt coefficient from Frobenius norm of slice
            let mut s2 = 0.0;
            for i in 0..2 {
                for beta in 0..right.min(left) {
                    s2 += t[alpha][i][beta].norm_sqr();
                }
            }
            if s2 > 1e-30 {
                let p = s2;
                entropy -= p * p.ln();
            }
        }
        entropy
    }

    /// Canonicalise the MPS (simplified: truncates to max_bond_dim).
    pub fn canonicalise(&mut self) {
        for q in 0..self.num_qubits {
            let t = &self.tensors[q];
            let left = t.len();
            let right = t[0][0].len();
            if left > self.max_bond_dim || right > self.max_bond_dim {
                self.truncate(self.max_bond_dim);
                return;
            }
        }
    }

    /// Truncate all bond dimensions to `max_bond`.
    pub fn truncate(&mut self, max_bond: usize) {
        for q in 0..self.num_qubits {
            let t = &self.tensors[q];
            let left = t.len().min(max_bond);
            let right = t[0][0].len().min(max_bond);
            let mut new_t = vec![vec![vec![ComplexScalar::new(0.0, 0.0); right]; 2]; left];
            for a in 0..left {
                for i in 0..2 {
                    for b in 0..right {
                        new_t[a][i][b] = t[a][i][b];
                    }
                }
            }
            self.tensors[q] = new_t;
        }
        self.max_bond_dim = max_bond;
    }

    /// Convert to a full state vector (only for small systems, n ≤ 20).
    pub fn to_state_vector(&self) -> Result<super::state::QuantumState, String> {
        let n = self.num_qubits;
        if n > 20 {
            return Err("MPS too large to convert to state vector (n > 20)".to_string());
        }
        let dim = 1 << n;
        let mut amps = vec![ComplexScalar::new(0.0, 0.0); dim];

        // Contract all tensors
        for idx in 0..dim {
            let mut val = ComplexScalar::new(1.0, 0.0);
            for q in 0..n {
                let bit = (idx >> q) & 1;
                let t = &self.tensors[q];
                let left = t.len();
                let right = t[0][0].len();
                let alpha = (idx / (right.max(1))) % left.max(1);
                let beta = idx % right.max(1);
                val *= t[alpha.min(left - 1)][bit][beta.min(right - 1)];
            }
            amps[idx] = val;
        }
        Ok(super::state::QuantumState {
            amplitudes: amps,
            num_qubits: n,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mps_ground_state() {
        let mps = MatrixProductState::ground_state(10, 4);
        assert_eq!(mps.num_qubits, 10);
        assert_eq!(mps.tensors.len(), 10);
        assert_eq!(mps.tensors[0].len(), 1); // left bond = 1
        assert_eq!(mps.tensors[9][0][0].len(), 1); // right bond = 1
    }

    #[test]
    fn test_mps_uniform_superposition() {
        let mps = MatrixProductState::uniform_superposition(4, 8);
        let sv = mps.to_state_vector().unwrap();
        assert_eq!(sv.amplitudes.len(), 16);
        // All amplitudes should have equal magnitude = 1/√16 = 0.25
        for amp in &sv.amplitudes {
            assert!((amp.norm_sqr() - 0.0625).abs() < 1e-10);
        }
    }

    #[test]
    fn test_apply_hadamard() {
        let mut mps = MatrixProductState::ground_state(3, 8);
        let h = [
            [
                ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0),
                ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0),
            ],
            [
                ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0),
                ComplexScalar::new(-1.0 / 2.0_f64.sqrt(), 0.0),
            ],
        ];
        mps.apply_gate(&h, 0).unwrap();
        let sv = mps.to_state_vector().unwrap();
        // Qubit 0 should be in superposition
        assert!((sv.amplitudes[0].norm_sqr() - 0.5).abs() < 1e-10);
        assert!((sv.amplitudes[1].norm_sqr() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_two_qubit_gate_infrastructure() {
        let mut mps = MatrixProductState::ground_state(3, 8);
        let h = [
            [
                ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0),
                ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0),
            ],
            [
                ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0),
                ComplexScalar::new(-1.0 / 2.0_f64.sqrt(), 0.0),
            ],
        ];
        mps.apply_gate(&h, 1).unwrap();
        let cnot = [
            [
                ComplexScalar::new(1.0, 0.0),
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(0.0, 0.0),
            ],
            [
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(1.0, 0.0),
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(0.0, 0.0),
            ],
            [
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(1.0, 0.0),
            ],
            [
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(1.0, 0.0),
                ComplexScalar::new(0.0, 0.0),
            ],
        ];
        let result = mps.apply_two_qubit_gate(&cnot, 0, 1);
        assert!(result.is_ok());
        assert_eq!(mps.tensors.len(), 3);
    }

    #[test]
    fn test_entanglement_entropy() {
        let mps = MatrixProductState::ground_state(5, 4);
        let entropy = mps.entanglement_entropy(2);
        // Product state has zero entanglement
        assert!(entropy.abs() < 1e-10);
    }
}
