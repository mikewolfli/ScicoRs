//! Quantum algorithms: VQE, QAOA, Grover search, HHL, QFT.

use super::gates::{GateOperation, GateType, QuantumCircuit, SingleQubitGate};
use super::state::{ComplexScalar, QuantumState};
use crate::core::types::Scalar;

/// VQE optimizer type.
#[derive(Debug, Clone)]
pub enum VqeOptimizer {
    /// Gradient descent.
    GradientDescent {
        learning_rate: Scalar,
        max_iter: usize,
    },
    /// COBYLA-style (simplified).
    Cobyla { max_iter: usize, tolerance: Scalar },
    /// SPSA (simultaneous perturbation stochastic approximation).
    Spsa {
        max_iter: usize,
        alpha: Scalar,
        c: Scalar,
    },
}

/// Variational Quantum Eigensolver.
pub struct VqeSolver {
    /// Hamiltonian matrix.
    pub hamiltonian: Vec<Vec<ComplexScalar>>,
    /// Ansatz circuit (with parameterized gates).
    pub ansatz_circuit: QuantumCircuit,
    /// Optimizer configuration.
    pub optimizer: VqeOptimizer,
}

impl VqeSolver {
    /// Create a new VQE solver.
    pub fn new(
        hamiltonian: Vec<Vec<ComplexScalar>>,
        ansatz: QuantumCircuit,
        optimizer: VqeOptimizer,
    ) -> Self {
        Self {
            hamiltonian,
            ansatz_circuit: ansatz,
            optimizer,
        }
    }

    /// Compute the energy expectation value ⟨ψ(θ)|H|ψ(θ)⟩.
    pub fn energy_expectation(&self, params: &[Scalar]) -> Result<Scalar, String> {
        let dim = self.hamiltonian.len();
        let initial = QuantumState::ground_state((dim as Scalar).log2().ceil() as usize);
        let circuit = self.bind_parameters(params)?;
        let state = circuit.simulate(&initial)?;

        // ⟨ψ|H|ψ⟩
        let mut energy = 0.0;
        for i in 0..dim {
            for j in 0..dim {
                energy +=
                    (state.amplitudes[i].conj() * self.hamiltonian[i][j] * state.amplitudes[j]).re;
            }
        }
        Ok(energy)
    }

    fn bind_parameters(&self, params: &[Scalar]) -> Result<QuantumCircuit, String> {
        let mut circuit = self.ansatz_circuit.clone();
        let mut param_index = 0;

        for op in &mut circuit.operations {
            if let GateType::Single(
                SingleQubitGate::RotationX(theta)
                | SingleQubitGate::RotationY(theta)
                | SingleQubitGate::RotationZ(theta),
            ) = &mut op.gate
            {
                let value = params.get(param_index).ok_or_else(|| {
                    format!(
                        "VQE parameter count mismatch: expected at least {}, got {}",
                        param_index + 1,
                        params.len()
                    )
                })?;
                *theta = *value;
                param_index += 1;
            }
        }

        if param_index > params.len() {
            return Err(format!(
                "VQE parameter count mismatch: circuit uses {}, got {}",
                param_index,
                params.len()
            ));
        }

        Ok(circuit)
    }

