//! Phase change models: melting, solidification, and evaporation.
//!
//! Provides a 1D phase change solver using the effective heat capacity
//! method for melting/solidification problems, and an evaporation rate
//! model based on mass transfer.

use crate::core::types::Scalar;

/// 1D phase change solver using the effective heat capacity method.
///
/// Models melting/solidification by smoothing the latent heat over a
/// small temperature interval around the melting point. The effective
/// heat capacity cp_eff = cp + L / ΔT over the mush zone.
pub struct PhaseChange1D {
    /// Thermal conductivity of the solid phase (W/(m·K)).
    pub k_solid: Scalar,
    /// Thermal conductivity of the liquid phase (W/(m·K)).
    pub k_liquid: Scalar,
    /// Latent heat of fusion (J/kg).
    pub latent_heat: Scalar,
    /// Melting temperature (K).
    pub melt_temp: Scalar,
    /// Specific heat capacity (J/(kg·K)).
    pub cp: Scalar,
    /// Density (kg/m³).
    pub rho: Scalar,
    /// Temperature at each grid point (K).
    pub temperature: Vec<Scalar>,
    /// Liquid fraction at each grid point (0.0 = solid, 1.0 = liquid).
    pub liquid_fraction: Vec<Scalar>,
}

impl PhaseChange1D {
    /// Compute the effective specific heat capacity at temperature T.
    ///
    /// The latent heat is smeared over a temperature interval of ±ΔT/2
    /// around the melting point:
    ///   cp_eff = cp + L / ΔT   if |T - T_melt| < ΔT/2
    ///   cp_eff = cp             otherwise
    pub fn effective_cp(&self, t: Scalar, delta_t: Scalar) -> Scalar {
        if delta_t <= 0.0 {
            return self.cp;
        }
        let half_band = delta_t / 2.0;
        if (t - self.melt_temp).abs() < half_band {
            self.cp + self.latent_heat / delta_t
        } else {
            self.cp
        }
    }

    /// Perform one explicit time step using the effective heat capacity method.
    ///
    /// The energy equation is:
    ///   ρ · cp_eff · ∂T/∂t = ∂/∂x (k(T) · ∂T/∂x)
    ///
    /// Temperature-dependent thermal conductivity is interpolated between
    /// solid and liquid values based on the liquid fraction.
    pub fn step(&mut self, dt: Scalar, t_left: Scalar, t_right: Scalar) -> Result<(), String> {
        let n = self.temperature.len();
        if n < 2 {
            return Err("PhaseChange1D requires at least 2 cells".to_string());
        }
        if dt <= 0.0 {
            return Err("Time step must be positive".to_string());
        }
        if self.rho <= 0.0 || self.cp <= 0.0 {
            return Err("Density and specific heat must be positive".to_string());
        }

        let dx = 1.0 / (n as Scalar); // normalized domain [0, 1]
        let dx2 = dx * dx;

        // Update liquid fraction based on current temperature
        let dt_mush = 1.0; // half-width of mush zone (K)
        for i in 0..n {
            let t = self.temperature[i];
            if t <= self.melt_temp - dt_mush {
                self.liquid_fraction[i] = 0.0;
            } else if t >= self.melt_temp + dt_mush {
                self.liquid_fraction[i] = 1.0;
            } else {
                self.liquid_fraction[i] = (t - (self.melt_temp - dt_mush)) / (2.0 * dt_mush);
            }
        }

        let mut new_temp = self.temperature.clone();

        for i in 1..(n - 1) {
            // Interpolate thermal conductivity based on liquid fraction
            let k_left = self.k_solid
                + (self.k_liquid - self.k_solid) * self.liquid_fraction[i - 1];
            let k_right = self.k_solid
                + (self.k_liquid - self.k_solid) * self.liquid_fraction[i + 1];
            let k_center = self.k_solid
                + (self.k_liquid - self.k_solid) * self.liquid_fraction[i];

            // Harmonic mean of conductivities at interfaces
            let k_east = 2.0 * k_center * k_right / (k_center + k_right + 1e-30);
            let k_west = 2.0 * k_center * k_left / (k_center + k_left + 1e-30);

            // Effective heat capacity
            let cp_eff = self.effective_cp(self.temperature[i], 2.0 * dt_mush);
            if cp_eff <= 0.0 {
                return Err("Effective heat capacity must be positive".to_string());
            }

            let diffusion = (k_east * (self.temperature[i + 1] - self.temperature[i])
                - k_west * (self.temperature[i] - self.temperature[i - 1]))
                / dx2;

            new_temp[i] = self.temperature[i] + dt * diffusion / (self.rho * cp_eff);

            // Clamp to valid temperature range
            if new_temp[i].is_nan() || new_temp[i].is_infinite() {
                new_temp[i] = self.temperature[i];
            }
        }

        // Apply fixed-temperature boundary conditions
        new_temp[0] = t_left;
        new_temp[n - 1] = t_right;

        self.temperature = new_temp;
        Ok(())
    }
}

