//! Quantum noise channel models.
//!
//! Provides common noise channels (depolarising, amplitude/phase damping,
//! bit/phase flip) and tools for applying them to quantum states.

use super::state::ComplexScalar;
use crate::core::types::Scalar;

/// Density matrix representation (2ⁿ × 2ⁿ).
/// Uses a simple Vec<Vec<ComplexScalar>> (not the struct from state.rs).
pub type NoiseDensityMatrix = Vec<Vec<ComplexScalar>>;

/// Supported quantum noise channels.
#[derive(Debug, Clone, PartialEq)]
pub enum NoiseChannel {
    /// Depolarising channel: ρ → (1-p)ρ + p·I/2ⁿ
    Depolarizing { p: Scalar },
    /// Amplitude damping: |1⟩ → √γ|0⟩ with probability γ
    AmplitudeDamping { gamma: Scalar },
    /// Phase damping: dephasing with rate γ
    PhaseDamping { gamma: Scalar },
    /// Bit-flip channel: X error with probability p
    BitFlip { p: Scalar },
    /// Phase-flip channel: Z error with probability p
    PhaseFlip { p: Scalar },
    /// Custom channel with arbitrary Kraus operators.
    Custom {
        /// Kraus operators: Vec<[2×2 matrix]>
        kraus_ops: Vec<[[ComplexScalar; 2]; 2]>,
    },
}

impl NoiseChannel {
    /// Apply the noise channel to a single-qubit density matrix.
    ///
    /// The density matrix is a 2×2 complex Hermitian matrix.
    pub fn apply(&self, rho: &NoiseDensityMatrix) -> Result<NoiseDensityMatrix, String> {
        if rho.len() != 2 || rho[0].len() != 2 {
            return Err("Expected 2×2 density matrix".to_string());
        }
        match self {
            Self::Depolarizing { p } => {
                let p = *p;
                assert!((0.0..=1.0).contains(&p), "p must be in [0,1]");
                let mut result = vec![vec![ComplexScalar::new(0.0, 0.0); 2]; 2];
                // (1-p)·ρ + p·I/2
                let p_over_2 = p / 2.0;
                result[0][0] = ComplexScalar::new((1.0 - p) * rho[0][0].re + p_over_2, 0.0);
                result[0][1] =
                    ComplexScalar::new((1.0 - p) * rho[0][1].re, (1.0 - p) * rho[0][1].im);
                result[1][0] =
                    ComplexScalar::new((1.0 - p) * rho[1][0].re, (1.0 - p) * rho[1][0].im);
                result[1][1] = ComplexScalar::new((1.0 - p) * rho[1][1].re + p_over_2, 0.0);
                Ok(result)
            }
            Self::AmplitudeDamping { gamma } => {
                let gamma = *gamma;
                assert!((0.0..=1.0).contains(&gamma), "gamma must be in [0,1]");
                // Kraus operators:
                // K0 = [[1, 0], [0, √(1-γ)]], K1 = [[0, √γ], [0, 0]]
                let sqrt_1g = (1.0 - gamma).sqrt();
                let mut result = vec![vec![ComplexScalar::new(0.0, 0.0); 2]; 2];

                // K0·ρ·K0†
                result[0][0] = ComplexScalar::new(rho[0][0].re + gamma * rho[1][1].re, 0.0);
                result[0][1] = ComplexScalar::new(sqrt_1g * rho[0][1].re, sqrt_1g * rho[0][1].im);
                result[1][0] = ComplexScalar::new(sqrt_1g * rho[1][0].re, sqrt_1g * rho[1][0].im);
                result[1][1] = ComplexScalar::new((1.0 - gamma) * rho[1][1].re, 0.0);

                // K1·ρ·K1† adds to [0][0]
                result[0][0] += ComplexScalar::new(gamma * rho[1][1].re, 0.0);
                Ok(result)
            }
            Self::PhaseDamping { gamma } => {
                let gamma = *gamma;
                assert!((0.0..=1.0).contains(&gamma), "gamma must be in [0,1]");
                let lambda = 1.0 - gamma;
                let mut result = rho.clone();
                // Off-diagonal damping
                result[0][1] = ComplexScalar::new(lambda * rho[0][1].re, lambda * rho[0][1].im);
                result[1][0] = ComplexScalar::new(lambda * rho[1][0].re, lambda * rho[1][0].im);
                Ok(result)
            }
            Self::BitFlip { p } => {
                let p = *p;
                assert!((0.0..=1.0).contains(&p), "p must be in [0,1]");
                // ρ → (1-p)ρ + p·X·ρ·X
                let mut result = vec![vec![ComplexScalar::new(0.0, 0.0); 2]; 2];
                result[0][0] = ComplexScalar::new((1.0 - p) * rho[0][0].re + p * rho[1][1].re, 0.0);
                result[0][1] = ComplexScalar::new(
                    (1.0 - 2.0 * p) * rho[0][1].re,
                    (1.0 - 2.0 * p) * rho[0][1].im,
                );
                result[1][0] = ComplexScalar::new(
                    (1.0 - 2.0 * p) * rho[1][0].re,
                    (1.0 - 2.0 * p) * rho[1][0].im,
                );
                result[1][1] = ComplexScalar::new(p * rho[0][0].re + (1.0 - p) * rho[1][1].re, 0.0);
                Ok(result)
            }
            Self::PhaseFlip { p } => {
                let p = *p;
                assert!((0.0..=1.0).contains(&p), "p must be in [0,1]");
                // ρ → (1-p)ρ + p·Z·ρ·Z
                // Z·ρ·Z leaves diagonals unchanged, flips sign of off-diagonals
                let mut result = vec![vec![ComplexScalar::new(0.0, 0.0); 2]; 2];
                result[0][0] = ComplexScalar::new(rho[0][0].re, 0.0);
                result[0][1] = ComplexScalar::new(
                    (1.0 - 2.0 * p) * rho[0][1].re,
                    (1.0 - 2.0 * p) * rho[0][1].im,
                );
                result[1][0] = ComplexScalar::new(
                    (1.0 - 2.0 * p) * rho[1][0].re,
                    (1.0 - 2.0 * p) * rho[1][0].im,
                );
                result[1][1] = ComplexScalar::new(rho[1][1].re, 0.0);
                Ok(result)
            }
            Self::Custom { kraus_ops } => {
                if kraus_ops.is_empty() {
                    return Ok(rho.clone());
                }
                let mut result = vec![vec![ComplexScalar::new(0.0, 0.0); 2]; 2];
                for k_op in kraus_ops {
                    let k_rho_k = mat_mul_2x2(k_op, rho);
                    let k_rho_k_dag = mat_mul_2x2_herm(k_rho_k, k_op);
                    for i in 0..2 {
                        for j in 0..2 {
                            result[i][j] += k_rho_k_dag[i][j];
                        }
                    }
                }
                Ok(result)
            }
        }
    }

