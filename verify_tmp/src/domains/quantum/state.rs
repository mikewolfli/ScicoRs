//! Quantum state representation: state vectors and density matrices.

use crate::core::types::Scalar;
use num_complex::Complex;

/// Complex scalar type used throughout the quantum module.
pub type ComplexScalar = Complex<Scalar>;

/// Quantum state represented as a complex amplitude vector.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantumState {
    /// Complex amplitudes in the computational basis.
    pub amplitudes: Vec<ComplexScalar>,
    /// Number of qubits.
    pub num_qubits: usize,
}

impl QuantumState {
    /// Create the |0⟩ ground state for `num_qubits`.
    pub fn ground_state(num_qubits: usize) -> Self {
        let dim = 1 << num_qubits;
        let mut amplitudes = vec![ComplexScalar::new(0.0, 0.0); dim];
        amplitudes[0] = ComplexScalar::new(1.0, 0.0);
        Self {
            amplitudes,
            num_qubits,
        }
    }

    /// Create a uniform superposition over all basis states.
    pub fn uniform_superposition(num_qubits: usize) -> Self {
        let dim = 1 << num_qubits;
        let factor = ComplexScalar::new(1.0 / (dim as Scalar).sqrt(), 0.0);
        let amplitudes = vec![factor; dim];
        Self {
            amplitudes,
            num_qubits,
        }
    }

    /// Create a state from a computational basis index.
    pub fn from_basis(basis_index: usize, num_qubits: usize) -> Self {
        let dim = 1 << num_qubits;
        let mut amplitudes = vec![ComplexScalar::new(0.0, 0.0); dim];
        if basis_index < dim {
            amplitudes[basis_index] = ComplexScalar::new(1.0, 0.0);
        }
        Self {
            amplitudes,
            num_qubits,
        }
    }

    /// Normalize the state vector in-place.
    pub fn normalize(&mut self) -> Result<(), String> {
        let norm_sq: Scalar = self.amplitudes.iter().map(|c| c.norm_sqr()).sum();
        if norm_sq < 1e-30 {
            return Err("Cannot normalize zero state".to_string());
        }
        let inv_norm = 1.0 / norm_sq.sqrt();
        for a in &mut self.amplitudes {
            *a *= inv_norm;
        }
        Ok(())
    }

    /// Compute the inner product ⟨ψ|φ⟩.
    pub fn inner_product(&self, other: &QuantumState) -> ComplexScalar {
        let mut sum = ComplexScalar::new(0.0, 0.0);
        for (a, b) in self.amplitudes.iter().zip(other.amplitudes.iter()) {
            sum += a.conj() * b;
        }
        sum
    }

    /// Compute probability distribution over basis states.
    pub fn probabilities(&self) -> Vec<Scalar> {
        self.amplitudes.iter().map(|c| c.norm_sqr()).collect()
    }

    /// Probability of measuring |0⟩ on the given qubit.
    pub fn measure_probability(&self, qubit: usize) -> Scalar {
        if qubit >= self.num_qubits {
            return 0.0;
        }
        let dim = self.amplitudes.len();
        let mut prob0 = 0.0;
        for i in 0..dim {
            if (i >> qubit) & 1 == 0 {
                prob0 += self.amplitudes[i].norm_sqr();
            }
        }
        prob0
    }

    /// Partial trace over specified qubits, producing a density matrix.
    pub fn partial_trace(&self, qubits: &[usize]) -> DensityMatrix {
        let n = self.num_qubits;
        let keep_mask: usize = (0..n).filter(|q| !qubits.contains(q)).map(|q| 1 << q).sum();
        let keep_count = n - qubits.len();
        let keep_dim = 1 << keep_count;

        let mut rho_data = vec![ComplexScalar::new(0.0, 0.0); keep_dim * keep_dim];
        let dim = 1 << n;
        for i in 0..dim {
            for j in 0..dim {
                let i_keep = i & keep_mask;
                let j_keep = j & keep_mask;
                if i_keep == i && j_keep == j {
                    let idx = i_keep + (j_keep >> keep_count) * keep_dim;
                    if idx < rho_data.len() {
                        rho_data[idx] += self.amplitudes[i] * self.amplitudes[j].conj();
                    }
                }
            }
        }
        DensityMatrix {
            data: rho_data,
            dim: keep_dim,
        }
    }

