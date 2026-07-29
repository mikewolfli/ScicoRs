//! Lindblad master equation solver for open quantum systems.
//!
//! dρ/dt = -i/ℏ[H,ρ] + Σⱼ(Lⱼ·ρ·Lⱼ† - ½{Lⱼ†·Lⱼ, ρ})

use super::state::{ComplexScalar, DensityMatrix};
use crate::core::types::Scalar;

/// Lindblad master equation solver.
pub struct LindbladSolver {
    /// System Hamiltonian.
    pub hamiltonian: Vec<Vec<ComplexScalar>>,
    /// Jump operators (each is a dim×dim matrix).
    pub jump_operators: Vec<Vec<Vec<ComplexScalar>>>,
    /// Time step.
    pub dt: Scalar,
}

impl LindbladSolver {
    /// Create a new Lindblad solver.
    pub fn new(
        hamiltonian: Vec<Vec<ComplexScalar>>,
        jump_operators: Vec<Vec<Vec<ComplexScalar>>>,
        dt: Scalar,
    ) -> Self {
        Self {
            hamiltonian,
            jump_operators,
            dt,
        }
    }

    /// Compute the Lindblad right-hand side: dρ/dt.
    fn lindblad_rhs(&self, rho: &[ComplexScalar], dim: usize) -> Vec<ComplexScalar> {
        let mut drho = vec![ComplexScalar::new(0.0, 0.0); dim * dim];

        // -i[H, ρ] term
        for i in 0..dim {
            for j in 0..dim {
                let mut commutator = ComplexScalar::new(0.0, 0.0);
                for k in 0..dim {
                    commutator += self.hamiltonian[i][k] * rho[k * dim + j]
                        - rho[i * dim + k] * self.hamiltonian[k][j];
                }
                drho[i * dim + j] += -ComplexScalar::new(0.0, 1.0) * commutator;
            }
        }

        // Dissipator sum: Σⱼ(Lⱼ·ρ·Lⱼ† - ½{Lⱼ†·Lⱼ, ρ})
        for op in &self.jump_operators {
            // L·ρ (op is Vec<Vec<ComplexScalar>>, indexed as op[row][col])
            let mut l_rho = vec![ComplexScalar::new(0.0, 0.0); dim * dim];
            for i in 0..dim {
                for j in 0..dim {
                    let mut s = ComplexScalar::new(0.0, 0.0);
                    for k in 0..dim {
                        s += op[i][k] * rho[k * dim + j];
                    }
                    l_rho[i * dim + j] = s;
                }
            }
            // L·ρ·L†
            for i in 0..dim {
                for j in 0..dim {
                    let mut s = ComplexScalar::new(0.0, 0.0);
                    for k in 0..dim {
                        s += l_rho[i * dim + k] * op[j][k].conj();
                    }
                    drho[i * dim + j] += s;
                }
            }

            // L†·L
            let mut l_dag_l = vec![ComplexScalar::new(0.0, 0.0); dim * dim];
            for i in 0..dim {
                for j in 0..dim {
                    let mut s = ComplexScalar::new(0.0, 0.0);
                    for k in 0..dim {
                        s += op[k][i].conj() * op[k][j];
                    }
                    l_dag_l[i * dim + j] = s;
                }
            }
            // -½·{L†·L, ρ}
            for i in 0..dim {
                for j in 0..dim {
                    let mut anticommutator = ComplexScalar::new(0.0, 0.0);
                    for k in 0..dim {
                        anticommutator += l_dag_l[i * dim + k] * rho[k * dim + j]
                            + rho[i * dim + k] * l_dag_l[k * dim + j];
                    }
                    drho[i * dim + j] += -0.5 * anticommutator;
                }
            }
        }

        drho
    }

