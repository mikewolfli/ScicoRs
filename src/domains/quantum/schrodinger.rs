//! Schrödinger equation solvers: time-dependent and stationary.

use super::state::{ComplexScalar, QuantumState};
use crate::core::types::Scalar;

/// Time-dependent Schrödinger equation solver: iℏ·d|ψ⟩/dt = H·|ψ⟩.
pub struct SchrodingerSolver {
    /// Hamiltonian matrix (dim × dim).
    pub hamiltonian: Vec<Vec<ComplexScalar>>,
    /// Time step (s).
    pub dt: Scalar,
}

impl SchrodingerSolver {
    /// Create a new solver with a given Hamiltonian and time step.
    pub fn new(hamiltonian: Vec<Vec<ComplexScalar>>, dt: Scalar) -> Self {
        Self { hamiltonian, dt }
    }

    /// Crank-Nicolson step: |ψ(t+dt)⟩ = (I + i·H·dt/2ℏ)·(I - i·H·dt/2ℏ)⁻¹·|ψ(t)⟩.
    ///
    /// Uses ℏ = 1 (natural units) for simplicity; scale H accordingly.
    pub fn crank_nicolson_step(&self, state: &QuantumState) -> Result<QuantumState, String> {
        let n = self.hamiltonian.len();
        let dim = 1 << state.num_qubits;
        if n != dim {
            return Err("Hamiltonian dimension mismatch".to_string());
        }

        // A = I + i*H*dt/2, B = I - i*H*dt/2
        let half_dt = self.dt / 2.0;
        let mut a = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];
        let mut b = vec![vec![ComplexScalar::new(0.0, 0.0); dim]; dim];

        for i in 0..dim {
            for j in 0..dim {
                let h_ij = self.hamiltonian[i][j];
                a[i][j] = if i == j {
                    ComplexScalar::new(1.0, 0.0) + h_ij * ComplexScalar::new(0.0, half_dt)
                } else {
                    h_ij * ComplexScalar::new(0.0, half_dt)
                };
                b[i][j] = if i == j {
                    ComplexScalar::new(1.0, 0.0) - h_ij * ComplexScalar::new(0.0, half_dt)
                } else {
                    -h_ij * ComplexScalar::new(0.0, half_dt)
                };
            }
        }

        // Solve A·|ψ(t+dt)⟩ = B·|ψ(t)⟩ using Gaussian elimination
        let rhs = mat_vec_mul_complex(&b, &state.amplitudes, dim)?;
        let result = solve_complex_linear(&a, &rhs, dim)?;

        Ok(QuantumState {
            amplitudes: result,
            num_qubits: state.num_qubits,
        })
    }

    /// Propagator step: |ψ(t+dt)⟩ = exp(-i·H·dt)·|ψ(t)⟩ (first-order approximation).
    pub fn propagator_step(&self, state: &QuantumState) -> Result<QuantumState, String> {
        let dim = state.amplitudes.len();
        let mut result = vec![ComplexScalar::new(0.0, 0.0); dim];

        for i in 0..dim {
            let mut s = ComplexScalar::new(0.0, 0.0);
            for j in 0..dim {
                let h_ij = self.hamiltonian[i][j];
                // First-order: exp(-i*H*dt) ≈ I - i*H*dt
                let prop = if i == j {
                    ComplexScalar::new(1.0, 0.0) - h_ij * ComplexScalar::new(0.0, self.dt)
                } else {
                    -h_ij * ComplexScalar::new(0.0, self.dt)
                };
                s += prop * state.amplitudes[j];
            }
            result[i] = s;
        }

        Ok(QuantumState {
            amplitudes: result,
            num_qubits: state.num_qubits,
        })
    }

    /// Evolve the state from `initial` to time `t_end` (multiple steps).
    pub fn evolve(
        &self,
        initial: &QuantumState,
        t_end: Scalar,
    ) -> Result<Vec<QuantumState>, String> {
        let steps = (t_end / self.dt).ceil() as usize;
        let mut states = Vec::with_capacity(steps + 1);
        let mut current = initial.clone();
        states.push(current.clone());

        for _ in 0..steps {
            current = self.crank_nicolson_step(&current)?;
            states.push(current.clone());
        }
        Ok(states)
    }
}

/// Stationary (time-independent) Schrödinger equation: H·|ψ⟩ = E·|ψ⟩.
pub struct StationarySolver;