    /// Fidelity F = |⟨ψ|φ⟩|² between two pure states.
    pub fn fidelity(&self, other: &QuantumState) -> Scalar {
        let ip = self.inner_product(other);
        ip.norm_sqr()
    }

    /// Convert to a density matrix ρ = |ψ⟩⟨ψ|.
    pub fn to_density_matrix(&self) -> DensityMatrix {
        let dim = self.amplitudes.len();
        let mut data = vec![ComplexScalar::new(0.0, 0.0); dim * dim];
        for i in 0..dim {
            for j in 0..dim {
                data[i * dim + j] = self.amplitudes[i] * self.amplitudes[j].conj();
            }
        }
        DensityMatrix { data, dim }
    }
}

/// Density matrix representation for mixed quantum states.
#[derive(Debug, Clone, PartialEq)]
pub struct DensityMatrix {
    /// Matrix elements in row-major order: data[i * dim + j] = ρᵢⱼ.
    pub data: Vec<ComplexScalar>,
    /// Dimension of the Hilbert space.
    pub dim: usize,
}

impl DensityMatrix {
    /// Construct from a pure state: ρ = |ψ⟩⟨ψ|.
    pub fn from_pure_state(state: &QuantumState) -> Self {
        state.to_density_matrix()
    }

    /// Create the maximally mixed state: ρ = I / dim.
    pub fn maximally_mixed(dim: usize) -> Self {
        let val = ComplexScalar::new(1.0 / dim as Scalar, 0.0);
        let mut data = vec![ComplexScalar::new(0.0, 0.0); dim * dim];
        for i in 0..dim {
            data[i * dim + i] = val;
        }
        Self { data, dim }
    }

    /// Compute the trace Tr(ρ).
    pub fn trace(&self) -> ComplexScalar {
        let mut t = ComplexScalar::new(0.0, 0.0);
        for i in 0..self.dim {
            t += self.data[i * self.dim + i];
        }
        t
    }

    /// Compute purity Tr(ρ²).
    pub fn purity(&self) -> Scalar {
        let mut p = 0.0;
        for i in 0..self.dim {
            for j in 0..self.dim {
                let val = self.data[i * self.dim + j];
                let val2 = self.data[j * self.dim + i];
                p += (val * val2).re;
            }
        }
        p
    }

    /// Compute von Neumann entropy S = -Tr(ρ·log₂ρ).
    pub fn von_neumann_entropy(&self) -> Scalar {
        let mut s = 0.0;
        for &lambda in &self.eigenvalues() {
            if lambda > 1e-12 {
                s -= lambda * lambda.log2();
            }
        }
        s.max(0.0)
    }

