//! Cardiac electromechanical coupling model (simplified).
use crate::core::types::Scalar;
#[derive(Debug, Clone)]
pub struct CardiacModel {
    pub v: Scalar,
    pub ca_transient: Vec<Scalar>,
    pub contractility: Scalar,
}
impl CardiacModel {
    pub fn new() -> Self {
        Self {
            v: -70.0,
            ca_transient: vec![0.0; 10],
            contractility: 1.0,
        }
    }
    pub fn step(&mut self, i_stim: Scalar, dt: Scalar) {
        self.v += dt * (0.1 * i_stim - 0.01 * (self.v + 70.0));
        // Calcium transient is driven by stimulation / depolarization and
        // decays with a time constant; it does not ramp unconditionally.
        let drive = i_stim.max(0.0) * 0.1 + (self.v + 70.0).max(0.0) * 0.002;
        for ca in self.ca_transient.iter_mut() {
            *ca = (*ca + dt * (drive - 0.5 * *ca)).clamp(0.0, 1.0);
        }
        // Contractility tracks the mean calcium level, linking the electrical
        // and mechanical sides of the model.
        let mean_ca: Scalar = self.ca_transient.iter().sum::<Scalar>()
            / self.ca_transient.len().max(1) as Scalar;
        self.contractility = 0.5 + 0.5 * mean_ca;
    }
    pub fn frank_starling(volume: Scalar, contractility: Scalar) -> Scalar {
        contractility * (0.5 + 0.5 * (-((volume - 100.0) / 30.0).powi(2)).exp()).max(0.0)
    }
}
impl Default for CardiacModel {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cardiac_new() {
        let cm = CardiacModel::new();
        assert_eq!(cm.ca_transient.len(), 10);
    }
    #[test]
    fn test_step() {
        let mut cm = CardiacModel::new();
        cm.step(10.0, 0.01);
        assert!(cm.ca_transient[0] > 0.0);
    }
    #[test]
    fn test_frank() {
        let sv = CardiacModel::frank_starling(100.0, 1.0);
        assert!(sv > 0.0);
    }
}
