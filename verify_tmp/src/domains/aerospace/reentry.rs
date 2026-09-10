//! Reentry trajectory simulation (simplified).

use crate::core::types::Scalar;
use crate::domains::aerospace::physics::IsaAtmosphere;
#[derive(Debug, Clone)]
pub struct ReentryState {
    pub altitude: Scalar,
    pub velocity: Scalar,
    pub heat_flux: Scalar,
    pub deceleration: Scalar,
}
#[derive(Debug, Clone)]
pub struct ReentryTrajectory {
    pub states: Vec<ReentryState>,
    pub ballistic_coefficient: Scalar,
}
impl ReentryTrajectory {
    pub fn new() -> Self {
        Self {
            states: Vec::new(),
            ballistic_coefficient: 100.0,
        }
    }
    pub fn propagate(&mut self, dt: Scalar, t_end: Scalar) -> Result<(), String> {
        if dt <= 0.0 {
            return Err(String::from("bad dt"));
        }
        let mut a = 120000.0_f64;
        let mut v = 7800.0_f64;
        self.states.clear();
        for _ in 0..=(t_end / dt) as usize {
            let rho = IsaAtmosphere::density(a);
            let d = 0.5 * rho * v * v * 12.0 / self.ballistic_coefficient;
            let de = d + 9.81 * (6371e3 / (6371e3 + a)).powi(2);
            let q = 1.83e-8 * rho.powf(0.5) * v.powi(3);
            self.states.push(ReentryState {
                altitude: a,
                velocity: v,
                heat_flux: q,
                deceleration: de,
            });
            v -= de * dt;
            a -= v * dt;
            if a <= 0.0 {
                break;
            }
        }
        Ok(())
    }
    pub fn max_heat_flux(&self) -> Scalar {
        let heat_fluxes: Vec<Scalar> = self.states.iter().map(|s| s.heat_flux).collect();
        crate::core::compute::vector::vec_max_abs(&heat_fluxes).unwrap_or(0.0)
    }
}
impl Default for ReentryTrajectory {
    fn default() -> Self {
        Self::new()
    }
}
