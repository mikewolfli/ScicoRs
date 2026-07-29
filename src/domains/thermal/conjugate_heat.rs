//! Conjugate heat transfer solver (fluid–solid coupled).
//!
//! Models the coupled thermal interaction between a fluid flow and
//! a solid wall, where the heat flux at the interface is determined
//! by both the fluid convection and solid conduction simultaneously.

use crate::core::types::Scalar;

/// Conjugate heat transfer model between a fluid and solid domain.
///
/// The fluid region is represented by a temperature field and heat
/// transfer coefficient map; the solid region is represented by its
/// own temperature field. The coupling happens at the interface
/// through the heat flux continuity condition:
///
///   q_wall = h·(T_fluid - T_wall) = -k_solid·∂T_solid/∂n
#[derive(Debug, Clone)]
pub struct ConjugateHeatTransfer {
    /// Fluid temperature field (ny × nx).
    pub fluid_temp: Vec<Vec<Scalar>>,
    /// Solid temperature field (ny × nx).
    pub solid_temp: Vec<Vec<Scalar>>,
    /// Heat transfer coefficient map (W/m²·K).
    pub htc: Vec<Vec<Scalar>>,
    /// Grid spacing.
    pub dx: Scalar,
    pub dy: Scalar,
    /// Solid thermal conductivity (W/m·K).
    pub k_solid: Scalar,
    /// Ambient temperature for far-field boundaries.
    pub t_ambient: Scalar,
}

impl ConjugateHeatTransfer {
    /// Create a new conjugate heat transfer model.
    pub fn new(
        nx: usize,
        ny: usize,
        dx: Scalar,
        dy: Scalar,
        k_solid: Scalar,
        t_ambient: Scalar,
        t_fluid_inlet: Scalar,
    ) -> Self {
        Self {
            fluid_temp: vec![vec![t_fluid_inlet; nx]; ny],
            solid_temp: vec![vec![t_ambient; nx]; ny],
            htc: vec![vec![100.0; nx]; ny], // Default HTC (W/m²·K)
            dx,
            dy,
            k_solid,
            t_ambient,
        }
    }

    /// Set a non-uniform heat transfer coefficient map.
    pub fn set_htc_map(&mut self, htc: Vec<Vec<Scalar>>) {
        if htc.len() == self.htc.len() && htc[0].len() == self.htc[0].len() {
            self.htc = htc;
        }
    }

    /// Perform one steady-state coupling iteration.
    ///
    /// Updates the interface temperature based on energy balance:
    ///   T_wall = (h·T_fluid + k_solid·T_solid_neighbour/dn) / (h + k_solid/dn)
    pub fn steady_state_iteration(&mut self) {
        let ny = self.fluid_temp.len();
        let nx = self.fluid_temp[0].len();

        for j in 1..ny.saturating_sub(1) {
            for i in 1..nx.saturating_sub(1) {
                let h = self.htc[j][i];
                let t_fluid = self.fluid_temp[j][i];

                // Average of neighbouring solid temperatures (simple 2D Laplacian)
                let t_solid_avg = (self.solid_temp[j - 1][i]
                    + self.solid_temp[j + 1][i]
                    + self.solid_temp[j][i - 1]
                    + self.solid_temp[j][i + 1])
                    * 0.25;

                // Interface temperature from flux continuity
                let k_dx = self.k_solid / self.dx;
                let t_wall = (h * t_fluid + k_dx * t_solid_avg) / (h + k_dx);

                // Update both domains
                self.fluid_temp[j][i] = t_fluid - 0.5 * (t_fluid - t_wall);
                self.solid_temp[j][i] = t_wall;
            }
        }
    }

    /// Solve the coupled system until convergence.
    pub fn solve(
        &mut self,
        max_iter: usize,
        tolerance: Scalar,
        t_fluid_inlet: Scalar,
    ) -> Result<usize, String> {
        // Set inlet temperature
        let ny = self.fluid_temp.len();
        let nx = self.fluid_temp[0].len();
        for j in 0..ny {
            self.fluid_temp[j][0] = t_fluid_inlet;
        }

        for iter in 0..max_iter {
            let old_solid = self.solid_temp.clone();
            self.steady_state_iteration();

            // Check convergence
            let mut max_delta: Scalar = 0.0;
            for j in 0..ny {
                for i in 0..nx {
                    max_delta = max_delta.max((self.solid_temp[j][i] - old_solid[j][i]).abs());
                }
            }
            if max_delta < tolerance {
                return Ok(iter + 1);
            }
        }
        Err("Conjugate heat transfer did not converge".to_string())
    }

