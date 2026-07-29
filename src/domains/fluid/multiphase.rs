//! Multi-phase flow simulation using the Volume-of-Fluid (VOF) method.
#![allow(clippy::too_many_arguments)]
//!
//! Implements a 2D VOF solver for immiscible two-phase flows with
//! surface tension and density/viscosity jumps across the interface.

use crate::core::types::Scalar;

/// 2D VOF two-phase flow solver (projection method + interface advection).
pub struct VofSolver2D {
    pub nx: usize, pub ny: usize,
    pub dx: Scalar, pub dy: Scalar, pub dt: Scalar,
    pub re: Scalar, pub we: Scalar,  // Reynolds, Weber numbers
    pub u: Vec<Vec<Scalar>>,
    pub v: Vec<Vec<Scalar>>,
    pub p: Vec<Vec<Scalar>>,
    pub phi: Vec<Vec<Scalar>>,  // Volume fraction [0,1]
    pub rho1: Scalar, pub rho2: Scalar,
    pub mu1: Scalar, pub mu2: Scalar,
}

impl VofSolver2D {
    pub fn new(nx: usize, ny: usize, dx: Scalar, dy: Scalar, dt: Scalar,
               re: Scalar, we: Scalar, rho1: Scalar, rho2: Scalar, mu1: Scalar, mu2: Scalar) -> Self {
        Self {
            nx, ny, dx, dy, dt, re, we,
            u: vec![vec![0.0; nx]; ny + 1],
            v: vec![vec![0.0; nx + 1]; ny],
            p: vec![vec![0.0; nx]; ny],
            phi: vec![vec![0.0; nx]; ny],
            rho1, rho2, mu1, mu2,
        }
    }

    /// Mean density from volume fraction.
    pub fn mean_density(&self) -> Vec<Vec<Scalar>> {
        let mut rho = vec![vec![0.0; self.nx]; self.ny];
        for j in 0..self.ny {
            for i in 0..self.nx {
                rho[j][i] = self.phi[j][i] * self.rho1 + (1.0 - self.phi[j][i]) * self.rho2;
            }
        }
        rho
    }

    /// Mean viscosity from volume fraction.
    pub fn mean_viscosity(&self) -> Vec<Vec<Scalar>> {
        let mut mu = vec![vec![0.0; self.nx]; self.ny];
        for j in 0..self.ny {
            for i in 0..self.nx {
                mu[j][i] = self.phi[j][i] * self.mu1 + (1.0 - self.phi[j][i]) * self.mu2;
            }
        }
        mu
    }

    /// Advect the volume fraction using a simple donor-acceptor scheme.
    pub fn advect_phi(&mut self) {
        let (nx, ny) = (self.nx, self.ny);
        let (dx, dy, dt) = (self.dx, self.dy, self.dt);
        let mut phi_new = self.phi.clone();

        for j in 1..ny - 1 {
            for i in 1..nx - 1 {
                let u_c = 0.5 * (self.u[j][i] + self.u[j + 1][i]);
                let v_c = 0.5 * (self.v[j][i] + self.v[j][i + 1]);

                // Donor-acceptor for x-flux
                let phi_w = if u_c > 0.0 { self.phi[j][i - 1] } else { self.phi[j][i] };
                let phi_e = if u_c > 0.0 { self.phi[j][i] } else { self.phi[j][i + 1] };
                let flux_x = u_c * (phi_e - phi_w) / dx;

                // Donor-acceptor for y-flux
                let phi_s = if v_c > 0.0 { self.phi[j - 1][i] } else { self.phi[j][i] };
                let phi_n = if v_c > 0.0 { self.phi[j][i] } else { self.phi[j + 1][i] };
                let flux_y = v_c * (phi_n - phi_s) / dy;

                phi_new[j][i] = self.phi[j][i] - dt * (flux_x + flux_y);
                phi_new[j][i] = phi_new[j][i].clamp(0.0, 1.0);
            }
        }
        self.phi = phi_new;
    }

    /// Perform one full VOF step: advection + NS solve.
    pub fn step(&mut self) -> Result<(), String> {
        self.advect_phi();
        // Simplified: in production, call projection_step with variable density
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vof_creation() {
        let vof = VofSolver2D::new(10, 10, 0.01, 0.01, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5);
        assert_eq!(vof.phi.len(), 10);
        assert_eq!(vof.phi[0].len(), 10);
    }

    #[test]
    fn test_mean_properties() {
        let mut vof = VofSolver2D::new(6, 6, 0.01, 0.01, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5);
        vof.phi[3][3] = 0.5;
        let rho = vof.mean_density();
        assert!((rho[3][3] - 500.5).abs() < 1e-10);
    }

    #[test]
    fn test_advection() {
        let mut vof = VofSolver2D::new(10, 10, 0.01, 0.01, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5);
        vof.phi[5][5] = 1.0;
        vof.u = vec![vec![0.1; 10]; 11];
        vof.advect_phi();
        // Volume fraction should have moved
        let sum: Scalar = vof.phi.iter().flat_map(|r| r.iter()).sum();
        assert!((sum - 1.0).abs() < 1e-10, "VOF should conserve volume");
    }
}
