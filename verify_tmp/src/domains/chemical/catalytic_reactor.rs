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

    pub fn profile(&self, inlet: &[Scalar], kinetics: &ReactionKinetics, t: Scalar) -> Result<Vec<Vec<Scalar>>, String> {
        let n = self.activity.len().max(2);
        let dz = self.length / n as Scalar;
        let mut profiles = vec![vec![0.0; inlet.len()]; n];
        profiles[0].copy_from_slice(inlet);
        let mut conc = inlet.to_vec();
        for i in 1..n {
            // Catalyst activity decays exponentially with bed depth.
            let z = i as Scalar * dz;
            let activity = (-self.deactivation_rate * z).exp().max(0.0);
            let derivs = kinetics.concentration_derivatives(&conc, t);
            for j in 0..conc.len() {
                // Reaction rate scaled by local catalyst activity.
                conc[j] += derivs[j] * activity * dz;
                if conc[j] < 0.0 {
                    conc[j] = 0.0;
                }
            }
            profiles[i].copy_from_slice(&conc);
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
        // A → B with k = 1.0, consistent with the 2-species inlet.
        let kin = ReactionKinetics::new(vec![1.0], vec![vec![-1.0, 1.0]]);
        let p = r.profile(&[1.0, 0.0], &kin, 300.0).unwrap();
        assert_eq!(p.len(), 5);
        // Reactant is consumed along the bed; product is formed.
        assert!(p[4][0] < p[0][0]);
        assert!(p[4][1] > p[0][1]);
    }
}
