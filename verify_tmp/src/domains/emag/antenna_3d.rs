//! 3D antenna analysis using FDTD-computed near fields.
//!
//! Provides radiation pattern extraction, directivity, input impedance,
//! and S₁₁ computation from a 3D FDTD simulation.

use crate::core::types::Scalar;
use crate::domains::emag::fdtd3d::Fdtd3D;
use crate::domains::emag::physics::C;
use num_complex::Complex;
/// Alias for complex scalar type used in antenna computations.
type ComplexScalar = Complex<Scalar>;

/// Sampling directions above which the radiation-pattern loop runs on rayon.
const RAD_PATTERN_PAR_MIN: usize = 256;

/// 3D antenna geometry and excitation for post-processing FDTD results.
#[derive(Debug, Clone)]
pub struct Antenna3D {
    /// Feed point indices (node index, voltage amplitude).
    pub excitation: Vec<(usize, Scalar)>,
    /// Frequency for impedance and pattern calculation.
    pub frequency: Scalar,
    /// Number of angular samples for radiation pattern.
    pub n_theta: usize,
    pub n_phi: usize,
}

impl Antenna3D {
    /// Create a new antenna descriptor.
    pub fn new(frequency: Scalar, n_theta: usize, n_phi: usize) -> Self {
        assert!(frequency > 0.0);
        Self {
            excitation: Vec::new(),
            frequency,
            n_theta: n_theta.max(2),
            n_phi: n_phi.max(2),
        }
    }

    /// Add a feed point (grid index, excitation voltage).
    pub fn add_feed(&mut self, index: usize, voltage: Scalar) {
        self.excitation.push((index, voltage));
    }

    /// Compute the far-field radiation pattern from an FDTD simulation.
    ///
    /// Returns `Vec<[theta, phi, gain_dbi]>` for each sampling direction.
    ///
    /// Uses the equivalence principle: near E/H fields on a Huygens surface
    /// are transformed to far-field via Stratton-Chu integrals (simplified).
    pub fn radiation_pattern(&self, fdtd: &Fdtd3D) -> Vec<[Scalar; 3]> {
        // Total radiated power (Poynting vector integration)
        let total_power = self.total_radiated_power(fdtd);
        let n_dir = self.n_theta.saturating_mul(self.n_phi);
        // Each (theta, phi) direction is independent → parallel over directions.
        let compute = |idx: usize| -> [Scalar; 3] {
            let ti = idx / self.n_phi.max(1);
            let pi = idx % self.n_phi.max(1);
            let theta = std::f64::consts::PI * ti as Scalar / (self.n_theta.max(2) - 1) as Scalar;
            let phi = 2.0 * std::f64::consts::PI * pi as Scalar / (self.n_phi.max(2) - 1) as Scalar;
            // Unit vector in far-field direction
            let ux = theta.sin() * phi.cos();
            let uy = theta.sin() * phi.sin();
            let uz = theta.cos();

            // Simplified far-field: integrate equivalent currents on a
            // box surface bounding the antenna
            let (nx, ny, nz) = (fdtd.nx, fdtd.ny, fdtd.nz);
            let mut e_far = [0.0; 3];

            // Contribution from top face (z = nz-1)
            for j in 0..ny {
                for i in 0..nx {
                    let ex_val = fdtd
                        .ex
                        .get(nz)
                        .and_then(|p| p.get(j))
                        .and_then(|r| r.get(i))
                        .copied()
                        .unwrap_or(0.0);
                    let ey_val = fdtd
                        .ey
                        .get(nz)
                        .and_then(|p| p.get(j))
                        .and_then(|r| r.get(i + 1))
                        .copied()
                        .unwrap_or(0.0);
                    let phase = (ux * i as Scalar * fdtd.dx
                        + uy * j as Scalar * fdtd.dy
                        + uz * (nz - 1) as Scalar * fdtd.dz)
                        * 2.0
                        * std::f64::consts::PI
                        * self.frequency
                        / C;
                    let (c, s) = phase.sin_cos();
                    let ejkr = ComplexScalar::new(c, s);
                    e_far[0] += (ex_val * ejkr.re - ey_val * ejkr.im) * fdtd.dx * fdtd.dy;
                    e_far[1] += (ey_val * ejkr.re + ex_val * ejkr.im) * fdtd.dx * fdtd.dy;
                }
            }

            let gain = if total_power > 0.0 {
                let u = e_far[0] * e_far[0] + e_far[1] * e_far[1];
                10.0 * (4.0 * std::f64::consts::PI * u / total_power)
                    .log10()
                    .max(-50.0)
            } else {
                -50.0
            };

            [theta, phi, gain]
        };
        if n_dir >= RAD_PATTERN_PAR_MIN {
            use rayon::prelude::*;
            (0..n_dir).into_par_iter().map(compute).collect()
        } else {
            (0..n_dir).map(compute).collect()
        }
    }

