//! Tumor growth models: Gompertz kinetics, treatment response, invasion.

use crate::core::types::Scalar;

/// Tumor growth and treatment model.
pub struct TumorModel {
    pub growth_rate: Scalar,
    pub carrying_capacity: Scalar,
    pub initial_volume: Scalar,
}

impl TumorModel {
    /// Gompertz growth model: V(t) = K·(V₀/K)^(exp(-r·t))
    ///
    /// r = growth_rate, K = carrying_capacity, V₀ = initial_volume.
    pub fn gompertz_growth(&self, t: Scalar) -> Scalar {
        let exponent = (-self.growth_rate * t).exp();
        self.carrying_capacity * (self.initial_volume / self.carrying_capacity).powf(exponent)
    }

    /// Single-step treatment response (exponential kill).
    ///
    /// dV/dt = -k·C·V → V(t+dt) = V₀·exp(-k·C·dt)
    pub fn treatment_response(&self, drug_conc: Scalar, kill_rate: Scalar, dt: Scalar) -> Scalar {
        self.initial_volume * (-kill_rate * drug_conc * dt).exp()
    }

    /// Invasion depth from diffusion: d(t) = 2·√(D·t)
    pub fn invasion_depth(&self, t: Scalar, diffusivity: Scalar) -> Scalar {
        2.0 * (diffusivity * t).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gompertz_growth_at_t0() {
        let tm = TumorModel {
            growth_rate: 0.1,
            carrying_capacity: 100.0,
            initial_volume: 0.1,
        };
        let v0 = tm.gompertz_growth(0.0);
        assert!((v0 - 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_gompertz_growth_increases() {
        let tm = TumorModel {
            growth_rate: 0.1,
            carrying_capacity: 100.0,
            initial_volume: 0.1,
        };
        let v1 = tm.gompertz_growth(10.0);
        let v2 = tm.gompertz_growth(50.0);
        assert!(v1 > 0.1);
        assert!(v2 > v1);
    }

    #[test]
    fn test_gompertz_growth_approaches_carrying_capacity() {
        let tm = TumorModel {
            growth_rate: 0.2,
            carrying_capacity: 100.0,
            initial_volume: 0.1,
        };
        let v = tm.gompertz_growth(100.0);
        // Should approach but not exceed carrying capacity
        assert!(v < 100.5);
        assert!(v > 50.0);
    }

    #[test]
    fn test_gompertz_growth_no_growth() {
        let tm = TumorModel {
            growth_rate: 0.0,
            carrying_capacity: 100.0,
            initial_volume: 0.1,
        };
        let v = tm.gompertz_growth(100.0);
        assert!((v - 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_treatment_response_reduces_volume() {
        let tm = TumorModel {
            growth_rate: 0.1,
            carrying_capacity: 100.0,
            initial_volume: 10.0,
        };
        let v_new = tm.treatment_response(1.0, 0.5, 1.0);
        assert!(v_new < 10.0);
        assert!(v_new > 0.0);
    }

    #[test]
    fn test_treatment_response_no_effect() {
        let tm = TumorModel {
            growth_rate: 0.1,
            carrying_capacity: 100.0,
            initial_volume: 10.0,
        };
        let v_new = tm.treatment_response(0.0, 0.5, 1.0);
        assert!((v_new - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_invasion_depth_increases_with_time() {
        let tm = TumorModel {
            growth_rate: 0.1,
            carrying_capacity: 100.0,
            initial_volume: 0.1,
        };
        let d1 = tm.invasion_depth(10.0, 1e-9);
        let d2 = tm.invasion_depth(40.0, 1e-9);
        assert!(d2 > d1);
        assert!(d1 > 0.0);
    }

    #[test]
    fn test_invasion_depth_at_t0() {
        let tm = TumorModel {
            growth_rate: 0.1,
            carrying_capacity: 100.0,
            initial_volume: 0.1,
        };
        let d = tm.invasion_depth(0.0, 1e-9);
        assert!((d).abs() < 1e-12);
    }

    #[test]
    fn test_invasion_depth_scales_with_diffusivity() {
        let tm = TumorModel {
            growth_rate: 0.1,
            carrying_capacity: 100.0,
            initial_volume: 0.1,
        };
        let d_low = tm.invasion_depth(10.0, 1e-10);
        let d_high = tm.invasion_depth(10.0, 1e-8);
        assert!(d_high > d_low);
    }
}
