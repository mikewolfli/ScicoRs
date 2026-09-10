//! Heat conduction solvers for 1D and 2D steady-state and transient problems.
//!
//! Provides Fourier's law, thermal resistance networks (series/parallel),
//! 1D transient heat conduction via FTCS and Crank-Nicolson schemes, 2D
//! steady-state via Gauss-Seidel iteration, and boundary condition types.

use crate::core::types::Scalar;

/// Fourier's law for 1D steady conduction: Q = -k·A·(ΔT/Δx).
///
/// Returns the heat flow rate (W) through a wall of conductivity `k`,
/// cross-sectional area `a`, with temperatures `t_hot` and `t_cold`
/// separated by thickness `dx`.
pub fn fourier_law_1d(k: Scalar, a: Scalar, t_hot: Scalar, t_cold: Scalar, dx: Scalar) -> Scalar {
    if dx <= 0.0 || k <= 0.0 || a <= 0.0 {
        return 0.0;
    }
    k * a * (t_hot - t_cold) / dx
}

/// Thermal resistance network for series/parallel wall configurations.
///
/// Each resistance is stored as `(length, area, conductivity)` from which
/// R = L / (k·A) is computed.
pub struct ThermalResistance {
    /// List of (length, area, thermal conductivity) tuples.
    pub resistances: Vec<(Scalar, Scalar, Scalar)>,
    /// Whether the resistances are arranged in parallel.
    pub parallel: bool,
}

impl ThermalResistance {
    /// Total thermal resistance for series configuration (K/W).
    ///
    /// R_total = Σ L_i / (k_i · A_i)
    pub fn series_resistance(&self) -> Scalar {
        let mut total = 0.0;
        for &(length, area, k) in &self.resistances {
            if k <= 0.0 || area <= 0.0 {
                continue;
            }
            total += length / (k * area);
        }
        total
    }

    /// Total thermal resistance for parallel configuration (K/W).
    ///
    /// 1/R_total = Σ 1/R_i
    pub fn parallel_resistance(&self) -> Scalar {
        let mut inv_total = 0.0;
        for &(length, area, k) in &self.resistances {
            if k <= 0.0 || area <= 0.0 || length <= 0.0 {
                continue;
            }
            inv_total += k * area / length;
        }
        if inv_total <= 0.0 {
            return Scalar::INFINITY;
        }
        1.0 / inv_total
    }

    /// Heat flow rate (W) through the network given a temperature difference.
    pub fn heat_flow(&self, delta_t: Scalar) -> Scalar {
        let r_total = if self.parallel {
            self.parallel_resistance()
        } else {
            self.series_resistance()
        };
        if r_total.is_infinite() || r_total <= 0.0 {
            return 0.0;
        }
        delta_t / r_total
    }
}

/// Boundary condition types for heat conduction problems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundaryCondition {
    /// Dirichlet — fixed temperature (K).
    FixedTemp(Scalar),
    /// Neumann — fixed heat flux (W/m²).
    FixedHeatFlux(Scalar),
    /// Robin — convection: h·(T_surface - T_ambient). Parameters: (h, T_ambient).
    Convection(Scalar, Scalar),
    /// Adiabatic — zero heat flux (dT/dn = 0).
    Adiabatic,
}

/// 1D transient heat conduction solver using finite differences.
///
/// Solves ∂T/∂t = α·∂²T/∂x² on a uniform grid.
/// Supports FTCS (explicit) and Crank-Nicolson (implicit) time stepping.
pub struct HeatConduction1D {
    /// Thermal diffusivity α = k/(ρ·cp) (m²/s).
    pub alpha: Scalar,
    /// Total length of the domain (m).
    pub length: Scalar,
    /// Number of grid cells.
    pub n_cells: usize,
    /// Temperature at each cell center (K).
    pub temperature: Vec<Scalar>,
}

impl HeatConduction1D {
    /// Create a new 1D heat conduction problem.
    pub fn new(alpha: Scalar, length: Scalar, n_cells: usize, initial_temp: Scalar) -> Self {
        let temperature = vec![initial_temp; n_cells];
        Self {
            alpha,
            length,
            n_cells,
            temperature,
        }
    }

