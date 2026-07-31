//! Magnetohydrodynamics (MHD) simulation for astrophysical plasmas.
//!
//! Provides a 2D ideal MHD solver using the finite-volume method with
//! Harten-Lax-van Leer (HLL) approximate Riemann solver, suitable for
//! simulating plasma dynamics in astrophysical contexts such as solar
//! flares, accretion disks, and interstellar medium.

#![allow(clippy::too_many_arguments, clippy::upper_case_acronyms)]
#![allow(non_snake_case)]

use crate::core::types::Scalar;

/// 2D ideal MHD solver using finite-volume method.
///
/// Conserved variables: [ρ, ρv_x, ρv_y, ρv_z, B_x, B_y, B_z, E]
/// where E is total energy density.
#[derive(Debug, Clone)]
pub struct Mhd2D {
    pub nx: usize,
    pub ny: usize,
    pub dx: Scalar,
    pub dy: Scalar,
    pub dt: Scalar,
    pub gamma: Scalar,
    /// Conserved variables: [ρ, ρvx, ρvy, ρvz, Bx, By, Bz, E] at each cell.
    pub U: Vec<Vec<[Scalar; 8]>>,
}

impl Mhd2D {
    /// Create a new 2D MHD solver.
    pub fn new(nx: usize, ny: usize, dx: Scalar, dy: Scalar, dt: Scalar, gamma: Scalar) -> Self {
        assert!(nx > 1 && ny > 1);
        let U = vec![vec![[0.0; 8]; nx]; ny];
        Self {
            nx,
            ny,
            dx,
            dy,
            dt,
            gamma,
            U,
        }
    }

    /// Convert conserved variables to primitive: [ρ, vx, vy, vz, Bx, By, Bz, p]
    fn primitive(&self, u: &[Scalar; 8]) -> [Scalar; 8] {
        let rho = u[0].max(1e-30);
        let vx = u[1] / rho;
        let vy = u[2] / rho;
        let vz = u[3] / rho;
        let bx = u[4];
        let by = u[5];
        let bz = u[6];
        let kin = 0.5 * rho * (vx * vx + vy * vy + vz * vz);
        let mag = 0.5 * (bx * bx + by * by + bz * bz);
        let p = (self.gamma - 1.0) * (u[7] - kin - mag);
        [rho, vx, vy, vz, bx, by, bz, p.max(1e-30)]
    }

    /// Compute the x-direction flux from primitive variables.
    fn flux_x(&self, w: &[Scalar; 8]) -> [Scalar; 8] {
        let [rho, vx, vy, vz, bx, by, bz, p] = *w;
        let ptot = p + 0.5 * (bx * bx + by * by + bz * bz);
        [
            rho * vx,
            rho * vx * vx + ptot - bx * bx,
            rho * vx * vy - bx * by,
            rho * vx * vz - bx * bz,
            0.0, // Bx flux = 0 (divergence-free constraint)
            vx * by - vy * bx,
            vx * bz - vz * bx,
            (u_energy(rho, vx, vy, vz, p) + ptot) * vx - bx * (vx * bx + vy * by + vz * bz),
        ]
    }

    /// Compute the y-direction flux from primitive variables.
    fn flux_y(&self, w: &[Scalar; 8]) -> [Scalar; 8] {
        let [rho, vx, vy, vz, bx, by, bz, p] = *w;
        let ptot = p + 0.5 * (bx * bx + by * by + bz * bz);
        [
            rho * vy,
            rho * vx * vy - by * bx,
            rho * vy * vy + ptot - by * by,
            rho * vy * vz - by * bz,
            vy * bx - vx * by,
            0.0, // By flux = 0
            vy * bz - vz * by,
            (u_energy(rho, vx, vy, vz, p) + ptot) * vy - by * (vx * bx + vy * by + vz * bz),
        ]
    }

    /// Fast magnetosonic speed for a given primitive state.
    fn fast_speed(&self, w: &[Scalar; 8]) -> Scalar {
        let [rho, _vx, _vy, _vz, bx, by, bz, p] = *w;
        let cs2 = self.gamma * p / rho;
        let ca2 = (bx * bx + by * by + bz * bz) / rho;
        ((cs2 + ca2 + ((cs2 + ca2).powi(2) - 4.0 * cs2 * (bx * bx) / (rho * rho)).sqrt()) / 2.0)
            .sqrt()
    }

