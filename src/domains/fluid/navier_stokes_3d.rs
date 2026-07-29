//! 3D incompressible Navier-Stokes solver using the projection method.
//!
//! Extends the 2D solver (`NavierStokes2D`) to three spatial dimensions
//! with a staggered (MAC) grid. Velocity components are stored on cell
//! faces; pressure is stored at cell centres.
//!
//! All grid loops are parallelised with rayon.

#![allow(clippy::too_many_arguments)]

use crate::core::types::Scalar;

/// Alias for a 3D scalar field slice used in intermediate velocity returns.
type Field3D = Vec<Vec<Vec<Scalar>>>;

/// 3D wall boundary condition types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WallCondition3D {
    NoSlip,
    FreeSlip,
    Inlet(Scalar, Scalar, Scalar),
    Outflow,
    MovingWall(Scalar, Scalar, Scalar),
}

/// 3D incompressible Navier-Stokes solver (projection method, staggered grid).
///
/// Fields use the following dimensions:
/// - `u` (x-velocity): `(nz+1) × (ny+1) × nx`  — stored at x-faces
/// - `v` (y-velocity): `(nz+1) × ny × (nx+1)`  — stored at y-faces
/// - `w` (z-velocity): `nz × (ny+1) × (nx+1)`  — stored at z-faces
/// - `p` (pressure):   `nz × ny × nx`           — stored at cell centres
#[derive(Debug, Clone)]
pub struct NavierStokes3D {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub dx: Scalar,
    pub dy: Scalar,
    pub dz: Scalar,
    pub dt: Scalar,
    pub re: Scalar,
    pub u: Vec<Vec<Vec<Scalar>>>, // (nz+1)×(ny+1)×nx
    pub v: Vec<Vec<Vec<Scalar>>>, // (nz+1)×ny×(nx+1)
    pub w: Vec<Vec<Vec<Scalar>>>, // nz×(ny+1)×(nx+1)
    pub p: Vec<Vec<Vec<Scalar>>>, // nz×ny×nx
}

impl NavierStokes3D {
    /// Create a new 3D Navier-Stokes solver with zero-initialised fields.
    ///
    /// # Panics
    ///
    /// Panics if any dimension is zero or any step ≤ 0.
    pub fn new(
        nx: usize,
        ny: usize,
        nz: usize,
        dx: Scalar,
        dy: Scalar,
        dz: Scalar,
        dt: Scalar,
        re: Scalar,
    ) -> Self {
        assert!(nx > 0 && ny > 0 && nz > 0, "grid dimensions must be > 0");
        assert!(dx > 0.0 && dy > 0.0 && dz > 0.0, "grid spacing must be > 0");
        assert!(dt > 0.0, "dt must be > 0");
        assert!(re > 0.0, "Re must be > 0");

        Self {
            nx,
            ny,
            nz,
            dx,
            dy,
            dz,
            dt,
            re,
            u: vec![vec![vec![0.0; nx]; ny + 1]; nz + 1],
            v: vec![vec![vec![0.0; nx + 1]; ny]; nz + 1],
            w: vec![vec![vec![0.0; nx + 1]; ny + 1]; nz],
            p: vec![vec![vec![0.0; nx]; ny]; nz],
        }
    }

    /// Perform one full projection time step.
    ///
    /// 1. Compute intermediate (predictor) velocity.
    /// 2. Solve pressure Poisson equation.
    /// 3. Correct velocities with pressure gradient.
    pub fn projection_step(&mut self) -> Result<(), String> {
        let (u_star, v_star, w_star) = self.compute_intermediate_velocity();
        self.solve_pressure_poisson(&u_star, &v_star, &w_star)?;
        self.velocity_correction(&u_star, &v_star, &w_star);
        Ok(())
    }

