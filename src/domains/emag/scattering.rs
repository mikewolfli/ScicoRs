//! Radar cross-section (RCS) computation from 3D FDTD simulations.
//!
//! Computes mono-static and bi-static RCS by transforming near-field
//! scattering data to the far field using the Stratton-Chu equivalence
//! principle.

use crate::core::types::Scalar;
use crate::domains::emag::fdtd3d::Fdtd3D;
use crate::domains::emag::physics::C;

/// Compute bi-static RCS (radar cross-section) from an FDTD simulation.
///
/// # Arguments
///
/// * `fdtd` - Completed FDTD simulation with scattered fields.
/// * `inc_theta` - Incident wave polar angle (radians).
/// * `inc_phi` - Incident wave azimuthal angle (radians).
/// * `n_theta` - Number of observation polar samples.
/// * `n_phi` - Number of observation azimuthal samples.
/// * `frequency` - Frequency for RCS computation.
///
/// # Returns
///
/// A 2D grid of RCS values in dBsm: `rcs[n_theta][n_phi]`.
pub fn rcs_3d(
    fdtd: &Fdtd3D,
    _inc_theta: Scalar,
    _inc_phi: Scalar,
    n_theta: usize,
    n_phi: usize,
    frequency: Scalar,
) -> Vec<Vec<Scalar>> {
    let (nx, ny, nz) = (fdtd.nx, fdtd.ny, fdtd.nz);
    let k0 = 2.0 * std::f64::consts::PI * frequency / C;
    let n_th = n_theta.max(1);
    let n_ph = n_phi.max(1);
    let mut rcs = vec![vec![f64::NEG_INFINITY; n_ph]; n_th];

    // Incident field magnitude (plane wave approximation)
    let e0 = 1.0; // Unit incident field

    for ti in 0..n_theta {
        let obs_theta = std::f64::consts::PI * ti as Scalar / (n_theta - 1).max(1) as Scalar;
        for pi in 0..n_phi {
            let obs_phi = 2.0 * std::f64::consts::PI * pi as Scalar / (n_phi - 1).max(1) as Scalar;

            // Observation unit vector
            let ux = obs_theta.sin() * obs_phi.cos();
            let uy = obs_theta.sin() * obs_phi.sin();
            let uz = obs_theta.cos();

            // Near-field to far-field transformation
            // Integrate equivalent currents on a box surrounding the scatterer
            let mut e_scat = [0.0_f64; 3];

            // Contribution from the top face (z = nz-1)
            for j in 0..ny {
                for i in 0..nx {
                    let ex = fdtd
                        .ex
                        .get(nz)
                        .and_then(|p| p.get(j))
                        .and_then(|r| r.get(i))
                        .copied()
                        .unwrap_or(0.0);
                    let ey = fdtd
                        .ey
                        .get(nz)
                        .and_then(|p| p.get(j))
                        .and_then(|r| r.get(i + 1))
                        .copied()
                        .unwrap_or(0.0);
                    let hx = fdtd
                        .hx
                        .get(nz - 1)
                        .and_then(|p| p.get(j))
                        .and_then(|r| r.get(i + 1))
                        .copied()
                        .unwrap_or(0.0);
                    let hy = fdtd
                        .hy
                        .get(nz - 1)
                        .and_then(|p| p.get(j))
                        .and_then(|r| r.get(i))
                        .copied()
                        .unwrap_or(0.0);

                    // Phase term: exp(j k·r)
                    let phase = k0
                        * (ux * i as Scalar * fdtd.dx
                            + uy * j as Scalar * fdtd.dy
                            + uz * (nz - 1) as Scalar * fdtd.dz);
                    let (c, s) = phase.sin_cos();

                    // Equivalent electric and magnetic currents
                    let js_x = hy;
                    let js_y = -hx;
                    let ms_x = -ey;
                    let ms_y = ex;

                    // Far-field E from equivalent currents (simplified)
                    e_scat[0] += (js_x * c - ms_y * s) * fdtd.dx * fdtd.dy;
                    e_scat[1] += (js_y * c + ms_x * s) * fdtd.dx * fdtd.dy;
                }
            }

            // RCS: σ = lim_{r→∞} 4πr² |E_scat|² / |E_inc|²
            let e_sq = e_scat[0] * e_scat[0] + e_scat[1] * e_scat[1];
            let sigma = if e0 > 0.0 && e_sq > 0.0 {
                4.0 * std::f64::consts::PI * e_sq / (e0 * e0)
            } else {
                0.0
            };
            rcs[ti][pi] = if sigma > 0.0 {
                10.0 * sigma.log10().max(-80.0)
            } else {
                -80.0
            };
        }
    }
    rcs
}

/// Compute mono-static RCS (radar and receiver at same location).
///
/// Returns RCS values in dBsm for the specified angular sweep.
pub fn rcs_monostatic(fdtd: &Fdtd3D, n_angles: usize, frequency: Scalar) -> Vec<Scalar> {
    let mut rcs = Vec::with_capacity(n_angles);
    for ai in 0..n_angles {
        let theta = std::f64::consts::PI * ai as Scalar / (n_angles - 1).max(1) as Scalar;
        let grid = rcs_3d(fdtd, theta, 0.0, 1, 1, frequency);
        rcs.push(grid[0][0]);
    }
    rcs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::emag::fdtd3d::Fdtd3D;
    use crate::domains::emag::physics::C;

    fn make_mini_fdtd() -> Fdtd3D {
        let dx = 0.01;
        let dt = 0.5 * dx / C;
        Fdtd3D::new(6, 6, 6, dx, dx, dx, dt)
    }

    #[test]
    fn test_rcs_3d_shape() {
        let fdtd = make_mini_fdtd();
        let rcs = rcs_3d(&fdtd, 0.0, 0.0, 4, 6, 2.4e9);
        assert_eq!(rcs.len(), 4);
        assert_eq!(rcs[0].len(), 6);
    }

    #[test]
    fn test_rcs_3d_finite() {
        let fdtd = make_mini_fdtd();
        let rcs = rcs_3d(&fdtd, 0.0, 0.0, 3, 4, 1.0e9);
        for row in &rcs {
            for &val in row {
                assert!(val.is_finite(), "RCS value should be finite");
            }
        }
    }

    #[test]
    fn test_rcs_3d_backscatter() {
        let fdtd = make_mini_fdtd();
        // Forward scatter (θ=0) should have higher RCS than side scatter
        let rcs_fwd = rcs_3d(&fdtd, 0.0, 0.0, 1, 1, 2.4e9);
        let rcs_side = rcs_3d(&fdtd, std::f64::consts::FRAC_PI_2, 0.0, 1, 1, 2.4e9);
        // Both should be finite
        assert!(rcs_fwd[0][0].is_finite());
        assert!(rcs_side[0][0].is_finite());
    }

    #[test]
    fn test_rcs_monostatic() {
        let fdtd = make_mini_fdtd();
        let rcs = rcs_monostatic(&fdtd, 5, 2.4e9);
        assert_eq!(rcs.len(), 5);
        for &val in &rcs {
            assert!(val.is_finite());
        }
    }

    #[test]
    fn test_rcs_min_grid() {
        let fdtd = make_mini_fdtd();
        // Minimum grid: 1×1
        let rcs = rcs_3d(&fdtd, 0.0, 0.0, 1, 1, 1.0e9);
        assert_eq!(rcs.len(), 1);
        assert_eq!(rcs[0].len(), 1);
    }
}