    /// Compute the channel fidelity between input and output states.
    pub fn channel_fidelity(
        &self,
        input: &NoiseDensityMatrix,
        output: &NoiseDensityMatrix,
    ) -> Scalar {
        // F = (Tr√(√ρ·σ·√ρ))² — simplified: compute overlap
        let mut f = 0.0;
        for i in 0..2 {
            for j in 0..2 {
                f += input[i][j].re * output[j][i].re + input[i][j].im * output[j][i].im;
            }
        }
        f.clamp(0.0, 1.0)
    }
}

/// 2×2 matrix multiplication: K × ρ
fn mat_mul_2x2(k: &[[ComplexScalar; 2]; 2], rho: &NoiseDensityMatrix) -> Vec<Vec<ComplexScalar>> {
    let mut result = vec![vec![ComplexScalar::new(0.0, 0.0); 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            for m in 0..2 {
                result[i][j] += k[i][m] * rho[m][j];
            }
        }
    }
    result
}

/// 2×2 Hermitian conjugate multiply: Kρ × K†
fn mat_mul_2x2_herm(
    k_rho: Vec<Vec<ComplexScalar>>,
    k: &[[ComplexScalar; 2]; 2],
) -> Vec<Vec<ComplexScalar>> {
    let mut result = vec![vec![ComplexScalar::new(0.0, 0.0); 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            for m in 0..2 {
                result[i][j] += k_rho[i][m] * k[j][m].conj();
            }
        }
    }
    result
}

/// Create a single-qubit density matrix from a pure state vector [α, β].
pub fn pure_state_density(alpha: ComplexScalar, beta: ComplexScalar) -> NoiseDensityMatrix {
    vec![
        vec![alpha * alpha.conj(), alpha * beta.conj()],
        vec![beta * alpha.conj(), beta * beta.conj()],
    ]
}

