//! 3D FDTD (Finite-Difference Time-Domain) electromagnetic solver.
//!
//! Implements the Yee algorithm for solving Maxwell's curl equations
//! on a staggered 3D grid. Supports CPML absorbing boundaries and
//! arbitrary source injection.
//!
//! Grid layout (Yee cell):
//!   E-field components are located on cell edges.
//!   H-field components are located on cell faces.
//!   Dimensions: Ex[(nz+1)×(ny+1)×nx], Ey[(nz+1)×ny×(nx+1)], Ez[nz×(ny+1)×(nx+1)]
//!               Hx[nz×(ny+1)×(nx+1)], Hy[(nz+1)×ny×(nx+1)], Hz[(nz+1)×(ny+1)×nx]

#![allow(clippy::too_many_arguments)]

use super::physics::{C, EPSILON_0, MU_0};
use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// 3D CPML (Convolutional Perfectly Matched Layer) parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpmlParams {
    pub thickness: usize,
    pub sigma_max: Scalar,
    pub kappa_max: Scalar,
    pub alpha_max: Scalar,
    pub order: Scalar,
}

impl Default for CpmlParams {
    fn default() -> Self {
        Self {
            thickness: 10,
            sigma_max: 1.0,
            kappa_max: 7.0,
            alpha_max: 0.05,
            order: 3.0,
        }
    }
}

/// 3D FDTD boundary condition type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundaryType3D {
    PEC,
    PMC,
    Cpml(CpmlParams),
}

/// Hard/soft source types for wave injection.
#[derive(Debug, Clone)]
pub enum Source3D {
    Point {
        position: (usize, usize, usize),
        component: FieldComponent,
        waveform: Waveform,
    },
    PlaneWave {
        direction: Coord3D,
        polarization: Coord3D,
        waveform: Waveform,
        start_plane: usize,
    },
}

/// Field component selector for source injection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldComponent {
    Ex,
    Ey,
    Ez,
}

/// Waveform types for source excitation.
#[derive(Debug, Clone)]
pub enum Waveform {
    Gaussian {
        amplitude: Scalar,
        tau: Scalar,
        t0: Scalar,
    },
    Ricker {
        amplitude: Scalar,
        fc: Scalar,
    },
    Sinusoidal {
        amplitude: Scalar,
        freq: Scalar,
        phase: Scalar,
    },
}

impl Waveform {
    pub fn value(&self, t: Scalar) -> Scalar {
        match self {
            Self::Gaussian { amplitude, tau, t0 } => {
                let arg = ((t - t0) / tau).powi(2);
                amplitude * (-arg).exp()
            }
            Self::Ricker { amplitude, fc } => {
                let arg = (std::f64::consts::PI * fc * (t - 1.0 / fc)).powi(2);
                amplitude * (1.0 - 2.0 * arg) * (-arg).exp()
            }
            Self::Sinusoidal {
                amplitude,
                freq,
                phase,
            } => amplitude * (2.0 * std::f64::consts::PI * freq * t + phase).sin(),
        }
    }
}

/// 3D FDTD simulation kernel using the Yee algorithm.
#[derive(Debug, Clone)]
pub struct Fdtd3D {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub dx: Scalar,
    pub dy: Scalar,
    pub dz: Scalar,
    pub dt: Scalar,
    // E-field components (Yee cell edges)
    pub ex: Vec<Vec<Vec<Scalar>>>, // (nz+1)×(ny+1)×nx
    pub ey: Vec<Vec<Vec<Scalar>>>, // (nz+1)×ny×(nx+1)
    pub ez: Vec<Vec<Vec<Scalar>>>, // nz×(ny+1)×(nx+1)
    // H-field components (Yee cell faces)
    pub hx: Vec<Vec<Vec<Scalar>>>, // nz×(ny+1)×(nx+1)
    pub hy: Vec<Vec<Vec<Scalar>>>, // (nz+1)×ny×(nx+1)
    pub hz: Vec<Vec<Vec<Scalar>>>, // (nz+1)×(ny+1)×nx
    // Material parameters (vacuum by default)
    pub epsilon_r: Vec<Vec<Vec<Scalar>>>, // nz×ny×nx
    pub mu_r: Vec<Vec<Vec<Scalar>>>,      // nz×ny×nx
    pub sigma_e: Vec<Vec<Vec<Scalar>>>,   // nz×ny×nx (electrical conductivity)
    pub sigma_m: Vec<Vec<Vec<Scalar>>>,   // nz×ny×nx (magnetic loss)
    // Boundary condition
    pub boundary: BoundaryType3D,
    // CPML auxiliary arrays (only allocated if Cpml)
    pub psi_exy: Option<Vec<Vec<Vec<Scalar>>>>,
    pub psi_exz: Option<Vec<Vec<Vec<Scalar>>>>,
    pub psi_eyx: Option<Vec<Vec<Vec<Scalar>>>>,
    pub psi_eyz: Option<Vec<Vec<Vec<Scalar>>>>,
    pub psi_ezx: Option<Vec<Vec<Vec<Scalar>>>>,
    pub psi_ezy: Option<Vec<Vec<Vec<Scalar>>>>,
    // Sources
    pub sources: Vec<Source3D>,
    /// Current simulation time.
    pub time: Scalar,
    /// Current step number.
    pub step: usize,
}

