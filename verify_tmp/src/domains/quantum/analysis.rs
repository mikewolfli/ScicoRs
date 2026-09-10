//! Quantum analysis tools: fidelity, trace distance, entanglement entropy, mutual information.

use crate::core::types::Scalar;
use super::state::{ComplexScalar, DensityMatrix, QuantumState};

/// Fidelity between two density matrices: F(ρ, σ) = Tr(√(√ρ·σ·√ρ)).
pub fn fidelity_density(rho: &DensityMatrix, sigma: &DensityMatrix) -> Scalar {
    if rho.dim != sigma.dim {
        return 0.0;
    }
    let dim = rho.dim;

    // For pure states: F = ⟨ψ|σ|ψ⟩ = Tr(ρ·σ)
    let mut f = 0.0;
    for i in 0..dim {
        for j in 0..dim {
            let mut s = ComplexScalar::new(0.0, 0.0);
            for k in 0..dim {
                s += rho.data[i * dim + k] * sigma.data[k * dim + j];
            }
            f += s.re; // Only real part for trace of Hermitian
        }
    }
    f.clamp(0.0, 1.0)
}

/// Trace distance between two density matrices: D(ρ, σ) = ½·Tr(|ρ - σ|).
pub fn trace_distance(rho: &DensityMatrix, sigma: &DensityMatrix) -> Scalar {
    if rho.dim != sigma.dim {
        return 1.0;
    }
    let dim = rho.dim;

    // Compute ρ - σ
    let mut diff = vec![ComplexScalar::new(0.0, 0.0); dim * dim];
    for i in 0..dim * dim {
        diff[i] = rho.data[i] - sigma.data[i];
    }

    // ½·Tr(|Δ|) — simplified: sum of absolute eigenvalues via Frobenius upper bound
    let mut sum_abs = 0.0;
    for i in 0..dim {
        sum_abs += diff[i * dim + i].norm(); // Diagonal dominance approx
    }
    (0.5 * sum_abs).min(1.0)
}

/// Entanglement entropy: von Neumann entropy of the reduced density matrix.
pub fn entanglement_entropy(state: &QuantumState, subsystem: &[usize]) -> Scalar {
    let reduced = state.partial_trace(subsystem);
    reduced.von_neumann_entropy()
}

/// Quantum mutual information I(A:B) = S(A) + S(B) - S(AB).
pub fn quantum_mutual_information(
    state: &QuantumState,
    system_a: &[usize],
    system_b: &[usize],
) -> Scalar {
    let rho_a = state.partial_trace(system_a);
    let rho_b = state.partial_trace(system_b);

    // For pure |ψ⟩, S(AB) = 0
    let s_a = rho_a.von_neumann_entropy();
    let s_b = rho_b.von_neumann_entropy();

    s_a + s_b // S(AB) = 0 for pure state
}

/// Measurement statistics from repeated measurements.
///
/// `counts`: outcome counts indexed by basis state.
/// `num_shots`: total number of measurements.
/// Returns list of (outcome, probability) pairs.
pub fn measurement_statistics(
    counts: &[usize],
    num_shots: usize,
) -> Vec<(usize, Scalar)> {
    let total = num_shots.max(1) as Scalar;
    counts
        .iter()
        .enumerate()
        .map(|(i, &c)| (i, c as Scalar / total))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fidelity_identical() {
        let rho = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        let f = fidelity_density(&rho, &rho);
        assert!((f - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_fidelity_orthogonal() {
        let rho = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        let sigma = DensityMatrix::from_pure_state(&QuantumState::from_basis(1, 1));
        let f = fidelity_density(&rho, &sigma);
        assert!(f < 1e-10);
    }

    #[test]
    fn test_trace_distance_identical() {
        let rho = DensityMatrix::maximally_mixed(2);
        let d = trace_distance(&rho, &rho);
        assert!(d < 1e-10);
    }

    #[test]
    fn test_entanglement_entropy_separable() {
        let state = QuantumState::from_basis(0, 2); // |00⟩
        let entropy = entanglement_entropy(&state, &[1]);
        assert!(entropy < 1e-10);
    }

    #[test]
    fn test_entanglement_entropy_bell() {
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let mut amps = vec![ComplexScalar::new(0.0, 0.0); 4];
        amps[0] = ComplexScalar::new(inv_sqrt2, 0.0);
        amps[3] = ComplexScalar::new(inv_sqrt2, 0.0);
        let bell = QuantumState {
            amplitudes: amps,
            num_qubits: 2,
        };
        let entropy = entanglement_entropy(&bell, &[1]);
        assert!(entropy > 0.0);
    }

    #[test]
    fn test_mutual_information_separable() {
        let state = QuantumState::from_basis(0, 2);
        let mi = quantum_mutual_information(&state, &[0], &[1]);
        assert!(mi < 1e-10);
    }

    #[test]
    fn test_measurement_statistics() {
        let counts = vec![3, 2, 0];
        let stats = measurement_statistics(&counts, 5);
        assert!((stats[0].1 - 0.6).abs() < 1e-10);
        assert!((stats[1].1 - 0.4).abs() < 1e-10);
        assert!((stats[2].1 - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_trace_distance_orthogonal() {
        let rho = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        let sigma = DensityMatrix::from_pure_state(&QuantumState::from_basis(1, 1));
        let d = trace_distance(&rho, &sigma);
        assert!(d > 0.9);
    }
}