    /// Run the optimization.
    pub fn optimize(&mut self) -> Result<(Scalar, Vec<Scalar>), String> {
        match &self.optimizer {
            VqeOptimizer::GradientDescent {
                learning_rate,
                max_iter,
            } => {
                let mut params = vec![0.1; 2]; // 2-parameter ansatz
                let mut best_energy = Scalar::MAX;
                let mut best_params = params.clone();

                for _iter in 0..*max_iter {
                    let energy = self.energy_expectation(&params)?;
                    if energy < best_energy {
                        best_energy = energy;
                        best_params = params.clone();
                    }
                    // Simple finite-difference gradient
                    let eps = 1e-6;
                    let mut grad = vec![0.0; params.len()];
                    for i in 0..params.len() {
                        let mut params_plus = params.clone();
                        params_plus[i] += eps;
                        let e_plus = self.energy_expectation(&params_plus)?;
                        grad[i] = (e_plus - energy) / eps;
                    }
                    for i in 0..params.len() {
                        params[i] -= learning_rate * grad[i];
                    }
                }
                Ok((best_energy, best_params))
            }
            VqeOptimizer::Cobyla { max_iter, .. } => {
                let mut params = vec![0.1; 2];
                let mut best_energy = Scalar::MAX;
                let mut best_params = params.clone();
                for _iter in 0..*max_iter {
                    let energy = self.energy_expectation(&params)?;
                    if energy < best_energy {
                        best_energy = energy;
                        best_params = params.clone();
                    }
                    // Simple random perturbation
                    for p in &mut params {
                        *p += 0.01;
                    }
                }
                Ok((best_energy, best_params))
            }
            VqeOptimizer::Spsa { max_iter, alpha, c } => {
                let mut params = vec![0.1; 2];
                let mut best_energy = Scalar::MAX;
                let mut best_params = params.clone();
                let a = *alpha;
                let c_val = *c;

                for k in 0..*max_iter {
                    let ak = a / (k as Scalar + 1.0);
                    let ck = c_val / ((k as Scalar + 1.0).powf(0.101));

                    // Random perturbation vector Δ
                    let delta: Vec<Scalar> = (0..params.len())
                        .map(|_| if rand_bool() { 1.0 } else { -1.0 })
                        .collect();

                    let mut params_plus = params.clone();
                    let mut params_minus = params.clone();
                    for i in 0..params.len() {
                        params_plus[i] += ck * delta[i];
                        params_minus[i] -= ck * delta[i];
                    }

                    let e_plus = self.energy_expectation(&params_plus)?;
                    let e_minus = self.energy_expectation(&params_minus)?;
                    let energy = self.energy_expectation(&params)?;

                    if energy < best_energy {
                        best_energy = energy;
                        best_params = params.clone();
                    }

                    for i in 0..params.len() {
                        let g = (e_plus - e_minus) / (2.0 * ck * delta[i]);
                        params[i] -= ak * g;
                    }
                }
                Ok((best_energy, best_params))
            }
        }
    }
}

/// Fast deterministic boolean generator (xorshift64*).
///
/// Replaces the previous `SystemTime::now().subsec_nanos().is_multiple_of(2)`
/// which was both slow and unreproducible.
fn rand_bool() -> bool {
    use std::sync::atomic::{AtomicU64, Ordering};

    static STATE: AtomicU64 = AtomicU64::new(0x4d59_5df4_d0f3_3173);

    let mut x = STATE.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    STATE.store(x, Ordering::Relaxed);
    (x & 1) == 1
}

/// Quantum Approximate Optimization Algorithm.
pub struct QaoaSolver {
    /// Cost Hamiltonian H_C.
    pub cost_hamiltonian: Vec<Vec<ComplexScalar>>,
    /// Mixer Hamiltonian H_B.
    pub mixer_hamiltonian: Vec<Vec<ComplexScalar>>,
    /// Number of QAOA layers.
    pub p_layers: usize,
}

impl QaoaSolver {
    /// Create a new QAOA solver.
    pub fn new(
        cost_h: Vec<Vec<ComplexScalar>>,
        mixer_h: Vec<Vec<ComplexScalar>>,
        p: usize,
    ) -> Self {
        Self {
            cost_hamiltonian: cost_h,
            mixer_hamiltonian: mixer_h,
            p_layers: p,
        }
    }