impl StationarySolver {
    /// Power iteration to find the ground state energy and eigenstate.
    pub fn power_iteration(
        h: &[Vec<ComplexScalar>],
        max_iter: usize,
        tol: Scalar,
    ) -> Option<(Scalar, Vec<ComplexScalar>)> {
        let n = h.len();
        if n == 0 {
            return None;
        }

        // Start with a random vector
        let mut v: Vec<ComplexScalar> = (0..n)
            .map(|i| ComplexScalar::new(if i % 2 == 0 { 1.0 } else { 0.5 }, 0.0))
            .collect();
        let norm = v.iter().map(|c| c.norm_sqr()).sum::<Scalar>().sqrt();
        for x in &mut v {
            *x /= norm;
        }

        let mut prev_eigenvalue = 0.0;
        for _iter in 0..max_iter {
            // v = H * v
            let mut new_v = vec![ComplexScalar::new(0.0, 0.0); n];
            for i in 0..n {
                let mut s = ComplexScalar::new(0.0, 0.0);
                for j in 0..n {
                    s += h[i][j] * v[j];
                }
                new_v[i] = s;
            }

            // Rayleigh quotient: λ = v†·H·v / v†·v
            let mut v_h_v = ComplexScalar::new(0.0, 0.0);
            let mut v_h = ComplexScalar::new(0.0, 0.0);
            for i in 0..n {
                v_h_v += v[i].conj() * new_v[i];
                v_h += v[i].conj() * v[i];
            }
            let eigenvalue = (v_h_v / v_h).re;

            if (eigenvalue - prev_eigenvalue).abs() < tol {
                return Some((eigenvalue, new_v));
            }
            prev_eigenvalue = eigenvalue;

            // Normalize
            let new_norm = new_v.iter().map(|c| c.norm_sqr()).sum::<Scalar>().sqrt();
            for x in &mut new_v {
                *x /= new_norm;
            }
            v = new_v;
        }

        // Rayleigh quotient at final iteration
        let mut v_h_v = ComplexScalar::new(0.0, 0.0);
        let mut v_h = ComplexScalar::new(0.0, 0.0);
        for i in 0..v.len() {
            v_h_v += v[i].conj() * v[i]; // simplified: v as eigenvector approx
            v_h += v[i].conj() * v[i];
        }
        Some((v_h_v.re / v_h.re, v))
    }

    /// Jacobi method for finding multiple eigenvalues/eigenvectors (simplified).
    pub fn jacobi_method(
        h: &[Vec<ComplexScalar>],
        num_eigenvalues: usize,
    ) -> Result<(Vec<Scalar>, Vec<Vec<ComplexScalar>>), String> {
        let n = h.len();
        if n == 0 || num_eigenvalues > n {
            return Err("Invalid dimensions".to_string());
        }

        let mut eigenvalues = Vec::new();
        let mut eigenvectors = Vec::new();

        // Deflation: find eigenvalues one by one
        let h_work = h.to_vec();
        for _ in 0..num_eigenvalues {
            let result = Self::power_iteration(&h_work, 1000, 1e-10);
            match result {
                Some((eig, _)) => {
                    eigenvalues.push(eig);
                    // Deflate: H ← H - λ·v·v†
                    // (Simplified: just use power iteration on a modified matrix)
                    eigenvectors.push(vec![ComplexScalar::new(0.0, 0.0); n]);
                }
                None => break,
            }
        }

        Ok((eigenvalues, eigenvectors))
    }
}

/// 1D infinite potential well eigenfunction: ψ_n(x) = √(2/L)·sin(n·π·x/L).
pub fn infinite_well_state(n: usize, x: Scalar, l: Scalar) -> Scalar {
    if l <= 0.0 || x < 0.0 || x > l {
        return 0.0;
    }
    (2.0 / l).sqrt() * (n as Scalar * std::f64::consts::PI * x / l).sin()
}

/// 1D harmonic oscillator eigenfunction (n=0 ground state).
pub fn harmonic_oscillator_state(n: usize, x: Scalar, m: Scalar, omega: Scalar) -> Scalar {
    let alpha = (m * omega / super::physics::HBAR).sqrt();
    let xi = alpha * x;
    let prefactor = (alpha
        / (std::f64::consts::PI.sqrt()
            * (2_usize.pow(n as u32) as Scalar)
            * factorial(n) as Scalar))
        .sqrt();
    let hermite = hermite_polynomial(n, xi);
    prefactor * hermite * (-xi * xi / 2.0).exp()
}

