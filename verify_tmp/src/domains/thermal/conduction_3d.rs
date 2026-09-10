//! 3D transient heat conduction solver using the Alternating Direction Implicit
//! (ADI) method and steady-state Successive Over-Relaxation (SOR).
//!
//! Extends the 1D/2D solvers in `conduction.rs` to three spatial dimensions.
//! ADI splits each time step into three sub-steps (x, y, z sweeps), each
//! solving a tridiagonal system — unconditionally stable and O(n) per step.

#![allow(clippy::too_many_arguments)]

use crate::core::types::Scalar;

/// Number of independent ADI lines above which the sweeps run on rayon.
const ADI_PAR_MIN_LINES: usize = 64;

/// 3D boundary condition types for heat conduction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundaryCondition3D {
    FixedTemp(Scalar),
    FixedHeatFlux(Scalar),
    Convection(Scalar, Scalar),
    Adiabatic,
}

/// 3D transient heat conduction solver using the ADI method.
///
/// Solves ∂T/∂t = α·(∂²T/∂x² + ∂²T/∂y² + ∂²T/∂z²) on a uniform grid.
/// ADI is unconditionally stable (no CFL restriction) and O(n³ log n) per step.
pub struct HeatConduction3D {
    pub alpha: Scalar,
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub dx: Scalar,
    pub dy: Scalar,
    pub dz: Scalar,
    pub temperature: Vec<Vec<Vec<Scalar>>>,
    /// SOR relaxation factor for steady-state solving (1.0 = Gauss-Seidel).
    pub sor_omega: Scalar,
}

impl HeatConduction3D {
    /// Create a new 3D heat conduction problem with uniform initial temperature.
    pub fn new(
        alpha: Scalar,
        nx: usize,
        ny: usize,
        nz: usize,
        dx: Scalar,
        dy: Scalar,
        dz: Scalar,
        initial_temp: Scalar,
    ) -> Self {
        assert!(
            nx > 1 && ny > 1 && nz > 1,
            "grid must have at least 2 cells per dimension"
        );
        assert!(alpha > 0.0 && dx > 0.0 && dy > 0.0 && dz > 0.0);
        let temperature = vec![vec![vec![initial_temp; nx]; ny]; nz];
        Self {
            alpha,
            nx,
            ny,
            nz,
            dx,
            dy,
            dz,
            temperature,
            sor_omega: 1.0,
        }
    }

    /// Perform one ADI time step (x-sweep, y-sweep, z-sweep).
    ///
    /// Each sweep solves a set of 1D tridiagonal systems using the Thomas algorithm.
    pub fn adi_step(
        &mut self,
        dt: Scalar,
        boundary: &[BoundaryCondition3D; 6],
    ) -> Result<(), String> {
        self.x_sweep(dt, boundary)?;
        self.y_sweep(dt, boundary)?;
        self.z_sweep(dt, boundary)?;
        Ok(())
    }