    /// Build the QAOA circuit for given angles (γ, β).
    pub fn build_circuit(&self, gamma: &[Scalar], beta: &[Scalar]) -> QuantumCircuit {
        let n_qubits = (self.cost_hamiltonian.len() as Scalar).log2().ceil() as usize;
        let mut circuit = QuantumCircuit::new(n_qubits.max(1));

        // Initial superposition
        for q in 0..n_qubits {
            circuit.add_gate(GateOperation::new(
                GateType::Single(SingleQubitGate::Hadamard),
                vec![q],
            ));
        }

        // Alternating layers
        for layer in 0..self.p_layers {
            // Cost layer: exp(-i·γ·H_C)
            let g = if layer < gamma.len() {
                gamma[layer]
            } else {
                0.0
            };
            if g.abs() > 1e-10 {
                for q in 0..n_qubits {
                    circuit.add_gate(GateOperation::new(
                        GateType::Single(SingleQubitGate::RotationZ(g)),
                        vec![q],
                    ));
                }
            }

            // Mixer layer: exp(-i·β·H_B)
            let b = if layer < beta.len() { beta[layer] } else { 0.0 };
            if b.abs() > 1e-10 {
                for q in 0..n_qubits {
                    circuit.add_gate(GateOperation::new(
                        GateType::Single(SingleQubitGate::RotationX(b)),
                        vec![q],
                    ));
                }
            }
        }

        circuit
    }

    /// Run the QAOA optimization.
    pub fn optimize(&mut self) -> Result<(Scalar, Vec<Scalar>, Vec<Scalar>), String> {
        let mut gamma = vec![0.1; self.p_layers];
        let mut beta = vec![0.1; self.p_layers];

        let dim = self.cost_hamiltonian.len();
        let initial = QuantumState::uniform_superposition((dim as Scalar).log2().ceil() as usize);

        // Simple optimization: random search
        let mut best_energy = Scalar::MAX;
        let mut best_gamma = gamma.clone();
        let mut best_beta = beta.clone();

        for _ in 0..50 {
            let circuit = self.build_circuit(&gamma, &beta);
            let state = circuit.simulate(&initial)?;

            let mut energy = 0.0;
            for i in 0..dim {
                for j in 0..dim {
                    energy += (state.amplitudes[i].conj()
                        * self.cost_hamiltonian[i][j]
                        * state.amplitudes[j])
                        .re;
                }
            }

            if energy < best_energy {
                best_energy = energy;
                best_gamma = gamma.clone();
                best_beta = beta.clone();
            }

            // Perturb parameters
            for g in &mut gamma {
                *g += 0.05;
            }
            for b in &mut beta {
                *b += 0.05;
            }
        }

        Ok((best_energy, best_gamma, best_beta))
    }
}

/// Grover search algorithm.
///
/// `oracle` is a function that returns true for the marked item.
/// `num_qubits` determines the search space size.
/// `num_solutions` is the (approximate) number of marked items.
pub fn grover_search(
    oracle: Box<dyn Fn(usize) -> bool>,
    num_qubits: usize,
    num_solutions: usize,
) -> Option<usize> {
    let n = 1 << num_qubits;
    let num_iterations = (std::f64::consts::PI / 4.0
        * (n as Scalar / num_solutions.max(1) as Scalar).sqrt())
    .ceil() as usize;

    // Start in uniform superposition
    let mut state = QuantumState::uniform_superposition(num_qubits);

    for _ in 0..num_iterations.min(100) {
        // Oracle: flip sign of marked state
        for i in 0..n {
            if oracle(i) {
                state.amplitudes[i] = -state.amplitudes[i];
            }
        }

        // Diffusion operator: 2|s⟩⟨s| - I
        let mean: ComplexScalar =
            state.amplitudes.iter().sum::<ComplexScalar>() / ComplexScalar::new(n as Scalar, 0.0);
        for i in 0..n {
            state.amplitudes[i] = 2.0 * mean - state.amplitudes[i];
        }
    }

    // Measure
    let probs = state.probabilities();
    let max_idx = probs
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);

    if oracle(max_idx) { Some(max_idx) } else { None }
}

/// HHL linear system solver (simplified interface).
///
/// Solves A·x = b where A is Hermitian.
/// For small systems, delegates to classical dense solve as a proxy.
/// In a real quantum implementation this would use phase estimation + controlled rotations.
pub fn hhl_solver(
    a: &[Vec<ComplexScalar>],
    b: &[ComplexScalar],
    _num_qubits: usize,
) -> Result<Vec<ComplexScalar>, String> {
    crate::core::compute::matrix::solve_complex(a, b).map_err(|e| format!("HHL: {}", e.message))
}

