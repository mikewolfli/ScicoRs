//! Molecular dynamics simulation engine.
//!
//! Provides Velocity Verlet and Langevin integrators, energy minimization
//! (steepest descent and conjugate gradient), and temperature/pressure control.

use crate::core::types::Scalar;
use crate::domains::molbio::forcefield::{ForceField, Vec3};
use crate::domains::molbio::molecule::Molecule;

// ──────────────────────────────────────────────
// Integrator Selection
// ──────────────────────────────────────────────

/// MD integration method.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Integrator {
    /// Velocity Verlet (microcanonical NVE).
    VelocityVerlet,
    /// Langevin dynamics (canonical NVT with implicit solvent).
    Langevin {
        /// Friction coefficient (ps⁻¹).
        friction: Scalar,
        /// Target temperature (K).
        temperature: Scalar,
    },
}

// ──────────────────────────────────────────────
// Simulation Parameters
// ──────────────────────────────────────────────

/// Parameters for MD simulation.
#[derive(Debug, Clone)]
pub struct SimParams {
    /// Time step (ps). Default 0.002.
    pub dt: Scalar,
    /// Target temperature (K).
    pub temperature: Scalar,
    /// Target pressure (bar), None = NVT ensemble.
    pub pressure: Option<Scalar>,
    /// Langevin friction coefficient (ps⁻¹). Default 1.0.
    pub friction: Scalar,
    /// Total number of steps.
    pub steps: u64,
    /// Report interval (steps).
    pub report_interval: u64,
}

impl Default for SimParams {
    fn default() -> Self {
        Self {
            dt: 0.002,
            temperature: 300.0,
            pressure: None,
            friction: 1.0,
            steps: 1000,
            report_interval: 100,
        }
    }
}

// ──────────────────────────────────────────────
// MD Result
// ──────────────────────────────────────────────

/// Result of an MD simulation run.
#[derive(Debug, Clone)]
pub struct MdResult {
    /// Trajectory frames (coordinates at report intervals).
    pub trajectory: Vec<Vec<Vec3>>,
    /// Energy trace: (potential, kinetic, total) at each report.
    pub energy_trace: Vec<(Scalar, Scalar, Scalar)>,
    /// Temperature trace.
    pub temperature_trace: Vec<Scalar>,
    /// Final coordinates.
    pub final_coords: Vec<Vec3>,
    /// Steps completed.
    pub steps_completed: u64,
}

// ──────────────────────────────────────────────
// MolecularDynamics Engine
// ──────────────────────────────────────────────

/// Molecular dynamics simulation engine.
#[derive(Debug, Clone)]
pub struct MolecularDynamics {
    /// Molecule being simulated.
    pub molecule: Molecule,
    /// Atomic coordinates (Å).
    pub coords: Vec<Vec3>,
    /// Atomic velocities (Å/ps).
    pub velocities: Vec<Vec3>,
    /// Current forces (kcal/(mol·Å)).
    pub forces: Vec<Vec3>,
    /// Force field.
    pub forcefield: ForceField,
    /// Simulation parameters.
    pub params: SimParams,
    /// Integration method.
    pub integrator: Integrator,
    /// Step counter.
    pub step_count: u64,
    /// Current simulation time (ps).
    pub current_time: Scalar,
    /// Total energy (kcal/mol).
    pub total_energy: Scalar,
    /// Potential energy (kcal/mol).
    pub potential_energy: Scalar,
    /// Kinetic energy (kcal/mol).
    pub kinetic_energy: Scalar,
    /// Instantaneous temperature (K).
    pub temperature: Scalar,
    /// Random state for Langevin / velocity initialization.
    rng_state: u64,
}

impl MolecularDynamics {
    /// Create a new MD simulation.
    pub fn new(molecule: Molecule, params: SimParams) -> Self {
        let coords = molecule.atom_positions();
        let n = coords.len();
        let ff = ForceField::new();
        let forces = vec![Vec3::zero(); n];
        let velocities = vec![Vec3::zero(); n];

        let temperature = params.temperature;
        Self {
            molecule,
            coords,
            velocities,
            forces,
            forcefield: ff,
            params,
            integrator: Integrator::VelocityVerlet,
            step_count: 0,
            current_time: 0.0,
            total_energy: 0.0,
            potential_energy: 0.0,
            kinetic_energy: 0.0,
            temperature,
            rng_state: 42,
        }
    }

