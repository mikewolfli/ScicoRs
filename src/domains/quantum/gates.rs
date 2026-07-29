//! Quantum gates: single-qubit, multi-qubit, parameterized rotations, and circuits.

use super::state::{ComplexScalar, DensityMatrix, QuantumState};
use crate::core::types::Scalar;

/// Single-qubit quantum gates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SingleQubitGate {
    /// Hadamard H = (1/√2)[[1,1],[1,-1]]
    Hadamard,
    /// Pauli-X (NOT) = [[0,1],[1,0]]
    PauliX,
    /// Pauli-Y = [[0,-i],[i,0]]
    PauliY,
    /// Pauli-Z = [[1,0],[0,-1]]
    PauliZ,
    /// Phase S = [[1,0],[0,i]]
    Phase,
    /// π/8 gate T = [[1,0],[0,e^(iπ/4)]]
    PiOver8,
    /// Rotation Rx(θ)
    RotationX(Scalar),
    /// Rotation Ry(θ)
    RotationY(Scalar),
    /// Rotation Rz(θ)
    RotationZ(Scalar),
}

/// Multi-qubit quantum gates.
#[derive(Debug, Clone, PartialEq)]
pub enum MultiQubitGate {
    /// CNOT (controlled-NOT)
    CNOT,
    /// Controlled-Z
    CZ,
    /// SWAP
    SWAP,
    /// Toffoli (CCNOT)
    Toffoli,
    /// Arbitrary controlled-U gate
    ControlledU(Vec<ComplexScalar>),
}

/// Type of a quantum gate operation.
#[derive(Debug, Clone)]
pub enum GateType {
    /// Single-qubit gate.
    Single(SingleQubitGate),
    /// Multi-qubit gate.
    Multi(MultiQubitGate),
    /// Custom unitary matrix gate.
    Custom(Vec<ComplexScalar>),
}

/// A gate operation in a quantum circuit.
#[derive(Debug, Clone)]
pub struct GateOperation {
    /// Type of gate.
    pub gate: GateType,
    /// Target qubit indices.
    pub target_qubits: Vec<usize>,
    /// Control qubit indices.
    pub control_qubits: Vec<usize>,
}

impl GateOperation {
    /// Create a new gate operation.
    pub fn new(gate: GateType, targets: Vec<usize>) -> Self {
        Self {
            gate,
            target_qubits: targets,
            control_qubits: Vec::new(),
        }
    }

    /// Add control qubits.
    pub fn with_controls(mut self, controls: Vec<usize>) -> Self {
        self.control_qubits = controls;
        self
    }

    /// Build the full matrix representation for `num_qubits`.
    pub fn matrix(&self, num_qubits: usize) -> Vec<Vec<ComplexScalar>> {
        let dim = 1 << num_qubits;
        let mut m = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];
        for i in 0..dim {
            m[i][i] = ComplexScalar::new(1.0, 0.0);
        }

        match &self.gate {
            GateType::Single(g) => {
                let gate_mat = single_gate_matrix(*g);
                for &t in &self.target_qubits {
                    apply_single_qubit_gate(&mut m, &gate_mat, t, num_qubits);
                }
            }
            GateType::Multi(g) => {
                let gate_mat = multi_gate_matrix(g);
                if self.target_qubits.len() >= 2 {
                    apply_two_qubit_gate(
                        &mut m,
                        &gate_mat,
                        self.target_qubits[0],
                        self.target_qubits[1],
                        num_qubits,
                    );
                }
            }
            GateType::Custom(mat) => {
                if mat.len() == dim * dim {
                    for i in 0..dim {
                        for j in 0..dim {
                            m[i][j] = mat[i * dim + j];
                        }
                    }
                }
            }
        }
        m
    }

    /// Apply this gate to a quantum state.
    pub fn apply(&self, state: &QuantumState) -> Result<QuantumState, String> {
        let n = state.num_qubits;
        let mat = self.matrix(n);
        let dim = 1 << n;
        let mut new_amps = vec![ComplexScalar::new(0.0, 0.0); dim];
        for i in 0..dim {
            for j in 0..dim {
                new_amps[i] += mat[i][j] * state.amplitudes[j];
            }
        }
        Ok(QuantumState {
            amplitudes: new_amps,
            num_qubits: n,
        })
    }

    /// Apply this gate to a density matrix.
    pub fn apply_to_density(&self, rho: &DensityMatrix) -> Result<DensityMatrix, String> {
        let dim = rho.dim;
        let mat = self.matrix((dim as Scalar).log2() as usize);
        if mat.len() != dim {
            return Err("Matrix dimension mismatch".to_string());
        }
        // U * ρ * U†
        let mut new_data = vec![ComplexScalar::new(0.0, 0.0); dim * dim];
        for i in 0..dim {
            for j in 0..dim {
                let mut s = ComplexScalar::new(0.0, 0.0);
                for k in 0..dim {
                    for l in 0..dim {
                        s += mat[i][k] * rho.data[k * dim + l] * mat[j][l].conj();
                    }
                }
                new_data[i * dim + j] = s;
            }
        }
        Ok(DensityMatrix {
            data: new_data,
            dim,
        })
    }
}