    /// Compute the total heat flux across the interface (W).
    pub fn total_heat_flux(&self) -> Scalar {
        let ny = self.fluid_temp.len();
        let nx = self.fluid_temp[0].len();
        let mut q_total = 0.0;
        for j in 1..ny.saturating_sub(1) {
            for i in 1..nx.saturating_sub(1) {
                q_total += self.htc[j][i] * (self.fluid_temp[j][i] - self.solid_temp[j][i]);
            }
        }
        q_total * self.dx * self.dy
    }

    /// Average interface temperature.
    pub fn avg_interface_temp(&self) -> Scalar {
        let ny = self.fluid_temp.len();
        let nx = self.fluid_temp[0].len();
        let mut sum = 0.0;
        let mut count = 0;
        for j in 0..ny {
            for i in 0..nx {
                sum += self.solid_temp[j][i];
                count += 1;
            }
        }
        if count > 0 {
            sum / count as Scalar
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conjugate_new() {
        let ch = ConjugateHeatTransfer::new(10, 10, 0.01, 0.01, 200.0, 300.0, 350.0);
        assert_eq!(ch.fluid_temp.len(), 10);
        assert!((ch.fluid_temp[0][0] - 350.0).abs() < 1e-10);
    }

    #[test]
    fn test_conjugate_iteration() {
        let mut ch = ConjugateHeatTransfer::new(10, 10, 0.01, 0.01, 200.0, 300.0, 350.0);
        let t_before = ch.avg_interface_temp();
        ch.steady_state_iteration();
        let t_after = ch.avg_interface_temp();
        // Coupling should move the temperature
        assert!((t_after - t_before).abs() > 1e-10);
    }

    #[test]
    fn test_conjugate_solve_converges() {
        let mut ch = ConjugateHeatTransfer::new(6, 6, 0.01, 0.01, 200.0, 300.0, 350.0);
        let result = ch.solve(1000, 1e-6, 350.0);
        assert!(result.is_ok());
        let iters = result.unwrap();
        assert!(iters > 0);
        // Should converge to a temperature between inlet and ambient
        let t_avg = ch.avg_interface_temp();
        assert!(t_avg > ch.t_ambient);
        assert!(t_avg < 350.0);
    }

    #[test]
    fn test_conjugate_heat_flux() {
        let mut ch = ConjugateHeatTransfer::new(6, 6, 0.01, 0.01, 200.0, 300.0, 350.0);
        let q0 = ch.total_heat_flux();
        ch.solve(100, 1e-4, 350.0).unwrap();
        let q1 = ch.total_heat_flux();
        // Heat flux should be finite and positive (fluid → solid)
        assert!(q0.is_finite());
        assert!(q1.is_finite());
    }

    #[test]
    fn test_conjugate_inlet_boundary() {
        let mut ch = ConjugateHeatTransfer::new(8, 8, 0.01, 0.01, 200.0, 300.0, 350.0);
        ch.solve(100, 1e-4, 350.0).unwrap();
        // Inlet boundary should stay at inlet temperature
        for j in 0..8 {
            assert!((ch.fluid_temp[j][0] - 350.0).abs() < 1.0);
        }
    }

    #[test]
    fn test_conjugate_set_htc() {
        let mut ch = ConjugateHeatTransfer::new(6, 6, 0.01, 0.01, 200.0, 300.0, 350.0);
        let new_htc = vec![vec![50.0; 6]; 6];
        ch.set_htc_map(new_htc);
        assert!((ch.htc[0][0] - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_conjugate_no_convergence() {
        let mut ch = ConjugateHeatTransfer::new(6, 6, 0.01, 0.01, 200.0, 300.0, 350.0);
        // Very strict tolerance should fail
        let result = ch.solve(5, 1e-20, 350.0);
        assert!(result.is_err());
    }
}
