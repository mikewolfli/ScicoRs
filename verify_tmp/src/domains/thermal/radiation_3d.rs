//! 3D thermal radiation solver using the Discrete Ordinates Method (DOM).
//!
//! Solves the radiative transfer equation (RTE) for participating media
//! with absorption, scattering, and emission on a uniform 3D grid.

#![allow(clippy::too_many_arguments)]
#![allow(clippy::manual_is_multiple_of)]

use crate::core::types::Scalar;
use crate::domains::thermal::physics::SIGMA_SB;

/// Quadrature set for DOM angular discretisation.
#[derive(Debug, Clone)]
pub struct DomQuadrature {
    pub n_dir: usize,
    pub mu: Vec<Scalar>,  // x-direction cosines
    pub eta: Vec<Scalar>, // y-direction cosines
    pub xi: Vec<Scalar>,  // z-direction cosines
    pub weights: Vec<Scalar>,
}

impl DomQuadrature {
    /// Create an S_N-level quadrature set (even N).
    pub fn s_n(n: usize) -> Self {
        assert!(n >= 2 && n % 2 == 0, "S_N requires even N ≥ 2");
        let n_dir = n * (n + 2);
        let mut mu = Vec::with_capacity(n_dir);
        let mut eta = Vec::with_capacity(n_dir);
        let mut xi = Vec::with_capacity(n_dir);
        let mut weights = Vec::with_capacity(n_dir);

        // Level-symmetric quadrature (simplified: equally spaced)
        let step = 2.0 / (n + 1) as Scalar;
        let mut idx = 0;
        for i in 0..n / 2 {
            for j in 0..n / 2 {
                for k in 0..n / 2 {
                    let m = (i as Scalar + 1.0) * step - 1.0;
                    let e = (j as Scalar + 1.0) * step - 1.0;
                    let x = (k as Scalar + 1.0) * step - 1.0;
                    let norm = (m * m + e * e + x * x).sqrt();
                    if norm > 0.0 && norm <= 1.0 {
                        // Octant symmetry: all 8 sign combinations
                        for si in 0..2 {
                            for sj in 0..2 {
                                for sk in 0..2 {
                                    let m_s = if si == 0 { m } else { -m };
                                    let e_s = if sj == 0 { e } else { -e };
                                    let x_s = if sk == 0 { x } else { -x };
                                    mu.push(m_s / norm);
                                    eta.push(e_s / norm);
                                    xi.push(x_s / norm);
                                    weights.push(1.0 / n_dir as Scalar);
                                    idx += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        // If too few directions, pad with evenly distributed ones
        while mu.len() < n_dir {
            let t = idx as Scalar * 0.1;
            mu.push(t.cos());
            eta.push((t * 0.5).cos());
            xi.push((t * 0.3).cos());
            weights.push(1.0 / n_dir as Scalar);
            idx += 1;
        }
        // Truncate if too many
        mu.truncate(n_dir);
        eta.truncate(n_dir);
        xi.truncate(n_dir);
        weights.truncate(n_dir);
        // Normalize weights to 4π
        let w_sum: Scalar = weights.iter().sum();
        if w_sum > 0.0 {
            for w in &mut weights {
                *w *= 4.0 * std::f64::consts::PI / w_sum;
            }
        }
        Self {
            n_dir,
            mu,
            eta,
            xi,
            weights,
        }
    }
}

/// Discrete Ordinates Method (DOM) solver for radiative transfer in a
/// participating 3D medium.
#[derive(Debug, Clone)]
pub struct DomRadiation3D {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub dx: Scalar,
    pub dy: Scalar,
    pub dz: Scalar,
    pub absorption: Scalar, // κ_a (absorption coefficient, m⁻¹)
    pub scattering: Scalar, // σ_s (scattering coefficient, m⁻¹)
    pub temperature: Vec<Vec<Vec<Scalar>>>,
    /// Radiative intensity at each cell and direction: I[k][j][i][d]
    pub intensity: Vec<Vec<Vec<Vec<Scalar>>>>,
    /// Incident radiation G = ∫ I dΩ
    pub incident_radiation: Vec<Vec<Vec<Scalar>>>,
    /// Radiative heat source term (W/m³)
    pub radiative_source: Vec<Vec<Vec<Scalar>>>,
    pub quad: DomQuadrature,
    /// Wall emissivity (assumed uniform for simplicity)
    pub wall_emissivity: Scalar,
}

impl DomRadiation3D {
    /// Create a new DOM solver.
    pub fn new(
        nx: usize,
        ny: usize,
        nz: usize,
        dx: Scalar,
        dy: Scalar,
        dz: Scalar,
        absorption: Scalar,
        scattering: Scalar,
        s_n: usize,
        initial_temp: Scalar,
    ) -> Self {
        assert!(nx > 1 && ny > 1 && nz > 1);
        let quad = DomQuadrature::s_n(s_n);
        let n_dir = quad.n_dir;
        Self {
            nx,
            ny,
            nz,
            dx,
            dy,
            dz,
            absorption,
            scattering,
            temperature: vec![vec![vec![initial_temp; nx]; ny]; nz],
            intensity: vec![vec![vec![vec![0.0; n_dir]; nx]; ny]; nz],
            incident_radiation: vec![vec![vec![0.0; nx]; ny]; nz],
            radiative_source: vec![vec![vec![0.0; nx]; ny]; nz],
            quad,
            wall_emissivity: 0.9,
        }
    }

    /// Perform one DOM iteration (solve RTE along each direction).
    ///
    /// Uses the step (forward-Euler) scheme for spatial discretisation.
    pub fn solve(&mut self) -> Result<(), String> {
        let beta = self.absorption + self.scattering; // Extinction coefficient
        if beta < 1e-30 {
            return Err("Extinction coefficient too small".to_string());
        }
        let omega = if beta > 0.0 {
            self.scattering / beta
        } else {
            0.0
        }; // Albedo

        // Reset incident radiation and source (per-cell independent → rayon).
        use rayon::prelude::*;
        self.incident_radiation
            .par_iter_mut()
            .zip(self.radiative_source.par_iter_mut())
            .for_each(|(inc_plane, src_plane)| {
                for j in 0..self.ny {
                    for i in 0..self.nx {
                        inc_plane[j][i] = 0.0;
                        src_plane[j][i] = 0.0;
                    }
                }
            });

        let n_dir = self.quad.n_dir;

        // Sweep each direction
        for d in 0..n_dir {
            let mu_d = self.quad.mu[d];
            let eta_d = self.quad.eta[d];
            let xi_d = self.quad.xi[d];
            let w_d = self.quad.weights[d];

            // Determine sweep direction based on sign of direction cosines
            let (k_start, k_end, k_step) = if xi_d >= 0.0 {
                (0, self.nz, 1)
            } else {
                (self.nz - 1, usize::MAX, usize::MAX.wrapping_sub(1))
            };
            let (j_start, j_end, j_step) = if eta_d >= 0.0 {
                (0, self.ny, 1)
            } else {
                (self.ny - 1, usize::MAX, usize::MAX.wrapping_sub(1))
            };
            let (i_start, i_end, i_step) = if mu_d >= 0.0 {
                (0, self.nx, 1)
            } else {
                (self.nx - 1, usize::MAX, usize::MAX.wrapping_sub(1))
            };

            let mut kk = k_start;
            while kk < k_end {
                let mut jj = j_start;
                while jj < j_end {
                    let mut ii = i_start;
                    while ii < i_end {
                        let temp = self.temperature[kk][jj][ii];
                        // Blackbody intensity: I_b = n²σT⁴/π
                        let i_b = SIGMA_SB * temp.powi(4) / std::f64::consts::PI;

                        // Upwind intensities
                        let i_upwind_x = if mu_d.abs() > 1e-30 && ii.wrapping_sub(i_step) < self.nx
                        {
                            self.intensity[kk][jj][ii.wrapping_sub(i_step)][d]
                        } else {
                            // Wall boundary: diffuse emission + reflection
                            self.wall_intensity(temp, d)
                        };
                        let i_upwind_y = if eta_d.abs() > 1e-30 && jj.wrapping_sub(j_step) < self.ny
                        {
                            self.intensity[kk][jj.wrapping_sub(j_step)][ii][d]
                        } else {
                            self.wall_intensity(temp, d)
                        };
                        let i_upwind_z = if xi_d.abs() > 1e-30 && kk.wrapping_sub(k_step) < self.nz
                        {
                            self.intensity[kk.wrapping_sub(k_step)][jj][ii][d]
                        } else {
                            self.wall_intensity(temp, d)
                        };

                        // Step scheme: I = (I_b + ω·Φ + I_up/ds) / (1/ds + β)
                        let ds_x = if mu_d.abs() > 1e-30 {
                            self.dx / mu_d.abs()
                        } else {
                            1e30
                        };
                        let ds_y = if eta_d.abs() > 1e-30 {
                            self.dy / eta_d.abs()
                        } else {
                            1e30
                        };
                        let ds_z = if xi_d.abs() > 1e-30 {
                            self.dz / xi_d.abs()
                        } else {
                            1e30
                        };
                        let _ds_min = ds_x.min(ds_y).min(ds_z);

                        // Source term: S = (1-ω)I_b + (ω/4π)G
                        let scattering_source = omega * self.incident_radiation[kk][jj][ii]
                            / (4.0 * std::f64::consts::PI);
                        let source = (1.0 - omega) * i_b + scattering_source;

                        let i_new =
                            (source + i_upwind_x / ds_x + i_upwind_y / ds_y + i_upwind_z / ds_z)
                                / (1.0 / ds_x + 1.0 / ds_y + 1.0 / ds_z + beta);

                        self.intensity[kk][jj][ii][d] = i_new;

                        // Accumulate incident radiation
                        self.incident_radiation[kk][jj][ii] += i_new * w_d;

                        ii = ii.wrapping_add(i_step);
                    }
                    jj = jj.wrapping_add(j_step);
                }
                kk = kk.wrapping_add(k_step);
            }
        }

        // Compute radiative heat source: ∇·q_r = κ_a(4πI_b - G).
        // Per-cell independent (reads temperature/incident, writes source) → rayon.
        let temperature = &self.temperature;
        let incident = &self.incident_radiation;
        self.radiative_source
            .par_iter_mut()
            .enumerate()
            .for_each(|(k, src_plane)| {
                for j in 0..self.ny {
                    for i in 0..self.nx {
                        let i_b = SIGMA_SB * temperature[k][j][i].powi(4) / std::f64::consts::PI;
                        src_plane[j][i] = self.absorption
                            * (4.0 * std::f64::consts::PI * i_b - incident[k][j][i]);
                    }
                }
            });

        Ok(())
    }

    /// Wall intensity with diffuse emission and reflection.
    fn wall_intensity(&self, temp: Scalar, _dir: usize) -> Scalar {
        let eps = self.wall_emissivity;
        let i_b = SIGMA_SB * temp.powi(4) / std::f64::consts::PI;
        eps * i_b
            + (1.0 - eps)
                * self
                    .incident_radiation
                    .first()
                    .and_then(|p| p.first())
                    .and_then(|r| r.first())
                    .copied()
                    .unwrap_or(0.0)
                / (4.0 * std::f64::consts::PI)
    }

    /// Get the radiative heat source field (W/m³).
    pub fn heat_source(&self) -> &[Vec<Vec<Scalar>>] {
        &self.radiative_source
    }

    /// Get incident radiation field (W/m²).
    pub fn incident(&self) -> &[Vec<Vec<Scalar>>] {
        &self.incident_radiation
    }

    /// Compute net radiative heat flux on a boundary face.
    pub fn boundary_heat_flux(&self, face: &str) -> Scalar {
        let (k, j, i) = match face {
            "xmin" => (self.nz / 2, self.ny / 2, 0),
            "xmax" => (self.nz / 2, self.ny / 2, self.nx - 1),
            "ymin" => (self.nz / 2, 0, self.nx / 2),
            "ymax" => (self.nz / 2, self.ny - 1, self.nx / 2),
            "zmin" => (0, self.ny / 2, self.nx / 2),
            "zmax" => (self.nz - 1, self.ny / 2, self.nx / 2),
            _ => return 0.0,
        };
        // Net flux: sum of I·n·w over all directions
        let mut q = 0.0;
        for d in 0..self.quad.n_dir {
            let i_val = self.intensity[k][j][i][d];
            let n_dot_s = match face {
                "xmin" => -self.quad.mu[d],
                "xmax" => self.quad.mu[d],
                "ymin" => -self.quad.eta[d],
                "ymax" => self.quad.eta[d],
                "zmin" => -self.quad.xi[d],
                "zmax" => self.quad.xi[d],
                _ => 0.0,
            };
            q += i_val * n_dot_s * self.quad.weights[d];
        }
        q
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dom_quadrature_s2() {
        let q = DomQuadrature::s_n(2);
        assert!(q.n_dir >= 2);
        assert_eq!(q.mu.len(), q.n_dir);
        assert_eq!(q.weights.len(), q.n_dir);
        // Weights should sum to 4π
        let w_sum: Scalar = q.weights.iter().sum();
        assert!((w_sum - 4.0 * std::f64::consts::PI).abs() < 1.0);
    }

    #[test]
    fn test_dom_quadrature_s4() {
        let q = DomQuadrature::s_n(4);
        assert!(q.n_dir >= 4);
        let w_sum: Scalar = q.weights.iter().sum();
        assert!((w_sum - 4.0 * std::f64::consts::PI).abs() < 1.0);
    }

    #[test]
    fn test_dom_new() {
        let dom = DomRadiation3D::new(4, 4, 4, 0.1, 0.1, 0.1, 1.0, 0.5, 2, 300.0);
        assert_eq!(dom.nx, 4);
        assert!((dom.wall_emissivity - 0.9).abs() < 1e-10);
    }

    #[test]
    fn test_dom_solve() {
        let mut dom = DomRadiation3D::new(4, 4, 4, 0.1, 0.1, 0.1, 1.0, 0.5, 2, 500.0);
        let result = dom.solve();
        assert!(result.is_ok());
        // After solve, incident radiation should be positive
        for k in 0..dom.nz {
            for j in 0..dom.ny {
                for i in 0..dom.nx {
                    assert!(
                        dom.incident_radiation[k][j][i] >= 0.0,
                        "incident radiation must be ≥ 0"
                    );
                }
            }
        }
    }

    #[test]
    fn test_dom_hotter_temperature_more_radiation() {
        let mut dom_cold = DomRadiation3D::new(4, 4, 4, 0.1, 0.1, 0.1, 1.0, 0.0, 2, 300.0);
        let mut dom_hot = DomRadiation3D::new(4, 4, 4, 0.1, 0.1, 0.1, 1.0, 0.0, 2, 600.0);
        dom_cold.solve().unwrap();
        dom_hot.solve().unwrap();
        let g_cold: Scalar = dom_cold
            .incident_radiation
            .iter()
            .flat_map(|k| k.iter().flat_map(|j| j.iter()))
            .sum();
        let g_hot: Scalar = dom_hot
            .incident_radiation
            .iter()
            .flat_map(|k| k.iter().flat_map(|j| j.iter()))
            .sum();
        assert!(g_hot > g_cold, "hotter medium should have more radiation");
    }

    #[test]
    fn test_dom_heat_source() {
        let mut dom = DomRadiation3D::new(4, 4, 4, 0.1, 0.1, 0.1, 2.0, 0.0, 2, 400.0);
        dom.solve().unwrap();
        let src = dom.heat_source();
        for k in 0..dom.nz {
            for j in 0..dom.ny {
                for i in 0..dom.nx {
                    assert!(src[k][j][i].is_finite());
                }
            }
        }
    }

    #[test]
    fn test_dom_boundary_flux() {
        let mut dom = DomRadiation3D::new(4, 4, 4, 0.1, 0.1, 0.1, 1.0, 0.0, 2, 500.0);
        dom.solve().unwrap();
        let q = dom.boundary_heat_flux("xmax");
        assert!(q.is_finite());
    }

    #[test]
    fn test_dom_incident() {
        let mut dom = DomRadiation3D::new(4, 4, 4, 0.1, 0.1, 0.1, 1.0, 0.0, 2, 300.0);
        dom.solve().unwrap();
        let inc = dom.incident();
        assert_eq!(inc.len(), 4);
    }

    #[test]
    fn test_dom_zero_extinction_error() {
        let mut dom = DomRadiation3D::new(4, 4, 4, 0.1, 0.1, 0.1, 0.0, 0.0, 2, 300.0);
        assert!(dom.solve().is_err());
    }

    #[test]
    fn test_dom_source_parallel_matches_serial_reference() {
        // The radiative-source pass runs on rayon; verify each cell equals the
        // serial formula ∇·q_r = κ_a(4πI_b − G) recomputed inline.
        let mut dom = DomRadiation3D::new(6, 6, 6, 0.1, 0.1, 0.1, 0.5, 0.1, 2, 300.0);
        for k in 0..dom.nz {
            for j in 0..dom.ny {
                for i in 0..dom.nx {
                    dom.temperature[k][j][i] =
                        300.0 + (i as Scalar) * 10.0 + (j as Scalar) * 5.0 + (k as Scalar) * 3.0;
                }
            }
        }
        dom.solve().unwrap();

        for k in 0..dom.nz {
            for j in 0..dom.ny {
                for i in 0..dom.nx {
                    let i_b = SIGMA_SB * dom.temperature[k][j][i].powi(4) / std::f64::consts::PI;
                    let expected = dom.absorption
                        * (4.0 * std::f64::consts::PI * i_b - dom.incident_radiation[k][j][i]);
                    assert!(
                        (dom.radiative_source[k][j][i] - expected).abs() < 1e-12,
                        "source mismatch [{k}][{j}][{i}]: {} vs {}",
                        dom.radiative_source[k][j][i],
                        expected
                    );
                }
            }
        }
    }
}