    /// HLL approximate Riemann solver flux in x-direction.
    fn hll_flux_x(&self, ul: &[Scalar; 8], ur: &[Scalar; 8]) -> [Scalar; 8] {
        let wl = self.primitive(ul);
        let wr = self.primitive(ur);
        let sl = self.fast_speed(&wl);
        let sr = self.fast_speed(&wr);
        let fl = self.flux_x(&wl);
        let fr = self.flux_x(&wr);

        let mut hll = [0.0; 8];
        for i in 0..8 {
            hll[i] = if sl >= 0.0 {
                fl[i]
            } else if sr <= 0.0 {
                fr[i]
            } else {
                (sr * fl[i] - sl * fr[i] + sl * sr * (ur[i] - ul[i])) / (sr - sl)
            };
        }
        hll
    }

    /// Perform one MHD time step using the HLL scheme.
    pub fn step(&mut self) -> Result<(), String> {
        let (nx, ny) = (self.nx, self.ny);
        let (dx, dy, dt) = (self.dx, self.dy, self.dt);

        let mut U_new = self.U.clone();

        // x-direction fluxes — each row writes disjoint U_new cells and only
        // reads the immutable self.U, so rows run on rayon.
        use rayon::prelude::*;
        U_new.par_iter_mut().enumerate().for_each(|(j, row)| {
            for i in 1..nx - 1 {
                let flux = self.hll_flux_x(&self.U[j][i - 1], &self.U[j][i + 1]);
                for k in 0..8 {
                    row[i][k] -=
                        dt / dx * (flux[k] - self.flux_x(&self.primitive(&self.U[j][i]))[k]);
                }
            }
        });

        // y-direction fluxes — interior rows only.
        U_new.par_iter_mut().enumerate().for_each(|(j, row)| {
            if j == 0 || j == ny - 1 {
                return;
            }
            for i in 0..nx {
                let flux = self.hll_flux_x(&self.U[j - 1][i], &self.U[j + 1][i]);
                // Use hll_flux_x as proxy for y-flux (simplified)
                // In production, replace with proper y-direction HLL
                for k in 0..8 {
                    row[i][k] -=
                        dt / dy * (flux[k] - self.flux_y(&self.primitive(&self.U[j][i]))[k]);
                }
            }
        });

        self.U = U_new;
        Ok(())
    }

    /// Compute the Alfvén speed at each cell.
    pub fn alfven_speed(&self) -> Vec<Vec<Scalar>> {
        let mut va = vec![vec![0.0; self.nx]; self.ny];
        for j in 0..self.ny {
            for i in 0..self.nx {
                let w = self.primitive(&self.U[j][i]);
                let rho = w[0];
                if rho > 1e-30 {
                    let b2 = w[4] * w[4] + w[5] * w[5] + w[6] * w[6];
                    va[j][i] = (b2 / rho).sqrt();
                }
            }
        }
        va
    }

    /// Compute the plasma beta (ratio of thermal to magnetic pressure).
    pub fn plasma_beta(&self) -> Vec<Vec<Scalar>> {
        let mut beta = vec![vec![0.0; self.nx]; self.ny];
        for j in 0..self.ny {
            for i in 0..self.nx {
                let w = self.primitive(&self.U[j][i]);
                let p = w[7];
                let b2 = w[4] * w[4] + w[5] * w[5] + w[6] * w[6];
                if b2 > 1e-30 {
                    beta[j][i] = 2.0 * p / b2;
                } else {
                    beta[j][i] = Scalar::INFINITY;
                }
            }
        }
        beta
    }

    /// Compute total kinetic energy in the domain.
    pub fn total_kinetic_energy(&self) -> Scalar {
        let mut e = 0.0;
        for j in 0..self.ny {
            for i in 0..self.nx {
                let w = self.primitive(&self.U[j][i]);
                e += 0.5 * w[0] * (w[1].powi(2) + w[2].powi(2) + w[3].powi(2));
            }
        }
        e * self.dx * self.dy
    }

    /// Compute total magnetic energy in the domain.
    pub fn total_magnetic_energy(&self) -> Scalar {
        let mut e = 0.0;
        for j in 0..self.ny {
            for i in 0..self.nx {
                let w = self.primitive(&self.U[j][i]);
                e += 0.5 * (w[4].powi(2) + w[5].powi(2) + w[6].powi(2));
            }
        }
        e * self.dx * self.dy
    }
}