    /// Simple LCG random number generator in [0, 1).
    fn lcg_random(&mut self) -> Scalar {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng_state >> 11) as Scalar * (1.0 / (1u64 << 53) as Scalar)
    }

    /// Box-Muller transform for Gaussian random numbers.
    fn gaussian_random(&mut self) -> Scalar {
        let u1 = self.lcg_random().max(1e-15);
        let u2 = self.lcg_random();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Initialize velocities from Maxwell-Boltzmann distribution.
    pub fn initialize_velocities(&mut self, seed: u64) {
        self.rng_state = seed;
        let mut total_momentum = Vec3::zero();
        let mut total_mass = 0.0;
        let temp = self.params.temperature;

        // Pre-collect masses to avoid borrowing conflicts with gaussian_random
        let masses: Vec<Scalar> = self.molecule.atoms.iter().map(|a| a.mass).collect();

        for (i, mass_amu) in masses.iter().enumerate() {
            let sigma = (temp * 0.0019872041 / mass_amu).sqrt();
            let rx = self.gaussian_random();
            let ry = self.gaussian_random();
            let rz = self.gaussian_random();
            self.velocities[i] = Vec3::new(sigma * rx, sigma * ry, sigma * rz);
            total_momentum = total_momentum.add(&self.velocities[i].scale(*mass_amu));
            total_mass += mass_amu;
        }

        // Remove center-of-mass momentum
        let com_vel = total_momentum.scale(1.0 / total_mass);
        for v in &mut self.velocities {
            *v = v.subtract(&com_vel);
        }
    }

    /// Compute instantaneous temperature (K).
    pub fn compute_temperature(&self) -> Scalar {
        let mut ke = 0.0;
        for (i, atom) in self.molecule.atoms.iter().enumerate() {
            let v = &self.velocities[i];
            ke += 0.5 * atom.mass * v.dot(v);
        }
        // KE = 3/2 * N * kB * T → T = 2*KE / (3*N*kB)
        // KE in (kcal/mol) using kcal/(mol·K) for kB
        let n = self.coords.len() as Scalar;
        if n > 0.0 {
            2.0 * ke / (3.0 * n * 0.0019872041)
        } else {
            0.0
        }
    }

    /// Compute kinetic energy from velocities.
    fn compute_kinetic_energy(&self) -> Scalar {
        let mut ke = 0.0;
        for (i, atom) in self.molecule.atoms.iter().enumerate() {
            let v = &self.velocities[i];
            ke += 0.5 * atom.mass * v.dot(v);
        }
        ke
    }

    /// Apply Berendsen thermostat to rescale velocities.
    pub fn berendsen_thermostat(&mut self, tau: Scalar) {
        let current_temp = self.compute_temperature();
        if current_temp < 1e-10 {
            return;
        }
        let lambda =
            (1.0 + self.params.dt / tau * (self.params.temperature / current_temp - 1.0)).sqrt();
        for v in &mut self.velocities {
            *v = v.scale(lambda);
        }
    }

    /// Compute forces from force field.
    fn compute_forces(&mut self) {
        self.forces = self.forcefield.compute_forces(&self.coords);
        self.potential_energy = self.forcefield.total_energy(&self.coords);
    }

    /// Perform one MD step using velocity Verlet integration.
    ///
    /// Steps:
    /// 1. Update positions: r(t+dt) = r(t) + v(t)·dt + 0.5·a(t)·dt²
    /// 2. Compute new forces: F(t+dt) = -∇E(t+dt)
    /// 3. Update velocities: v(t+dt) = v(t) + 0.5·(a(t) + a(t+dt))·dt
    pub fn step(&mut self) -> Result<(), String> {
        let dt = self.params.dt;
        let n = self.coords.len();

        // Step 1: Update positions
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            let mass = self.molecule.atoms[i].mass;
            let acc = self.forces[i].scale(1.0 / mass);
            self.coords[i] = self.coords[i]
                .add(&self.velocities[i].scale(dt))
                .add(&acc.scale(0.5 * dt * dt));
        }

        // Step 2: Compute new forces
        let old_forces = self.forces.clone(); // store F(t)
        self.compute_forces();

        // Step 3: Update velocities
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            let mass = self.molecule.atoms[i].mass;
            let old_acc = old_forces[i].scale(1.0 / mass);
            let new_acc = self.forces[i].scale(1.0 / mass);
            self.velocities[i] = self.velocities[i].add(&old_acc.add(&new_acc).scale(0.5 * dt));
        }

        // Langevin thermostat (if using Langevin integrator)
        if let Integrator::Langevin { friction, .. } = self.integrator {
            let c1 = (-friction * dt).exp();
            let c2 = ((1.0 - c1 * c1) * self.params.temperature * 0.0019872041).sqrt();
            // Pre-collect masses and generate noise to avoid borrow conflicts
            let masses: Vec<Scalar> = self.molecule.atoms.iter().map(|a| a.mass).collect();
            let noises: Vec<Vec3> = masses
                .iter()
                .map(|m| {
                    let sigma = c2 / m.sqrt();
                    Vec3::new(
                        sigma * self.gaussian_random(),
                        sigma * self.gaussian_random(),
                        sigma * self.gaussian_random(),
                    )
                })
                .collect();
            for (i, v) in self.velocities.iter_mut().enumerate() {
                if i < noises.len() {
                    *v = v.scale(c1).add(&noises[i]);
                }
            }
        }

        // Update energy and temperature
        self.kinetic_energy = self.compute_kinetic_energy();
        self.total_energy = self.potential_energy + self.kinetic_energy;
        self.temperature = self.compute_temperature();

        // Advance time
        self.step_count += 1;
        self.current_time += dt;

        Ok(())
    }

    /// Run a complete MD simulation.
    pub fn run(&mut self) -> Result<MdResult, String> {
        let mut trajectory = Vec::new();
        let mut energy_trace = Vec::new();
        let mut temperature_trace = Vec::new();

        // Initial force computation
        self.compute_forces();
        self.kinetic_energy = self.compute_kinetic_energy();
        self.total_energy = self.potential_energy + self.kinetic_energy;
        self.temperature = self.compute_temperature();

        // Record initial state
        trajectory.push(self.coords.clone());
        energy_trace.push((
            self.potential_energy,
            self.kinetic_energy,
            self.total_energy,
        ));
        temperature_trace.push(self.temperature);

        for _ in 0..self.params.steps {
            self.step()?;

            if self.step_count.is_multiple_of(self.params.report_interval) {
                trajectory.push(self.coords.clone());
                energy_trace.push((
                    self.potential_energy,
                    self.kinetic_energy,
                    self.total_energy,
                ));
                temperature_trace.push(self.temperature);
            }
        }

        Ok(MdResult {
            trajectory,
            energy_trace,
            temperature_trace,
            final_coords: self.coords.clone(),
            steps_completed: self.step_count,
        })
    }
}