    /// Compute directivity (max gain in dBi).
    pub fn directivity(&self, fdtd: &Fdtd3D) -> Scalar {
        let pattern = self.radiation_pattern(fdtd);
        pattern
            .iter()
            .map(|p| p[2])
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Estimate input impedance from FDTD fields at the feed point.
    pub fn input_impedance(&self, fdtd: &Fdtd3D) -> ComplexScalar {
        if self.excitation.is_empty() {
            return ComplexScalar::new(50.0, 0.0);
        }
        let (idx, voltage) = self.excitation[0];
        // Map linear index to (k, j, i)
        let nxy = fdtd.nx * fdtd.ny;
        let k = idx / nxy;
        let j = (idx % nxy) / fdtd.nx;
        let i = idx % fdtd.nx;
        // Current at feed: I = ∮ H·dl
        let hx_around = fdtd
            .hx
            .get(k)
            .and_then(|p| p.get(j))
            .and_then(|r| r.get(i + 1))
            .copied()
            .unwrap_or(0.0);
        let hy_around = fdtd
            .hy
            .get(k)
            .and_then(|p| p.get(j))
            .and_then(|r| r.get(i))
            .copied()
            .unwrap_or(0.0);
        let current = (hx_around * fdtd.dx + hy_around * fdtd.dy).abs().max(1e-30);
        let r_in = (voltage / current).max(0.0);
        // Reactive part estimated from phase difference
        let x_in = 0.0; // Simplified: resistive approximation
        ComplexScalar::new(r_in, x_in)
    }

    /// Compute S₁₁ (return loss) at the feed point.
    pub fn s11(&self, fdtd: &Fdtd3D, z0: Scalar) -> ComplexScalar {
        let z_in = self.input_impedance(fdtd);
        let gamma = (z_in - ComplexScalar::new(z0, 0.0)) / (z_in + ComplexScalar::new(z0, 0.0));
        gamma
    }

    /// Total radiated power from Poynting vector integration.
    fn total_radiated_power(&self, fdtd: &Fdtd3D) -> Scalar {
        let (nx, ny, nz) = (fdtd.nx, fdtd.ny, fdtd.nz);
        let mut power = 0.0;
        // Integrate Poynting vector (E×H) over a box surface
        // Top face (z = nz-1)
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
                let sz = ex * hy - ey * hx;
                power += sz.abs() * fdtd.dx * fdtd.dy;
            }
        }
        power
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::emag::fdtd3d::Fdtd3D;

    fn make_mini_fdtd() -> Fdtd3D {
        let c0 = C;
        let dx = 0.01;
        let dt = 0.5 * dx / c0; // CFL-safe
        Fdtd3D::new(6, 6, 6, dx, dx, dx, dt)
    }

    #[test]
    fn test_antenna_new() {
        let ant = Antenna3D::new(2.4e9, 36, 72);
        assert!((ant.frequency - 2.4e9).abs() < 1.0);
        assert_eq!(ant.n_theta, 36);
        assert_eq!(ant.n_phi, 72);
    }

    #[test]
    fn test_antenna_add_feed() {
        let mut ant = Antenna3D::new(1.0e9, 10, 10);
        ant.add_feed(0, 1.0);
        assert_eq!(ant.excitation.len(), 1);
        assert!((ant.excitation[0].1 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_radiation_pattern_length() {
        let fdtd = make_mini_fdtd();
        let ant = Antenna3D::new(2.4e9, 4, 6);
        let pattern = ant.radiation_pattern(&fdtd);
        assert_eq!(pattern.len(), 24); // 4 × 6
    }

    #[test]
    fn test_radiation_pattern_values() {
        let fdtd = make_mini_fdtd();
        let ant = Antenna3D::new(2.4e9, 3, 4);
        let pattern = ant.radiation_pattern(&fdtd);
        // All gain values should be finite
        for p in &pattern {
            assert!(
                p[2].is_finite(),
                "gain should be finite at theta={}, phi={}",
                p[0],
                p[1]
            );
        }
    }

    #[test]
    fn test_directivity() {
        let fdtd = make_mini_fdtd();
        let ant = Antenna3D::new(2.4e9, 6, 8);
        let d = ant.directivity(&fdtd);
        assert!(d.is_finite());
    }

    #[test]
    fn test_input_impedance_default() {
        let fdtd = make_mini_fdtd();
        let ant = Antenna3D::new(1.0e9, 4, 4);
        let z = ant.input_impedance(&fdtd);
        assert!(z.re > 0.0);
    }

    #[test]
    fn test_s11() {
        let fdtd = make_mini_fdtd();
        let ant = Antenna3D::new(1.0e9, 4, 4);
        let s11 = ant.s11(&fdtd, 50.0);
        assert!(s11.norm_sqr().is_finite());
    }

    #[test]
    fn test_total_radiated_power() {
        let fdtd = make_mini_fdtd();
        let ant = Antenna3D::new(1.0e9, 4, 4);
        let p = ant.total_radiated_power(&fdtd);
        assert!(p >= 0.0);
    }

    #[test]
    fn test_antenna_feed_impedance() {
        let mut ant = Antenna3D::new(2.4e9, 4, 4);
        ant.add_feed(14, 1.0); // centre-ish of 6×6×6
        let fdtd = make_mini_fdtd();
        let z = ant.input_impedance(&fdtd);
        assert!(z.re > 0.0);
    }
}