/// Add Quantum Fourier Transform gates to a circuit.
pub fn quantum_fourier_transform(circuit: &mut QuantumCircuit, qubits: &[usize]) {
    let m = qubits.len();
    for i in 0..m {
        // Hadamard on qubit i
        circuit.add_gate(GateOperation::new(
            GateType::Single(SingleQubitGate::Hadamard),
            vec![qubits[i]],
        ));

        // Controlled phase rotations
        for j in 1..(m - i) {
            let angle = std::f64::consts::PI / (1 << j) as Scalar;
            circuit.add_gate(
                GateOperation::new(
                    GateType::Single(SingleQubitGate::RotationZ(angle)),
                    vec![qubits[i + j]],
                )
                .with_controls(vec![qubits[i]]),
            );
        }
    }

    // Swap qubits for correct output order
    for i in 0..m / 2 {
        let src = qubits[i];
        let dst = qubits[m - 1 - i];
        circuit.add_gate(GateOperation::new(
            GateType::Multi(super::gates::MultiQubitGate::SWAP),
            vec![src, dst],
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vqe_creation() {
        let h = vec![
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(-1.0, 0.0)],
        ];
        let qc = QuantumCircuit::new(1);
        let optimizer = VqeOptimizer::GradientDescent {
            learning_rate: 0.1,
            max_iter: 10,
        };
        let solver = VqeSolver::new(h, qc, optimizer);
        assert!(solver.energy_expectation(&[0.1, 0.2]).is_ok());
    }

    #[test]
    fn test_qaoa_creation() {
        let h = vec![
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(-1.0, 0.0)],
        ];
        let mixer = vec![
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0)],
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
        ];
        let solver = QaoaSolver::new(h, mixer, 2);
        let circuit = solver.build_circuit(&[0.1, 0.2], &[0.3, 0.4]);
        assert_eq!(circuit.num_qubits, 1);
    }

    #[test]
    fn test_grover_search() {
        let target = 42;
        let oracle = Box::new(move |x: usize| x == target);
        let result = grover_search(oracle, 6, 1);
        assert_eq!(result, Some(42));
    }

    #[test]
    fn test_hhl_solver() {
        let a = vec![
            vec![ComplexScalar::new(2.0, 0.0), ComplexScalar::new(1.0, 0.0)],
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(2.0, 0.0)],
        ];
        let b = vec![ComplexScalar::new(3.0, 0.0), ComplexScalar::new(4.0, 0.0)];
        let x = hhl_solver(&a, &b, 2).unwrap();
        assert!((x[0].re - 2.0 / 3.0).abs() < 1e-10);
        assert!((x[1].re - 5.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_quantum_fourier_transform() {
        let mut qc = QuantumCircuit::new(3);
        quantum_fourier_transform(&mut qc, &[0, 1, 2]);
        assert!(qc.operations.len() > 3);
    }

    #[test]
    fn test_vqe_optimize_gradient() {
        let h = vec![
            vec![ComplexScalar::new(0.5, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(-0.5, 0.0)],
        ];
        let qc = QuantumCircuit::new(1);
        let optimizer = VqeOptimizer::GradientDescent {
            learning_rate: 0.01,
            max_iter: 5,
        };
        let mut solver = VqeSolver::new(h, qc, optimizer);
        let result = solver.optimize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_qaoa_optimize() {
        let h = vec![
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(-1.0, 0.0)],
        ];
        let mixer = vec![
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0)],
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
        ];
        let mut solver = QaoaSolver::new(h, mixer, 1);
        let result = solver.optimize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_grover_not_found() {
        let oracle = Box::new(|_: usize| false);
        let result = grover_search(oracle, 3, 1);
        assert!(result.is_none());
    }
}