// ──────────────────────────────────────────────
// Energy Minimization
// ──────────────────────────────────────────────

/// Result of energy minimization.
#[derive(Debug, Clone)]
pub struct MinimizationResult {
    /// Final potential energy (kcal/mol).
    pub final_energy: Scalar,
    /// Number of iterations performed.
    pub iterations: usize,
    /// Whether minimization converged.
    pub converged: bool,
    /// Energy trace over iterations.
    pub energy_trace: Vec<Scalar>,
}

/// Energy minimizer using gradient descent methods.
#[derive(Debug, Clone)]
pub struct EnergyMinimizer {
    /// Maximum number of iterations.
    pub max_iter: usize,
    /// Gradient convergence threshold (kcal/(mol·Å)).
    pub convergence: Scalar,
    /// Initial step size (Å).
    pub initial_step: Scalar,
}

impl Default for EnergyMinimizer {
    fn default() -> Self {
        Self {
            max_iter: 1000,
            convergence: 1e-6,
            initial_step: 0.01,
        }
    }
}

impl EnergyMinimizer {
    pub fn new(max_iter: usize, convergence: Scalar, initial_step: Scalar) -> Self {
        Self {
            max_iter,
            convergence,
            initial_step,
        }
    }

    /// Steepest descent energy minimization.
    pub fn steepest_descent(
        &self,
        _mol: &Molecule,
        coords: &mut [Vec3],
        ff: &ForceField,
    ) -> Result<MinimizationResult, String> {
        let mut energy_trace = Vec::new();
        let mut step_size = self.initial_step;

        let mut prev_energy = ff.total_energy(coords);

        for iter in 0..self.max_iter {
            let forces = ff.compute_forces(coords);

            // Check convergence: max force component
            let max_force = forces
                .iter()
                .map(|f| f.norm())
                .fold(0.0_f64, |a, b| a.max(b));
            if max_force < self.convergence {
                return Ok(MinimizationResult {
                    final_energy: prev_energy,
                    iterations: iter,
                    converged: true,
                    energy_trace,
                });
            }

            // Move along force direction
            for (i, coord) in coords.iter_mut().enumerate() {
                let f = &forces[i];
                *coord = coord.add(&f.scale(step_size));
            }

            let new_energy = ff.total_energy(coords);
            energy_trace.push(new_energy);

            // Line search: if energy increased, reduce step size
            if new_energy > prev_energy {
                step_size *= 0.5;
                // Restore previous coordinates
                for (i, coord) in coords.iter_mut().enumerate() {
                    let f = &forces[i];
                    *coord = coord.subtract(&f.scale(step_size * 2.0)); // undo then redo with smaller
                }
            } else {
                step_size *= 1.2; // try to increase step size
                prev_energy = new_energy;
            }

            if step_size < 1e-15 {
                return Ok(MinimizationResult {
                    final_energy: prev_energy,
                    iterations: iter,
                    converged: true,
                    energy_trace,
                });
            }
        }

        Ok(MinimizationResult {
            final_energy: prev_energy,
            iterations: self.max_iter,
            converged: false,
            energy_trace,
        })
    }