/// Evaporation rate (kg/s) from a liquid surface.
///
/// ṁ = h_m · A · (p_sat - p_amb) / (R_specific · T)
///
/// where h_m is the mass transfer coefficient (m/s), A is the surface
/// area (m²), p_sat is the saturation pressure (Pa), p_amb is the ambient
/// partial pressure (Pa), R_specific is the specific gas constant (J/(kg·K)),
/// and T is the temperature (K). For water at 20°C, R_specific ≈ 461.5 J/(kg·K).
pub fn evaporation_rate(
    area: Scalar,
    pressure_sat: Scalar,
    pressure_ambient: Scalar,
    mass_transfer_coeff: Scalar,
) -> Scalar {
    if area <= 0.0 || mass_transfer_coeff <= 0.0 {
        return 0.0;
    }
    let delta_p = pressure_sat - pressure_ambient;
    if delta_p <= 0.0 {
        return 0.0;
    }
    // Simplified: assume water at 20°C, R_specific ≈ 461.5
    const R_SPECIFIC_WATER: Scalar = 461.5;
    const T_REF: Scalar = 293.15;
    mass_transfer_coeff * area * delta_p / (R_SPECIFIC_WATER * T_REF)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assertions_on_constants)]
    use super::*;

    #[test]
    fn test_effective_cp_solid() {
        let pc = PhaseChange1D {
            k_solid: 0.5,
            k_liquid: 0.6,
            latent_heat: 334000.0,
            melt_temp: 273.15,
            cp: 2000.0,
            rho: 1000.0,
            temperature: vec![260.0],
            liquid_fraction: vec![0.0],
        };
        let cp = pc.effective_cp(260.0, 2.0);
        // Far from melt: just cp
        assert!((cp - 2000.0).abs() < 1e-10);
    }

    #[test]
    fn test_effective_cp_mush_zone() {
        let pc = PhaseChange1D {
            k_solid: 0.5,
            k_liquid: 0.6,
            latent_heat: 334000.0,
            melt_temp: 273.15,
            cp: 2000.0,
            rho: 1000.0,
            temperature: vec![273.15],
            liquid_fraction: vec![0.5],
        };
        let cp = pc.effective_cp(273.15, 2.0);
        let expected = 2000.0 + 334000.0 / 2.0;
        assert!((cp - expected).abs() < 1e-10);
    }

    #[test]
    fn test_effective_cp_liquid() {
        let pc = PhaseChange1D {
            k_solid: 0.5,
            k_liquid: 0.6,
            latent_heat: 334000.0,
            melt_temp: 273.15,
            cp: 2000.0,
            rho: 1000.0,
            temperature: vec![300.0],
            liquid_fraction: vec![1.0],
        };
        let cp = pc.effective_cp(300.0, 2.0);
        // Far from melt: just cp
        assert!((cp - 2000.0).abs() < 1e-10);
    }

    #[test]
    fn test_phase_change_step_basic() {
        let mut pc = PhaseChange1D {
            k_solid: 0.5,
            k_liquid: 0.6,
            latent_heat: 334000.0,
            melt_temp: 273.15,
            cp: 2000.0,
            rho: 1000.0,
            temperature: vec![260.0, 265.0, 270.0, 275.0, 280.0],
            liquid_fraction: vec![0.0; 5],
        };
        let result = pc.step(0.1, 260.0, 300.0);
        assert!(result.is_ok());
        // Temperature should be finite
        for &t in &pc.temperature {
            assert!(t.is_finite());
        }
    }

    #[test]
    fn test_phase_change_step_invalid_cells() {
        let mut pc = PhaseChange1D {
            k_solid: 0.5,
            k_liquid: 0.6,
            latent_heat: 334000.0,
            melt_temp: 273.15,
            cp: 2000.0,
            rho: 1000.0,
            temperature: vec![300.0],
            liquid_fraction: vec![1.0],
        };
        assert!(pc.step(0.1, 300.0, 300.0).is_err());
    }

    #[test]
    fn test_phase_change_step_zero_dt() {
        let mut pc = PhaseChange1D {
            k_solid: 0.5,
            k_liquid: 0.6,
            latent_heat: 334000.0,
            melt_temp: 273.15,
            cp: 2000.0,
            rho: 1000.0,
            temperature: vec![260.0, 270.0, 280.0],
            liquid_fraction: vec![0.0; 3],
        };
        assert!(pc.step(0.0, 260.0, 280.0).is_err());
    }

    #[test]
    fn test_evaporation_rate_basic() {
        let rate = evaporation_rate(1.0, 2338.0, 1500.0, 0.01);
        assert!(rate > 0.0);
    }

    #[test]
    fn test_evaporation_rate_zero_area() {
        let rate = evaporation_rate(0.0, 2338.0, 1500.0, 0.01);
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_evaporation_rate_saturated() {
        // When p_sat <= p_ambient, no evaporation
        let rate = evaporation_rate(1.0, 1000.0, 1500.0, 0.01);
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_liquid_fraction_update() {
        let mut pc = PhaseChange1D {
            k_solid: 0.5,
            k_liquid: 0.6,
            latent_heat: 334000.0,
            melt_temp: 273.15,
            cp: 2000.0,
            rho: 1000.0,
            temperature: vec![260.0, 273.15, 280.0],
            liquid_fraction: vec![0.0; 3],
        };
        let _ = pc.step(0.1, 260.0, 280.0);
        // After step, liquid_fraction should be updated
        assert_eq!(pc.liquid_fraction[0], 0.0); // solid
        assert_eq!(pc.liquid_fraction[2], 1.0); // liquid
    }
}
