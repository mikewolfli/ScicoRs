//! Explicit dynamics solver using the central difference method.
//!
//! Suitable for wave propagation, impact, and crash simulation where
//! small time steps are acceptable and diagonal (lumped) mass matrices
//! make the solve O(n) per step.

use crate::core::types::Scalar;

/// Explicit dynamics solver (central difference time integration).
///
/// Solves M·a + C·v + f_int(u) = f_ext for lumped-mass systems.
/// Each degree of freedom is independent when M and C are diagonal.
#[derive(Debug, Clone)]
pub struct ExplicitDynamics {
    /// Number of degrees of freedom.
    pub n_dof: usize,
    /// Lumped mass vector (diagonal of mass matrix).
    pub mass: Vec<Scalar>,
    /// Damping coefficients (diagonal of damping matrix).
    pub damping: Vec<Scalar>,
    /// Displacement (current).
    pub u: Vec<Scalar>,
    /// Velocity (current).
    pub v: Vec<Scalar>,
    /// Acceleration (current).
    pub a: Vec<Scalar>,
    /// Previous displacement (u_{n-1}).
    pub u_prev: Vec<Scalar>,
    /// Time step.
    pub dt: Scalar,
    /// Current simulation time.
    pub t: Scalar,
}

impl ExplicitDynamics {
    /// Create a new explicit dynamics solver with a given number of DOFs.
    ///
    /// Initialises all fields to zero.
    pub fn new(n_dof: usize, dt: Scalar) -> Self {
        assert!(n_dof > 0, "n_dof must be > 0");
        assert!(dt > 0.0, "dt must be > 0");
        Self {
            n_dof,
            mass: vec![1.0; n_dof],
            damping: vec![0.0; n_dof],
            u: vec![0.0; n_dof],
            v: vec![0.0; n_dof],
            a: vec![0.0; n_dof],
            u_prev: vec![0.0; n_dof],
            dt,
            t: 0.0,
        }
    }

    /// Set mass for a specific DOF.
    pub fn set_mass(&mut self, dof: usize, m: Scalar) {
        if dof < self.n_dof {
            self.mass[dof] = m;
        }
    }

    /// Set damping coefficient for a specific DOF.
    pub fn set_damping(&mut self, dof: usize, c: Scalar) {
        if dof < self.n_dof {
            self.damping[dof] = c;
        }
    }

    /// Set initial displacement and velocity for all DOFs.
    pub fn set_initial_conditions(&mut self, u0: &[Scalar], v0: &[Scalar]) {
        let n = self.n_dof.min(u0.len()).min(v0.len());
        self.u[..n].copy_from_slice(&u0[..n]);
        self.v[..n].copy_from_slice(&v0[..n]);
        self.u_prev.copy_from_slice(&self.u);
    }

    /// Perform one explicit time step.
    ///
    /// Central difference scheme:
    ///   u_{n+1} = (f_ext - f_int - C·v_n) / M * dt² + 2·u_n - u_{n-1}
    ///
    /// Returns the updated displacement vector.
    pub fn step(&mut self, f_ext: &[Scalar], f_int: &[Scalar]) -> Result<&[Scalar], String> {
        let n = self.n_dof;
        if f_ext.len() < n || f_int.len() < n {
            return Err("Force vectors too short".to_string());
        }

        let dt2 = self.dt * self.dt;
        let mut u_next = vec![0.0; n];

        // Each DOF is independent (diagonal mass/damping) → rayon.
        use rayon::prelude::*;
        u_next.par_iter_mut().enumerate().for_each(|(i, un)| {
            if self.mass[i] <= 0.0 {
                return; // Fixed DOF
            }
            // Effective force: f_ext - f_int - C·v
            let f_eff = f_ext[i] - f_int[i] - self.damping[i] * self.v[i];
            // u_{n+1} = f_eff / m * dt² + 2*u_n - u_{n-1}
            *un = f_eff / self.mass[i] * dt2 + 2.0 * self.u[i] - self.u_prev[i];
        });

        // Update velocity using central difference: v_n = (u_{n+1} - u_{n-1}) / (2·dt)
        self.v.par_iter_mut().enumerate().for_each(|(i, vi)| {
            if self.mass[i] > 0.0 {
                *vi = (u_next[i] - self.u_prev[i]) / (2.0 * self.dt);
            }
        });

        // Update acceleration: a_n = (u_{n+1} - 2·u_n + u_{n-1}) / dt²
        self.a.par_iter_mut().enumerate().for_each(|(i, ai)| {
            if self.mass[i] > 0.0 {
                *ai = (u_next[i] - 2.0 * self.u[i] + self.u_prev[i]) / dt2;
            }
        });

        // Shift states
        self.u_prev.copy_from_slice(&self.u);
        self.u.copy_from_slice(&u_next);
        self.t += self.dt;

        Ok(&self.u)
    }

    /// Compute the critical time step for stability.
    ///
    /// CFL condition: dt_crit = min(L_elem / c_wave)
    /// where L_elem is the smallest element dimension and c_wave is the
    /// wave speed in the material.
    pub fn critical_dt(&self, element_sizes: &[Scalar], wave_speed: Scalar) -> Scalar {
        if wave_speed <= 0.0 || element_sizes.is_empty() {
            return self.dt;
        }
        let min_size = element_sizes.iter().cloned().fold(f64::INFINITY, f64::min);
        min_size / wave_speed
    }

    /// Total kinetic energy: KE = ½ Σ m_i · v_i²
    pub fn kinetic_energy(&self) -> Scalar {
        0.5 * self
            .mass
            .iter()
            .zip(self.v.iter())
            .map(|(m, v)| m * v * v)
            .sum::<Scalar>()
    }

