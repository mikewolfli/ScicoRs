//! Quantum Physics & Quantum Computing Simulation (Phase 30).
//!
//! Provides quantum state representations (state vectors, density matrices),
//! quantum gates (single-qubit, multi-qubit, parameterized rotations),
//! Schrödinger equation solvers (time-dependent/stationary), Lindblad master
//! equation solver for open quantum systems, quantum measurement (projective,
//! POVM), quantum algorithms (VQE, QAOA, Grover, HHL, QFT), and quantum
//! analysis tools (fidelity, entanglement entropy, trace distance).

#![allow(
    clippy::approx_constant,
    clippy::useless_vec,
    clippy::excessive_precision
)]

pub mod algorithms;
pub mod analysis;
pub mod gates;
pub mod lindblad;
pub mod measurement;
pub mod mps;
pub mod noise_channel;
pub mod physics;
pub mod qec;
pub mod schrodinger;
pub mod state;

pub use algorithms::{
    QaoaSolver, VqeOptimizer, VqeSolver, grover_search, quantum_fourier_transform,
};
pub use analysis::{
    entanglement_entropy, fidelity_density, measurement_statistics, quantum_mutual_information,
    trace_distance,
};
pub use gates::{
    GateOperation, GateType, MultiQubitGate, QuantumCircuit, SingleQubitGate, cnot_matrix,
    hadamard_matrix, pauli_x_matrix, pauli_y_matrix, pauli_z_matrix, rotation_x, rotation_y,
    rotation_z, swap_matrix, toffoli_matrix,
};
pub use lindblad::{
    LindbladSolver, amplitude_damping, dephasing_channel, depolarizing_channel, phase_flip_channel,
    spontaneous_emission, thermal_bath,
};
pub use measurement::{
    MeasurementResult, PovmMeasurement, bell_inequality_violation, computational_basis_measurement,
    concurrence, projective_measurement, quantum_state_tomography,
};
pub use mps::MatrixProductState;
pub use noise_channel::{NoiseChannel, NoiseDensityMatrix, pure_state_density, PAULI_X, PAULI_Y, PAULI_Z};
pub use physics::*;
pub use qec::{LogicalQubit, QuantumCode};
pub use schrodinger::{
    SchrodingerSolver, StationarySolver, harmonic_oscillator_state, infinite_well_state,
};
pub use state::{ComplexScalar, DensityMatrix, QuantumState};