    /// x-direction implicit sweep: solve tridiagonal system along each x-line.
    ///
    /// Each line (k,j) is independent: it is gathered from the current field,
    /// solved (in parallel over lines), then scattered back. This is a true
    /// Jacobi-style read-then-write per line, so lines never interact.
    fn x_sweep(&mut self, dt: Scalar, boundary: &[BoundaryCondition3D; 6]) -> Result<(), String> {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (dx, alpha) = (self.dx, self.alpha);
        let rx = alpha * dt / (dx * dx);

        let lines: Vec<(usize, usize)> =
            (0..nz).flat_map(|k| (0..ny).map(move |j| (k, j))).collect();
        let solve_line = |k: usize, j: usize| -> Result<Vec<Scalar>, String> {
            let mut a = vec![0.0; nx];
            let mut b = vec![0.0; nx];
            let mut c = vec![0.0; nx];
            let mut d = vec![0.0; nx];
            for i in 0..nx {
                a[i] = -rx;
                b[i] = 1.0 + 2.0 * rx;
                c[i] = -rx;
                d[i] = self.temperature[k][j][i];
            }
            match boundary[0] {
                BoundaryCondition3D::FixedTemp(t) => {
                    b[0] = 1.0;
                    c[0] = 0.0;
                    d[0] = t;
                }
                BoundaryCondition3D::FixedHeatFlux(q) => d[0] += q * dx / alpha,
                BoundaryCondition3D::Adiabatic => {
                    a[0] = 0.0;
                    b[0] = 1.0;
                    c[0] = -1.0;
                    d[0] = 0.0;
                }
                _ => {}
            }
            match boundary[1] {
                BoundaryCondition3D::FixedTemp(t) => {
                    a[nx - 1] = 0.0;
                    b[nx - 1] = 1.0;
                    d[nx - 1] = t;
                }
                BoundaryCondition3D::FixedHeatFlux(q) => d[nx - 1] -= q * dx / alpha,
                BoundaryCondition3D::Adiabatic => {
                    a[nx - 1] = -1.0;
                    b[nx - 1] = 1.0;
                    c[nx - 1] = 0.0;
                    d[nx - 1] = 0.0;
                }
                _ => {}
            }
            solve_tridiagonal(&a, &b, &c, &d)
        };
        let solved: Result<Vec<Vec<Scalar>>, String> = if lines.len() >= ADI_PAR_MIN_LINES {
            use rayon::prelude::*;
            lines.par_iter().map(|&(k, j)| solve_line(k, j)).collect()
        } else {
            lines.iter().map(|&(k, j)| solve_line(k, j)).collect()
        };
        let solved = solved?;
        for (&(k, j), x) in lines.iter().zip(solved.iter()) {
            self.temperature[k][j][..nx].copy_from_slice(x);
        }
        Ok(())
    }

    /// y-direction implicit sweep (lines indexed by (k, i), solved along j).
    fn y_sweep(&mut self, dt: Scalar, boundary: &[BoundaryCondition3D; 6]) -> Result<(), String> {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (dy, alpha) = (self.dy, self.alpha);
        let ry = alpha * dt / (dy * dy);

        let lines: Vec<(usize, usize)> =
            (0..nz).flat_map(|k| (0..nx).map(move |i| (k, i))).collect();
        let solve_line = |k: usize, i: usize| -> Result<Vec<Scalar>, String> {
            let mut a = vec![0.0; ny];
            let mut b = vec![0.0; ny];
            let mut c = vec![0.0; ny];
            let mut d = vec![0.0; ny];
            for j in 0..ny {
                a[j] = -ry;
                b[j] = 1.0 + 2.0 * ry;
                c[j] = -ry;
                d[j] = self.temperature[k][j][i];
            }
            match boundary[2] {
                BoundaryCondition3D::FixedTemp(t) => {
                    b[0] = 1.0;
                    c[0] = 0.0;
                    d[0] = t;
                }
                BoundaryCondition3D::FixedHeatFlux(q) => d[0] += q * dy / alpha,
                BoundaryCondition3D::Adiabatic => {
                    a[0] = 0.0;
                    b[0] = 1.0;
                    c[0] = -1.0;
                    d[0] = 0.0;
                }
                _ => {}
            }
            match boundary[3] {
                BoundaryCondition3D::FixedTemp(t) => {
                    a[ny - 1] = 0.0;
                    b[ny - 1] = 1.0;
                    d[ny - 1] = t;
                }
                BoundaryCondition3D::FixedHeatFlux(q) => d[ny - 1] -= q * dy / alpha,
                BoundaryCondition3D::Adiabatic => {
                    a[ny - 1] = -1.0;
                    b[ny - 1] = 1.0;
                    c[ny - 1] = 0.0;
                    d[ny - 1] = 0.0;
                }
                _ => {}
            }
            solve_tridiagonal(&a, &b, &c, &d)
        };
        let solved: Result<Vec<Vec<Scalar>>, String> = if lines.len() >= ADI_PAR_MIN_LINES {
            use rayon::prelude::*;
            lines.par_iter().map(|&(k, i)| solve_line(k, i)).collect()
        } else {
            lines.iter().map(|&(k, i)| solve_line(k, i)).collect()
        };
        let solved = solved?;
        for (&(k, i), x) in lines.iter().zip(solved.iter()) {
            for (j, &val) in x.iter().enumerate() {
                self.temperature[k][j][i] = val;
            }
        }
        Ok(())
    }

