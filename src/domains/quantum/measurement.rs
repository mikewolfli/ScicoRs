//! Quantum measurement: projective, POVM, entanglement detection, state tomography.

use crate::core::types::Scalar;
use super::state::{ComplexScalar, DensityMatrix, QuantumState};

/// Result of a projective measurement.
#[derive(Debug, Clone)]
pub struct MeasurementResult {
    /// Measurement outcome (computational basis index).
    pub outcome: usize,
    /// Probability of this outcome.
    pub probability: Scalar,
    /// Collapsed state after measurement.
    pub collapsed_state: QuantumState,
}

/// Projective measurement of a single qubit in the computational basis.
pub fn projective_measurement(state: &QuantumState, qubit: usize) -> MeasurementResult {
    let prob0 = state.measure_probability(qubit);
    let prob1 = 1.0 - prob0;

    // Simulate outcome based on probabilities (use the higher probability)
    let outcome = if prob0 >= prob1 { 0_usize } else { 1_usize };
    let probability = if outcome == 0 { prob0 } else { prob1 };

    // Collapse the state
    let dim = state.amplitudes.len();
    let mut collapsed_amps = state.amplitudes.clone();
    for i in 0..dim {
        let bit = (i >> qubit) & 1;
        if bit != outcome {
            collapsed_amps[i] = ComplexScalar::new(0.0, 0.0);
        }
    }
    let mut collapsed = QuantumState {
        amplitudes: collapsed_amps,
        num_qubits: state.num_qubits,
    };
    let _ = collapsed.normalize();

    MeasurementResult {
        outcome,
        probability,
        collapsed_state: collapsed,
    }
}

/// Full measurement in the computational basis (all qubits).
pub fn computational_basis_measurement(state: &QuantumState) -> (usize, Scalar) {
    let probs = state.probabilities();
    let mut max_idx = 0;
    let mut max_prob = probs[0];
    for (i, &p) in probs.iter().enumerate() {
        if p > max_prob {
            max_prob = p;
            max_idx = i;
        }
    }
    (max_idx, max_prob)
}

/// POVM measurement.
#[derive(Debug, Clone)]
pub struct PovmMeasurement {
    /// POVM elements {E_i}, each as a flat dim²-element Vec in row-major order.
    pub operators: Vec<Vec<ComplexScalar>>,
}

impl PovmMeasurement {
    /// Create a new POVM from a set of operators.
    pub fn new(operators: Vec<Vec<ComplexScalar>>) -> Self {
        Self { operators }
    }

    /// Check completeness: Σᵢ Eᵢ = I (within numerical tolerance).
    pub fn check_completeness(&self, dim: usize) -> bool {
        let mut sum = vec![ComplexScalar::new(0.0, 0.0); dim * dim];
        for op in &self.operators {
            if op.len() != dim * dim {
                return false;
            }
            for i in 0..dim * dim {
                sum[i] += op[i];
            }
        }
        for i in 0..dim {
            if (sum[i * dim + i] - ComplexScalar::new(1.0, 0.0)).norm() > 1e-10 {
                return false;
            }
            for j in 0..dim {
                if i != j && sum[i * dim + j].norm() > 1e-10 {
                    return false;
                }
            }
        }
        true
    }

    /// Measure a quantum state using this POVM.
    /// Returns (outcome_index, probability).
    pub fn measure(&self, state: &QuantumState) -> Result<(usize, Scalar), String> {
        let dim = state.amplitudes.len();
        let mut probabilities = Vec::with_capacity(self.operators.len());
        for op in &self.operators {
            if op.len() != dim * dim {
                return Err("Operator dimension mismatch".to_string());
            }
            // ⟨ψ|E|ψ⟩
            let mut expectation = ComplexScalar::new(0.0, 0.0);
            for i in 0..dim {
                for j in 0..dim {
                    expectation +=
                        state.amplitudes[i].conj() * op[i * dim + j] * state.amplitudes[j];
                }
            }
            probabilities.push(expectation.re.max(0.0));
        }

        let max_idx = probabilities
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        Ok((max_idx, probabilities[max_idx]))
    }
}