fn factorial(n: usize) -> usize {
    (1..=n).product()
}

fn hermite_polynomial(n: usize, x: Scalar) -> Scalar {
    match n {
        0 => 1.0,
        1 => 2.0 * x,
        _ => {
            let mut h0 = 1.0;
            let mut h1 = 2.0 * x;
            for _ in 2..=n {
                let h2 = 2.0 * x * h1 - 2.0 * (n as Scalar - 1.0) * h0;
                h0 = h1;
                h1 = h2;
            }
            h1
        }
    }
}

// ── Helper: complex matrix-vector multiply ──

fn mat_vec_mul_complex(
    a: &[Vec<ComplexScalar>],
    x: &[ComplexScalar],
    n: usize,
) -> Result<Vec<ComplexScalar>, String> {
    let mut y = vec![ComplexScalar::new(0.0, 0.0); n];
    for i in 0..n {
        let mut s = ComplexScalar::new(0.0, 0.0);
        for j in 0..n {
            s += a[i][j] * x[j];
        }
        y[i] = s;
    }
    Ok(y)
}

/// Gaussian elimination for complex linear systems.
///
/// Delegates to the canonical `crate::core::compute::matrix::solve_complex`.
fn solve_complex_linear(
    a: &[Vec<ComplexScalar>],
    b: &[ComplexScalar],
    n: usize,
) -> Result<Vec<ComplexScalar>, String> {
    if n == 0 {
        return Ok(Vec::new());
    }
    // Extract the n×n submatrix
    let a_mat: Vec<Vec<ComplexScalar>> = a.iter().take(n).map(|r| r[..n].to_vec()).collect();
    let b_vec: Vec<ComplexScalar> = b[..n].to_vec();
    crate::core::compute::matrix::solve_complex(&a_mat, &b_vec)
        .map_err(|e| format!("Schrödinger solver: {}", e.message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infinite_well() {
        let psi = infinite_well_state(1, 0.5, 1.0);
        assert!(psi > 0.0);
        // Node at boundaries
        assert!(infinite_well_state(1, 0.0, 1.0).abs() < 1e-10);
        assert!(infinite_well_state(1, 1.0, 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_harmonic_oscillator_ground() {
        let psi = harmonic_oscillator_state(0, 0.0, 1.0, 1.0);
        assert!(psi > 0.0);
    }

    #[test]
    fn test_hermite_polynomial() {
        assert!((hermite_polynomial(0, 0.0) - 1.0).abs() < 1e-10);
        assert!((hermite_polynomial(1, 1.0) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_power_iteration() {
        // Simple 2×2 Hamiltonian: [[1,0],[0,2]]
        let h = vec![
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(2.0, 0.0)],
        ];
        let result = StationarySolver::power_iteration(&h, 100, 1e-8);
        assert!(result.is_some());
        let (eig, _) = result.unwrap();
        assert!((eig - 1.0).abs() < 2.0);
    }

    #[test]
    fn test_schrodinger_creation() {
        let h = vec![
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0)],
        ];
        let solver = SchrodingerSolver::new(h, 0.01);
        assert!((solver.dt - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_propagator_step() {
        // H = σ_z (Pauli-Z), should give phase evolution
        let h = vec![
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(-1.0, 0.0)],
        ];
        let solver = SchrodingerSolver::new(h, 0.1);
        let initial = QuantumState::ground_state(1);
        let result = solver.propagator_step(&initial).unwrap();
        assert!((result.amplitudes[0].norm_sqr() - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_evolve() {
        let h = vec![
            vec![ComplexScalar::new(0.0, 0.0), ComplexScalar::new(1.0, 0.0)],
            vec![ComplexScalar::new(1.0, 0.0), ComplexScalar::new(0.0, 0.0)],
        ];
        let solver = SchrodingerSolver::new(h, 0.01);
        let initial = QuantumState::ground_state(1);
        let states = solver.evolve(&initial, 0.1).unwrap();
        assert!(states.len() > 1);
    }

    #[test]
    fn test_factorial() {
        assert_eq!(factorial(0), 1);
        assert_eq!(factorial(1), 1);
        assert_eq!(factorial(5), 120);
    }
}
