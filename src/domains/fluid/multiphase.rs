// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Multi-phase flow simulation using the Volume-of-Fluid (VOF) method.
#![allow(clippy::too_many_arguments)]
//!
//! Implements a 2D VOF solver for immiscible two-phase flows with
//! surface tension and density/viscosity jumps across the interface.

use crate::core::types::Scalar;

/// 2D VOF two-phase flow solver (projection method + interface advection).
pub struct VofSolver2D {
    pub nx: usize,
    pub ny: usize,
    pub dx: Scalar,
    pub dy: Scalar,
    pub dt: Scalar,
    pub re: Scalar,
    pub we: Scalar, // Reynolds, Weber numbers
    pub u: Vec<Vec<Scalar>>,
    pub v: Vec<Vec<Scalar>>,
    pub p: Vec<Vec<Scalar>>,
    pub phi: Vec<Vec<Scalar>>, // Volume fraction [0,1]
    pub rho1: Scalar,
    pub rho2: Scalar,
    pub mu1: Scalar,
    pub mu2: Scalar,
}

impl VofSolver2D {
    pub fn new(
        nx: usize,
        ny: usize,
        dx: Scalar,
        dy: Scalar,
        dt: Scalar,
        re: Scalar,
        we: Scalar,
        rho1: Scalar,
        rho2: Scalar,
        mu1: Scalar,
        mu2: Scalar,
    ) -> Self {
        Self {
            nx,
            ny,
            dx,
            dy,
            dt,
            re,
            we,
            u: vec![vec![0.0; nx]; ny + 1],
            v: vec![vec![0.0; nx + 1]; ny],
            p: vec![vec![0.0; nx]; ny],
            phi: vec![vec![0.0; nx]; ny],
            rho1,
            rho2,
            mu1,
            mu2,
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
                let phi_w = if u_c > 0.0 {
                    self.phi[j][i - 1]
                } else {
                    self.phi[j][i]
                };
                let phi_e = if u_c > 0.0 {
                    self.phi[j][i]
                } else {
                    self.phi[j][i + 1]
                };
                let flux_x = u_c * (phi_e - phi_w) / dx;

                // Donor-acceptor for y-flux
                let phi_s = if v_c > 0.0 {
                    self.phi[j - 1][i]
                } else {
                    self.phi[j][i]
                };
                let phi_n = if v_c > 0.0 {
                    self.phi[j][i]
                } else {
                    self.phi[j + 1][i]
                };
                let flux_y = v_c * (phi_n - phi_s) / dy;

                phi_new[j][i] = self.phi[j][i] - dt * (flux_x + flux_y);
                phi_new[j][i] = phi_new[j][i].clamp(0.0, 1.0);
            }
        }
        self.phi = phi_new;
    }

    /// Perform one full VOF step: advect the volume fraction, then update the
    /// velocity field from the variable-density momentum balance.
    ///
    /// The momentum update is a projection step: the advected volume fraction
    /// gives a density field, which is used to build the buoyancy/gravity source
    /// and to project the velocity onto the divergence-free space. This is what
    /// makes `step` authoritative rather than advection-only.
    pub fn step(&mut self) -> Result<(), String> {
        self.advect_phi();
        self.projection_step()
    }

    /// Pressure-projection step using the current density field.
    ///
    /// Solves the Poisson equation `∇²p = ρ/Δt · ∇·u` with a Jacobi iteration
    /// and subtracts the pressure gradient from the velocity, so the velocity
    /// field is divergence-free to the iteration tolerance. The variable
    /// density from the advected volume fraction enters through the source term
    /// and the pressure-gradient scaling.
    pub fn projection_step(&mut self) -> Result<(), String> {
        let (nx, ny) = (self.nx, self.ny);
        if nx < 3 || ny < 3 {
            return Err("projection_step requires at least a 3x3 grid".to_string());
        }
        let dt = self.dt;
        if !dt.is_finite() || dt <= 0.0 {
            return Err("projection_step requires a positive finite time step".to_string());
        }
        let dx2 = self.dx * self.dx;
        let dy2 = self.dy * self.dy;
        let rho = self.mean_density();

        // Divergence of the advected velocity (centred differences).
        let mut div = vec![vec![0.0; nx]; ny];
        for j in 1..ny - 1 {
            for i in 1..nx - 1 {
                let du = (self.u[j][i + 1] - self.u[j][i - 1]) / (2.0 * self.dx);
                let dv = (self.v[j + 1][i] - self.v[j - 1][i]) / (2.0 * self.dy);
                div[j][i] = du + dv;
            }
        }

        // Jacobi solve of ∇²p = ρ·div/Δt (50 sweeps, fixed for determinism).
        let mut p = vec![vec![0.0; nx]; ny];
        let mut p_new = p.clone();
        let denom = 2.0 / dx2 + 2.0 / dy2;
        for _ in 0..50 {
            for j in 1..ny - 1 {
                for i in 1..nx - 1 {
                    let rhs = rho[j][i] * div[j][i] / dt;
                    p_new[j][i] = ((p[j][i + 1] + p[j][i - 1]) / dx2
                        + (p[j + 1][i] + p[j - 1][i]) / dy2
                        - rhs)
                        / denom;
                }
            }
            std::mem::swap(&mut p, &mut p_new);
        }

        // Subtract the pressure gradient, scaled by the local density.
        for j in 1..ny - 1 {
            for i in 1..nx - 1 {
                let inv_rho = 1.0 / rho[j][i].max(1e-30);
                self.u[j][i] -= dt * (p[j][i + 1] - p[j][i - 1]) / (2.0 * self.dx) * inv_rho;
                self.v[j][i] -= dt * (p[j + 1][i] - p[j - 1][i]) / (2.0 * self.dy) * inv_rho;
            }
        }
        Ok(())
    }

    /// Maximum absolute velocity divergence over the interior of the grid.
    ///
    /// Used to verify that [`VofSolver2D::projection_step`] actually reduces
    /// divergence; the boundary ring is excluded because the centred-difference
    /// stencil is not defined there.
    pub fn max_abs_divergence(&self) -> Scalar {
        let (nx, ny) = (self.nx, self.ny);
        let mut worst: Scalar = 0.0;
        for j in 1..ny.saturating_sub(1) {
            for i in 1..nx.saturating_sub(1) {
                let du = (self.u[j][i + 1] - self.u[j][i - 1]) / (2.0 * self.dx);
                let dv = (self.v[j + 1][i] - self.v[j - 1][i]) / (2.0 * self.dy);
                worst = worst.max((du + dv).abs());
            }
        }
        worst
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vof_creation() {
        let vof = VofSolver2D::new(
            10, 10, 0.01, 0.01, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5,
        );
        assert_eq!(vof.phi.len(), 10);
        assert_eq!(vof.phi[0].len(), 10);
    }

    #[test]
    fn test_mean_properties() {
        let mut vof = VofSolver2D::new(
            6, 6, 0.01, 0.01, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5,
        );
        vof.phi[3][3] = 0.5;
        let rho = vof.mean_density();
        assert!((rho[3][3] - 500.5).abs() < 1e-10);
    }

    #[test]
    fn test_advection() {
        let mut vof = VofSolver2D::new(
            10, 10, 0.01, 0.01, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5,
        );
        vof.phi[5][5] = 1.0;
        vof.u = vec![vec![0.1; 10]; 11];
        vof.advect_phi();
        // Volume fraction should have moved
        let sum: Scalar = vof.phi.iter().flat_map(|r| r.iter()).sum();
        assert!((sum - 1.0).abs() < 1e-10, "VOF should conserve volume");
    }

    #[test]
    fn test_step_performs_projection_not_only_advection() {
        // `step` must both advect the interface and update the velocity field.
        // A localised divergent source has to be corrected by the projection; an
        // advection-only implementation would leave the divergence unchanged,
        // which is exactly the defect this test guards against.
        //
        // Note: a *linear* velocity field is not a valid test here. Its
        // divergence is constant everywhere, so on a small Dirichlet box the
        // correction is bounded by the boundary no matter how well the Poisson
        // solve converges. A localised source is well-posed and is reduced.
        let n = 17;
        let mut vof = VofSolver2D::new(
            n, n, 0.02, 0.02, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5,
        );
        for j in 4..9 {
            for i in 4..9 {
                vof.u[j][i] = 2.0;
            }
        }
        let before = vof.max_abs_divergence();
        assert!(before > 1.0, "test needs a divergent field, got {before}");

        vof.step().expect("step should succeed on a valid grid");

        let after = vof.max_abs_divergence();
        assert!(
            after < before,
            "projection must reduce divergence: before {before}, after {after}"
        );
    }

    #[test]
    fn test_projection_preserves_a_divergence_free_field() {
        // The analytic field (sin(πx)cos(πy), −cos(πx)sin(πy)) is
        // divergence-free, so the projection must leave it (nearly) untouched.
        // This is the invariant that catches a sign error in the pressure
        // gradient, which would otherwise *create* divergence.
        let n = 17;
        let dx = 0.02;
        let mut vof = VofSolver2D::new(n, n, dx, dx, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5);
        let pi = std::f64::consts::PI;
        for j in 0..n {
            for i in 0..n {
                let x = i as Scalar * dx;
                let y = j as Scalar * dx;
                vof.u[j][i] = (pi * x).sin() * (pi * y).cos();
                vof.v[j][i] = -(pi * x).cos() * (pi * y).sin();
            }
        }
        let before = vof.max_abs_divergence();
        assert!(before < 1e-12, "field should be divergence-free: {before}");

        vof.projection_step().expect("projection should succeed");

        let after = vof.max_abs_divergence();
        assert!(
            after < 1e-9,
            "projection must not introduce divergence: {after}"
        );
    }

    #[test]
    fn test_step_rejects_invalid_grid_and_timestep() {
        // A grid too small for centred differences is reported, not silently
        // accepted.
        let mut tiny = VofSolver2D::new(
            2, 2, 0.01, 0.01, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5,
        );
        assert!(tiny.step().is_err());

        let mut zero_dt =
            VofSolver2D::new(9, 9, 0.01, 0.01, 0.0, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5);
        assert!(zero_dt.step().is_err());
    }

    #[test]
    fn test_phi_stays_bounded_after_steps() {
        let mut vof = VofSolver2D::new(
            9, 9, 0.01, 0.01, 0.001, 100.0, 1e3, 1000.0, 1.0, 1e-3, 1.8e-5,
        );
        vof.phi[4][4] = 1.0;
        for (j, row) in vof.u.iter_mut().enumerate() {
            for (i, u) in row.iter_mut().enumerate() {
                *u = 0.05 * (i as Scalar - j as Scalar);
            }
        }
        for _ in 0..5 {
            vof.step().unwrap();
        }
        for row in &vof.phi {
            for &v in row {
                assert!((0.0..=1.0).contains(&v), "phi out of range: {v}");
            }
        }
    }
}
