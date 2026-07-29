//! Multibody system dynamics solver.
//!
//! Provides `MultibodySystem` integrating rigid bodies with constraints,
//! external forces, and gravity using semi‑implicit Euler integration.
//! Includes Lagrangian dynamics formulation, constraint force computation,
//! and total energy diagnostics.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use crate::domains::multibody::body::RigidBody;
use crate::domains::multibody::constraints::{Constraint, ConstraintSolver};
use crate::domains::multibody::physics::GRAVITY;

/// An external force acting on a rigid body.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalForce {
    /// ID of the body to which the force is applied.
    pub body_id: String,
    /// Force vector in world coordinates (N).
    pub force: [Scalar; 3],
    /// Application point in world coordinates (m).
    pub application_point: Coord3D,
}

/// A multibody dynamics system.
///
/// Manages a collection of rigid bodies, kinematic constraints, and external
/// forces, and provides time-integration and analysis methods.
#[derive(Debug, Clone)]
pub struct MultibodySystem {
    /// All rigid bodies in the system.
    pub bodies: Vec<RigidBody>,
    /// All kinematic constraints.
    pub constraints: Vec<Constraint>,
    /// External forces applied to bodies.
    pub forces: Vec<ExternalForce>,
    /// Gravitational acceleration vector (m/s²).
    pub gravity: [Scalar; 3],
}