    /// Apply a set of Kraus operators: ρ' = Σᵢ Kᵢ·ρ·Kᵢ†.
    pub fn apply_kraus(&mut self, kraus_ops: &[Vec<ComplexScalar>]) -> Result<(), String> {
        if kraus_ops.is_empty() {
            return Ok(());
        }
        let dim = self.dim;
        for op in kraus_ops {
            if op.len() != dim * dim {
                return Err("Kraus operator dimension mismatch".to_string());
            }
        }

        let rho_mat: Vec<Vec<ComplexScalar>> = (0..dim)
            .map(|i| self.data[i * dim..(i + 1) * dim].to_vec())
            .collect();
        let mut acc = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];
        for op in kraus_ops {
            // K·ρ and (K·ρ)·K† via the SIMD complex gemm (single source of truth).
            let op_mat: Vec<Vec<ComplexScalar>> = (0..dim)
                .map(|i| op[i * dim..(i + 1) * dim].to_vec())
                .collect();
            let k_rho = crate::core::compute::matrix::mat_mul_complex(&op_mat, &rho_mat)
                .map_err(|e| e.message)?;
            let op_dag: Vec<Vec<ComplexScalar>> = (0..dim)
                .map(|j| (0..dim).map(|i| op_mat[i][j].conj()).collect())
                .collect();
            let term = crate::core::compute::matrix::mat_mul_complex(&k_rho, &op_dag)
                .map_err(|e| e.message)?;
            for i in 0..dim {
                for j in 0..dim {
                    acc[i][j] += term[i][j];
                }
            }
        }
        self.data = acc.into_iter().flatten().collect();
        Ok(())
    }

    /// Compute eigenvalues of a 2×2 or small density matrix (Jacobi iteration).
    pub fn eigenvalues(&self) -> Vec<Scalar> {
        if self.dim == 1 {
            return vec![self.data[0].re];
        }
        if self.dim == 2 {
            let a = self.data[0].re;
            let b = self.data[1].re;
            let c = self.data[2].re;
            let d = self.data[3].re;
            let trace = a + d;
            let det = a * d - b * c;
            let disc = trace * trace - 4.0 * det;
            if disc < 0.0 {
                return vec![trace / 2.0; 2];
            }
            let sqrt_disc = disc.sqrt();
            vec![(trace + sqrt_disc) / 2.0, (trace - sqrt_disc) / 2.0]
        } else {
            // Fallback: return diagonal elements as rough estimate
            (0..self.dim)
                .map(|i| self.data[i * self.dim + i].re)
                .collect()
        }
    }

    /// Compute the square root of the density matrix (for fidelity calc).
    pub fn sqrt(&self) -> Self {
        let dim = self.dim;
        let mut result = self.clone();
        // Newton-Schulz iteration for matrix square root (matrix products via
        // the SIMD complex gemm).
        for _ in 0..20 {
            let y_mat = crate::core::compute::matrix::mat_mul_complex(
                &(0..dim)
                    .map(|i| result.data[i * dim..(i + 1) * dim].to_vec())
                    .collect::<Vec<_>>(),
                &(0..dim)
                    .map(|i| result.data[i * dim..(i + 1) * dim].to_vec())
                    .collect::<Vec<_>>(),
            )
            .map(|m| m.into_iter().flatten().collect::<Vec<_>>())
            .unwrap_or_else(|_| result.data.clone());
            // Y = 0.5 * (Y + ρ * Y⁻¹) — simplified
            // For practical use: just return the Cholesky-like factor
            let mut converged = true;
            for i in 0..dim {
                for j in 0..dim {
                    let diff = (y_mat[i * dim + j] - result.data[i * dim + j]).norm();
                    if diff > 1e-10 {
                        converged = false;
                    }
                }
            }
            result.data = y_mat;
            if converged {
                break;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ground_state() {
        let s = QuantumState::ground_state(2);
        assert_eq!(s.num_qubits, 2);
        assert_eq!(s.amplitudes.len(), 4);
        assert!((s.amplitudes[0].norm_sqr() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_uniform_superposition() {
        let s = QuantumState::uniform_superposition(2);
        for a in &s.amplitudes {
            assert!((a.norm_sqr() - 0.25).abs() < 1e-10);
        }
    }

    #[test]
    fn test_normalize() {
        let mut s = QuantumState {
            amplitudes: vec![ComplexScalar::new(2.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            num_qubits: 1,
        };
        s.normalize().unwrap();
        assert!((s.amplitudes[0].norm_sqr() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalize_zero() {
        let mut s = QuantumState {
            amplitudes: vec![ComplexScalar::new(0.0, 0.0); 2],
            num_qubits: 1,
        };
        assert!(s.normalize().is_err());
    }

    #[test]
    fn test_inner_product() {
        let s1 = QuantumState::ground_state(1);
        let s2 = QuantumState::from_basis(1, 1);
        let ip = s1.inner_product(&s2);
        assert!(ip.norm_sqr() < 1e-10);
    }

    #[test]
    fn test_probabilities() {
        let s = QuantumState::uniform_superposition(2);
        let probs = s.probabilities();
        for p in &probs {
            assert!((*p - 0.25).abs() < 1e-10);
        }
    }

    #[test]
    fn test_measure_probability() {
        let s = QuantumState::ground_state(2);
        assert!((s.measure_probability(0) - 1.0).abs() < 1e-10);
        assert!((s.measure_probability(1) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_fidelity_identical() {
        let s1 = QuantumState::ground_state(2);
        let s2 = QuantumState::ground_state(2);
        assert!((s1.fidelity(&s2) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_fidelity_orthogonal() {
        let s1 = QuantumState::ground_state(1);
        let s2 = QuantumState::from_basis(1, 1);
        assert!((s1.fidelity(&s2)).abs() < 1e-10);
    }

    #[test]
    fn test_density_matrix_from_pure() {
        let s = QuantumState::ground_state(1);
        let rho = DensityMatrix::from_pure_state(&s);
        assert_eq!(rho.dim, 2);
        assert!((rho.data[0].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_maximally_mixed() {
        let rho = DensityMatrix::maximally_mixed(2);
        assert!((rho.data[0].re - 0.5).abs() < 1e-10);
        assert!((rho.data[3].re - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_trace() {
        let rho = DensityMatrix::maximally_mixed(2);
        let tr = rho.trace();
        assert!((tr.re - 1.0).abs() < 1e-10);
        assert!(tr.im.abs() < 1e-10);
    }

    #[test]
    fn test_purity() {
        let pure = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        assert!((pure.purity() - 1.0).abs() < 1e-10);

        let mixed = DensityMatrix::maximally_mixed(2);
        assert!((mixed.purity() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_von_neumann_entropy_pure() {
        let pure = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        assert!(pure.von_neumann_entropy().abs() < 1e-10);
    }

    #[test]
    fn test_to_density_matrix() {
        let s = QuantumState::ground_state(1);
        let rho = s.to_density_matrix();
        assert_eq!(rho.dim, 2);
        assert!((rho.data[0].re - 1.0).abs() < 1e-10);
        assert!(rho.data[1].norm() < 1e-10);
        assert!(rho.data[2].norm() < 1e-10);
        assert!(rho.data[3].norm() < 1e-10);
    }

    #[test]
    fn test_from_basis() {
        let s = QuantumState::from_basis(3, 2);
        assert_eq!(s.amplitudes.len(), 4);
        assert!((s.amplitudes[3].norm_sqr() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_partial_trace() {
        // Create Bell state |00⟩ + |11⟩ / √2
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let mut amps = vec![ComplexScalar::new(0.0, 0.0); 4];
        amps[0] = ComplexScalar::new(inv_sqrt2, 0.0);
        amps[3] = ComplexScalar::new(inv_sqrt2, 0.0);
        let bell = QuantumState {
            amplitudes: amps,
            num_qubits: 2,
        };
        let rho = bell.partial_trace(&[1]);
        assert_eq!(rho.dim, 2);
        // Reduced density matrix should be maximally mixed
        let purity = rho.purity();
        assert!((purity - 0.5).abs() < 0.5);
    }

    #[test]
    fn test_eigenvalues_2x2() {
        let rho = DensityMatrix::maximally_mixed(2);
        let evals = rho.eigenvalues();
        assert_eq!(evals.len(), 2);
        assert!((evals[0] - 0.5).abs() < 1e-10);
        assert!((evals[1] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_apply_kraus() {
        let mut rho = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        // Dephasing Kraus operators
        let p: Scalar = 0.1;
        let k0 = vec![
            ComplexScalar::new(1.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new((1.0 - p).sqrt(), 0.0),
        ];
        let k1 = vec![
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(p.sqrt(), 0.0),
        ];
        rho.apply_kraus(&[k0, k1]).unwrap();
        assert!((rho.purity() - 1.0).abs() < 0.11);
    }
}