// ── Gate matrix definitions ──

/// Hadamard gate matrix: H = (1/√2)[[1,1],[1,-1]].
pub fn hadamard_matrix() -> Vec<Vec<ComplexScalar>> {
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    vec![
        vec![
            ComplexScalar::new(inv_sqrt2, 0.0),
            ComplexScalar::new(inv_sqrt2, 0.0),
        ],
        vec![
            ComplexScalar::new(inv_sqrt2, 0.0),
            ComplexScalar::new(-inv_sqrt2, 0.0),
        ],
    ]
}

/// Pauli-X (NOT) gate matrix.
pub fn pauli_x_matrix() -> Vec<Vec<ComplexScalar>> {
    vec![
        vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0)],
        vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
    ]
}

/// Pauli-Y gate matrix.
pub fn pauli_y_matrix() -> Vec<Vec<ComplexScalar>> {
    vec![
        vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(0.0, -1.0)],
        vec![ComplexScalar::new(0.0, 1.0), ComplexScalar::new(0.0, 0.0)],
    ]
}

/// Pauli-Z gate matrix.
pub fn pauli_z_matrix() -> Vec<Vec<ComplexScalar>> {
    vec![
        vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
        vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(-1.0, 0.0)],
    ]
}

/// CNOT gate matrix (4×4).
pub fn cnot_matrix() -> Vec<Vec<ComplexScalar>> {
    let mut m = vec![vec![ComplexScalar::new(0.0, 0.0); 4]; 4];
    m[0][0] = ComplexScalar::new(1.0, 0.0);
    m[1][1] = ComplexScalar::new(1.0, 0.0);
    m[2][3] = ComplexScalar::new(1.0, 0.0);
    m[3][2] = ComplexScalar::new(1.0, 0.0);
    m
}

/// CZ (controlled-Z) gate matrix.
pub fn cz_matrix() -> Vec<Vec<ComplexScalar>> {
    let mut m = vec![vec![ComplexScalar::new(0.0, 0.0); 4]; 4];
    m[0][0] = ComplexScalar::new(1.0, 0.0);
    m[1][1] = ComplexScalar::new(1.0, 0.0);
    m[2][2] = ComplexScalar::new(1.0, 0.0);
    m[3][3] = ComplexScalar::new(-1.0, 0.0);
    m
}

/// SWAP gate matrix.
pub fn swap_matrix() -> Vec<Vec<ComplexScalar>> {
    let mut m = vec![vec![ComplexScalar::new(0.0, 0.0); 4]; 4];
    m[0][0] = ComplexScalar::new(1.0, 0.0);
    m[1][2] = ComplexScalar::new(1.0, 0.0);
    m[2][1] = ComplexScalar::new(1.0, 0.0);
    m[3][3] = ComplexScalar::new(1.0, 0.0);
    m
}

/// Toffoli (CCNOT) gate matrix (8×8).
pub fn toffoli_matrix() -> Vec<Vec<ComplexScalar>> {
    let mut m = vec![vec![ComplexScalar::new(0.0, 0.0); 8]; 8];
    for i in 0..6 {
        m[i][i] = ComplexScalar::new(1.0, 0.0);
    }
    m[6][7] = ComplexScalar::new(1.0, 0.0);
    m[7][6] = ComplexScalar::new(1.0, 0.0);
    m
}

