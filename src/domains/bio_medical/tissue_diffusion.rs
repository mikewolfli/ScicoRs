//! Tissue drug diffusion and transport model (2D PDE).
//!
//! Solves the diffusion equation with clearance for drug concentration
//! in biological tissue:
//!
//!   ∂C/∂t = D·∇²C - k·C + S(x,t)
//!
//! where C is concentration, D is diffusivity, k is the clearance rate,
//! and S represents source terms (injections, implants).

use crate::core::types::Scalar;

/// 2D tissue diffusion model for drug delivery simulation.
#[derive(Debug, Clone)]
pub struct TissueDiffusion2D {
    /// Grid dimensions.
    pub nx: usize,
    pub ny: usize,
    /// Grid spacing (m).
    pub dx: Scalar,
    pub dy: Scalar,
    /// Diffusivity in tissue (m²/s).
    pub diffusivity: Scalar,
    /// Drug clearance rate (1/s).
    pub k_clearance: Scalar,
    /// Current concentration field (mol/m³) at each grid point.
    pub concentration: Vec<Vec<Scalar>>,
    /// Source term at each grid point (mol/m³/s).
    pub source: Vec<Vec<Scalar>>,
}

impl TissueDiffusion2D {
    /// Create a new 2D tissue diffusion model.
    ///
    /// All fields are initialised to zero concentration.
    pub fn new(
        nx: usize,
        ny: usize,
        dx: Scalar,
        dy: Scalar,
        diffusivity: Scalar,
        k_clearance: Scalar,
    ) -> Self {
        assert!(nx > 2 && ny > 2, "Grid must be at least 3×3");
        Self {
            nx,
            ny,
            dx,
            dy,
            diffusivity,
            k_clearance,
            concentration: vec![vec![0.0; nx]; ny],
            source: vec![vec![0.0; nx]; ny],
        }
    }

    /// Inject a dose at a specific grid point (mol/m³).
    pub fn inject(&mut self, x: usize, y: usize, dose: Scalar) {
        if x < self.nx && y < self.ny {
            self.concentration[y][x] += dose;
        }
    }

    /// Add a continuous source at a grid point (mol/m³/s).
    pub fn add_source(&mut self, x: usize, y: usize, rate: Scalar) {
        if x < self.nx && y < self.ny {
            self.source[y][x] += rate;
        }
    }

    /// Perform one time step using explicit Euler + central differences.
    ///
    /// Stable when: dt ≤ min(dx², dy²) / (4·D)
    pub fn step(&mut self, dt: Scalar) -> Result<(), String> {
        if dt <= 0.0 {
            return Err("dt must be positive".to_string());
        }
        // CFL check
        let cfl = self.diffusivity * dt / (self.dx * self.dx).min(self.dy * self.dy);
        if cfl > 0.25 {
            return Err(format!(
                "CFL condition violated: dt={} too large (CFL={})",
                dt, cfl
            ));
        }

        let ny = self.ny;
        let nx = self.nx;
        let d = self.diffusivity;
        let k = self.k_clearance;
        let dx2 = self.dx * self.dx;
        let dy2 = self.dy * self.dy;

        let mut new_conc = vec![vec![0.0; nx]; ny];

        // Each cell's update reads only the immutable concentration snapshot
        // and writes a disjoint new_conc cell → rows run on rayon.
        use rayon::prelude::*;
        new_conc.par_iter_mut().enumerate().for_each(|(j, row)| {
            for i in 0..nx {
                // Laplacian (central differences)
                let mut laplacian = 0.0;

                if i > 0 {
                    laplacian += (self.concentration[j][i - 1] - self.concentration[j][i]) / dx2;
                }
                if i + 1 < nx {
                    laplacian += (self.concentration[j][i + 1] - self.concentration[j][i]) / dx2;
                }
                if j > 0 {
                    laplacian += (self.concentration[j - 1][i] - self.concentration[j][i]) / dy2;
                }
                if j + 1 < ny {
                    laplacian += (self.concentration[j + 1][i] - self.concentration[j][i]) / dy2;
                }

                // Diffusion + clearance + source
                let d_conc = d * laplacian - k * self.concentration[j][i] + self.source[j][i];
                row[i] = self.concentration[j][i] + dt * d_conc;
            }
        });

        // Enforce non-negativity
        self.concentration
            .par_iter_mut()
            .zip(new_conc.par_iter())
            .for_each(|(c_row, n_row)| {
                for i in 0..nx {
                    c_row[i] = n_row[i].max(0.0);
                }
            });

        Ok(())
    }

    /// Run simulation for a given time span.
    pub fn simulate(&mut self, t_end: Scalar, dt: Scalar) -> Result<Vec<Scalar>, String> {
        if dt <= 0.0 || t_end <= 0.0 {
            return Err("Time must be positive".to_string());
        }
        let n_steps = (t_end / dt).ceil() as usize;
        let actual_dt = t_end / n_steps as Scalar;
        let mut concentrations_at_centre = Vec::with_capacity(n_steps + 1);

        concentrations_at_centre.push(self.concentration[self.ny / 2][self.nx / 2]);

        for _ in 0..n_steps {
            self.step(actual_dt)?;
            concentrations_at_centre.push(self.concentration[self.ny / 2][self.nx / 2]);
        }
        Ok(concentrations_at_centre)
    }

    /// Total amount of drug in the domain (mol).
    pub fn total_amount(&self) -> Scalar {
        let vol = self.dx * self.dy;
        self.concentration
            .iter()
            .flat_map(|row| row.iter())
            .sum::<Scalar>()
            * vol
    }