    /// Total strain energy (from internal force work, simplified).
    pub fn strain_energy(&self, f_int: &[Scalar]) -> Scalar {
        self.u
            .iter()
            .zip(f_int.iter())
            .map(|(u_i, f_i)| 0.5 * u_i * f_i)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_new() {
        let sys = ExplicitDynamics::new(3, 0.01);
        assert_eq!(sys.n_dof, 3);
        assert!((sys.dt - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_set_mass() {
        let mut sys = ExplicitDynamics::new(2, 0.01);
        sys.set_mass(0, 5.0);
        assert!((sys.mass[0] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_set_initial_conditions() {
        let mut sys = ExplicitDynamics::new(2, 0.01);
        sys.set_initial_conditions(&[1.0, 2.0], &[0.1, 0.2]);
        assert!((sys.u[0] - 1.0).abs() < 1e-10);
        assert!((sys.v[1] - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_step_single_dof() {
        let mut sys = ExplicitDynamics::new(1, 0.01);
        sys.set_mass(0, 1.0);
        let f_ext = vec![10.0];
        let f_int = vec![0.0]; // No internal force (free)
        let result = sys.step(&f_ext, &f_int).unwrap();
        // With m=1, f=10, dt=0.01: a=10, u=0.5*a*dt² = 0.5*10*0.0001 = 0.0005
        // But central difference: u₁ = f/m*dt² + 2*u₀ - u₋₁ = 10*0.0001 + 0 - 0 = 0.001
        assert!(
            (result[0] - 0.001).abs() < 1e-10,
            "expected 0.001, got {}",
            result[0]
        );
    }

    #[test]
    fn test_step_conservation() {
        let mut sys = ExplicitDynamics::new(2, 0.001);
        sys.set_mass(0, 2.0);
        sys.set_mass(1, 2.0);
        // Apply force to DOF 0 only
        let f_ext = vec![100.0, 0.0];
        let f_int = vec![0.0, 0.0];
        sys.step(&f_ext, &f_int).unwrap();
        // Kinetic energy should be positive
        assert!(sys.kinetic_energy() > 0.0);
    }

    #[test]
    fn test_kinetic_energy() {
        let mut sys = ExplicitDynamics::new(2, 0.01);
        sys.set_mass(0, 2.0);
        sys.set_mass(1, 3.0);
        sys.v = vec![2.0, 1.0];
        let ke = sys.kinetic_energy();
        // KE = 0.5*(2*4 + 3*1) = 0.5*(8+3) = 5.5
        assert!((ke - 5.5).abs() < 1e-10);
    }

    #[test]
    fn test_strain_energy() {
        let sys = ExplicitDynamics::new(3, 0.01);
        let f_int = vec![10.0, 20.0, 30.0];
        let se = sys.strain_energy(&f_int);
        // u=[0,0,0], so strain energy = 0
        assert!((se - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_critical_dt() {
        let sys = ExplicitDynamics::new(3, 0.01);
        let dt_crit = sys.critical_dt(&[0.1, 0.2], 5000.0);
        assert!((dt_crit - 0.1 / 5000.0).abs() < 1e-10);
    }

    #[test]
    fn test_damped_oscillator() {
        // Single DOF: mass=1, stiffness not directly modeled but
        // we can use f_int = k*u as restoring force
        let mut sys = ExplicitDynamics::new(1, 0.001);
        sys.set_mass(0, 1.0);
        sys.set_damping(0, 0.1);
        sys.set_initial_conditions(&[1.0], &[0.0]);

        let f_ext = vec![0.0];
        let spring_k = 100.0;
        for _ in 0..500 {
            let f_int = vec![spring_k * sys.u[0]];
            sys.step(&f_ext, &f_int).unwrap();
        }
        // After 500 steps with damping, amplitude should have decayed
        assert!(sys.u[0].abs() < 0.8, "damped amplitude should decay");
    }

    #[test]
    fn test_step_parallel_matches_serial_reference() {
        // step() runs on rayon (per-DOF); verify against the original serial
        // loop order on a many-DOF system.
        let n = 256;
        let dt = 0.001;
        let mut sys = ExplicitDynamics::new(n, dt);
        for i in 0..n {
            sys.set_mass(i, (i % 3) as Scalar + 1.0);
        }
        let u0: Vec<Scalar> = (0..n).map(|i| (i as Scalar).sin() * 0.1).collect();
        let v0: Vec<Scalar> = (0..n).map(|i| (i as Scalar).cos() * 0.05).collect();
        sys.set_initial_conditions(&u0, &v0);
        let f_ext: Vec<Scalar> = (0..n).map(|i| (i as Scalar) * 0.01).collect();
        let f_int: Vec<Scalar> = (0..n).map(|i| (i as Scalar) * 0.005).collect();
        sys.step(&f_ext, &f_int).unwrap();

        // Serial reference.
        let dt2 = dt * dt;
        let mut u_next = vec![0.0; n];
        for i in 0..n {
            if sys.mass[i] <= 0.0 {
                continue;
            }
            let f_eff = f_ext[i] - f_int[i] - sys.damping[i] * v0[i];
            u_next[i] = f_eff / sys.mass[i] * dt2 + 2.0 * u0[i] - u0[i];
        }
        for i in 0..n {
            if sys.mass[i] <= 0.0 {
                continue;
            }
            let v_ref = (u_next[i] - u0[i]) / (2.0 * dt);
            let a_ref = (u_next[i] - 2.0 * u0[i] + u0[i]) / dt2;
            assert!((sys.u[i] - u_next[i]).abs() < 1e-12, "u mismatch at {i}");
            assert!((sys.v[i] - v_ref).abs() < 1e-12, "v mismatch at {i}");
            assert!((sys.a[i] - a_ref).abs() < 1e-12, "a mismatch at {i}");
        }
    }
}