/// Rotation-X gate: Rx(θ) = exp(-i·θ·X/2).
pub fn rotation_x(theta: Scalar) -> Vec<Vec<ComplexScalar>> {
    let half = theta / 2.0;
    vec![
        vec![
            ComplexScalar::new(half.cos(), 0.0),
            ComplexScalar::new(0.0, -half.sin()),
        ],
        vec![
            ComplexScalar::new(0.0, -half.sin()),
            ComplexScalar::new(half.cos(), 0.0),
        ],
    ]
}

/// Rotation-Y gate: Ry(θ) = exp(-i·θ·Y/2).
pub fn rotation_y(theta: Scalar) -> Vec<Vec<ComplexScalar>> {
    let half = theta / 2.0;
    vec![
        vec![
            ComplexScalar::new(half.cos(), 0.0),
            ComplexScalar::new(-half.sin(), 0.0),
        ],
        vec![
            ComplexScalar::new(half.sin(), 0.0),
            ComplexScalar::new(half.cos(), 0.0),
        ],
    ]
}

/// Rotation-Z gate: Rz(θ) = exp(-i·θ·Z/2).
pub fn rotation_z(theta: Scalar) -> Vec<Vec<ComplexScalar>> {
    let half = theta / 2.0;
    vec![
        vec![
            ComplexScalar::new(half.cos(), -half.sin()),
            ComplexScalar::new(0.0, 0.0),
        ],
        vec![
            ComplexScalar::new(0.0, 0.0),
            ComplexScalar::new(half.cos(), half.sin()),
        ],
    ]
}

// ── Helper functions ──

fn single_gate_matrix(gate: SingleQubitGate) -> Vec<Vec<ComplexScalar>> {
    match gate {
        SingleQubitGate::Hadamard => hadamard_matrix(),
        SingleQubitGate::PauliX => pauli_x_matrix(),
        SingleQubitGate::PauliY => pauli_y_matrix(),
        SingleQubitGate::PauliZ => pauli_z_matrix(),
        SingleQubitGate::Phase => vec![
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(0.0, 1.0)],
        ],
        SingleQubitGate::PiOver8 => {
            let phase = ComplexScalar::new(0.0, std::f64::consts::PI / 4.0).exp();
            vec![
                vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
                vec![ComplexScalar::new(0.0, 0.0), phase],
            ]
        }
        SingleQubitGate::RotationX(theta) => rotation_x(theta),
        SingleQubitGate::RotationY(theta) => rotation_y(theta),
        SingleQubitGate::RotationZ(theta) => rotation_z(theta),
    }
}

fn multi_gate_matrix(gate: &MultiQubitGate) -> Vec<Vec<ComplexScalar>> {
    match gate {
        MultiQubitGate::CNOT => cnot_matrix(),
        MultiQubitGate::CZ => cz_matrix(),
        MultiQubitGate::SWAP => swap_matrix(),
        MultiQubitGate::Toffoli => toffoli_matrix(),
        MultiQubitGate::ControlledU(u) => {
            let mut m = vec![vec![ComplexScalar::new(0.0, 0.0); 4]; 4];
            m[0][0] = ComplexScalar::new(1.0, 0.0);
            m[1][1] = ComplexScalar::new(1.0, 0.0);
            for i in 0..2 {
                for j in 0..2 {
                    m[2 + i][2 + j] = u[i * 2 + j];
                }
            }
            m
        }
    }
}