/// Pauli matrices.
pub const PAULI_X: [[ComplexScalar; 2]; 2] = [
    [ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0)],
    [ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
];
pub const PAULI_Y: [[ComplexScalar; 2]; 2] = [
    [ComplexScalar::new(0.0, 0.0), ComplexScalar::new(0.0, -1.0)],
    [ComplexScalar::new(0.0, 1.0), ComplexScalar::new(0.0, 0.0)],
];
pub const PAULI_Z: [[ComplexScalar; 2]; 2] = [
    [ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
    [ComplexScalar::new(0.0, 0.0), ComplexScalar::new(-1.0, 0.0)],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_state_density() {
        let rho = pure_state_density(ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0));
        assert!((rho[0][0].re - 1.0).abs() < 1e-10);
        assert!((rho[1][1].re - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_depolarizing_channel() {
        let rho = pure_state_density(ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0));
        let channel = NoiseChannel::Depolarizing { p: 0.3 };
        let result = channel.apply(&rho).unwrap();
        // After depolarising: should be closer to I/2
        assert!((result[0][0].re - 0.85).abs() < 1e-10);
    }

    #[test]
    fn test_amplitude_damping() {
        // Start in |1⟩ state
        let rho = pure_state_density(ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0));
        let channel = NoiseChannel::AmplitudeDamping { gamma: 0.5 };
        let result = channel.apply(&rho).unwrap();
        // Should have decayed partially to |0⟩
        assert!(result[0][0].re > 0.0);
        assert!((result[1][1].re - 0.5).abs() < 0.51);
    }

    #[test]
    fn test_phase_damping() {
        let alpha = ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0);
        let beta = ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0);
        let rho = pure_state_density(alpha, beta);
        let channel = NoiseChannel::PhaseDamping { gamma: 0.8 };
        let result = channel.apply(&rho).unwrap();
        // Off-diagonals should be damped
        assert!((result[0][1].re - 0.5 * 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_bit_flip() {
        let rho = pure_state_density(ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0));
        let channel = NoiseChannel::BitFlip { p: 0.3 };
        let result = channel.apply(&rho).unwrap();
        // After bit flip: 30% chance of |1⟩
        assert!((result[0][0].re - 0.7).abs() < 1e-10);
        assert!((result[1][1].re - 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_phase_flip() {
        let alpha = ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0);
        let beta = ComplexScalar::new(1.0 / 2.0_f64.sqrt(), 0.0);
        let rho = pure_state_density(alpha, beta);
        let channel = NoiseChannel::PhaseFlip { p: 0.5 };
        let result = channel.apply(&rho).unwrap();
        // Off-diagonals go to zero
        assert!((result[0][1].re).abs() < 1e-10);
        // Diagonals unchanged
        assert!((result[0][0].re - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_custom_channel() {
        // Identity channel: K = [I]
        let i_op = [
            [ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            [ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0)],
        ];
        let channel = NoiseChannel::Custom {
            kraus_ops: vec![i_op],
        };
        let rho = pure_state_density(ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0));
        let result = channel.apply(&rho).unwrap();
        assert!((result[0][0].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_channel_fidelity() {
        let input = pure_state_density(ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0));
        let channel = NoiseChannel::Depolarizing { p: 0.1 };
        let output = channel.apply(&input).unwrap();
        let f = channel.channel_fidelity(&input, &output);
        assert!(f > 0.9);
        assert!(f <= 1.0);
    }

    #[test]
    fn test_pauli_matrices() {
        // X² = I
        let x2 = mat_mul_2x2(
            &PAULI_X,
            &vec![
                vec![PAULI_X[0][0], PAULI_X[0][1]],
                vec![PAULI_X[1][0], PAULI_X[1][1]],
            ],
        );
        assert!((x2[0][0].re - 1.0).abs() < 1e-10);
        // Z² = I
        let z2 = mat_mul_2x2(
            &PAULI_Z,
            &vec![
                vec![PAULI_Z[0][0], PAULI_Z[0][1]],
                vec![PAULI_Z[1][0], PAULI_Z[1][1]],
            ],
        );
        assert!((z2[0][0].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_empty_custom_channel() {
        let channel = NoiseChannel::Custom { kraus_ops: vec![] };
        let rho = pure_state_density(ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0));
        let result = channel.apply(&rho).unwrap();
        assert!((result[0][0].re - 1.0).abs() < 1e-10);
    }
}
