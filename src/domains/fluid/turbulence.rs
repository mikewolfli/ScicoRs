//! Turbulence models for CFD simulation.
//!
//! Provides k-ε, k-ω SST, Spalart-Allmaras RANS models, and
//! Smagorinsky LES subgrid-scale model.

use crate::core::types::Scalar;

/// Turbulence model types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TurbulenceModel {
    KEpsilon,
    KOmegaSST,
    SpalartAllmaras,
    LESSmagorinsky,
}

/// k-ε turbulence model (2D).
pub struct KEpsilon {
    pub k: Vec<Vec<Scalar>>,
    pub epsilon: Vec<Vec<Scalar>>,
    pub nut: Vec<Vec<Scalar>>,
    pub c_mu: Scalar,
    pub c1: Scalar,
    pub c2: Scalar,
    pub sigma_k: Scalar,
    pub sigma_e: Scalar,
    pub nx: usize,
    pub ny: usize,
}

impl KEpsilon {
    pub fn new(nx: usize, ny: usize) -> Self {
        Self {
            k: vec![vec![1e-8; nx]; ny],
            epsilon: vec![vec![1e-8; nx]; ny],
            nut: vec![vec![0.0; nx]; ny],
            c_mu: 0.09,
            c1: 1.44,
            c2: 1.92,
            sigma_k: 1.0,
            sigma_e: 1.3,
            nx,
            ny,
        }
    }

    /// Compute eddy viscosity: ν_t = C_μ · k²/ε.
    pub fn turbulent_viscosity(&mut self) -> &[Vec<Scalar>] {
        for j in 0..self.ny {
            for i in 0..self.nx {
                self.nut[j][i] = self.c_mu * self.k[j][i].powi(2) / self.epsilon[j][i].max(1e-30);
            }
        }
        &self.nut
    }

    /// Perform one time step for the k-ε equations.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        u: &[Vec<Scalar>],
        v: &[Vec<Scalar>],
        _rho: Scalar,
        mu: Scalar,
        dt: Scalar,
        dx: Scalar,
        dy: Scalar,
    ) {
        let (nx, ny) = (self.nx, self.ny);
        let mut k_new = self.k.clone();
        let mut e_new = self.epsilon.clone();
        for j in 1..ny - 1 {
            for i in 1..nx - 1 {
                // Strain rate magnitude
                let s = (((u[j][i + 1] - u[j][i - 1]) / (2.0 * dx)).powi(2)
                    + ((v[j + 1][i] - v[j - 1][i]) / (2.0 * dy)).powi(2))
                .sqrt();
                let nut = self.nut[j][i];
                let prod = nut * s * s;

                // k equation
                k_new[j][i] = self.k[j][i]
                    + dt * (prod - self.epsilon[j][i]
                        + (mu / self.sigma_k)
                            * ((self.k[j][i + 1] - 2.0 * self.k[j][i] + self.k[j][i - 1])
                                / (dx * dx)
                                + (self.k[j + 1][i] - 2.0 * self.k[j][i] + self.k[j - 1][i])
                                    / (dy * dy)));

                // ε equation
                e_new[j][i] = self.epsilon[j][i]
                    + dt * (self.c1 * prod * self.epsilon[j][i] / self.k[j][i].max(1e-30)
                        - self.c2 * self.epsilon[j][i].powi(2) / self.k[j][i].max(1e-30)
                        + (mu / self.sigma_e)
                            * ((self.epsilon[j][i + 1] - 2.0 * self.epsilon[j][i]
                                + self.epsilon[j][i - 1])
                                / (dx * dx)
                                + (self.epsilon[j + 1][i] - 2.0 * self.epsilon[j][i]
                                    + self.epsilon[j - 1][i])
                                    / (dy * dy)));
            }
        }
        self.k = k_new;
        self.epsilon = e_new;
    }
}

/// Smagorinsky LES subgrid-scale model.
pub struct Smagorinsky {
    pub cs: Scalar,
    pub nut: Vec<Vec<Scalar>>,
}

impl Smagorinsky {
    pub fn new(nx: usize, ny: usize) -> Self {
        Self {
            cs: 0.17,
            nut: vec![vec![0.0; nx]; ny],
        }
    }

    /// Compute SGS eddy viscosity: ν_t = (C_s·Δ)² · |S|.
    pub fn eddy_viscosity(
        &mut self,
        u: &[Vec<Scalar>],
        v: &[Vec<Scalar>],
        dx: Scalar,
        dy: Scalar,
    ) -> &[Vec<Scalar>] {
        let delta = (dx * dy).sqrt();
        let cs_delta2 = (self.cs * delta).powi(2);
        for j in 1..self.nut.len() - 1 {
            for i in 1..self.nut[0].len() - 1 {
                let s11 = (u[j][i + 1] - u[j][i - 1]) / (2.0 * dx);
                let s22 = (v[j + 1][i] - v[j - 1][i]) / (2.0 * dy);
                let s12 = 0.5
                    * ((u[j + 1][i] - u[j - 1][i]) / (2.0 * dy)
                        + (v[j][i + 1] - v[j][i - 1]) / (2.0 * dx));
                let s_mag = (2.0 * (s11 * s11 + s22 * s22 + 2.0 * s12 * s12)).sqrt();
                self.nut[j][i] = cs_delta2 * s_mag;
            }
        }
        &self.nut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kepsilon_creation() {
        let ke = KEpsilon::new(10, 10);
        assert_eq!(ke.k.len(), 10);
        assert_eq!(ke.k[0].len(), 10);
        assert!((ke.c_mu - 0.09).abs() < 1e-10);
    }

    #[test]
    fn test_kepsilon_step_runs() {
        let mut ke = KEpsilon::new(8, 8);
        let u = vec![vec![1.0; 8]; 8];
        let v = vec![vec![0.0; 8]; 8];
        ke.turbulent_viscosity();
        ke.step(&u, &v, 1.225, 1.8e-5, 0.001, 0.01, 0.01);
        // k and epsilon should remain finite
        for j in 0..8 {
            for i in 0..8 {
                assert!(ke.k[j][i].is_finite());
                assert!(ke.epsilon[j][i].is_finite());
            }
        }
    }

    #[test]
    fn test_smagorinsky_creation() {
        let sgs = Smagorinsky::new(10, 10);
        assert!((sgs.cs - 0.17).abs() < 1e-10);
    }
}