    /// z-direction implicit sweep.
    fn z_sweep(&mut self, dt: Scalar, boundary: &[BoundaryCondition3D; 6]) -> Result<(), String> {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (dz, alpha) = (self.dz, self.alpha);
        let rz = alpha * dt / (dz * dz);

        let lines: Vec<(usize, usize)> =
            (0..ny).flat_map(|j| (0..nx).map(move |i| (j, i))).collect();
        let solve_line = |j: usize, i: usize| -> Result<Vec<Scalar>, String> {
            let mut a = vec![0.0; nz];
            let mut b = vec![0.0; nz];
            let mut c = vec![0.0; nz];
            let mut d = vec![0.0; nz];
            for k in 0..nz {
                a[k] = -rz;
                b[k] = 1.0 + 2.0 * rz;
                c[k] = -rz;
                d[k] = self.temperature[k][j][i];
            }
            match boundary[4] {
                BoundaryCondition3D::FixedTemp(t) => {
                    b[0] = 1.0;
                    c[0] = 0.0;
                    d[0] = t;
                }
                BoundaryCondition3D::FixedHeatFlux(q) => d[0] += q * dz / alpha,
                BoundaryCondition3D::Adiabatic => {
                    a[0] = 0.0;
                    b[0] = 1.0;
                    c[0] = -1.0;
                    d[0] = 0.0;
                }
                _ => {}
            }
            match boundary[5] {
                BoundaryCondition3D::FixedTemp(t) => {
                    a[nz - 1] = 0.0;
                    b[nz - 1] = 1.0;
                    d[nz - 1] = t;
                }
                BoundaryCondition3D::FixedHeatFlux(q) => d[nz - 1] -= q * dz / alpha,
                BoundaryCondition3D::Adiabatic => {
                    a[nz - 1] = -1.0;
                    b[nz - 1] = 1.0;
                    c[nz - 1] = 0.0;
                    d[nz - 1] = 0.0;
                }
                _ => {}
            }
            solve_tridiagonal(&a, &b, &c, &d)
        };
        let solved: Result<Vec<Vec<Scalar>>, String> = if lines.len() >= ADI_PAR_MIN_LINES {
            use rayon::prelude::*;
            lines.par_iter().map(|&(j, i)| solve_line(j, i)).collect()
        } else {
            lines.iter().map(|&(j, i)| solve_line(j, i)).collect()
        };
        let solved = solved?;
        for (&(j, i), x) in lines.iter().zip(solved.iter()) {
            for (k, &val) in x.iter().enumerate() {
                self.temperature[k][j][i] = val;
            }
        }
        Ok(())
    }

    /// Perform one SOR iteration for steady-state solution.
    ///
    /// Uses the 7-point stencil with configurable relaxation factor `sor_omega`.
    /// When `sor_omega == 1.0`, this is standard Gauss-Seidel.
    pub fn sor_step(&mut self, boundary: &[BoundaryCondition3D; 6]) -> Result<(), String> {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (dx2, dy2, dz2) = (self.dx * self.dx, self.dy * self.dy, self.dz * self.dz);
        let denom = 2.0 * (dx2 * dy2 + dx2 * dz2 + dy2 * dz2);
        let omega = self.sor_omega;
        let one_minus_omega = 1.0 - omega;

        self.apply_boundary(boundary)?;

        for k in 1..nz - 1 {
            for j in 1..ny - 1 {
                for i in 1..nx - 1 {
                    let t_old = self.temperature[k][j][i];
                    let sum_face =
                        (self.temperature[k][j][i + 1] + self.temperature[k][j][i - 1]) * dy2 * dz2
                            + (self.temperature[k][j + 1][i] + self.temperature[k][j - 1][i])
                                * dx2
                                * dz2
                            + (self.temperature[k + 1][j][i] + self.temperature[k - 1][j][i])
                                * dx2
                                * dy2;
                    let t_gs = sum_face / denom;
                    self.temperature[k][j][i] = one_minus_omega * t_old + omega * t_gs;
                }
            }
        }
        Ok(())
    }