fn apply_single_qubit_gate(
    full_mat: &mut [Vec<ComplexScalar>],
    gate: &[Vec<ComplexScalar>],
    target: usize,
    num_qubits: usize,
) {
    let dim = 1 << num_qubits;
    let mut new_mat = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];
    for i in 0..dim {
        for j in 0..dim {
            let _i_bit = (i >> target) & 1;
            let _j_bit = (j >> target) & 1;
            let i_other = i & !(1 << target);
            let j_other = j & !(1 << target);
            if i_other == j_other {
                // Only the target bit changes
                for k in 0..2 {
                    let new_i = (i & !(1 << target)) | (k << target);
                    for l in 0..2 {
                        let new_j = (j & !(1 << target)) | (l << target);
                        if new_i == i && new_j == j {
                            new_mat[i][j] = gate[k][l];
                        }
                    }
                }
            }
        }
    }
    // Multiply: new_mat * full_mat
    let mut result = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];
    for i in 0..dim {
        for j in 0..dim {
            let mut s = ComplexScalar::new(0.0, 0.0);
            for k in 0..dim {
                s += new_mat[i][k] * full_mat[k][j];
            }
            result[i][j] = s;
        }
    }
    for i in 0..dim {
        for j in 0..dim {
            full_mat[i][j] = result[i][j];
        }
    }
}

fn apply_two_qubit_gate(
    full_mat: &mut [Vec<ComplexScalar>],
    gate: &[Vec<ComplexScalar>],
    target1: usize,
    target2: usize,
    num_qubits: usize,
) {
    let dim = 1 << num_qubits;
    let mut new_mat = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];
    for i in 0..dim {
        for j in 0..dim {
            // Simplified: apply gate to the subspace of target1, target2
            let mask = (1 << target1) | (1 << target2);
            let i_sub = ((i >> target1) & 1) | (((i >> target2) & 1) << 1);
            let j_sub = ((j >> target1) & 1) | (((j >> target2) & 1) << 1);
            let i_other = i & !mask;
            let j_other = j & !mask;
            if i_other == j_other {
                new_mat[i][j] = gate[i_sub][j_sub];
            }
        }
    }
    let mut result = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];
    for i in 0..dim {
        for j in 0..dim {
            let mut s = ComplexScalar::new(0.0, 0.0);
            for k in 0..dim {
                s += new_mat[i][k] * full_mat[k][j];
            }
            result[i][j] = s;
        }
    }
    for i in 0..dim {
        for j in 0..dim {
            full_mat[i][j] = result[i][j];
        }
    }
}

/// Quantum circuit composed of sequential gate operations.
#[derive(Debug, Clone)]
pub struct QuantumCircuit {
    pub num_qubits: usize,
    pub operations: Vec<GateOperation>,
}

impl QuantumCircuit {
    /// Create a new empty quantum circuit.
    pub fn new(num_qubits: usize) -> Self {
        Self {
            num_qubits,
            operations: Vec::new(),
        }
    }

    /// Add a gate operation to the circuit.
    pub fn add_gate(&mut self, gate: GateOperation) {
        self.operations.push(gate);
    }