impl MultibodySystem {
    /// Create a new empty multibody system with default gravity.
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            constraints: Vec::new(),
            forces: Vec::new(),
            gravity: GRAVITY,
        }
    }

    /// Create a multibody system with custom gravity.
    pub fn with_gravity(gravity: [Scalar; 3]) -> Self {
        Self {
            bodies: Vec::new(),
            constraints: Vec::new(),
            forces: Vec::new(),
            gravity,
        }
    }

    /// Add a rigid body to the system.
    pub fn add_body(&mut self, body: RigidBody) {
        self.bodies.push(body);
    }

    /// Add a constraint.
    pub fn add_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint);
    }

    /// Add an external force.
    pub fn add_force(&mut self, force: ExternalForce) {
        self.forces.push(force);
    }

    /// Assemble translational and rotational accelerations from all forces.
    ///
    /// Returns `(linear_accelerations, angular_accelerations)` where each
    /// is a `Vec<[Scalar; 3]>` indexed by body.
    ///
    /// Includes:
    /// - Gravity force: `m·g`
    /// - External forces: `F_ext`
    /// - Gyroscopic term (simplified): `-ω × (I·ω)`
    pub fn assemble_eom(&self) -> (Vec<[Scalar; 3]>, Vec<[Scalar; 3]>) {
        let n = self.bodies.len();
        let mut lin_acc = vec![[0.0; 3]; n];
        let mut ang_acc = vec![[0.0; 3]; n];

        // Build a map from body_id to index
        let _id_map: std::collections::HashMap<&str, usize> = self
            .bodies
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id.as_str(), i))
            .collect();

        for (i, body) in self.bodies.iter().enumerate() {
            let m = if body.mass > 0.0 {
                1.0 / body.mass
            } else {
                0.0
            };

            // Gravity
            lin_acc[i][0] += self.gravity[0];
            lin_acc[i][1] += self.gravity[1];
            lin_acc[i][2] += self.gravity[2];

            // External forces
            for f in &self.forces {
                if f.body_id == body.id {
                    lin_acc[i][0] += f.force[0] * m;
                    lin_acc[i][1] += f.force[1] * m;
                    lin_acc[i][2] += f.force[2] * m;

                    // Torque from off-COM force: τ = r × F
                    let r = [
                        f.application_point.x - body.position.x,
                        f.application_point.y - body.position.y,
                        f.application_point.z - body.position.z,
                    ];
                    let torque = [
                        r[1] * f.force[2] - r[2] * f.force[1],
                        r[2] * f.force[0] - r[0] * f.force[2],
                        r[0] * f.force[1] - r[1] * f.force[0],
                    ];
                    let inv_i = body.inverse_inertia();
                    ang_acc[i][0] +=
                        inv_i[0][0] * torque[0] + inv_i[0][1] * torque[1] + inv_i[0][2] * torque[2];
                    ang_acc[i][1] +=
                        inv_i[1][0] * torque[0] + inv_i[1][1] * torque[1] + inv_i[1][2] * torque[2];
                    ang_acc[i][2] +=
                        inv_i[2][0] * torque[0] + inv_i[2][1] * torque[1] + inv_i[2][2] * torque[2];
                }
            }

            // Gyroscopic term: -I⁻¹ · (ω × I·ω)
            let w = body.angular_velocity;
            let inertia = body.inertia;
            let iw = [
                inertia[0][0] * w[0] + inertia[0][1] * w[1] + inertia[0][2] * w[2],
                inertia[1][0] * w[0] + inertia[1][1] * w[1] + inertia[1][2] * w[2],
                inertia[2][0] * w[0] + inertia[2][1] * w[1] + inertia[2][2] * w[2],
            ];
            let gyro = [
                w[1] * iw[2] - w[2] * iw[1],
                w[2] * iw[0] - w[0] * iw[2],
                w[0] * iw[1] - w[1] * iw[0],
            ];
            let inv_i: [[Scalar; 3]; 3] = body.inverse_inertia();
            let inv_i00 = inv_i[0][0];
            let inv_i01 = inv_i[0][1];
            let inv_i02 = inv_i[0][2];
            let inv_i10 = inv_i[1][0];
            let inv_i11 = inv_i[1][1];
            let inv_i12 = inv_i[1][2];
            let inv_i20 = inv_i[2][0];
            let inv_i21 = inv_i[2][1];
            let inv_i22 = inv_i[2][2];
            let idx = i;
            ang_acc[idx][0] -= inv_i00 * gyro[0] + inv_i01 * gyro[1] + inv_i02 * gyro[2];
            ang_acc[idx][1] -= inv_i10 * gyro[0] + inv_i11 * gyro[1] + inv_i12 * gyro[2];
            ang_acc[idx][2] -= inv_i20 * gyro[0] + inv_i21 * gyro[1] + inv_i22 * gyro[2];
        }

        (lin_acc, ang_acc)
    }

    /// Compute the Lagrangian dynamics equations.
    ///
    /// Returns the generalized accelerations as a flat vector
    /// `[ẍ₁, ÿ₁, z̈₁, ω̇₁_x, ω̇₁_y, ω̇₁_z, ẍ₂, ...]`.
    pub fn lagrangian_dynamics(&self) -> Vec<Scalar> {
        let (lin_acc, ang_acc) = self.assemble_eom();
        let n = self.bodies.len();
        let mut q_ddot = Vec::with_capacity(n * 6);
        for i in 0..n {
            q_ddot.push(lin_acc[i][0]);
            q_ddot.push(lin_acc[i][1]);
            q_ddot.push(lin_acc[i][2]);
            q_ddot.push(ang_acc[i][0]);
            q_ddot.push(ang_acc[i][1]);
            q_ddot.push(ang_acc[i][2]);
        }
        q_ddot
    }

    /// Perform one semi-implicit Euler integration step.
    ///
    /// Updates velocities first (from forces), then positions, then
    /// applies constraint stabilization.
    ///
    /// # Algorithm
    /// 1. Compute accelerations from gravity + external forces + gyroscopics
    /// 2. Update linear and angular velocities: `v += a·dt`, `ω += α·dt`
    /// 3. Update positions: `x += v·dt`
    /// 4. Update orientation: `q += ½·ω·q·dt` (normalized)
    /// 5. Solve constraints (Lagrange multiplier) to enforce constraints
    pub fn semi_implicit_euler_step(&mut self, dt: Scalar) -> Result<(), String> {
        if dt <= 0.0 {
            return Err("Time step must be positive".to_string());
        }

        let (lin_acc, ang_acc) = self.assemble_eom();

        // Update velocities
        for (i, body) in self.bodies.iter_mut().enumerate() {
            body.linear_velocity[0] += lin_acc[i][0] * dt;
            body.linear_velocity[1] += lin_acc[i][1] * dt;
            body.linear_velocity[2] += lin_acc[i][2] * dt;
            body.angular_velocity[0] += ang_acc[i][0] * dt;
            body.angular_velocity[1] += ang_acc[i][1] * dt;
            body.angular_velocity[2] += ang_acc[i][2] * dt;
        }

        // Update positions
        for body in self.bodies.iter_mut() {
            body.position = Coord3D::new(
                body.position.x + body.linear_velocity[0] * dt,
                body.position.y + body.linear_velocity[1] * dt,
                body.position.z + body.linear_velocity[2] * dt,
            );

            // Update orientation: q += 0.5 * ω * q * dt
            let w = body.angular_velocity;
            let q = body.orientation;
            let omega_q = crate::domains::multibody::body::Quaternion {
                w: 0.0,
                x: w[0],
                y: w[1],
                z: w[2],
            };
            let dq = omega_q.multiply(&q);
            body.orientation = crate::domains::multibody::body::Quaternion {
                w: q.w + 0.5 * dq.w * dt,
                x: q.x + 0.5 * dq.x * dt,
                y: q.y + 0.5 * dq.y * dt,
                z: q.z + 0.5 * dq.z * dt,
            }
            .normalize();
        }

        // Apply constraint stabilization
        if !self.constraints.is_empty() {
            let solver = ConstraintSolver::new(self.constraints.clone());
            solver.solve_lagrange_multipliers(&mut self.bodies, dt)?;
        }

        Ok(())
    }

    /// Compute the constraint forces acting on each body.
    ///
    /// Returns a vector of `[force_x, force_y, force_z]` for each body
    /// representing the net constraint force in world coordinates.
    pub fn constraint_forces(&self) -> Vec<[Scalar; 3]> {
        let n = self.bodies.len();
        let mut forces = vec![[0.0; 3]; n];

        let id_map: std::collections::HashMap<&str, usize> = self
            .bodies
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id.as_str(), i))
            .collect();

        for c in &self.constraints {
            let ba_opt = self.bodies.iter().find(|b| b.id == c.body_a);
            let bb_opt = self.bodies.iter().find(|b| b.id == c.body_b);
            if let (Some(ba), Some(bb)) = (ba_opt, bb_opt) {
                let jac = c.jacobian(ba, bb);
                // Use the position error scaled by a large stiffness as
                // an approximation of constraint force magnitude
                let pos_err = c.position_error(ba, bb);
                let stiffness = 1e6;
                for (i, row) in jac.j_rows.iter().enumerate() {
                    let lambda = if i < pos_err.len() {
                        stiffness * pos_err[i]
                    } else {
                        0.0
                    };
                    if let Some(&idx) = id_map.get(c.body_a.as_str()) {
                        forces[idx][0] += row[0] * lambda;
                        forces[idx][1] += row[1] * lambda;
                        forces[idx][2] += row[2] * lambda;
                    }
                    if let Some(&idx) = id_map.get(c.body_b.as_str()) {
                        forces[idx][0] -= row[0] * lambda;
                        forces[idx][1] -= row[1] * lambda;
                        forces[idx][2] -= row[2] * lambda;
                    }
                }
            }
        }
        forces
    }

    /// Compute the total mechanical energy of the system.
    ///
    /// `E_total = Σ KE_i + Σ PE_grav_i`
    pub fn total_energy(&self) -> Scalar {
        let mut energy = 0.0;
        for body in &self.bodies {
            // Kinetic energy
            energy += body.kinetic_energy();
            // Gravitational potential energy: m * g · r
            energy += body.mass
                * (self.gravity[0] * body.position.x
                    + self.gravity[1] * body.position.y
                    + self.gravity[2] * body.position.z);
        }
        energy
    }

    /// Find a body by its ID.
    pub fn get_body(&self, id: &str) -> Option<&RigidBody> {
        self.bodies.iter().find(|b| b.id == id)
    }

    /// Get a mutable reference to a body by ID.
    pub fn get_body_mut(&mut self, id: &str) -> Option<&mut RigidBody> {
        self.bodies.iter_mut().find(|b| b.id == id)
    }

    /// Remove all bodies, constraints, and forces.
    pub fn clear(&mut self) {
        self.bodies.clear();
        self.constraints.clear();
        self.forces.clear();
    }
}

