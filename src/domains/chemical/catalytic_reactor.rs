//! Catalytic fixed-bed reactor with catalyst deactivation.

use crate::core::types::Scalar;
use crate::domains::chemical::ReactionKinetics;

/// Catalytic reactor model.
#[derive(Debug, Clone)]
pub struct CatalyticReactor {
    pub length: Scalar, pub diameter: Scalar,
    pub epsilon: Scalar, pub rho_cat: Scalar,
    pub deactivation_rate: Scalar,
    pub activity: Vec<Scalar>,
}

impl CatalyticReactor {
    pub fn new(length: Scalar, diameter: Scalar, epsilon: Scalar, rho_cat: Scalar, deact_rate: Scalar, n_points: usize) -> Self {
        Self { length, diameter, epsilon, rho_cat, deactivation_rate: deact_rate, activity: vec![1.0; n_points] }
    }

    pub fn profile(&self, inlet: &[Scalar], _kinetics: &ReactionKinetics, _t: Scalar) -> Result<Vec<Vec<Scalar>>, String> {
        let n = self.activity.len();
        let dz = self.length / n.max(1) as Scalar;
        let mut profiles = vec![vec![0.0; inlet.len()]; n];
        if n > 0 { profiles[0].copy_from_slice(inlet); }
        for i in 1..n {
            let deact = (-self.deactivation_rate * i as Scalar * dz).exp();
            for j in 0..inlet.len() {
                profiles[i][j] = profiles[i - 1][j] * deact;
            }
        }
        Ok(profiles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::chemical::ReactionKinetics;
    #[test]
    fn test_reactor_new() {
        let r = CatalyticReactor::new(1.0, 0.1, 0.4, 2000.0, 0.01, 10);
        assert_eq!(r.activity.len(), 10);
    }
    #[test]
    fn test_profile() {
        let r = CatalyticReactor::new(1.0, 0.1, 0.4, 2000.0, 0.01, 5);
        let kin = ReactionKinetics::new(vec![1.0], vec![vec![-1.0]]);
        let p = r.profile(&[1.0, 0.5], &kin, 300.0).unwrap();
        assert_eq!(p.len(), 5);
    }
}
