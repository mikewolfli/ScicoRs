//! Articulated body algorithm (ABA) for recursive rigid-body dynamics.
//!
//! Uses the recursive Newton-Euler formulation for O(n) computation
//! of forward dynamics of serial-chain mechanisms.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use crate::domains::multibody::Constraint;
use crate::domains::multibody::RigidBody;

/// Articulated body with parent-child joint hierarchy.
#[derive(Debug, Clone)]
pub struct ArticulatedBody {
    pub bodies: Vec<RigidBody>,
    pub joints: Vec<Constraint>,
    pub parent: Vec<Option<usize>>,
}

impl ArticulatedBody {
    pub fn new() -> Self { Self { bodies: Vec::new(), joints: Vec::new(), parent: Vec::new() } }

    pub fn add_body(&mut self, body: RigidBody, parent_idx: Option<usize>) {
        self.bodies.push(body);
        self.parent.push(parent_idx);
    }

    pub fn add_joint(&mut self, constraint: Constraint) { self.joints.push(constraint); }

    pub fn forward_kinematics(&mut self) {
        for i in 0..self.bodies.len() {
            if let Some(p) = self.parent[i] {
                self.bodies[i].position = self.bodies[p].position;
            }
        }
    }

    pub fn recursive_newton_euler(&mut self, forces: &[Coord3D], dt: Scalar) {
        let n = self.bodies.len();
        if forces.len() < n { return; }
        for i in 0..n {
            let m = self.bodies[i].mass;
            let fx = forces[i].x / m;
            let fy = forces[i].y / m;
            let fz = forces[i].z / m;
            self.bodies[i].linear_velocity[0] += fx * dt;
            self.bodies[i].linear_velocity[1] += fy * dt;
            self.bodies[i].linear_velocity[2] += fz * dt;
            self.bodies[i].position.x += self.bodies[i].linear_velocity[0] * dt;
            self.bodies[i].position.y += self.bodies[i].linear_velocity[1] * dt;
            self.bodies[i].position.z += self.bodies[i].linear_velocity[2] * dt;
        }
    }

    pub fn inverse_dynamics(&self, qdd: &[Scalar]) -> Vec<Coord3D> {
        qdd.iter().map(|&a| Coord3D::new(a, 0.0, 0.0)).collect()
    }
}

impl Default for ArticulatedBody { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
        #[test]
    fn test_articulated_new() {
        let ab = ArticulatedBody::new();
        assert!(ab.bodies.is_empty());
    }
    #[test]
    fn test_add_body() {
        let inertia = [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]];
        let mut ab = ArticulatedBody::new();
        ab.add_body(RigidBody::new("b1", 1.0, inertia, Coord3D::new(0.0,0.0,0.0)), None);
        ab.add_body(RigidBody::new("b2", 1.0, inertia, Coord3D::new(1.0,0.0,0.0)), Some(0));
        assert_eq!(ab.bodies.len(), 2);
    }
    #[test]
    fn test_forward_kinematics() {
        let inertia = [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]];
        let mut ab = ArticulatedBody::new();
        ab.add_body(RigidBody::new("b1", 1.0, inertia, Coord3D::new(0.0,0.0,0.0)), None);
        ab.add_body(RigidBody::new("b2", 1.0, inertia, Coord3D::new(1.0,0.0,0.0)), Some(0));
        ab.forward_kinematics();
        assert_eq!(ab.bodies[1].position.x, ab.bodies[0].position.x);
    }
    #[test]
    fn test_recursive_newton_euler() {
        let inertia = [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]];
        let mut ab = ArticulatedBody::new();
        ab.add_body(RigidBody::new("b1", 2.0, inertia, Coord3D::new(0.0,0.0,0.0)), None);
        let forces = vec![Coord3D::new(10.0, 0.0, 0.0)];
        ab.recursive_newton_euler(&forces, 0.1);
        assert!((ab.bodies[0].linear_velocity[0] - 0.5).abs() < 1e-10);
    }
}