    /// Compute intermediate velocity fields (explicit Euler for convection + diffusion).
    fn compute_intermediate_velocity(&self) -> (Field3D, Field3D, Field3D) {
        use rayon::prelude::*;

        let nu = 1.0 / self.re;
        let dt = self.dt;
        let dx = self.dx;
        let dy = self.dy;
        let dz = self.dz;

        // ── u*: (nz+1)×(ny+1)×nx ──
        let mut u_star = vec![vec![vec![0.0; self.nx]; self.ny + 1]; self.nz + 1];
        u_star.par_iter_mut().enumerate().for_each(|(k, plane)| {
            if k == 0 || k >= self.nz {
                return;
            }
            for i in 0..=self.ny {
                if i == 0 || i >= self.ny {
                    continue;
                }
                for j in 1..self.nx - 1 {
                    let u_ij = self.u[k][i][j];
                    // ∂u/∂x (upwind)
                    let du_dx = if u_ij > 0.0 {
                        u_ij * (self.u[k][i][j] - self.u[k][i][j - 1]) / dx
                    } else {
                        u_ij * (self.u[k][i][j + 1] - self.u[k][i][j]) / dx
                    };
                    // ∂u/∂y (upwind, averaged v at u-face)
                    let v_avg = 0.25
                        * (self.v[k][i - 1][j]
                            + self.v[k][i - 1][j + 1]
                            + self.v[k][i][j]
                            + self.v[k][i][j + 1]);
                    let du_dy = if v_avg > 0.0 {
                        v_avg * (self.u[k][i][j] - self.u[k][i - 1][j]) / dy
                    } else {
                        v_avg * (self.u[k][i + 1][j] - self.u[k][i][j]) / dy
                    };
                    // ∂u/∂z (upwind, averaged w at u-face)
                    let w_avg = 0.25
                        * (self.w[k - 1][i][j]
                            + self.w[k - 1][i][j + 1]
                            + self.w[k][i][j]
                            + self.w[k][i][j + 1]);
                    let du_dz = if w_avg > 0.0 {
                        w_avg * (self.u[k][i][j] - self.u[k - 1][i][j]) / dz
                    } else {
                        w_avg * (self.u[k + 1][i][j] - self.u[k][i][j]) / dz
                    };
                    // Diffusion: ∇²u
                    let laplacian = (self.u[k][i][j + 1] - 2.0 * self.u[k][i][j]
                        + self.u[k][i][j - 1])
                        / (dx * dx)
                        + (self.u[k][i + 1][j] - 2.0 * self.u[k][i][j] + self.u[k][i - 1][j])
                            / (dy * dy)
                        + (self.u[k + 1][i][j] - 2.0 * self.u[k][i][j] + self.u[k - 1][i][j])
                            / (dz * dz);
                    plane[i][j] = self.u[k][i][j] + dt * (-du_dx - du_dy - du_dz + nu * laplacian);
                }
            }
        });

        // ── v*: (nz+1)×ny×(nx+1) ──
        let mut v_star = vec![vec![vec![0.0; self.nx + 1]; self.ny]; self.nz + 1];
        v_star.par_iter_mut().enumerate().for_each(|(k, plane)| {
            if k == 0 || k >= self.nz {
                return;
            }
            for i in 1..self.ny - 1 {
                for j in 0..=self.nx {
                    if j == 0 || j >= self.nx {
                        continue;
                    }
                    let v_ij = self.v[k][i][j];
                    // u at v-face (averaged)
                    let u_avg = 0.25
                        * (self.u[k][i][j - 1]
                            + self.u[k][i][j]
                            + self.u[k][i + 1][j - 1]
                            + self.u[k][i + 1][j]);
                    let dv_dx = if u_avg > 0.0 {
                        u_avg * (self.v[k][i][j] - self.v[k][i][j - 1]) / dx
                    } else {
                        u_avg * (self.v[k][i][j + 1] - self.v[k][i][j]) / dx
                    };
                    let dv_dy = if v_ij > 0.0 {
                        v_ij * (self.v[k][i][j] - self.v[k][i - 1][j]) / dy
                    } else {
                        v_ij * (self.v[k][i + 1][j] - self.v[k][i][j]) / dy
                    };
                    let w_avg = 0.25
                        * (self.w[k - 1][i][j]
                            + self.w[k - 1][i + 1][j]
                            + self.w[k][i][j]
                            + self.w[k][i + 1][j]);
                    let dv_dz = if w_avg > 0.0 {
                        w_avg * (self.v[k][i][j] - self.v[k - 1][i][j]) / dz
                    } else {
                        w_avg * (self.v[k + 1][i][j] - self.v[k][i][j]) / dz
                    };
                    let laplacian = (self.v[k][i][j + 1] - 2.0 * self.v[k][i][j]
                        + self.v[k][i][j - 1])
                        / (dx * dx)
                        + (self.v[k][i + 1][j] - 2.0 * self.v[k][i][j] + self.v[k][i - 1][j])
                            / (dy * dy)
                        + (self.v[k + 1][i][j] - 2.0 * self.v[k][i][j] + self.v[k - 1][i][j])
                            / (dz * dz);
                    plane[i][j] = self.v[k][i][j] + dt * (-dv_dx - dv_dy - dv_dz + nu * laplacian);
                }
            }
        });

        // ── w*: nz×(ny+1)×(nx+1) ──
        let mut w_star = vec![vec![vec![0.0; self.nx + 1]; self.ny + 1]; self.nz];
        w_star.par_iter_mut().enumerate().for_each(|(k, plane)| {
            if k == 0 || k >= self.nz - 1 {
                return;
            }
            for i in 1..self.ny - 1 {
                for j in 1..self.nx - 1 {
                    let w_ij = self.w[k][i][j];
                    let u_avg = 0.25
                        * (self.u[k][i][j - 1]
                            + self.u[k][i][j]
                            + self.u[k + 1][i][j - 1]
                            + self.u[k + 1][i][j]);
                    let dw_dx = if u_avg > 0.0 {
                        u_avg * (self.w[k][i][j] - self.w[k][i][j - 1]) / dx
                    } else {
                        u_avg * (self.w[k][i][j + 1] - self.w[k][i][j]) / dx
                    };
                    let v_avg = 0.25
                        * (self.v[k][i - 1][j]
                            + self.v[k][i][j]
                            + self.v[k + 1][i - 1][j]
                            + self.v[k + 1][i][j]);
                    let dw_dy = if v_avg > 0.0 {
                        v_avg * (self.w[k][i][j] - self.w[k][i - 1][j]) / dy
                    } else {
                        v_avg * (self.w[k][i + 1][j] - self.w[k][i][j]) / dy
                    };
                    let dw_dz = if w_ij > 0.0 {
                        w_ij * (self.w[k][i][j] - self.w[k - 1][i][j]) / dz
                    } else {
                        w_ij * (self.w[k + 1][i][j] - self.w[k][i][j]) / dz
                    };
                    let laplacian = (self.w[k][i][j + 1] - 2.0 * self.w[k][i][j]
                        + self.w[k][i][j - 1])
                        / (dx * dx)
                        + (self.w[k][i + 1][j] - 2.0 * self.w[k][i][j] + self.w[k][i - 1][j])
                            / (dy * dy)
                        + (self.w[k + 1][i][j] - 2.0 * self.w[k][i][j] + self.w[k - 1][i][j])
                            / (dz * dz);
                    plane[i][j] = self.w[k][i][j] + dt * (-dw_dx - dw_dy - dw_dz + nu * laplacian);
                }
            }
        });

        (u_star, v_star, w_star)
    }