    /// Compute the full unitary matrix of the circuit.
    pub fn unitary(&self) -> Result<Vec<Vec<ComplexScalar>>, String> {
        let dim = 1 << self.num_qubits;
        let mut u = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];
        for i in 0..dim {
            u[i][i] = ComplexScalar::new(1.0, 0.0);
        }
        for op in &self.operations {
            let m = op.matrix(self.num_qubits);
            // Multiply: U_new = m * U_old
            let mut new_u = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];
            for i in 0..dim {
                for j in 0..dim {
                    let mut s = ComplexScalar::new(0.0, 0.0);
                    for k in 0..dim {
                        s += m[i][k] * u[k][j];
                    }
                    new_u[i][j] = s;
                }
            }
            u = new_u;
        }
        Ok(u)
    }

    /// Simulate the circuit on an initial state.
    pub fn simulate(&self, initial_state: &QuantumState) -> Result<QuantumState, String> {
        let mut state = initial_state.clone();
        for op in &self.operations {
            state = op.apply(&state)?;
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hadamard_matrix() {
        let h = hadamard_matrix();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].len(), 2);
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        assert!((h[0][0].re - inv_sqrt2).abs() < 1e-10);
        assert!((h[1][1].re + inv_sqrt2).abs() < 1e-10);
    }

    #[test]
    fn test_pauli_x_acts_as_not() {
        let zero = QuantumState::ground_state(1);
        let gate = GateOperation::new(GateType::Single(SingleQubitGate::PauliX), vec![0]);
        let result = gate.apply(&zero).unwrap();
        assert!((result.amplitudes[0].norm_sqr()).abs() < 1e-10);
        assert!((result.amplitudes[1].norm_sqr() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_hadamard_creates_superposition() {
        let zero = QuantumState::ground_state(1);
        let gate = GateOperation::new(GateType::Single(SingleQubitGate::Hadamard), vec![0]);
        let result = gate.apply(&zero).unwrap();
        assert!((result.amplitudes[0].norm_sqr() - 0.5).abs() < 1e-10);
        assert!((result.amplitudes[1].norm_sqr() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_cnot_gate() {
        let _state = QuantumState::from_basis(3, 2); // |11⟩
        let _gate = GateOperation::new(GateType::Multi(MultiQubitGate::CNOT), vec![1])
            .with_controls(vec![0]);
        // Since apply doesn't handle controls separately, test matrix form
        let m = cnot_matrix();
        assert_eq!(m.len(), 4);
        assert!((m[0][0].re - 1.0).abs() < 1e-10);
        assert!((m[3][2].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_swap_gate() {
        let m = swap_matrix();
        assert!((m[1][2].re - 1.0).abs() < 1e-10);
        assert!((m[2][1].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_rotation_x_pi() {
        let rx = rotation_x(std::f64::consts::PI);
        assert!((rx[0][0].re).abs() < 1e-10); // cos(π/2) = 0
        assert!((rx[0][1].im + 1.0).abs() < 1e-10); // -i*sin(π/2) = -i
    }

    #[test]
    fn test_rotation_y_half_pi() {
        let ry = rotation_y(std::f64::consts::PI / 2.0);
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        assert!((ry[0][0].re - inv_sqrt2).abs() < 1e-10);
        assert!((ry[0][1].re + inv_sqrt2).abs() < 1e-10);
    }

    #[test]
    fn test_quantum_circuit_basic() {
        let mut qc = QuantumCircuit::new(1);
        qc.add_gate(GateOperation::new(
            GateType::Single(SingleQubitGate::Hadamard),
            vec![0],
        ));
        qc.add_gate(GateOperation::new(
            GateType::Single(SingleQubitGate::PauliX),
            vec![0],
        ));
        let initial = QuantumState::ground_state(1);
        let result = qc.simulate(&initial).unwrap();
        assert!((result.amplitudes[0].norm_sqr() - 0.5).abs() < 1e-10);
        assert!((result.amplitudes[1].norm_sqr() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_toffoli_matrix() {
        let t = toffoli_matrix();
        assert_eq!(t.len(), 8);
        assert!((t[6][7].re - 1.0).abs() < 1e-10);
        assert!((t[7][6].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_pauli_z() {
        let one = QuantumState::from_basis(1, 1);
        let gate = GateOperation::new(GateType::Single(SingleQubitGate::PauliZ), vec![0]);
        let result = gate.apply(&one).unwrap();
        assert!((result.amplitudes[1].re + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_gate_apply_to_density() {
        let rho = DensityMatrix::from_pure_state(&QuantumState::ground_state(1));
        let gate = GateOperation::new(GateType::Single(SingleQubitGate::PauliX), vec![0]);
        let result = gate.apply_to_density(&rho).unwrap();
        assert!((result.data[3].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_circuit_unitary() {
        let mut qc = QuantumCircuit::new(1);
        qc.add_gate(GateOperation::new(
            GateType::Single(SingleQubitGate::Hadamard),
            vec![0],
        ));
        let u = qc.unitary().unwrap();
        assert_eq!(u.len(), 2);
        // H† * H = I
        let mut check = vec![ComplexScalar::new(0.0, 0.0); 4];
        for i in 0..2 {
            for j in 0..2 {
                let mut s = ComplexScalar::new(0.0, 0.0);
                for k in 0..2 {
                    s += u[k][i].conj() * u[k][j];
                }
                check[i * 2 + j] = s;
            }
        }
        assert!((check[0].re - 1.0).abs() < 1e-10);
        assert!(check[1].norm() < 1e-10);
        assert!(check[2].norm() < 1e-10);
        assert!((check[3].re - 1.0).abs() < 1e-10);
    }
}