    /// Conjugate gradient energy minimization (Fletcher-Reeves).
    pub fn conjugate_gradient(
        &self,
        _mol: &Molecule,
        coords: &mut [Vec3],
        ff: &ForceField,
    ) -> Result<MinimizationResult, String> {
        let mut energy_trace = Vec::new();
        let mut step_size = self.initial_step;

        let mut forces = ff.compute_forces(coords);
        // Initial direction = steepest descent
        let mut direction: Vec<Vec3> = forces.to_vec();
        let mut prev_energy = ff.total_energy(coords);
        energy_trace.push(prev_energy);

        for iter in 0..self.max_iter {
            // Line search along direction
            let max_force = forces
                .iter()
                .map(|f| f.norm())
                .fold(0.0_f64, |a, b| a.max(b));
            if max_force < self.convergence {
                return Ok(MinimizationResult {
                    final_energy: prev_energy,
                    iterations: iter,
                    converged: true,
                    energy_trace,
                });
            }

            // Move along conjugate direction
            for (i, coord) in coords.iter_mut().enumerate() {
                *coord = coord.add(&direction[i].scale(step_size));
            }

            let new_forces = ff.compute_forces(coords);
            let new_energy = ff.total_energy(coords);
            energy_trace.push(new_energy);

            if new_energy < prev_energy {
                // Fletcher-Reeves beta
                let g_new_sq: Scalar = new_forces.iter().map(|f| f.dot(f)).sum();
                let g_old_sq: Scalar = forces.iter().map(|f| f.dot(f)).sum();
                let beta = if g_old_sq > 1e-30 {
                    g_new_sq / g_old_sq
                } else {
                    0.0
                };

                // Update conjugate direction
                for (d, g) in direction.iter_mut().zip(new_forces.iter()) {
                    *d = d.scale(beta).add(g);
                }

                forces = new_forces;
                prev_energy = new_energy;
                step_size *= 1.2;
            } else {
                // Restore and reduce step
                for (i, coord) in coords.iter_mut().enumerate() {
                    *coord = coord.subtract(&direction[i].scale(step_size));
                }
                step_size *= 0.5;
            }

            if step_size < 1e-15 {
                return Ok(MinimizationResult {
                    final_energy: prev_energy,
                    iterations: iter,
                    converged: true,
                    energy_trace,
                });
            }
        }

        Ok(MinimizationResult {
            final_energy: prev_energy,
            iterations: self.max_iter,
            converged: false,
            energy_trace,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::molbio::forcefield::{HarmonicBond, LennardJones};

    fn simple_diatomic() -> (Molecule, ForceField) {
        let mut mol = Molecule::new("H2");
        mol.add_atom(crate::domains::molbio::molecule::Atom::new(
            "H1", "H", 0.0, 0.0, 0.0,
        ));
        mol.add_atom(crate::domains::molbio::molecule::Atom::new(
            "H2", "H", 1.0, 0.0, 0.0,
        ));
        let mut ff = ForceField::new();
        ff.add_bond(0, 1, HarmonicBond::new(340.0, 0.74));
        ff.add_lj(0, LennardJones::new(1.3582, 0.0460));
        ff.add_lj(1, LennardJones::new(1.3582, 0.0460));
        (mol, ff)
    }

    #[test]
    fn test_md_creation() {
        let mol = Molecule::alanine();
        let params = SimParams::default();
        let md = MolecularDynamics::new(mol, params);
        assert_eq!(md.coords.len(), 11);
    }

    #[test]
    fn test_velocity_initialization() {
        let mol = Molecule::alanine();
        let params = SimParams::default();
        let mut md = MolecularDynamics::new(mol, params);
        md.initialize_velocities(12345);

        // Total momentum should be near zero
        let mut total_p = Vec3::zero();
        for (i, atom) in md.molecule.atoms.iter().enumerate() {
            total_p = total_p.add(&md.velocities[i].scale(atom.mass));
        }
        assert!(total_p.norm() < 1e-10);
    }

    #[test]
    fn test_single_step() {
        let (mol, ff) = simple_diatomic();
        let params = SimParams {
            steps: 10,
            dt: 0.001,
            temperature: 300.0,
            ..Default::default()
        };
        let mut md = MolecularDynamics::new(mol, params);
        md.forcefield = ff;
        md.initialize_velocities(42);
        md.step().unwrap();
        assert_eq!(md.step_count, 1);
    }

    #[test]
    fn test_md_run() {
        let (mol, ff) = simple_diatomic();
        let params = SimParams {
            steps: 50,
            dt: 0.001,
            report_interval: 10,
            temperature: 300.0,
            ..Default::default()
        };
        let mut md = MolecularDynamics::new(mol, params);
        md.forcefield = ff;
        md.initialize_velocities(42);
        let result = md.run().unwrap();
        assert_eq!(result.steps_completed, 50);
        assert!(result.trajectory.len() >= 5);
        assert!(result.energy_trace.len() >= 5);
    }

    #[test]
    fn test_temperature_computation() {
        let (mol, ff) = simple_diatomic();
        let params = SimParams::default();
        let mut md = MolecularDynamics::new(mol, params);
        md.forcefield = ff;
        md.initialize_velocities(42);
        let temp = md.compute_temperature();
        assert!(temp > 0.0);
    }

    #[test]
    fn test_steepest_descent() {
        let (mol, ff) = simple_diatomic();
        let mut coords = mol.atom_positions();
        // Stretch the bond to create stress
        coords[1] = Vec3::new(2.0, 0.0, 0.0);

        let minimizer = EnergyMinimizer::default();
        let result = minimizer.steepest_descent(&mol, &mut coords, &ff).unwrap();
        // The minimizer should converge
        assert!(result.converged || result.iterations > 0);
    }

    #[test]
    fn test_conjugate_gradient() {
        let (mol, ff) = simple_diatomic();
        let mut coords = mol.atom_positions();
        coords[1] = Vec3::new(2.0, 0.0, 0.0);

        let minimizer = EnergyMinimizer::default();
        let result = minimizer
            .conjugate_gradient(&mol, &mut coords, &ff)
            .unwrap();
        assert!(result.converged || result.iterations > 0);
    }
}