    /// RK4 integration step for the Lindblad equation.
    ///
    /// Uses the shared Butcher tableau coefficients from `runtime::solver::fixed_step`.
    pub fn rk4_step(&self, rho: &DensityMatrix) -> Result<DensityMatrix, String> {
        use crate::runtime::solver::fixed_step::{RK4_A, RK4_B};

        let dim = rho.dim;
        let data = &rho.data;

        let k1 = self.lindblad_rhs(data, dim);
        let mut tmp = vec![ComplexScalar::new(0.0, 0.0); dim * dim];

        // Stage 2: t + a₂₁·dt using k1
        let dt_a21 = self.dt * RK4_A[1][0]; // ½·dt
        for i in 0..dim * dim {
            tmp[i] = data[i] + dt_a21 * k1[i];
        }
        let k2 = self.lindblad_rhs(&tmp, dim);

        // Stage 3: t + a₃₂·dt using k2
        let dt_a32 = self.dt * RK4_A[2][1]; // ½·dt
        for i in 0..dim * dim {
            tmp[i] = data[i] + dt_a32 * k2[i];
        }
        let k3 = self.lindblad_rhs(&tmp, dim);

        // Stage 4: t + a₄₃·dt using k3
        let dt_a43 = self.dt * RK4_A[3][2]; // 1·dt
        for i in 0..dim * dim {
            tmp[i] = data[i] + dt_a43 * k3[i];
        }
        let k4 = self.lindblad_rhs(&tmp, dim);

        // Final combination: Σ b_i · k_i
        let b0 = RK4_B[0] * self.dt; // ¹/₆·dt
        let b1 = RK4_B[1] * self.dt; // ¹/₃·dt
        let b2 = RK4_B[2] * self.dt; // ¹/₃·dt
        let b3 = RK4_B[3] * self.dt; // ¹/₆·dt

        let mut new_data = vec![ComplexScalar::new(0.0, 0.0); dim * dim];
        for i in 0..dim * dim {
            new_data[i] = data[i] + b0 * k1[i] + b1 * k2[i] + b2 * k3[i] + b3 * k4[i];
        }

        Ok(DensityMatrix {
            data: new_data,
            dim,
        })
    }

    /// Evolve the system from `initial` to time `t_end`.
    pub fn evolve(
        &self,
        initial: &DensityMatrix,
        t_end: Scalar,
    ) -> Result<Vec<DensityMatrix>, String> {
        let steps = (t_end / self.dt).ceil() as usize;
        let mut states = Vec::with_capacity(steps + 1);
        let mut current = initial.clone();
        states.push(current.clone());

        for _ in 0..steps {
            current = self.rk4_step(&current)?;
            states.push(current.clone());
        }
        Ok(states)
    }
}

// ── Standard decoherence channels (Kraus operators) ──

