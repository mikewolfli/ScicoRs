//! Thermal Protection System (TPS): multi-layer heat shield modeling,
//! transient thermal response, and structural dynamics utilities.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// Thermal Protection System
// ──────────────────────────────────────────────

/// A single layer of a thermal protection system.
///
/// Each layer is defined by its material properties and geometric thickness.
pub struct TpsLayer {
    /// Layer thickness (m).
    pub thickness: Scalar,
    /// Thermal conductivity (W/(m·K)).
    pub k: Scalar,
    /// Specific heat capacity (J/(kg·K)).
    pub cp: Scalar,
    /// Density (kg/m³).
    pub rho: Scalar,
    /// Maximum service temperature (K).
    pub max_temp: Scalar,
    /// Surface emissivity (dimensionless, 0–1).
    pub emissivity: Scalar,
}

/// Multi-layer thermal protection system.
///
/// Solves the 1D transient heat conduction equation through a layered stack
/// using an explicit finite-difference scheme.
pub struct ThermalProtectionSystem {
    /// Ordered list of TPS layers (outermost first).
    pub layers: Vec<TpsLayer>,
}

impl ThermalProtectionSystem {
    /// Compute the transient thermal response of the TPS stack.
    ///
    /// Uses an explicit 1D finite-difference heat conduction model.
    ///
    /// Returns a vector of temperature profiles, one per time step, where each
    /// profile is a `Vec<Scalar>` of nodal temperatures from the outer surface
    /// to the inner surface.
    ///
    /// * `heat_flux` — applied heat flux at outer surface (W/m²)
    /// * `t_initial` — initial uniform temperature (K)
    /// * `t_end` — total simulation time (s)
    /// * `dt` — time step (s)
    pub fn thermal_response(
        &self,
        heat_flux: Scalar,
        t_initial: Scalar,
        t_end: Scalar,
        dt: Scalar,
    ) -> Vec<Vec<Scalar>> {
        if self.layers.is_empty() || dt <= 0.0 || t_end <= 0.0 {
            return vec![vec![t_initial]];
        }

        // Build mesh: allocate at least 5 nodes per layer
        let nodes_per_layer = 5usize;
        let total_nodes: usize = self.layers.len() * nodes_per_layer;
        let mut temperatures = vec![t_initial; total_nodes];

        // Build mesh coordinates and material properties
        let mut x = Vec::with_capacity(total_nodes);
        let mut k = Vec::with_capacity(total_nodes);
        let mut rho_cp = Vec::with_capacity(total_nodes);

        let mut x_curr = 0.0;
        for layer in &self.layers {
            let dx = layer.thickness / (nodes_per_layer as Scalar);
            for j in 0..nodes_per_layer {
                x.push(x_curr + (j as Scalar + 0.5) * dx);
                k.push(layer.k);
                rho_cp.push(layer.rho * layer.cp);
            }
            x_curr += layer.thickness;
        }

        let n_steps = (t_end / dt).ceil() as usize;
        let mut result = Vec::with_capacity(n_steps + 1);
        result.push(temperatures.clone());

        for _ in 0..n_steps {
            let prev = temperatures.clone();

            // Interior nodes (explicit Euler)
            for i in 1..(total_nodes - 1) {
                let dx_left = x[i] - x[i - 1];
                let dx_right = x[i + 1] - x[i];
                let dx_avg = 0.5 * (dx_left + dx_right);

                let flux_left = k[i] * (prev[i] - prev[i - 1]) / dx_left;
                let flux_right = k[i] * (prev[i + 1] - prev[i]) / dx_right;

                let d2t = (flux_right - flux_left) / dx_avg;
                temperatures[i] = prev[i] + dt * d2t / rho_cp[i];
            }

            // Outer surface (node 0): applied heat flux + radiation
            let dx0 = x[1] - x[0];
            let sigma = 5.670374419e-8; // Stefan-Boltzmann
            let t_surf = prev[0];
            let q_rad = sigma * self.layers[0].emissivity * t_surf.powi(4);
            let q_net = heat_flux - q_rad;
            let flux_surface = k[0] * (prev[1] - prev[0]) / dx0;
            temperatures[0] = prev[0] + dt * (q_net - flux_surface) / (dx0 * rho_cp[0]);

            // Inner surface (last node): adiabatic (insulated)
            let last = total_nodes - 1;
            let dx_last = x[last] - x[last - 1];
            temperatures[last] = prev[last] + dt * k[last] * (prev[last - 1] - prev[last])
                / (dx_last * dx_last * rho_cp[last]);

            result.push(temperatures.clone());
        }

        result
    }