    /// Perform one time step using the explicit FTCS (Forward Time, Central Space) scheme.
    ///
    /// Stability requires: α·Δt / Δx² ≤ 0.5.
    /// Returns an error if the stability criterion is not met or if parameters are invalid.
    pub fn ftcs_step(&mut self, dt: Scalar) -> Result<(), String> {
        let n = self.n_cells;
        if n < 2 {
            return Err("FTCS requires at least 2 cells".to_string());
        }
        if dt <= 0.0 {
            return Err("Time step must be positive".to_string());
        }
        if self.alpha <= 0.0 {
            return Err("Thermal diffusivity must be positive".to_string());
        }
        let dx = self.length / (n as Scalar);
        let r = self.alpha * dt / (dx * dx);
        if r > 0.5 {
            return Err(format!(
                "FTCS unstable: r = {:.6} exceeds 0.5 (dt = {}, dx = {})",
                r, dt, dx
            ));
        }
        let mut new_temp = self.temperature.clone();
        for i in 1..(n - 1) {
            new_temp[i] = self.temperature[i]
                + r * (self.temperature[i + 1] - 2.0 * self.temperature[i]
                    + self.temperature[i - 1]);
        }
        self.temperature = new_temp;
        Ok(())
    }

    /// Perform one time step using the Crank-Nicolson (implicit) scheme.
    ///
    /// Solves the tridiagonal system using the Thomas algorithm.
    /// Crank-Nicolson is unconditionally stable for linear diffusion.
    pub fn crank_nicolson_step(&mut self, dt: Scalar) -> Result<(), String> {
        let n = self.n_cells;
        if n < 2 {
            return Err("Crank-Nicolson requires at least 2 cells".to_string());
        }
        if dt <= 0.0 {
            return Err("Time step must be positive".to_string());
        }
        if self.alpha <= 0.0 {
            return Err("Thermal diffusivity must be positive".to_string());
        }
        let dx = self.length / (n as Scalar);
        let r = self.alpha * dt / (2.0 * dx * dx);

        // Build tridiagonal system: A * T^{n+1} = B * T^n
        // A: sub=(-r), diag=(1+2r), super=(-r)
        // B: sub=( r), diag=(1-2r), super=( r)

        let mut a_sub = vec![0.0; n - 1];
        let mut a_diag = vec![0.0; n];
        let mut a_super = vec![0.0; n - 1];
        let mut rhs = vec![0.0; n];

        for i in 0..n {
            a_diag[i] = 1.0 + 2.0 * r;
            if i > 0 {
                a_sub[i - 1] = -r;
            }
            if i < n - 1 {
                a_super[i] = -r;
            }
            rhs[i] = self.temperature[i];
            if i > 0 {
                rhs[i] += r * self.temperature[i - 1];
            }
            rhs[i] -= 2.0 * r * self.temperature[i];
            if i < n - 1 {
                rhs[i] += r * self.temperature[i + 1];
            }
        }

        // Thomas algorithm: forward sweep
        let mut c_prime = vec![0.0; n - 1];
        let mut d_prime = vec![0.0; n];
        c_prime[0] = a_super[0] / a_diag[0];
        d_prime[0] = rhs[0] / a_diag[0];
        for i in 1..n {
            let denom = a_diag[i] - a_sub[i - 1] * c_prime[i - 1];
            if denom.abs() < 1e-30 {
                return Err("Tridiagonal matrix is singular".to_string());
            }
            if i < n - 1 {
                c_prime[i] = a_super[i] / denom;
            }
            d_prime[i] = (rhs[i] - a_sub[i - 1] * d_prime[i - 1]) / denom;
        }

        // Thomas algorithm: backward substitution
        let mut new_temp = vec![0.0; n];
        new_temp[n - 1] = d_prime[n - 1];
        for i in (0..(n - 1)).rev() {
            new_temp[i] = d_prime[i] - c_prime[i] * new_temp[i + 1];
        }

        self.temperature = new_temp;
        Ok(())
    }

    /// Compute the steady-state temperature profile for fixed-temperature boundaries.
    ///
    /// Returns a vector of temperatures (K) at each cell center for the linear
    /// steady-state solution with Dirichlet boundaries at both ends.
    pub fn steady_state(&self, t_left: Scalar, t_right: Scalar) -> Vec<Scalar> {
        let n = self.n_cells;
        let mut profile = Vec::with_capacity(n);
        for i in 0..n {
            let x_frac = (i as Scalar + 0.5) / (n as Scalar);
            profile.push(t_left + (t_right - t_left) * x_frac);
        }
        profile
    }
}