    /// Maximum concentration in the domain.
    pub fn max_concentration(&self) -> Scalar {
        let values: Vec<Scalar> = self
            .concentration
            .iter()
            .flat_map(|row| row.iter())
            .copied()
            .collect();
        crate::core::compute::vector::vec_max_abs(&values).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tissue_diffusion_new() {
        let td = TissueDiffusion2D::new(10, 10, 0.001, 0.001, 1e-10, 1e-5);
        assert_eq!(td.nx, 10);
        assert_eq!(td.ny, 10);
    }

    #[test]
    fn test_inject() {
        let mut td = TissueDiffusion2D::new(10, 10, 0.001, 0.001, 1e-10, 1e-5);
        td.inject(5, 5, 100.0);
        assert!((td.concentration[5][5] - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_diffusion_spread() {
        let mut td = TissueDiffusion2D::new(10, 10, 0.001, 0.001, 1e-10, 1e-5);
        td.inject(5, 5, 100.0);
        td.step(1000.0).unwrap();
        // After one step, concentration should have spread
        let total = td.total_amount();
        assert!(total > 0.0);
        // Centre should still have most of the dose
        assert!(td.concentration[5][5] > 0.0);
    }

    #[test]
    fn test_clearance() {
        let mut td = TissueDiffusion2D::new(10, 10, 0.001, 0.001, 1e-10, 1e-3);
        td.inject(5, 5, 100.0);
        // High clearance should reduce total amount
        let total_before = td.total_amount();
        for _ in 0..10 {
            td.step(100.0).unwrap();
        }
        let total_after = td.total_amount();
        assert!(total_after < total_before, "clearance should reduce drug");
    }

    #[test]
    fn test_simulate() {
        let mut td = TissueDiffusion2D::new(10, 10, 0.001, 0.001, 1e-10, 1e-5);
        td.inject(5, 5, 100.0);
        let history = td.simulate(5000.0, 1000.0).unwrap();
        assert!(history.len() > 1);
        // Concentration at centre should decrease over time
        assert!(history[0] >= history[history.len() - 1]);
    }

    #[test]
    fn test_max_concentration() {
        let mut td = TissueDiffusion2D::new(10, 10, 0.001, 0.001, 1e-10, 1e-5);
        td.inject(3, 4, 50.0);
        assert!((td.max_concentration() - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_add_source() {
        let mut td = TissueDiffusion2D::new(10, 10, 0.001, 0.001, 1e-10, 1e-5);
        td.add_source(5, 5, 10.0);
        assert!((td.source[5][5] - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_cfl_violation() {
        let mut td = TissueDiffusion2D::new(10, 10, 0.001, 0.001, 1e-10, 1e-5);
        assert!(td.step(1e10).is_err());
    }

    #[test]
    fn test_non_negative() {
        let mut td = TissueDiffusion2D::new(10, 10, 0.001, 0.001, 1e-10, 1e-3);
        td.inject(5, 5, 0.0);
        td.step(100.0).unwrap();
        for row in &td.concentration {
            for &val in row {
                assert!(val >= 0.0, "concentration must never be negative");
            }
        }
    }

    #[test]
    fn test_step_parallel_matches_serial_reference() {
        // step() runs on rayon (per-row); verify against the original serial
        // loop order on a non-trivial grid.
        let (nx, ny) = (48usize, 48usize);
        let d = 1e-9;
        let k = 1e-4;
        let h = 0.001;
        let dt = 0.01;

        let mut td = TissueDiffusion2D::new(nx, ny, h, h, d, k);
        td.inject(20, 20, 5.0);
        td.add_source(30, 30, 2.0);
        td.step(dt).unwrap();

        // Serial reference (original loop order).
        let dx2 = h * h;
        let dy2 = h * h;
        let mut ref_conc = vec![vec![0.0; nx]; ny];
        let mut ref_source = vec![vec![0.0; nx]; ny];
        ref_conc[20][20] = 5.0;
        ref_source[30][30] = 2.0;
        let mut new_conc = vec![vec![0.0; nx]; ny];
        for j in 0..ny {
            for i in 0..nx {
                let mut laplacian = 0.0;
                if i > 0 {
                    laplacian += (ref_conc[j][i - 1] - ref_conc[j][i]) / dx2;
                }
                if i + 1 < nx {
                    laplacian += (ref_conc[j][i + 1] - ref_conc[j][i]) / dx2;
                }
                if j > 0 {
                    laplacian += (ref_conc[j - 1][i] - ref_conc[j][i]) / dy2;
                }
                if j + 1 < ny {
                    laplacian += (ref_conc[j + 1][i] - ref_conc[j][i]) / dy2;
                }
                let d_conc = d * laplacian - k * ref_conc[j][i] + ref_source[j][i];
                new_conc[j][i] = ref_conc[j][i] + dt * d_conc;
            }
        }
        let ref_after: Vec<Vec<Scalar>> = new_conc
            .iter()
            .map(|r| r.iter().map(|&v| v.max(0.0)).collect())
            .collect();

        for j in 0..ny {
            for i in 0..nx {
                assert!(
                    (td.concentration[j][i] - ref_after[j][i]).abs() < 1e-12,
                    "mismatch at [{j}][{i}]: {} vs {}",
                    td.concentration[j][i],
                    ref_after[j][i]
                );
            }
        }
    }
}