    /// Compute the back-face (inner surface) temperature after a given
    /// duration of constant heat flux application.
    ///
    /// Uses the full `thermal_response` and returns the final inner node
    /// temperature.
    pub fn back_face_temperature(&self, heat_flux: Scalar, duration: Scalar) -> Scalar {
        let response = self.thermal_response(heat_flux, 300.0, duration, duration.max(0.1) / 20.0);
        if let Some(&t) = response.last().and_then(|lp| lp.last()) {
            return t;
        }
        300.0
    }

    /// Compute the total heat capacity of the TPS stack (J/K).
    ///
    /// Sum of mᵢ · cpᵢ across all layers.
    pub fn total_heat_capacity(&self) -> Scalar {
        self.layers
            .iter()
            .map(|l| l.rho * l.thickness * l.cp)
            .sum()
    }
}

// ──────────────────────────────────────────────
// Structural Dynamics Utilities
// ──────────────────────────────────────────────

/// Load factor (g's) from total force and mass.
///
/// n = F_total / (m · g₀)
pub fn load_factor(total_force: Scalar, mass: Scalar) -> Scalar {
    if mass <= 0.0 {
        return 0.0;
    }
    total_force / (mass * 9.80665)
}

/// Shock response spectrum (SRS) computed as the peak response of a series
/// of single-degree-of-freedom (SDOF) systems subjected to a base acceleration
/// time history.
///
/// * `natural_freqs` — array of natural frequencies (Hz) for the SDOF oscillators
/// * `base_acceleration` — time-history of base acceleration as [(time_s, accel_m_s2), ...]
/// * `damping` — damping ratio (ζ), typically 0.01–0.05
///
/// Returns the peak absolute acceleration response for each natural frequency.
pub fn shock_response_sweep(
    natural_freqs: &[Scalar],
    base_acceleration: &[(Scalar, Scalar)],
    damping: Scalar,
) -> Vec<Scalar> {
    if natural_freqs.is_empty() || base_acceleration.len() < 2 {
        return Vec::new();
    }

    let mut results = Vec::with_capacity(natural_freqs.len());

    for &fn_hz in natural_freqs {
        let omega_n = 2.0 * core::f64::consts::PI * fn_hz;
        if omega_n <= 0.0 {
            results.push(0.0);
            continue;
        }

        // SDOF system: x'' + 2ζωₙ x' + ωₙ² x = -a_base(t)
        // Use Newmark-beta (average acceleration) integration
        let mut x = 0.0;
        let mut v = 0.0;
        let mut peak = 0.0;

        let gamma = 0.5;
        let beta = 0.25; // average acceleration (unconditionally stable)

        let zeta = damping;
        let _omega_d = omega_n * (1.0 - zeta * zeta).sqrt();

        for i in 1..base_acceleration.len() {
            let (t_prev, a_prev) = base_acceleration[i - 1];
            let (t_curr, a_curr) = base_acceleration[i];
            let dt = t_curr - t_prev;
            if dt <= 0.0 {
                continue;
            }

            let a_base_prev = -a_prev; // effective force
            let a_base_curr = -a_curr;

            // Newmark-beta predictor-corrector
            let k_eff = omega_n * omega_n + gamma * omega_n * 2.0 * zeta / (beta * dt)
                + 1.0 / (beta * dt * dt);

            let df = a_base_curr - a_base_prev
                + (omega_n * 2.0 * zeta / (beta * dt) + 1.0 / (beta * dt * dt)) * x
                + (omega_n * 2.0 * zeta / beta + 1.0 / (beta * dt)) * v;

            let dx = df / k_eff;

            let dv = (gamma / (beta * dt)) * dx - (gamma / beta) * v
                + dt * (1.0 - gamma / (2.0 * beta)) * a_base_prev;

            x += dx;
            v += dv;

            let accel = -2.0 * zeta * omega_n * v - omega_n * omega_n * x;
            let abs_accel = accel.abs();
            if abs_accel > peak {
                peak = abs_accel;
            }
        }

        results.push(peak);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tps_total_heat_capacity() {
        let tps = ThermalProtectionSystem {
            layers: vec![
                TpsLayer {
                    thickness: 0.01,
                    k: 0.5,
                    cp: 1000.0,
                    rho: 2000.0,
                    max_temp: 2000.0,
                    emissivity: 0.8,
                },
                TpsLayer {
                    thickness: 0.02,
                    k: 0.3,
                    cp: 800.0,
                    rho: 1500.0,
                    max_temp: 1500.0,
                    emissivity: 0.6,
                },
            ],
        };
        let cap = tps.total_heat_capacity();
        // Layer 1: 2000 * 0.01 * 1000 = 20000
        // Layer 2: 1500 * 0.02 * 800 = 24000
        // Total: 44000
        assert!((cap - 44_000.0).abs() < 0.1);
    }

    #[test]
    fn test_tps_thermal_response_nonempty() {
        let tps = ThermalProtectionSystem {
            layers: vec![TpsLayer {
                thickness: 0.02,
                k: 0.5,
                cp: 1000.0,
                rho: 2000.0,
                max_temp: 2000.0,
                emissivity: 0.8,
            }],
        };
        let response = tps.thermal_response(100_000.0, 300.0, 1.0, 0.01);
        assert!(!response.is_empty());
        // Outer surface should heat up
        let final_profile = response.last().unwrap();
        assert!(final_profile[0] > 300.0);
    }

    #[test]
    fn test_tps_back_face_temperature() {
        let tps = ThermalProtectionSystem {
            layers: vec![TpsLayer {
                thickness: 0.05,
                k: 0.3,
                cp: 800.0,
                rho: 1500.0,
                max_temp: 1500.0,
                emissivity: 0.6,
            }],
        };
        let t_back = tps.back_face_temperature(50_000.0, 10.0);
        assert!(t_back >= 300.0);
    }

    #[test]
    fn test_tps_empty_layers() {
        let tps = ThermalProtectionSystem {
            layers: vec![],
        };
        let response = tps.thermal_response(100_000.0, 300.0, 1.0, 0.01);
        assert_eq!(response.len(), 1);
    }

    #[test]
    fn test_load_factor_positive() {
        let n = load_factor(500_000.0, 50_000.0);
        assert!((n - 1.0197).abs() < 0.01);
    }

    #[test]
    fn test_load_factor_zero_mass() {
        let n = load_factor(1000.0, 0.0);
        assert!((n).abs() < 1e-10);
    }

    #[test]
    fn test_shock_response_sweep_empty() {
        let result = shock_response_sweep(&[], &[(0.0, 0.0)], 0.05);
        assert!(result.is_empty());
    }

    #[test]
    fn test_shock_response_sweep_single_freq() {
        let freqs = [10.0];
        let base: Vec<(Scalar, Scalar)> = (0..100).map(|i| {
            let t = i as Scalar * 0.001;
            let a = if t < 0.01 { 100.0 } else { 0.0 };
            (t, a)
        }).collect();
        let result = shock_response_sweep(&freqs, &base, 0.05);
        assert_eq!(result.len(), 1);
        assert!(result[0] > 0.0);
    }

    #[test]
    fn test_shock_response_sweep_multiple_freqs() {
        let freqs = [10.0, 100.0, 1000.0];
        let base = vec![(0.0, 0.0), (0.001, 100.0), (0.01, 0.0), (1.0, 0.0)];
        let result = shock_response_sweep(&freqs, &base, 0.05);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_tps_layer_fields() {
        let layer = TpsLayer {
            thickness: 0.01,
            k: 0.5,
            cp: 1000.0,
            rho: 2000.0,
            max_temp: 2000.0,
            emissivity: 0.8,
        };
        assert!(layer.thickness > 0.0);
        assert!(layer.emissivity > 0.0 && layer.emissivity <= 1.0);
    }
}