impl Default for MultibodySystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::multibody::body::RigidBody;

    fn simple_body(id: &str, pos: Coord3D) -> RigidBody {
        RigidBody::new(
            id,
            1.0,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            pos,
        )
    }

    #[test]
    fn test_system_new() {
        let sys = MultibodySystem::new();
        assert!(sys.bodies.is_empty());
        assert!(sys.constraints.is_empty());
        assert!(sys.forces.is_empty());
        assert_eq!(sys.gravity, GRAVITY);
    }

    #[test]
    fn test_add_body_and_force() {
        let mut sys = MultibodySystem::new();
        sys.add_body(simple_body("b1", Coord3D::new(0.0, 0.0, 0.0)));
        sys.add_force(ExternalForce {
            body_id: "b1".to_string(),
            force: [10.0, 0.0, 0.0],
            application_point: Coord3D::new(0.0, 0.0, 0.0),
        });
        assert_eq!(sys.bodies.len(), 1);
        assert_eq!(sys.forces.len(), 1);
    }

    #[test]
    fn test_assemble_eom_gravity_only() {
        let mut sys = MultibodySystem::new();
        sys.add_body(simple_body("b1", Coord3D::new(0.0, 0.0, 0.0)));
        let (lin_acc, ang_acc) = sys.assemble_eom();
        assert!((lin_acc[0][0]).abs() < 1e-12);
        assert!((lin_acc[0][1]).abs() < 1e-12);
        assert!((lin_acc[0][2] - (-9.80665)).abs() < 1e-10);
        assert!((ang_acc[0][0]).abs() < 1e-12);
    }

    #[test]
    fn test_semi_implicit_euler_step() {
        let mut sys = MultibodySystem::new();
        sys.add_body(simple_body("b1", Coord3D::new(0.0, 0.0, 0.0)));
        let result = sys.semi_implicit_euler_step(0.01);
        assert!(result.is_ok());
        // After one step with gravity, vy should be -9.80665*0.01 ≈ -0.098
        assert!((sys.bodies[0].linear_velocity[2] + 0.0980665).abs() < 1e-6);
        assert!((sys.bodies[0].position.z + 0.000980665).abs() < 1e-6);
    }

    #[test]
    fn test_semi_implicit_euler_step_zero_dt() {
        let mut sys = MultibodySystem::new();
        let result = sys.semi_implicit_euler_step(0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_total_energy() {
        let mut sys = MultibodySystem::new();
        sys.add_body(simple_body("b1", Coord3D::new(0.0, 0.0, 10.0)));
        // Gravitational PE = m * g * z = 1 * (-9.80665) * 10 = -98.0665
        // KE = 0
        let e = sys.total_energy();
        assert!((e + 98.0665).abs() < 1e-10);
    }

    #[test]
    fn test_constraint_forces() {
        let mut sys = MultibodySystem::new();
        sys.add_body(simple_body("a", Coord3D::new(0.0, 0.0, 0.0)));
        sys.add_body(simple_body("b", Coord3D::new(0.1, 0.0, 0.0)));
        let c = Constraint::new(
            "c1",
            crate::domains::multibody::constraints::ConstraintType::Spherical,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        sys.add_constraint(c);
        let cf = sys.constraint_forces();
        assert_eq!(cf.len(), 2);
    }

    #[test]
    fn test_get_body() {
        let mut sys = MultibodySystem::new();
        sys.add_body(simple_body("b1", Coord3D::new(1.0, 2.0, 3.0)));
        let b = sys.get_body("b1").unwrap();
        assert!((b.position.x - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_get_body_mut() {
        let mut sys = MultibodySystem::new();
        sys.add_body(simple_body("b1", Coord3D::new(0.0, 0.0, 0.0)));
        let b = sys.get_body_mut("b1").unwrap();
        b.linear_velocity = [1.0, 0.0, 0.0];
        assert!((b.linear_velocity[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_lagrangian_dynamics() {
        let mut sys = MultibodySystem::new();
        sys.add_body(simple_body("b1", Coord3D::new(0.0, 0.0, 0.0)));
        sys.add_body(simple_body("b2", Coord3D::new(1.0, 0.0, 0.0)));
        let qdd = sys.lagrangian_dynamics();
        assert_eq!(qdd.len(), 12); // 2 bodies × 6 DOF
    }

    #[test]
    fn test_clear() {
        let mut sys = MultibodySystem::new();
        sys.add_body(simple_body("b1", Coord3D::new(0.0, 0.0, 0.0)));
        sys.add_constraint(Constraint::new(
            "c1",
            crate::domains::multibody::constraints::ConstraintType::Fixed,
            "b1",
            "b1",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [1.0, 0.0, 0.0],
        ));
        sys.clear();
        assert!(sys.bodies.is_empty());
        assert!(sys.constraints.is_empty());
    }

    #[test]
    fn test_semi_implicit_euler_with_initial_velocity() {
        let mut sys = MultibodySystem::new();
        let mut b = simple_body("b1", Coord3D::new(0.0, 0.0, 0.0));
        b.linear_velocity = [5.0, 0.0, 0.0];
        sys.add_body(b);
        sys.semi_implicit_euler_step(0.1).unwrap();
        // Position should advance by v*dt = 5*0.1 = 0.5
        assert!((sys.bodies[0].position.x - 0.5).abs() < 1e-10);
    }
}