/// 2D steady-state heat conduction solver using Successive Over-Relaxation (SOR).
///
/// SOR accelerates the classical Gauss-Seidel method by applying a relaxation
/// factor ω > 1. Optimal ω for a 2D problem is typically near 2/(1+sin(π/N)).
pub struct HeatConduction2D {
    /// Thermal diffusivity α = k/(ρ·cp) (m²/s).
    pub alpha: Scalar,
    /// Thermal conductivity (W/(m·K)), used for flux and convection BCs.
    pub k: Scalar,
    /// Number of cells in the x-direction.
    pub nx: usize,
    /// Number of cells in the y-direction.
    pub ny: usize,
    /// Grid spacing in the x-direction (m).
    pub dx: Scalar,
    /// Grid spacing in the y-direction (m).
    pub dy: Scalar,
    /// Temperature at each grid point, stored as rows (K).
    pub temperature: Vec<Vec<Scalar>>,
    /// SOR relaxation factor (1.0 = Gauss-Seidel, >1 = over-relaxation).
    /// Typical range: 1.5–1.9. Set to 1.0 for standard Gauss-Seidel.
    pub sor_omega: Scalar,
}

impl HeatConduction2D {
    /// Perform one SOR (Successive Over-Relaxation) iteration step.
    ///
    /// When `sor_omega == 1.0`, this reduces to standard Gauss-Seidel.
    /// For faster convergence use `sor_omega > 1.0` (typically 1.5–1.9).
    ///
    /// Boundary conditions are applied on all four edges of the domain.
    /// Interior cells use the 5-point stencil:
    ///   T_gs(i,j) = (T(i+1,j) + T(i-1,j) + (dx²/dy²)·(T(i,j+1) + T(i,j-1)))
    ///                / (2·(1 + dx²/dy²))
    ///   T_new(i,j) = (1 - ω)·T_old(i,j) + ω·T_gs(i,j)
    pub fn gauss_seidel_step(&mut self, boundary: &[BoundaryCondition]) -> Result<(), String> {
        if self.nx < 2 || self.ny < 2 {
            return Err("Grid must have at least 2 cells in each direction".to_string());
        }
        if boundary.len() < 4 {
            return Err(
                "Must provide 4 boundary conditions (left, right, top, bottom)".to_string(),
            );
        }
        if self.dx <= 0.0 || self.dy <= 0.0 {
            return Err("Grid spacing must be positive".to_string());
        }

        let dx2 = self.dx * self.dx;
        let dy2 = self.dy * self.dy;
        let factor = dx2 / dy2;
        let denom = 2.0 * (1.0 + factor);
        let omega = self.sor_omega;
        let one_minus_omega = 1.0 - omega;

        // Apply boundary conditions
        self.apply_boundary(boundary)?;

        // SOR sweep over interior points
        for i in 1..(self.nx - 1) {
            for j in 1..(self.ny - 1) {
                let t_old = self.temperature[i][j];
                let sum_x = self.temperature[i + 1][j] + self.temperature[i - 1][j];
                let sum_y = self.temperature[i][j + 1] + self.temperature[i][j - 1];
                let t_gs = (sum_x + factor * sum_y) / denom;
                self.temperature[i][j] = one_minus_omega * t_old + omega * t_gs;
            }
        }

        Ok(())
    }