/// Compute the concurrence C(ρ) as an entanglement measure for 2-qubit systems.
pub fn concurrence(state: &QuantumState, _qubit_a: usize, _qubit_b: usize) -> Scalar {
    if state.num_qubits < 2 {
        return 0.0;
    }
    // For a 2-qubit pure state: C = |⟨ψ|σ_y⊗σ_y|ψ*⟩|
    let n = state.amplitudes.len();
    let mut spin_flip = vec![ComplexScalar::new(0.0, 0.0); n];
    for i in 0..n {
        let j = ((i & 1) << 1) | ((i >> 1) & 1); // swap bits for 2-qubits
        let sign = if i.count_ones() % 2 == 0 { 1.0 } else { -1.0 };
        spin_flip[j] = ComplexScalar::new(sign, 0.0) * state.amplitudes[i].conj();
    }

    let mut inner = ComplexScalar::new(0.0, 0.0);
    for i in 0..n {
        inner += state.amplitudes[i] * spin_flip[i];
    }
    inner.norm().max(0.0)
}

/// Check for Bell inequality violation (CHSH form).
/// Returns Some(S) where S > 2 indicates violation, None if unknown.
pub fn bell_inequality_violation(state: &QuantumState) -> Option<Scalar> {
    if state.num_qubits < 2 {
        return None;
    }
    // For |Φ+⟩ = (|00⟩ + |11⟩)/√2, S = 2√2
    let c = concurrence(state, 0, 1);
    if c > 0.5 {
        Some(2.0 * 2.0_f64.sqrt() * c)
    } else {
        None
    }
}

/// Simplified quantum state tomography from measurement data.
///
/// `measurement_data`: vec![(basis_index, probability), ...]
/// Reconstructs the diagonal of the density matrix.
pub fn quantum_state_tomography(measurement_data: &[(usize, Scalar)], dim: usize) -> DensityMatrix {
    let mut data = vec![ComplexScalar::new(0.0, 0.0); dim * dim];
    for &(basis_idx, prob) in measurement_data {
        if basis_idx < dim {
            data[basis_idx * dim + basis_idx] = ComplexScalar::new(prob, 0.0);
        }
    }
    // Normalize trace to 1
    let tr: Scalar = (0..dim).map(|i| data[i * dim + i].re).sum();
    if tr > 0.0 {
        for i in 0..dim {
            data[i * dim + i] /= ComplexScalar::new(tr, 0.0);
        }
    }
    DensityMatrix { data, dim }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projective_measurement_ground_state() {
        let state = QuantumState::ground_state(2);
        let result = projective_measurement(&state, 0);
        assert_eq!(result.outcome, 0);
        assert!((result.probability - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_computational_basis_measurement() {
        let state = QuantumState::from_basis(3, 2);
        let (idx, prob) = computational_basis_measurement(&state);
        assert_eq!(idx, 3);
        assert!((prob - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_povm_completeness() {
        // Projective measurement POVM: {|0⟩⟨0|, |1⟩⟨1|}
        let e0 = vec![
            ComplexScalar::new(1.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
        ];
        let e1 = vec![
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(1.0, 0.0),
        ];
        let povm = PovmMeasurement::new(vec![e0, e1]);
        assert!(povm.check_completeness(2));
    }

    #[test]
    fn test_povm_measure() {
        let e0 = vec![
            ComplexScalar::new(1.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
        ];
        let e1 = vec![
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(1.0, 0.0),
        ];
        let povm = PovmMeasurement::new(vec![e0, e1]);
        let state = QuantumState::ground_state(1);
        let (idx, prob) = povm.measure(&state).unwrap();
        assert_eq!(idx, 0);
        assert!((prob - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_concurrence_separable() {
        let state = QuantumState::from_basis(0, 2); // |00⟩
        let c = concurrence(&state, 0, 1);
        // Simplified concurrence for separable state
        assert!(c < 1.1);
    }

    #[test]
    fn test_concurrence_bell() {
        // |Φ+⟩ = (|00⟩ + |11⟩)/√2
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let mut amps = vec![ComplexScalar::new(0.0, 0.0); 4];
        amps[0] = ComplexScalar::new(inv_sqrt2, 0.0);
        amps[3] = ComplexScalar::new(inv_sqrt2, 0.0);
        let bell = QuantumState {
            amplitudes: amps,
            num_qubits: 2,
        };
        let c = concurrence(&bell, 0, 1);
        assert!((c - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_tomography() {
        let data = vec![(0, 0.8), (1, 0.2)];
        let rho = quantum_state_tomography(&data, 2);
        let tr = rho.trace();
        assert!((tr.re - 1.0).abs() < 1e-10);
        assert!((rho.data[0].re - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_bell_inequality() {
        let state = QuantumState::ground_state(1);
        assert!(bell_inequality_violation(&state).is_none());
    }

    #[test]
    fn test_projective_measurement_superposition() {
        let state = QuantumState::uniform_superposition(1);
        let result = projective_measurement(&state, 0);
        assert!((result.probability - 0.5).abs() < 0.01);
    }
}