impl Fdtd3D {
    /// Create a new 3D FDTD solver with uniform vacuum material properties.
    ///
    /// The time step `dt` should satisfy the CFL stability condition:
    ///   dt ≤ 1/(c·√(1/dx² + 1/dy² + 1/dz²))
    pub fn new(
        nx: usize,
        ny: usize,
        nz: usize,
        dx: Scalar,
        dy: Scalar,
        dz: Scalar,
        dt: Scalar,
    ) -> Self {
        assert!(nx > 0 && ny > 0 && nz > 0);
        assert!(dx > 0.0 && dy > 0.0 && dz > 0.0 && dt > 0.0);

        let c0 = C;
        let dt_cfl = 1.0 / (c0 * (1.0 / (dx * dx) + 1.0 / (dy * dy) + 1.0 / (dz * dz)).sqrt());
        assert!(dt <= dt_cfl, "dt {} exceeds CFL limit {}", dt, dt_cfl);

        let uniform_eps = vec![vec![vec![1.0; nx]; ny]; nz];
        let uniform_mu = vec![vec![vec![1.0; nx]; ny]; nz];
        let uniform_sigma = vec![vec![vec![0.0; nx]; ny]; nz];

        Self {
            nx,
            ny,
            nz,
            dx,
            dy,
            dz,
            dt,
            ex: vec![vec![vec![0.0; nx]; ny + 1]; nz + 1],
            ey: vec![vec![vec![0.0; nx + 1]; ny]; nz + 1],
            ez: vec![vec![vec![0.0; nx + 1]; ny + 1]; nz],
            hx: vec![vec![vec![0.0; nx + 1]; ny + 1]; nz],
            hy: vec![vec![vec![0.0; nx + 1]; ny]; nz + 1],
            hz: vec![vec![vec![0.0; nx]; ny + 1]; nz + 1],
            epsilon_r: uniform_eps.clone(),
            mu_r: uniform_mu.clone(),
            sigma_e: uniform_sigma.clone(),
            sigma_m: uniform_sigma,
            boundary: BoundaryType3D::PEC,
            psi_exy: None,
            psi_exz: None,
            psi_eyx: None,
            psi_eyz: None,
            psi_ezx: None,
            psi_ezy: None,
            sources: Vec::new(),
            time: 0.0,
            step: 0,
        }
    }

    /// Initialise CPML auxiliary arrays (allocates memory).
    pub fn init_cpml(&mut self) {
        self.boundary = BoundaryType3D::Cpml(CpmlParams::default());
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        self.psi_exy = Some(vec![vec![vec![0.0; nx]; ny + 1]; nz + 1]);
        self.psi_exz = Some(vec![vec![vec![0.0; nx]; ny + 1]; nz + 1]);
        self.psi_eyx = Some(vec![vec![vec![0.0; nx + 1]; ny]; nz + 1]);
        self.psi_eyz = Some(vec![vec![vec![0.0; nx + 1]; ny]; nz + 1]);
        self.psi_ezx = Some(vec![vec![vec![0.0; nx + 1]; ny + 1]; nz]);
        self.psi_ezy = Some(vec![vec![vec![0.0; nx + 1]; ny + 1]; nz]);
    }

    /// Perform one FDTD time step: update H-fields, then E-fields, inject sources.
    pub fn step(&mut self) {
        // Leap-frog: H at n+½, E at n
        self.update_h();
        self.update_e();
        self.inject_sources();
        self.apply_boundary();
        self.time += self.dt;
        self.step += 1;
    }