/// Helper: total energy density from primitive variables.
fn u_energy(rho: Scalar, vx: Scalar, vy: Scalar, vz: Scalar, p: Scalar) -> Scalar {
    0.5 * rho * (vx * vx + vy * vy + vz * vz) + p / (1.4 - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mhd_creation() {
        let mhd = Mhd2D::new(10, 10, 0.1, 0.1, 0.001, 5.0 / 3.0);
        assert_eq!(mhd.U.len(), 10);
        assert_eq!(mhd.U[0].len(), 10);
    }

    #[test]
    fn test_primitive_conversion() {
        let mhd = Mhd2D::new(4, 4, 0.1, 0.1, 0.001, 5.0 / 3.0);
        // Uniform density, zero velocity, zero B-field
        let u: [Scalar; 8] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0 / (5.0 / 3.0 - 1.0)];
        let w = mhd.primitive(&u);
        assert!((w[0] - 1.0).abs() < 1e-10);
        assert!((w[7] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_step_runs() {
        let mut mhd = Mhd2D::new(8, 8, 0.1, 0.1, 0.0005, 5.0 / 3.0);
        // Set non-trivial initial condition
        for j in 0..8 {
            for i in 0..8 {
                let rho = 1.0 + 0.1 * (i as Scalar).sin() * (j as Scalar).cos();
                let e_int = rho / (5.0 / 3.0 - 1.0);
                mhd.U[j][i] = [rho, 0.0, 0.0, 0.0, 0.1, 0.0, 0.0, e_int];
            }
        }
        mhd.step().unwrap();
        // Energy should be finite
        assert!(mhd.total_kinetic_energy().is_finite());
        assert!(mhd.total_magnetic_energy().is_finite());
    }

    #[test]
    fn test_alfven_speed() {
        let mut mhd = Mhd2D::new(4, 4, 0.1, 0.1, 0.001, 5.0 / 3.0);
        for j in 0..4 {
            for i in 0..4 {
                mhd.U[j][i] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.5];
            }
        }
        let va = mhd.alfven_speed();
        // B²/ρ = 1, so v_A = 1
        assert!((va[0][0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_step_parallel_matches_serial_reference() {
        // step() runs on rayon (per-row); verify against the original serial
        // flux-sweep order on a non-trivial grid.
        let (nx, ny) = (20usize, 20usize);
        let (dx, dy, dt) = (0.05, 0.05, 0.0002);
        let gamma = 5.0 / 3.0;
        let mut mhd = Mhd2D::new(nx, ny, dx, dy, dt, gamma);
        for j in 0..ny {
            for i in 0..nx {
                let rho = 1.0 + 0.1 * (i as Scalar).sin() * (j as Scalar).cos();
                let e_int = rho / (gamma - 1.0);
                mhd.U[j][i] = [rho, 0.05, -0.02, 0.0, 0.1, 0.0, 0.0, e_int];
            }
        }
        let u0 = mhd.U.clone();
        mhd.step().unwrap();

        // Serial reference: read the pre-step snapshot u0 (immutable), write
        // into a fresh buffer, exactly like the original U_new implementation.
        let mut u_ref = u0.clone();
        // x-direction
        for j in 0..ny {
            for i in 1..nx - 1 {
                let flux = mhd.hll_flux_x(&u0[j][i - 1], &u0[j][i + 1]);
                for k in 0..8 {
                    u_ref[j][i][k] -=
                        dt / dx * (flux[k] - mhd.flux_x(&mhd.primitive(&u0[j][i]))[k]);
                }
            }
        }
        // y-direction
        for j in 1..ny - 1 {
            for i in 0..nx {
                let flux = mhd.hll_flux_x(&u0[j - 1][i], &u0[j + 1][i]);
                for k in 0..8 {
                    u_ref[j][i][k] -=
                        dt / dy * (flux[k] - mhd.flux_y(&mhd.primitive(&u0[j][i]))[k]);
                }
            }
        }

        for j in 0..ny {
            for i in 0..nx {
                for k in 0..8 {
                    assert!(
                        (mhd.U[j][i][k] - u_ref[j][i][k]).abs() < 1e-12,
                        "mismatch [{j}][{i}][{k}]: {} vs {}",
                        mhd.U[j][i][k],
                        u_ref[j][i][k]
                    );
                }
            }
        }
    }
}