    /// Apply boundary conditions to the four edges of the domain.
    fn apply_boundary(&mut self, boundary: &[BoundaryCondition]) -> Result<(), String> {
        // boundary[0] = left (x=0), boundary[1] = right (x=nx-1)
        // boundary[2] = top (y=ny-1), boundary[3] = bottom (y=0)
        for (j, &ref bc) in [&boundary[0], &boundary[1]].iter().enumerate() {
            let i = j * (self.nx - 1);
            match bc {
                BoundaryCondition::FixedTemp(t) => {
                    for k in 0..self.ny {
                        self.temperature[i][k] = *t;
                    }
                }
                BoundaryCondition::FixedHeatFlux(q) => {
                    // Neumann BC: q = -k·dT/dn → T_surf = T_adj + q·dx/k.
                    for k in 0..self.ny {
                        let adj = if j == 0 {
                            self.temperature[i + 1][k]
                        } else {
                            self.temperature[i - 1][k]
                        };
                        let kk = if self.k > 0.0 { self.k } else { self.alpha };
                        self.temperature[i][k] = adj + q * self.dx / kk;
                    }
                }
                BoundaryCondition::Convection(h, t_ambient) => {
                    for k in 0..self.ny {
                        let dx = if j == 0 { self.dx } else { self.dx };
                        let t_surf = self.temperature[i][k];
                        // T_surface = (h·T_ambient·dx/k + T_adj) / (1 + h·dx/k)
                        let adj = if j == 0 {
                            self.temperature[i + 1][k]
                        } else {
                            self.temperature[i - 1][k]
                        };
                        let kk = if self.k > 0.0 { self.k } else { self.alpha };
                        self.temperature[i][k] =
                            (h * t_ambient * dx / kk + adj) / (1.0 + h * dx / kk);
                        if self.temperature[i][k].is_nan() || self.temperature[i][k].is_infinite() {
                            self.temperature[i][k] = t_surf;
                        }
                    }
                }
                BoundaryCondition::Adiabatic => {
                    // dT/dn = 0: copy adjacent interior value
                    for k in 0..self.ny {
                        let adj = if j == 0 {
                            self.temperature[i + 1][k]
                        } else {
                            self.temperature[i - 1][k]
                        };
                        self.temperature[i][k] = adj;
                    }
                }
            }
        }

        for (j, &ref bc) in [&boundary[2], &boundary[3]].iter().enumerate() {
            let k = if j == 0 { self.ny - 1 } else { 0 };
            match bc {
                BoundaryCondition::FixedTemp(t) => {
                    for i in 0..self.nx {
                        self.temperature[i][k] = *t;
                    }
                }
                BoundaryCondition::FixedHeatFlux(q) => {
                    for i in 0..self.nx {
                        let adj = if j == 0 {
                            self.temperature[i][k - 1]
                        } else {
                            self.temperature[i][k + 1]
                        };
                        let kk = if self.k > 0.0 { self.k } else { self.alpha };
                        self.temperature[i][k] = adj + q * self.dy / kk;
                    }
                }
                BoundaryCondition::Convection(h, t_ambient) => {
                    for i in 0..self.nx {
                        let t_surf = self.temperature[i][k];
                        let dy = self.dy;
                        let adj = if j == 0 {
                            self.temperature[i][k - 1]
                        } else {
                            self.temperature[i][k + 1]
                        };
                        let kk = if self.k > 0.0 { self.k } else { self.alpha };
                        self.temperature[i][k] =
                            (h * t_ambient * dy / kk + adj) / (1.0 + h * dy / kk);
                        if self.temperature[i][k].is_nan() || self.temperature[i][k].is_infinite() {
                            self.temperature[i][k] = t_surf;
                        }
                    }
                }
                BoundaryCondition::Adiabatic => {
                    for i in 0..self.nx {
                        let adj = if j == 0 {
                            self.temperature[i][k - 1]
                        } else {
                            self.temperature[i][k + 1]
                        };
                        self.temperature[i][k] = adj;
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if the solution has converged within the given tolerance.
    ///
    /// Returns `true` if the maximum residual (difference between adjacent
    /// iterations) is below `tolerance`.
    pub fn check_convergence(&self, tolerance: Scalar) -> bool {
        if self.nx < 2 || self.ny < 2 {
            return true;
        }
        let dx2 = self.dx * self.dx;
        let dy2 = self.dy * self.dy;
        let factor = dx2 / dy2;
        let denom = 2.0 * (1.0 + factor);

        for i in 1..(self.nx - 1) {
            for j in 1..(self.ny - 1) {
                let sum_x = self.temperature[i + 1][j] + self.temperature[i - 1][j];
                let sum_y = self.temperature[i][j + 1] + self.temperature[i][j - 1];
                let t_expected = (sum_x + factor * sum_y) / denom;
                let residual = (self.temperature[i][j] - t_expected).abs();
                if residual > tolerance {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fourier_law_1d_basic() {
        // Copper wall: k=401, A=0.01 m², ΔT=100 K, dx=0.1 m
        let q = fourier_law_1d(401.0, 0.01, 100.0, 0.0, 0.1);
        let expected = 401.0 * 0.01 * 100.0 / 0.1;
        assert!((q - expected).abs() < 1e-10);
    }

    #[test]
    fn test_fourier_law_1d_zero_dx() {
        let q = fourier_law_1d(1.0, 1.0, 100.0, 20.0, 0.0);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_fourier_law_1d_reverse_delta() {
        let q = fourier_law_1d(1.0, 1.0, 20.0, 100.0, 1.0);
        assert!(q < 0.0);
    }

    #[test]
    fn test_thermal_resistance_series() {
        // Two layers: 0.1 m brick (k=0.7), 0.05 m insulation (k=0.04), A=1 m²
        let tr = ThermalResistance {
            resistances: vec![(0.1, 1.0, 0.7), (0.05, 1.0, 0.04)],
            parallel: false,
        };
        let r = tr.series_resistance();
        let expected = 0.1 / 0.7 + 0.05 / 0.04;
        assert!((r - expected).abs() < 1e-10);
    }

    #[test]
    fn test_thermal_resistance_parallel() {
        let tr = ThermalResistance {
            resistances: vec![(0.1, 1.0, 0.7), (0.1, 1.0, 0.04)],
            parallel: true,
        };
        let r = tr.parallel_resistance();
        let inv = 0.7 / 0.1 + 0.04 / 0.1;
        let expected = 1.0 / inv;
        assert!((r - expected).abs() < 1e-10);
    }

    #[test]
    fn test_thermal_resistance_heat_flow() {
        let tr = ThermalResistance {
            resistances: vec![(0.1, 1.0, 0.7)],
            parallel: false,
        };
        let q = tr.heat_flow(50.0);
        let expected = 50.0 / (0.1 / 0.7);
        assert!((q - expected).abs() < 1e-10);
    }

    #[test]
    fn test_heat_conduction_1d_ftcs_stable() {
        let mut hc = HeatConduction1D::new(1e-4, 0.1, 10, 20.0);
        // r = alpha * dt / dx^2 = 1e-4 * dt / (0.01)^2 = dt
        // For r < 0.5, dt must be < 0.5
        let result = hc.ftcs_step(0.4);
        assert!(result.is_ok());
    }

    #[test]
    fn test_heat_conduction_1d_ftcs_unstable() {
        let mut hc = HeatConduction1D::new(1e-4, 0.1, 10, 20.0);
        let result = hc.ftcs_step(10.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_heat_conduction_1d_cn_step() {
        let mut hc = HeatConduction1D::new(1e-4, 0.1, 10, 20.0);
        let result = hc.crank_nicolson_step(1.0);
        assert!(result.is_ok());
        // After one step, temperatures should still be finite
        for &t in &hc.temperature {
            assert!(t.is_finite());
        }
    }

    #[test]
    fn test_heat_conduction_1d_steady_state() {
        let hc = HeatConduction1D::new(1.0, 1.0, 5, 0.0);
        let profile = hc.steady_state(100.0, 0.0);
        assert_eq!(profile.len(), 5);
        // Linear profile from 100 to 0
        for i in 0..5 {
            let x_frac = (i as Scalar + 0.5) / 5.0;
            let expected = 100.0 * (1.0 - x_frac);
            assert!((profile[i] - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn test_heat_conduction_1d_ftcs_invalid_params() {
        let mut hc = HeatConduction1D::new(0.0, 1.0, 5, 20.0);
        assert!(hc.ftcs_step(0.1).is_err());

        let mut hc2 = HeatConduction1D::new(1e-4, 1.0, 1, 20.0);
        assert!(hc2.ftcs_step(0.1).is_err());

        let mut hc3 = HeatConduction1D::new(1e-4, 1.0, 5, 20.0);
        assert!(hc3.ftcs_step(-0.1).is_err());
    }

    #[test]
    fn test_heat_conduction_2d_gauss_seidel() {
        let mut hc2d = HeatConduction2D {
            alpha: 1e-4,
            k: 50.0,
            nx: 10,
            ny: 10,
            dx: 0.01,
            dy: 0.01,
            temperature: vec![vec![20.0; 10]; 10],
            sor_omega: 1.0,
        };
        let bc = vec![
            BoundaryCondition::FixedTemp(100.0), // left
            BoundaryCondition::FixedTemp(0.0),   // right
            BoundaryCondition::Adiabatic,        // top
            BoundaryCondition::Adiabatic,        // bottom
        ];
        let result = hc2d.gauss_seidel_step(&bc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_heat_conduction_2d_check_convergence() {
        let hc2d = HeatConduction2D {
            alpha: 1e-4,
            k: 50.0,
            nx: 10,
            ny: 10,
            dx: 0.01,
            dy: 0.01,
            temperature: vec![vec![20.0; 10]; 10],
            sor_omega: 1.0,
        };
        // Uniform temperature should be converged
        assert!(hc2d.check_convergence(1e-6));
    }

    #[test]
    fn test_boundary_condition_equality() {
        assert_eq!(
            BoundaryCondition::FixedTemp(100.0),
            BoundaryCondition::FixedTemp(100.0)
        );
        assert_eq!(BoundaryCondition::Adiabatic, BoundaryCondition::Adiabatic);
        assert_ne!(
            BoundaryCondition::FixedTemp(10.0),
            BoundaryCondition::FixedTemp(20.0)
        );
    }
}