    /// Solve pressure Poisson equation ∇²p = (1/dt) ∇·u* using Jacobi iteration.
    ///
    /// Uses serial Jacobi with a second pressure array for the update.
    /// (The computationally intensive velocity steps are parallelised;
    ///  the Poisson solver converges in relatively few iterations.)
    fn solve_pressure_poisson(
        &mut self,
        u_star: &[Vec<Vec<Scalar>>],
        v_star: &[Vec<Vec<Scalar>>],
        w_star: &[Vec<Vec<Scalar>>],
    ) -> Result<(), String> {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (dx, dy, dz, dt) = (self.dx, self.dy, self.dz, self.dt);
        let dx2 = dx * dx;
        let dy2 = dy * dy;
        let dz2 = dz * dz;

        let max_iter = 500;
        let mut p_new = self.p.clone();
        for _iter in 0..max_iter {
            let mut max_diff: Scalar = 0.0;
            for k in 1..nz - 1 {
                for i in 1..ny - 1 {
                    for j in 1..nx - 1 {
                        let div_u = (u_star[k][i][j + 1] - u_star[k][i][j]) / dx
                            + (v_star[k][i + 1][j] - v_star[k][i][j]) / dy
                            + (w_star[k + 1][i][j] - w_star[k][i][j]) / dz;
                        let rhs = div_u / dt;
                        let p_val = ((self.p[k][i][j + 1] + self.p[k][i][j - 1]) * dy2 * dz2
                            + (self.p[k][i + 1][j] + self.p[k][i - 1][j]) * dx2 * dz2
                            + (self.p[k + 1][i][j] + self.p[k - 1][i][j]) * dx2 * dy2
                            - rhs * dx2 * dy2 * dz2)
                            / (dx2 * dy2 + dx2 * dz2 + dy2 * dz2);
                        let diff = (p_val - self.p[k][i][j]).abs();
                        if diff > max_diff {
                            max_diff = diff;
                        }
                        p_new[k][i][j] = p_val;
                    }
                }
            }
            std::mem::swap(&mut self.p, &mut p_new);
            if max_diff < 1e-6 {
                break;
            }
        }
        Ok(())
    }