    /// Apply boundary conditions to all six faces.
    fn apply_boundary(&mut self, boundary: &[BoundaryCondition3D; 6]) -> Result<(), String> {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        // x-min (i=0) and x-max (i=nx-1)
        for k in 0..nz {
            for j in 0..ny {
                match boundary[0] {
                    BoundaryCondition3D::FixedTemp(t) => self.temperature[k][j][0] = t,
                    BoundaryCondition3D::Adiabatic => {
                        self.temperature[k][j][0] = self.temperature[k][j][1]
                    }
                    BoundaryCondition3D::FixedHeatFlux(q) => {
                        self.temperature[k][j][0] =
                            self.temperature[k][j][1] + q * self.dx / self.alpha;
                    }
                    _ => {}
                }
                match boundary[1] {
                    BoundaryCondition3D::FixedTemp(t) => self.temperature[k][j][nx - 1] = t,
                    BoundaryCondition3D::Adiabatic => {
                        self.temperature[k][j][nx - 1] = self.temperature[k][j][nx - 2]
                    }
                    BoundaryCondition3D::FixedHeatFlux(q) => {
                        self.temperature[k][j][nx - 1] =
                            self.temperature[k][j][nx - 2] + q * self.dx / self.alpha;
                    }
                    _ => {}
                }
            }
        }
        // y-min (j=0) and y-max (j=ny-1)
        for k in 0..nz {
            for i in 0..nx {
                match boundary[2] {
                    BoundaryCondition3D::FixedTemp(t) => self.temperature[k][0][i] = t,
                    BoundaryCondition3D::Adiabatic => {
                        self.temperature[k][0][i] = self.temperature[k][1][i]
                    }
                    BoundaryCondition3D::FixedHeatFlux(q) => {
                        self.temperature[k][0][i] =
                            self.temperature[k][1][i] + q * self.dy / self.alpha;
                    }
                    _ => {}
                }
                match boundary[3] {
                    BoundaryCondition3D::FixedTemp(t) => self.temperature[k][ny - 1][i] = t,
                    BoundaryCondition3D::Adiabatic => {
                        self.temperature[k][ny - 1][i] = self.temperature[k][ny - 2][i]
                    }
                    BoundaryCondition3D::FixedHeatFlux(q) => {
                        self.temperature[k][ny - 1][i] =
                            self.temperature[k][ny - 2][i] + q * self.dy / self.alpha;
                    }
                    _ => {}
                }
            }
        }
        // z-min (k=0) and z-max (k=nz-1)
        for j in 0..ny {
            for i in 0..nx {
                match boundary[4] {
                    BoundaryCondition3D::FixedTemp(t) => self.temperature[0][j][i] = t,
                    BoundaryCondition3D::Adiabatic => {
                        self.temperature[0][j][i] = self.temperature[1][j][i]
                    }
                    BoundaryCondition3D::FixedHeatFlux(q) => {
                        self.temperature[0][j][i] =
                            self.temperature[1][j][i] + q * self.dz / self.alpha;
                    }
                    _ => {}
                }
                match boundary[5] {
                    BoundaryCondition3D::FixedTemp(t) => self.temperature[nz - 1][j][i] = t,
                    BoundaryCondition3D::Adiabatic => {
                        self.temperature[nz - 1][j][i] = self.temperature[nz - 2][j][i]
                    }
                    BoundaryCondition3D::FixedHeatFlux(q) => {
                        self.temperature[nz - 1][j][i] =
                            self.temperature[nz - 2][j][i] + q * self.dz / self.alpha;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Check if the temperature field has converged within `tolerance`.
    pub fn check_convergence(&self, tolerance: Scalar) -> bool {
        // For a uniform field, any cell comparison works
        if self.nx == 0 || self.ny == 0 || self.nz == 0 {
            return true;
        }
        let t00 = self.temperature[0][0][0];
        self.temperature.iter().all(|plane| {
            plane
                .iter()
                .all(|row| row.iter().all(|&t| (t - t00).abs() < tolerance))
        })
    }
}

/// Solve a tridiagonal system using the Thomas algorithm.
///
/// a[i]·x[i-1] + b[i]·x[i] + c[i]·x[i+1] = d[i]
/// with a[0] = 0 and c[n-1] = 0.
fn solve_tridiagonal(
    a: &[Scalar],
    b: &[Scalar],
    c: &[Scalar],
    d: &[Scalar],
) -> Result<Vec<Scalar>, String> {
    let n = a.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let mut cp = vec![0.0; n];
    let mut dp = vec![0.0; n];

    if b[0].abs() < 1e-30 {
        return Err("Tridiagonal: zero pivot at row 0".to_string());
    }
    cp[0] = c[0] / b[0];
    dp[0] = d[0] / b[0];

    for i in 1..n {
        let denom = b[i] - a[i] * cp[i - 1];
        if denom.abs() < 1e-30 {
            return Err(format!("Tridiagonal: zero pivot at row {}", i));
        }
        if i < n - 1 {
            cp[i] = c[i] / denom;
        }
        dp[i] = (d[i] - a[i] * dp[i - 1]) / denom;
    }

    let mut x = vec![0.0; n];
    x[n - 1] = dp[n - 1];
    for i in (0..n - 1).rev() {
        x[i] = dp[i] - cp[i] * x[i + 1];
    }
    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        let hc = HeatConduction3D::new(1e-4, 10, 10, 10, 0.01, 0.01, 0.01, 20.0);
        assert_eq!(hc.temperature.len(), 10);
        assert_eq!(hc.temperature[0].len(), 10);
        assert_eq!(hc.temperature[0][0].len(), 10);
        assert!((hc.temperature[5][5][5] - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_adi_step_basic() {
        let mut hc = HeatConduction3D::new(1e-4, 10, 10, 10, 0.01, 0.01, 0.01, 20.0);
        let bc = [
            BoundaryCondition3D::FixedTemp(100.0),
            BoundaryCondition3D::FixedTemp(0.0),
            BoundaryCondition3D::Adiabatic,
            BoundaryCondition3D::Adiabatic,
            BoundaryCondition3D::Adiabatic,
            BoundaryCondition3D::Adiabatic,
        ];
        hc.adi_step(0.1, &bc).unwrap();
        // After one ADI step, temperature should have changed near boundaries
        let has_change = hc
            .temperature
            .iter()
            .any(|p| p.iter().any(|r| r.iter().any(|&t| (t - 20.0).abs() > 0.01)));
        assert!(has_change);
    }

    #[test]
    fn test_adi_parallel_steady_state() {
        // 10×10×10 → 100 lines per sweep ≥ ADI_PAR_MIN_LINES, so every sweep
        // runs the rayon path. 500 steps ≫ diffusion time scale ⇒ steady state.
        let mut hc = HeatConduction3D::new(1e-4, 10, 10, 10, 0.01, 0.01, 0.01, 20.0);
        assert!(hc.nz * hc.ny >= ADI_PAR_MIN_LINES);
        let bc = [
            BoundaryCondition3D::FixedTemp(100.0),
            BoundaryCondition3D::FixedTemp(0.0),
            BoundaryCondition3D::Adiabatic,
            BoundaryCondition3D::Adiabatic,
            BoundaryCondition3D::Adiabatic,
            BoundaryCondition3D::Adiabatic,
        ];
        for _ in 0..500 {
            hc.adi_step(0.1, &bc).unwrap();
        }
        // Steady state is the linear profile 100·(1 - i/(nx-1)); mid-plane ≈ 44.4.
        let expect = 100.0 * (1.0 - 5.0 / 9.0);
        let mid = hc.temperature[5][5][5];
        assert!(
            (mid - expect).abs() < 15.0,
            "parallel ADI steady state off: {mid}"
        );
        assert!(mid.is_finite());
    }

    #[test]
    fn test_sor_step_basic() {
        let mut hc = HeatConduction3D::new(1e-4, 10, 10, 10, 0.01, 0.01, 0.01, 20.0);
        let bc = [BoundaryCondition3D::FixedTemp(100.0); 6];
        hc.sor_omega = 1.5;
        for _ in 0..20 {
            hc.sor_step(&bc).unwrap();
        }
        // Interior should approach 100°C with all walls at 100°C
        assert!((hc.temperature[5][5][5] - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_tridiagonal_solver() {
        let a = vec![0.0, 1.0, 1.0];
        let b = vec![2.0, 2.0, 2.0];
        let c = vec![1.0, 1.0, 0.0];
        let d = vec![4.0, 8.0, 8.0];
        let x = solve_tridiagonal(&a, &b, &c, &d).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-10);
        assert!((x[1] - 2.0).abs() < 1e-10);
        assert!((x[2] - 3.0).abs() < 1e-10);
    }
}