/// Amplitude damping channel (energy relaxation): K₀ = |0⟩⟨0| + √(1-γ)|1⟩⟨1|, K₁ = √γ|0⟩⟨1|.
pub fn amplitude_damping(gamma: Scalar) -> Vec<Vec<ComplexScalar>> {
    let sqrt_gamma = gamma.sqrt();
    let sqrt_one_minus = (1.0 - gamma).sqrt();
    vec![
        vec![
            ComplexScalar::new(1.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(sqrt_one_minus, 0.0),
        ],
        vec![
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(sqrt_gamma, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
        ],
    ]
}

/// Dephasing channel (phase damping): K₀ = √(1-γ)I, K₁ = √γ·Z.
pub fn dephasing_channel(gamma: Scalar) -> Vec<Vec<ComplexScalar>> {
    let sqrt_one_minus = (1.0 - gamma).sqrt();
    let sqrt_gamma = gamma.sqrt();
    vec![
        vec![
            ComplexScalar::new(sqrt_one_minus, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(sqrt_one_minus, 0.0),
        ],
        vec![
            ComplexScalar::new(sqrt_gamma, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(-sqrt_gamma, 0.0),
        ],
    ]
}

/// Depolarizing channel: ρ → (1-p)ρ + p·I/2 (for single qubit).
pub fn depolarizing_channel(p: Scalar) -> Vec<Vec<ComplexScalar>> {
    let sqrt_p = (p / 4.0).sqrt();
    let sqrt_one_minus = (1.0 - 3.0 * p / 4.0).sqrt();
    vec![
        vec![
            ComplexScalar::new(sqrt_one_minus, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(sqrt_one_minus, 0.0),
        ],
        vec![
            ComplexScalar::new(sqrt_p, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(sqrt_p, 0.0),
        ],
        vec![
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(sqrt_p, 0.0),
            ComplexScalar::new(sqrt_p, 0.0),
            ComplexScalar::new(0.0, 0.0),
        ],
        vec![
            ComplexScalar::new(0.0, sqrt_p),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(0.0, -sqrt_p),
        ],
    ]
}

/// Phase flip channel: K₀ = √(1-p)I, K₁ = √p·Z.
pub fn phase_flip_channel(p: Scalar) -> Vec<Vec<ComplexScalar>> {
    dephasing_channel(p)
}

/// Spontaneous emission jump operator: σ₋ = |0⟩⟨1|.
pub fn spontaneous_emission(gamma: Scalar) -> Vec<Vec<ComplexScalar>> {
    let sqrt_gamma = gamma.sqrt();
    vec![
        vec![
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(sqrt_gamma, 0.0),
        ],
        vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(0.0, 0.0)],
    ]
}

/// Thermal bath Lindblad operators.
pub fn thermal_bath(n_bar: Scalar, gamma: Scalar) -> Vec<Vec<Vec<ComplexScalar>>> {
    let gamma_n = (gamma * (n_bar + 1.0)).sqrt();
    let gamma_n_bar = (gamma * n_bar).sqrt();
    vec![
        // Decay: √(γ(n̄+1))·σ₋
        vec![
            vec![
                ComplexScalar::new(0.0, 0.0),
                ComplexScalar::new(gamma_n, 0.0),
            ],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(0.0, 0.0)],
        ],
        // Excitation: √(γ·n̄)·σ₊
        vec![
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![
                ComplexScalar::new(gamma_n_bar, 0.0),
                ComplexScalar::new(0.0, 0.0),
            ],
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amplitude_damping_kraus() {
        let ops = amplitude_damping(0.1);
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].len(), 4);
    }

    #[test]
    fn test_dephasing_kraus() {
        let ops = dephasing_channel(0.1);
        assert_eq!(ops.len(), 2);
    }

    #[test]
    fn test_depolarizing_kraus() {
        let ops = depolarizing_channel(0.1);
        assert_eq!(ops.len(), 4);
    }

    #[test]
    fn test_lindblad_creation() {
        let h = vec![
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(-1.0, 0.0)],
        ];
        let jump = spontaneous_emission(0.1);
        let solver = LindbladSolver::new(h, vec![jump], 0.01);
        assert!((solver.dt - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_rk4_step_preserves_trace() {
        let h = vec![
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0)],
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
        ];
        let jump = spontaneous_emission(0.01);
        let solver = LindbladSolver::new(h, vec![jump], 0.01);
        let initial =
            DensityMatrix::from_pure_state(&super::super::state::QuantumState::ground_state(1));
        let result = solver.rk4_step(&initial).unwrap();
        let tr = result.trace();
        assert!((tr.re - 1.0).abs() < 0.5);
    }

    #[test]
    fn test_evolve() {
        let h = vec![
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0)],
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
        ];
        let jump = amplitude_damping(0.1);
        let solver = LindbladSolver::new(h, vec![jump], 0.01);
        let initial =
            DensityMatrix::from_pure_state(&super::super::state::QuantumState::ground_state(1));
        let states = solver.evolve(&initial, 0.1).unwrap();
        assert!(states.len() > 1);
    }

    #[test]
    fn test_spontaneous_emission() {
        let op = spontaneous_emission(1.0);
        assert_eq!(op.len(), 2);
        assert_eq!(op[0].len(), 2);
    }

    #[test]
    fn test_thermal_bath() {
        let ops = thermal_bath(1.0, 0.1);
        assert_eq!(ops.len(), 2);
    }
}