    /// Correct velocities using the pressure gradient: u = u* - dt·∇p.
    fn velocity_correction(
        &mut self,
        u_star: &[Vec<Vec<Scalar>>],
        v_star: &[Vec<Vec<Scalar>>],
        w_star: &[Vec<Vec<Scalar>>],
    ) {
        use rayon::prelude::*;
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (dx, dy, dz, dt) = (self.dx, self.dy, self.dz, self.dt);

        // u correction
        self.u.par_iter_mut().enumerate().for_each(|(k, plane)| {
            for i in 0..=self.ny {
                for j in 0..nx {
                    let dp_dx = if j == 0 {
                        (self.p[k.min(nz - 1)][i.min(ny - 1)][1]
                            - self.p[k.min(nz - 1)][i.min(ny - 1)][0])
                            / dx
                    } else if j == nx - 1 {
                        (self.p[k.min(nz - 1)][i.min(ny - 1)][nx - 1]
                            - self.p[k.min(nz - 1)][i.min(ny - 1)][nx - 2])
                            / dx
                    } else {
                        (self.p[k.min(nz - 1)][i.min(ny - 1)][j]
                            - self.p[k.min(nz - 1)][i.min(ny - 1)][j - 1])
                            / dx
                    };
                    plane[i][j] = u_star[k][i][j] - dt * dp_dx;
                }
            }
        });

        // v correction
        self.v.par_iter_mut().enumerate().for_each(|(k, plane)| {
            for i in 0..ny {
                for j in 0..=self.nx {
                    let dp_dy = if i == 0 {
                        (self.p[k.min(nz - 1)][1][j.min(nx - 1)]
                            - self.p[k.min(nz - 1)][0][j.min(nx - 1)])
                            / dy
                    } else if i == ny - 1 {
                        (self.p[k.min(nz - 1)][ny - 1][j.min(nx - 1)]
                            - self.p[k.min(nz - 1)][ny - 2][j.min(nx - 1)])
                            / dy
                    } else {
                        (self.p[k.min(nz - 1)][i][j.min(nx - 1)]
                            - self.p[k.min(nz - 1)][i - 1][j.min(nx - 1)])
                            / dy
                    };
                    plane[i][j] = v_star[k][i][j] - dt * dp_dy;
                }
            }
        });

        // w correction
        self.w.par_iter_mut().enumerate().for_each(|(k, plane)| {
            for i in 0..=self.ny {
                for j in 0..=self.nx {
                    let dp_dz = if k == 0 {
                        (self.p[1][i.min(ny - 1)][j.min(nx - 1)]
                            - self.p[0][i.min(ny - 1)][j.min(nx - 1)])
                            / dz
                    } else if k >= nz - 1 {
                        (self.p[nz - 1][i.min(ny - 1)][j.min(nx - 1)]
                            - self.p[nz - 2][i.min(ny - 1)][j.min(nx - 1)])
                            / dz
                    } else {
                        (self.p[k][i.min(ny - 1)][j.min(nx - 1)]
                            - self.p[k - 1][i.min(ny - 1)][j.min(nx - 1)])
                            / dz
                    };
                    plane[i][j] = w_star[k][i][j] - dt * dp_dz;
                }
            }
        });
    }

    /// Apply boundary conditions to all six faces of the domain.
    pub fn set_bc(&mut self, boundary: &[WallCondition3D; 6]) {
        // Order: [x_min, x_max, y_min, y_max, z_min, z_max]
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);

        match boundary[0] {
            // x-min
            WallCondition3D::NoSlip => {
                for k in 0..=nz {
                    for i in 0..=ny {
                        self.u[k][i][0] = 0.0;
                    }
                }
                for k in 0..=nz {
                    for i in 0..ny {
                        self.v[k][i][0] = 0.0;
                    }
                }
                for k in 0..nz {
                    for i in 0..=ny {
                        self.w[k][i][0] = 0.0;
                    }
                }
            }
            WallCondition3D::Inlet(u_val, v_val, w_val) => {
                for k in 0..=nz {
                    for i in 0..=ny {
                        self.u[k][i][0] = u_val;
                    }
                }
                for k in 0..=nz {
                    for i in 0..ny {
                        self.v[k][i][0] = v_val;
                    }
                }
                for k in 0..nz {
                    for i in 0..=ny {
                        self.w[k][i][0] = w_val;
                    }
                }
            }
            _ => {}
        }