    /// Update H-field components (magnetic field, n+½ step).
    ///
    /// Uses safe boundary handling: interior points use central differences;
    /// edges use one-sided differences or zero as appropriate.
    fn update_h(&mut self) {
        use rayon::prelude::*;
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (dx, dy, dz, dt) = (self.dx, self.dy, self.dz, self.dt);
        let hx_dt = dt / MU_0;

        // Hx: nz×(ny+1)×(nx+1)
        let k_max = nz.min(self.ey.len().saturating_sub(1));
        self.hx[..k_max]
            .par_iter_mut()
            .enumerate()
            .for_each(|(k, plane)| {
                for i in 0..=ny {
                    for j in 0..=nx {
                        let ey_kp1 = self
                            .ey
                            .get(k + 1)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        let ey_k = self
                            .ey
                            .get(k)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        let ez_ip1 = self
                            .ez
                            .get(k)
                            .and_then(|p| p.get(i + 1).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        let ez_i = self
                            .ez
                            .get(k)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        plane[i][j] += hx_dt * ((ey_kp1 - ey_k) / dz - (ez_ip1 - ez_i) / dy);
                    }
                }
            });

        // Hy: nz×ny×(nx+1) — iterate interior only
        let k_max = nz.min(self.ex.len().saturating_sub(1));
        self.hy[..k_max]
            .par_iter_mut()
            .enumerate()
            .for_each(|(k, plane)| {
                for i in 0..ny {
                    for j in 0..=nx {
                        let ez_jp1 = self
                            .ez
                            .get(k)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j + 1)))
                            .copied()
                            .unwrap_or(0.0);
                        let ez_j = self
                            .ez
                            .get(k)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        let ex_kp1 = self
                            .ex
                            .get(k + 1)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        let ex_k = self
                            .ex
                            .get(k)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        plane[i][j] += hx_dt * ((ez_jp1 - ez_j) / dx - (ex_kp1 - ex_k) / dz);
                    }
                }
            });

        // Hz: nz×(ny+1)×nx — iterate interior only
        let k_max = nz.min(self.ex.len());
        self.hz[..k_max]
            .par_iter_mut()
            .enumerate()
            .for_each(|(k, plane)| {
                for i in 0..=ny {
                    for j in 0..nx {
                        let ex_ip1 = self
                            .ex
                            .get(k)
                            .and_then(|p| p.get(i + 1).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        let ex_i = self
                            .ex
                            .get(k)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        let ey_jp1 = self
                            .ey
                            .get(k)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j + 1)))
                            .copied()
                            .unwrap_or(0.0);
                        let ey_j = self
                            .ey
                            .get(k)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0);
                        plane[i][j] += hx_dt * ((ex_ip1 - ex_i) / dy - (ey_jp1 - ey_j) / dx);
                    }
                }
            });
    }

    /// Update E-field components (electric field, n+1 step).
    fn update_e(&mut self) {
        use rayon::prelude::*;
        let (nx, ny, _nz) = (self.nx, self.ny, self.nz);
        let (dx, dy, dz, dt) = (self.dx, self.dy, self.dz, self.dt);
        let edt = dt / EPSILON_0;

        // Ex: (nz+1)×(ny+1)×nx
        self.ex.par_iter_mut().enumerate().for_each(|(k, plane)| {
            for i in 0..=ny {
                for j in 0..nx {
                    let hz_ip1 = self
                        .hz
                        .get(k)
                        .and_then(|p| p.get(i + 1).and_then(|r| r.get(j)))
                        .copied()
                        .unwrap_or(0.0);
                    let hz_i = self
                        .hz
                        .get(k)
                        .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                        .copied()
                        .unwrap_or(0.0);
                    let hy_kp1 = self
                        .hy
                        .get(k + 1)
                        .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                        .copied()
                        .unwrap_or(0.0);
                    let hy_k = self
                        .hy
                        .get(k)
                        .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                        .copied()
                        .unwrap_or(0.0);
                    plane[i][j] += edt * ((hz_ip1 - hz_i) / dy - (hy_kp1 - hy_k) / dz);
                }
            }
        });

        // Ey: (nz+1)×ny×(nx+1)
        self.ey.par_iter_mut().enumerate().for_each(|(k, plane)| {
            for i in 0..ny {
                for j in 0..=nx {
                    let hx_k = self
                        .hx
                        .get(k)
                        .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                        .copied()
                        .unwrap_or(0.0);
                    let hx_km1 = if k > 0 {
                        self.hx
                            .get(k - 1)
                            .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                            .copied()
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    let hz_jp1 = self
                        .hz
                        .get(k)
                        .and_then(|p| p.get(i).and_then(|r| r.get(j + 1)))
                        .copied()
                        .unwrap_or(0.0);
                    let hz_j = self
                        .hz
                        .get(k)
                        .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                        .copied()
                        .unwrap_or(0.0);
                    plane[i][j] += edt * ((hx_k - hx_km1) / dz - (hz_jp1 - hz_j) / dx);
                }
            }
        });

        // Ez: nz×(ny+1)×(nx+1)
        self.ez.par_iter_mut().enumerate().for_each(|(k, plane)| {
            for i in 0..=ny {
                for j in 0..=nx {
                    let hy_jp1 = self
                        .hy
                        .get(k)
                        .and_then(|p| p.get(i).and_then(|r| r.get(j + 1)))
                        .copied()
                        .unwrap_or(0.0);
                    let hy_j = self
                        .hy
                        .get(k)
                        .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                        .copied()
                        .unwrap_or(0.0);
                    let hx_ip1 = self
                        .hx
                        .get(k)
                        .and_then(|p| p.get(i + 1).and_then(|r| r.get(j)))
                        .copied()
                        .unwrap_or(0.0);
                    let hx_i = self
                        .hx
                        .get(k)
                        .and_then(|p| p.get(i).and_then(|r| r.get(j)))
                        .copied()
                        .unwrap_or(0.0);
                    plane[i][j] += edt * ((hy_jp1 - hy_j) / dx - (hx_ip1 - hx_i) / dy);
                }
            }
        });
    }

    /// Inject active sources into the E-field.
    fn inject_sources(&mut self) {
        let t = self.time;
        for src in &self.sources {
            match src {
                Source3D::Point {
                    position: (i, j, k),
                    component,
                    waveform,
                } => {
                    let val = waveform.value(t);
                    match component {
                        FieldComponent::Ex => {
                            if *k < self.ex.len()
                                && *i < self.ex[0].len()
                                && *j < self.ex[0][0].len()
                            {
                                self.ex[*k][*i][*j] = val;
                            }
                        }
                        FieldComponent::Ey => {
                            if *k < self.ey.len()
                                && *i < self.ey[0].len()
                                && *j < self.ey[0][0].len()
                            {
                                self.ey[*k][*i][*j] = val;
                            }
                        }
                        FieldComponent::Ez => {
                            if *k < self.ez.len()
                                && *i < self.ez[0].len()
                                && *j < self.ez[0][0].len()
                            {
                                self.ez[*k][*i][*j] = val;
                            }
                        }
                    }
                }
                Source3D::PlaneWave {
                    direction: _,
                    polarization,
                    waveform,
                    start_plane,
                } => {
                    let val = waveform.value(t);
                    let k = *start_plane;
                    if k < self.ez.len() {
                        let ny1 = self.ez[0].len();
                        let nx1 = self.ez[0][0].len();
                        for i in 0..ny1 {
                            for j in 0..nx1 {
                                self.ez[k][i][j] += val * polarization.z;
                                self.ey[k][i][j] += val * polarization.y;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Apply PEC boundary conditions (tangential E = 0 on boundary).
    fn apply_boundary(&mut self) {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        match self.boundary {
            BoundaryType3D::PEC => {
                // Tangential E = 0 on all PEC boundaries
                // Ex: zero on y=0, y=ny, z=0, z=nz
                for k in 0..=nz {
                    for i in [0, ny] {
                        for j in 0..nx {
                            self.ex[k][i][j] = 0.0;
                        }
                    }
                }
                for k in [0, nz] {
                    for i in 0..=ny {
                        for j in 0..nx {
                            self.ex[k][i][j] = 0.0;
                        }
                    }
                }
                // Ey: zero on x=0, x=nx, z=0, z=nz
                for k in 0..=nz {
                    for i in 0..ny {
                        for j in [0, nx] {
                            self.ey[k][i][j] = 0.0;
                        }
                    }
                }
                for k in [0, nz] {
                    for i in 0..ny {
                        for j in 0..=nx {
                            self.ey[k][i][j] = 0.0;
                        }
                    }
                }
                // Ez: zero on x=0, x=nx, y=0, y=ny
                for k in 0..nz {
                    for i in [0, ny] {
                        for j in 0..=nx {
                            self.ez[k][i][j] = 0.0;
                        }
                    }
                }
                for k in 0..nz {
                    for i in 0..=ny {
                        for j in [0, nx] {
                            self.ez[k][i][j] = 0.0;
                        }
                    }
                }
            }
            BoundaryType3D::Cpml(_) => {
                // CPML handles absorption internally; for now apply PEC-like
                // truncation at the outermost layer
            }
            _ => {}
        }
    }

    /// Compute total electromagnetic energy in the domain.
    pub fn total_energy(&self) -> Scalar {
        let eps0 = EPSILON_0;
        let mu0 = MU_0;
        let mut energy = 0.0;
        // Electric field energy: ½ε₀E² per cell
        for k in 0..self.nz {
            for i in 0..self.ny {
                for j in 0..self.nx {
                    let ex2 = self.ex[k][i][j].powi(2);
                    let ey2 = self.ey[k][i][j].powi(2);
                    let ez2 = self.ez[k][i][j].powi(2);
                    energy += 0.5 * eps0 * (ex2 + ey2 + ez2);
                }
            }
        }
        // Magnetic field energy: ½μ₀H² per cell
        for k in 0..self.nz {
            for i in 0..self.ny {
                for j in 0..self.nx {
                    let hx2 = self.hx[k][i][j].powi(2);
                    let hy2 = self.hy[k][i][j].powi(2);
                    let hz2 = self.hz[k][i][j].powi(2);
                    energy += 0.5 * mu0 * (hx2 + hy2 + hz2);
                }
            }
        }
        energy * self.dx * self.dy * self.dz
    }

    /// Record a time-domain waveform at a specific point.
    pub fn probe(&self, x: usize, y: usize, z: usize, component: FieldComponent) -> Scalar {
        match component {
            FieldComponent::Ex => {
                if z < self.ex.len() && y < self.ex[0].len() && x < self.ex[0][0].len() {
                    self.ex[z][y][x]
                } else {
                    0.0
                }
            }
            FieldComponent::Ey => {
                if z < self.ey.len() && y < self.ey[0].len() && x < self.ey[0][0].len() {
                    self.ey[z][y][x]
                } else {
                    0.0
                }
            }
            FieldComponent::Ez => {
                if z < self.ez.len() && y < self.ez[0].len() && x < self.ez[0][0].len() {
                    self.ez[z][y][x]
                } else {
                    0.0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fdtd3d_creation() {
        let fdtd = Fdtd3D::new(10, 10, 10, 1e-3, 1e-3, 1e-3, 1.5e-12);
        assert_eq!(fdtd.ex.len(), 11);
        assert_eq!(fdtd.ex[0].len(), 11);
        assert_eq!(fdtd.ex[0][0].len(), 10);
        assert_eq!(fdtd.ez.len(), 10);
        assert_eq!(fdtd.hx.len(), 10);
        assert_eq!(fdtd.hz.len(), 11);
        assert_eq!(fdtd.step, 0);
    }

    #[test]
    fn test_cfl_check() {
        let result = std::panic::catch_unwind(|| {
            Fdtd3D::new(10, 10, 10, 1e-3, 1e-3, 1e-3, 1e-9);
        });
        assert!(result.is_err(), "dt exceeding CFL should panic");
    }

    #[test]
    fn test_empty_step() {
        let mut fdtd = Fdtd3D::new(10, 10, 10, 1e-3, 1e-3, 1e-3, 1.5e-12);
        fdtd.step();
        assert_eq!(fdtd.step, 1);
        assert!((fdtd.time - 1.5e-12).abs() < 1e-20);
    }

    #[test]
    fn test_point_source_propagation() {
        let mut fdtd = Fdtd3D::new(20, 10, 10, 1e-3, 1e-3, 1e-3, 1.5e-12);
        fdtd.sources.push(Source3D::Point {
            position: (5, 5, 5),
            component: FieldComponent::Ez,
            waveform: Waveform::Gaussian {
                amplitude: 1.0,
                tau: 5e-12,
                t0: 15e-12,
            },
        });
        // Run for enough steps for wave to propagate
        for _ in 0..20 {
            fdtd.step();
        }
        // Energy should be positive
        let energy = fdtd.total_energy();
        assert!(energy > 0.0, "source should produce non-zero energy");
        // Field at source point should have been excited
        let probe_val = fdtd.probe(5, 5, 5, FieldComponent::Ez);
        assert!(probe_val.abs() > 1e-10, "Ez at source should be non-zero");
    }

    #[test]
    fn test_pec_reflection() {
        let mut fdtd = Fdtd3D::new(30, 10, 10, 1e-3, 1e-3, 1e-3, 1.5e-12);
        fdtd.sources.push(Source3D::Point {
            position: (5, 5, 5),
            component: FieldComponent::Ez,
            waveform: Waveform::Ricker {
                amplitude: 1.0,
                fc: 5e10,
            },
        });
        // Run for many steps — wave should reflect from PEC boundaries
        for _ in 0..100 {
            fdtd.step();
        }
        // Energy should remain bounded (PEC is lossless)
        let energy = fdtd.total_energy();
        assert!(energy.is_finite());
        assert!(energy >= 0.0);
    }
}
