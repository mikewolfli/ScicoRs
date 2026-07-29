//! 2D compressible Navier-Stokes solver (density-based, finite-volume).
#![allow(clippy::too_many_arguments)]
#![allow(non_snake_case)]
//!
//! Solves the 2D compressible NS equations using a cell-centred finite-volume
//! formulation with Roe's approximate Riemann solver for convective fluxes
//! and central differences for viscous fluxes.

use crate::core::types::Scalar;

/// Conserved variables: [ρ, ρu, ρv, ρE] for 2D compressible flow.
pub type Conserved2D = [Scalar; 4];

/// 2D compressible Navier-Stokes solver (finite-volume, Roe scheme).
pub struct CompressibleNS2D {
    pub nx: usize,
    pub ny: usize,
    pub dx: Scalar,
    pub dy: Scalar,
    pub dt: Scalar,
    pub gamma: Scalar,
    pub pr: Scalar,
    pub mu: Scalar,
    /// Conserved variables at each cell centre.
    pub Q: Vec<Vec<Conserved2D>>,
    /// Wall boundary flag: true = no-slip adiabatic wall.
    pub wall_bc: bool,
}

impl CompressibleNS2D {
    pub fn new(
        nx: usize,
        ny: usize,
        dx: Scalar,
        dy: Scalar,
        dt: Scalar,
        gamma: Scalar,
        pr: Scalar,
        mu: Scalar,
    ) -> Self {
        assert!(nx > 2 && ny > 2, "minimum 3×3 grid required");
        let Q = vec![vec![[0.0; 4]; nx]; ny];
        Self {
            nx,
            ny,
            dx,
            dy,
            dt,
            gamma,
            pr,
            mu,
            Q,
            wall_bc: false,
        }
    }

    /// Primitive variables: [ρ, u, v, p, T] from conserved [ρ, ρu, ρv, ρE].
    pub fn primitive(&self, q: &Conserved2D) -> [Scalar; 5] {
        let rho = q[0].max(1e-30);
        let u = q[1] / rho;
        let v = q[2] / rho;
        let e = q[3] / rho;
        let ke = 0.5 * (u * u + v * v);
        let p = (self.gamma - 1.0) * rho * (e - ke);
        let p = p.max(1e-30);
        let T = p / (rho * (self.gamma - 1.0));
        [rho, u, v, p, T]
    }

    /// Flux vector in x-direction from primitive variables.
    fn flux_x(&self, w: &[Scalar; 5]) -> Conserved2D {
        let [rho, u, v, p, _T] = *w;
        let e = p / (rho * (self.gamma - 1.0)) + 0.5 * (u * u + v * v);
        [rho * u, rho * u * u + p, rho * u * v, rho * u * e + u * p]
    }

    /// Flux vector in y-direction.
    fn flux_y(&self, w: &[Scalar; 5]) -> Conserved2D {
        let [rho, u, v, p, _T] = *w;
        let e = p / (rho * (self.gamma - 1.0)) + 0.5 * (u * u + v * v);
        [rho * v, rho * u * v, rho * v * v + p, rho * v * e + v * p]
    }

    /// Speed of sound.
    fn sound_speed(&self, w: &[Scalar; 5]) -> Scalar {
        (self.gamma * w[3] / w[0].max(1e-30)).sqrt()
    }

    /// Roe average between left and right states.
    fn roe_average(&self, ql: &Conserved2D, qr: &Conserved2D) -> [Scalar; 5] {
        let wl = self.primitive(ql);
        let wr = self.primitive(qr);
        let rho_l = wl[0];
        let rho_r = wr[0];
        let sqrt_rho_l = rho_l.sqrt();
        let sqrt_rho_r = rho_r.sqrt();
        let sum = sqrt_rho_l + sqrt_rho_r;
        if sum < 1e-30 {
            return [0.0; 5];
        }
        let u = (sqrt_rho_l * wl[1] + sqrt_rho_r * wr[1]) / sum;
        let v = (sqrt_rho_l * wl[2] + sqrt_rho_r * wr[2]) / sum;
        let h_l = (ql[3] + wl[3]) / rho_l;
        let h_r = (qr[3] + wr[3]) / rho_r;
        let h = (sqrt_rho_l * h_l + sqrt_rho_r * h_r) / sum;
        let c = ((self.gamma - 1.0) * (h - 0.5 * (u * u + v * v)))
            .sqrt()
            .max(1e-30);
        [sum, u, v, h, c]
    }