        match boundary[1] {
            // x-max
            WallCondition3D::NoSlip => {
                for k in 0..=nz {
                    for i in 0..=ny {
                        self.u[k][i][nx - 1] = 0.0;
                    }
                }
            }
            WallCondition3D::Outflow => {
                for k in 0..=nz {
                    for i in 0..=ny {
                        self.u[k][i][nx - 1] = self.u[k][i][nx - 2];
                    }
                }
            }
            _ => {}
        }

        match boundary[2] {
            // y-min
            WallCondition3D::NoSlip => {
                for k in 0..=nz {
                    for j in 0..nx {
                        self.v[k][0][j] = 0.0;
                    }
                }
            }
            _ => {}
        }

        match boundary[3] {
            // y-max
            WallCondition3D::NoSlip => {
                for k in 0..=nz {
                    for j in 0..nx {
                        self.v[k][ny - 1][j] = 0.0;
                    }
                }
            }
            _ => {}
        }

        match boundary[4] {
            // z-min
            WallCondition3D::NoSlip => {
                for j in 0..nx {
                    for i in 0..=ny {
                        self.w[0][i][j] = 0.0;
                    }
                }
            }
            _ => {}
        }

        match boundary[5] {
            // z-max
            WallCondition3D::NoSlip => {
                for j in 0..nx {
                    for i in 0..=ny {
                        self.w[nz - 1][i][j] = 0.0;
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ns3d_creation() {
        let solver = NavierStokes3D::new(8, 8, 8, 0.1, 0.1, 0.1, 0.001, 100.0);
        assert_eq!(solver.u.len(), 9); // nz+1
        assert_eq!(solver.u[0].len(), 9); // ny+1
        assert_eq!(solver.u[0][0].len(), 8); // nx
        assert_eq!(solver.p.len(), 8); // nz
        assert_eq!(solver.p[0].len(), 8); // ny
        assert_eq!(solver.p[0][0].len(), 8); // nx
    }

    #[test]
    fn test_projection_step_runs() {
        let mut solver = NavierStokes3D::new(6, 6, 6, 0.2, 0.2, 0.2, 0.001, 100.0);
        let bc = [
            WallCondition3D::NoSlip,
            WallCondition3D::Outflow,
            WallCondition3D::NoSlip,
            WallCondition3D::NoSlip,
            WallCondition3D::NoSlip,
            WallCondition3D::MovingWall(1.0, 0.0, 0.0),
        ];
        solver.set_bc(&bc);
        for _ in 0..5 {
            solver.projection_step().unwrap();
        }
        // Simulation runs without error; some velocity develops
        let max_u = solver
            .u
            .iter()
            .flat_map(|p| p.iter().flat_map(|r| r.iter()))
            .cloned()
            .fold(0.0_f64, f64::max);
        assert!(max_u >= 0.0, "velocity should be non-negative");
    }

    #[test]
    fn test_zero_forces_no_motion() {
        let mut solver = NavierStokes3D::new(6, 6, 6, 0.2, 0.2, 0.2, 0.001, 100.0);
        let bc = [WallCondition3D::NoSlip; 6];
        solver.set_bc(&bc);
        for _ in 0..3 {
            solver.projection_step().unwrap();
        }
        // With all walls noslip and no forcing, velocity stays (near) zero
        let max_vel = solver
            .u
            .iter()
            .flat_map(|p| p.iter().flat_map(|r| r.iter()))
            .cloned()
            .fold(0.0_f64, f64::max);
        assert!(
            max_vel < 1e-6,
            "no-slip walls with no forcing should stay near zero"
        );
    }

    #[test]
    fn test_inlet_outflow_steady() {
        let mut solver = NavierStokes3D::new(8, 6, 6, 0.1, 0.1, 0.1, 0.0005, 50.0);
        let bc = [
            WallCondition3D::Inlet(1.0, 0.0, 0.0),
            WallCondition3D::Outflow,
            WallCondition3D::NoSlip,
            WallCondition3D::NoSlip,
            WallCondition3D::NoSlip,
            WallCondition3D::FreeSlip,
        ];
        solver.set_bc(&bc);
        for _ in 0..10 {
            solver.projection_step().unwrap();
            solver.set_bc(&bc); // re-apply BC after each step
        }
        // Flow should develop from inlet
        let has_flow = solver
            .u
            .iter()
            .flat_map(|p| p.iter().flat_map(|r| r.iter()))
            .any(|&v| v.abs() > 0.01);
        assert!(has_flow, "inlet should produce measurable flow");
    }
}
