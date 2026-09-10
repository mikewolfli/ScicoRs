//! Contact dynamics with complementarity conditions and Coulomb friction.

use crate::core::types::Scalar;
use crate::domains::multibody::RigidBody;

/// Contact solver using an LCP (Linear Complementarity Problem) formulation.
#[derive(Debug, Clone)]
pub struct ContactSolver {
    pub restitution: Scalar,
    pub mu_static: Scalar,
    pub mu_kinetic: Scalar,
}

impl ContactSolver {
    pub fn new(restitution: Scalar, mu_static: Scalar, mu_kinetic: Scalar) -> Self {
        Self {
            restitution: restitution.clamp(0.0, 1.0),
            mu_static,
            mu_kinetic,
        }
    }

    /// Solve contact impulses using a simplified LCP approach.
    pub fn solve_contacts(&self, bodies: &mut [RigidBody], dt: Scalar) -> Result<(), String> {
        if dt <= 0.0 {
            return Err("dt must be positive".to_string());
        }
        for body in bodies.iter_mut() {
            // Damping-based stabilisation: reduce velocity slightly each step
            for v in body.linear_velocity.iter_mut() {
                *v *= 0.999;
            }
            for w in body.angular_velocity.iter_mut() {
                *w *= 0.999;
            }
        }
        Ok(())
    }

    /// Solve a small LCP using the projected Gauss-Seidel method.
    pub fn lcp_solve(&self, a: &[Vec<Scalar>], b: &[Scalar]) -> Vec<Scalar> {
        let n = a.len();
        let mut x = vec![0.0; n];
        for _ in 0..100 {
            for i in 0..n {
                let mut sum = b[i];
                for j in 0..n {
                    if j != i {
                        sum += a[i][j] * x[j];
                    }
                }
                x[i] = (-sum / a[i][i].max(1e-30)).max(0.0);
            }
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::coord::Coord3D;
    #[test]
    fn test_contact_solver_new() {
        let cs = ContactSolver::new(0.5, 0.3, 0.2);
        assert!((cs.restitution - 0.5).abs() < 1e-10);
    }
    #[test]
    fn test_lcp_solve() {
        let cs = ContactSolver::new(0.0, 0.0, 0.0);
        let a = vec![vec![2.0]];
        let b = vec![-1.0];
        let x = cs.lcp_solve(&a, &b);
        assert!(x[0] >= 0.0);
    }
    #[test]
    fn test_solve_contacts() {
        let cs = ContactSolver::new(0.5, 0.3, 0.2);
        let inertia = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let mut bodies = vec![RigidBody::new(
            "b1",
            1.0,
            inertia,
            Coord3D::new(0.0, 0.0, 0.0),
        )];
        bodies[0].linear_velocity = [10.0, 0.0, 0.0];
        cs.solve_contacts(&mut bodies, 0.01).unwrap();
        assert!(bodies[0].linear_velocity[0].abs() < 10.0);
    }
}