    /// Roe flux in x-direction.
    fn roe_flux_x(&self, ql: &Conserved2D, qr: &Conserved2D) -> Conserved2D {
        let wl = self.primitive(ql);
        let wr = self.primitive(qr);
        let fl = self.flux_x(&wl);
        let fr = self.flux_x(&wr);
        let [rho_avg, u, v, h, c] = self.roe_average(ql, qr);

        // Difference in conserved variables
        let dq = [qr[0] - ql[0], qr[1] - ql[1], qr[2] - ql[2], qr[3] - ql[3]];

        // Roe wave speeds
        let gm1 = self.gamma - 1.0;
        let q2 = u * u + v * v;
        let dq_rho = dq[0];
        let dq_u = dq[1];
        let dq_v = dq[2];
        let dq_e = dq[3];

        let _dq_ke = dq_u * u + dq_v * v - u * dq_rho;
        let dq_p = gm1 * (dq_e - u * dq_u - v * dq_v + 0.5 * q2 * dq_rho);

        // Wave speeds
        let lam1 = u.abs();
        let lam2 = (u + c).abs();
        let lam3 = (u - c).abs();

        // Jump in characteristic variables
        let lambda_diff = (u + c).abs() - (u - c).abs();
        let alpha2 = gm1 / (c * c) * (dq_p - rho_avg * c * lambda_diff);
        let alpha1 = gm1 / (c * c) * (dq_p);
        let alphau = dq_rho - dq_p / (c * c);

        let mut flux = [0.0; 4];
        for i in 0..4 {
            flux[i] = 0.5 * (fl[i] + fr[i])
                - 0.5
                    * (lam3 * alpha1 * [1.0, u - c, v, h - u * c][i]
                        + lam1 * alphau * [1.0, u, v, 0.5 * q2][i]
                        + lam2 * alpha2 * [1.0, u + c, v, h + u * c][i]);
        }
        flux
    }

    /// Compute viscous fluxes (central difference).
    fn viscous_flux_x(&self, j: usize, i: usize) -> Conserved2D {
        let dx = self.dx;
        let mu = self.mu;
        let qc = &self.Q[j][i];
        let qe = &self.Q[j][(i + 1).min(self.nx - 1)];
        let qw = &self.Q[j][i.max(1) - 1];

        let wc = self.primitive(qc);
        let we = self.primitive(qe);
        let ww = self.primitive(qw);

        let du_dx = (we[1] - ww[1]) / (2.0 * dx);
        let dv_dx = (we[2] - ww[2]) / (2.0 * dx);
        let dT_dx = (we[4] - ww[4]) / (2.0 * dx);
        let div = du_dx; // + dv_dy (not available in 1D split)

        let tau_xx = 2.0 * mu * (du_dx - div / 3.0);
        let tau_xy = mu * dv_dx;
        let k = mu * self.gamma / (self.pr * (self.gamma - 1.0));
        let qx = -k * dT_dx;

        [
            0.0,
            tau_xx,
            tau_xy,
            u_term(wc[1], tau_xx, wc[2], tau_xy, qx),
        ]
    }

    /// Perform one time step using Roe's scheme.
    pub fn step(&mut self) -> Result<(), String> {
        let (nx, ny) = (self.nx, self.ny);
        let (dx, dy, dt) = (self.dx, self.dy, self.dt);
        let mut Q_new = self.Q.clone();

        // Convective fluxes (x-direction)
        for j in 0..ny {
            for i in 1..nx - 1 {
                let flux = self.roe_flux_x(&self.Q[j][i - 1], &self.Q[j][i + 1]);
                for k in 0..4 {
                    Q_new[j][i][k] -=
                        dt / dx * (flux[k] - self.flux_x(&self.primitive(&self.Q[j][i]))[k]);
                }
            }
        }

        // Convective fluxes (y-direction)
        for j in 1..ny - 1 {
            for i in 0..nx {
                let flux = self.roe_flux_x(&self.Q[j - 1][i], &self.Q[j + 1][i]);
                for k in 0..4 {
                    Q_new[j][i][k] -=
                        dt / dy * (flux[k] - self.flux_y(&self.primitive(&self.Q[j][i]))[k]);
                }
            }
        }

        // Viscous fluxes (simplified)
        if self.mu > 0.0 {
            for j in 1..ny - 1 {
                for i in 1..nx - 1 {
                    let vf = self.viscous_flux_x(j, i);
                    for k in 0..4 {
                        Q_new[j][i][k] += dt / (dx * dx) * vf[k];
                    }
                }
            }
        }

        self.Q = Q_new;
        Ok(())
    }

    /// Set freestream initial condition.
    pub fn set_freestream(&mut self, rho_inf: Scalar, u_inf: Scalar, v_inf: Scalar, p_inf: Scalar) {
        for j in 0..self.ny {
            for i in 0..self.nx {
                let e =
                    p_inf / ((self.gamma - 1.0) * rho_inf) + 0.5 * (u_inf * u_inf + v_inf * v_inf);
                self.Q[j][i] = [rho_inf, rho_inf * u_inf, rho_inf * v_inf, rho_inf * e];
            }
        }
    }

    /// Compute Mach number field.
    pub fn mach_number(&self) -> Vec<Vec<Scalar>> {
        let mut ma = vec![vec![0.0; self.nx]; self.ny];
        for j in 0..self.ny {
            for i in 0..self.nx {
                let w = self.primitive(&self.Q[j][i]);
                let v = (w[1] * w[1] + w[2] * w[2]).sqrt();
                let c = self.sound_speed(&w);
                ma[j][i] = if c > 0.0 { v / c } else { 0.0 };
            }
        }
        ma
    }
}

fn u_term(u: Scalar, tau_xx: Scalar, v: Scalar, tau_xy: Scalar, qx: Scalar) -> Scalar {
    u * tau_xx + v * tau_xy - qx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressible_creation() {
        let ns = CompressibleNS2D::new(10, 10, 0.01, 0.01, 1e-5, 1.4, 0.72, 1.8e-5);
        assert_eq!(ns.Q.len(), 10);
        assert_eq!(ns.Q[0].len(), 10);
    }

    #[test]
    fn test_primitive_conversion() {
        let ns = CompressibleNS2D::new(4, 4, 0.01, 0.01, 1e-5, 1.4, 0.72, 1.8e-5);
        let rho = 1.225;
        let u = 100.0;
        let v = 0.0;
        let p = 101325.0;
        let e = p / ((1.4 - 1.0) * rho) + 0.5 * u * u;
        let q = [rho, rho * u, rho * v, rho * e];
        let w = ns.primitive(&q);
        assert!((w[0] - rho).abs() / rho < 1e-6);
        assert!((w[1] - u).abs() / u < 1e-6);
        assert!((w[3] - p).abs() / p < 1e-6);
    }

    #[test]
    fn test_freestream_set() {
        let mut ns = CompressibleNS2D::new(8, 8, 0.01, 0.01, 1e-6, 1.4, 0.72, 0.0);
        ns.set_freestream(1.225, 50.0, 0.0, 101325.0);
        // Verify freestream is set correctly before time stepping
        for j in 0..8 {
            for i in 0..8 {
                let w = ns.primitive(&ns.Q[j][i]);
                assert!((w[0] - 1.225).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_mach_number() {
        let mut ns = CompressibleNS2D::new(6, 6, 0.01, 0.01, 1e-6, 1.4, 0.72, 0.0);
        ns.set_freestream(1.225, 340.0, 0.0, 101325.0);
        let ma = ns.mach_number();
        // M ≈ 340/340 = 1.0
        assert!((ma[3][3] - 1.0).abs() < 0.1);
    }
}
