//! Pharmacokinetics: compartment models, PK/PD parameters, Emax model.

use crate::core::types::Scalar;

/// Multi-compartment PK model.
pub struct CompartmentModel {
    pub volumes: Vec<Scalar>,
    pub clearance: Vec<Vec<Scalar>>,
}

impl CompartmentModel {
    /// One-compartment IV bolus: C(t) = (Dose/Vd)·exp(-ke·t)
    pub fn one_compartment(dose: Scalar, vd: Scalar, ke: Scalar, t: Scalar) -> Scalar {
        (dose / vd) * (-ke * t).exp()
    }

    /// One-compartment oral absorption (first-order):
    /// C(t) = (F·Dose·ka) / (Vd·(ka - ke)) · (exp(-ke·t) - exp(-ka·t))
    pub fn two_compartment_oral(ka: Scalar, ke: Scalar, vd: Scalar, dose: Scalar, t: Scalar, f: Scalar) -> Scalar {
        if (ka - ke).abs() < 1e-15 {
            // Absorption and elimination rates equal — use limiting form
            (f * dose / vd) * t * (-ke * t).exp()
        } else {
            (f * dose * ka) / (vd * (ka - ke)) * ((-ke * t).exp() - (-ka * t).exp())
        }
    }

    /// IV infusion steady-state concentration: C_ss = R_inf / CL
    pub fn iv_infusion_steady_state(infusion_rate: Scalar, clearance: Scalar) -> Scalar {
        infusion_rate / clearance
    }

    /// Simulate a multi-compartment model with Euler integration.
    ///
    /// `doses` — list of (time, amount) for input into the first compartment.
    /// Returns a Vec of concentration time-courses, one per compartment.
    pub fn simulate(&self, doses: &[(Scalar, Scalar)], dt: Scalar, t_end: Scalar, n_comp: usize) -> Vec<Vec<Scalar>> {
        let n = self.volumes.len().min(n_comp);
        let mut amounts = vec![0.0; n];
        let mut results: Vec<Vec<Scalar>> = vec![Vec::new(); n];
        let mut t = 0.0;
        let mut dose_idx = 0;

        while t < t_end {
            // Apply any doses at this time
            while dose_idx < doses.len() && (doses[dose_idx].0 - t).abs() < dt * 0.5 {
                amounts[0] += doses[dose_idx].1;
                dose_idx += 1;
            }

            // Compute derivatives
            let mut damounts = vec![0.0; n];
            for i in 0..n {
                for j in 0..n {
                    if i != j {
                        // Transfer from compartment j to i
                        damounts[i] += self.clearance[i][j] * amounts[j] / self.volumes[j];
                    }
                }
                // Elimination from compartment i
                if i < self.clearance.len() && i < self.clearance[i].len() {
                    damounts[i] -= self.clearance[i][i] * amounts[i] / self.volumes[i];
                }
            }

            // Euler step: damounts are mass rates (mass/time), so update the
            // amount directly (no extra volume factor).
            for i in 0..n {
                amounts[i] += damounts[i] * dt;
                if amounts[i] < 0.0 {
                    amounts[i] = 0.0;
                }
            }

            // Record concentrations
            for i in 0..n {
                results[i].push(amounts[i] / self.volumes[i]);
            }

            t += dt;
        }

        results
    }
}

/// Pharmacokinetic / pharmacodynamic parameters.
pub struct PkPdParams {
    pub bioavailability: Scalar,
    pub vd: Scalar,
    pub clearance: Scalar,
    pub half_life: Scalar,
    pub ec50: Scalar,
    pub e_max: Scalar,
    pub hill_coefficient: Scalar,
}

/// Emax (Hill) pharmacodynamic model.
///
/// E(C) = (E_max · C^Hill) / (EC50^Hill + C^Hill)
pub fn emax_model(concentration: Scalar, e_max: Scalar, ec50: Scalar, hill: Scalar) -> Scalar {
    // Use the fractional Hill exponent directly (powf) instead of truncating
    // to an integer, which would distort the dose-response curve.
    let c_hill = if concentration > 0.0 {
        concentration.powf(hill)
    } else if hill > 0.0 {
        0.0
    } else {
        f64::INFINITY
    };
    let ec50_hill = if ec50 > 0.0 { ec50.powf(hill) } else { 0.0 };
    let denom = ec50_hill + c_hill;
    if denom.is_finite() && denom > 0.0 {
        (e_max * c_hill) / denom
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::needless_range_loop)]
    use super::*;

    #[test]
    fn test_one_compartment_t0() {
        let c = CompartmentModel::one_compartment(100.0, 50.0, 0.1, 0.0);
        assert!((c - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_one_compartment_decay() {
        let c0 = CompartmentModel::one_compartment(100.0, 50.0, 0.1, 0.0);
        let c1 = CompartmentModel::one_compartment(100.0, 50.0, 0.1, 10.0);
        assert!(c1 < c0);
        assert!(c1 > 0.0);
    }

    #[test]
    fn test_two_compartment_oral_t0() {
        let c = CompartmentModel::two_compartment_oral(1.0, 0.1, 50.0, 100.0, 0.0, 1.0);
        assert!((c).abs() < 1e-10);
    }

    #[test]
    fn test_two_compartment_oral_peak() {
        let c = CompartmentModel::two_compartment_oral(1.0, 0.1, 50.0, 100.0, 5.0, 1.0);
        assert!(c > 0.0);
    }

    #[test]
    fn test_two_compartment_oral_equal_rates() {
        let c = CompartmentModel::two_compartment_oral(0.5, 0.5, 50.0, 100.0, 2.0, 1.0);
        assert!(c > 0.0);
    }

    #[test]
    fn test_iv_infusion_steady_state() {
        let css = CompartmentModel::iv_infusion_steady_state(50.0, 10.0);
        assert!((css - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_simulate_one_compartment() {
        let model = CompartmentModel {
            volumes: vec![50.0],
            clearance: vec![vec![0.1]],
        };
        let doses = vec![(0.0, 100.0)];
        let results = model.simulate(&doses, 0.1, 1.0, 1);
        assert!(!results.is_empty());
        assert!(!results[0].is_empty());
        assert!(results[0][0] > 0.0);
    }

    #[test]
    fn test_emax_model_zero_conc() {
        let e = emax_model(0.0, 100.0, 10.0, 1.0);
        assert!((e).abs() < 1e-10);
    }

    #[test]
    fn test_emax_model_at_ec50() {
        let e = emax_model(10.0, 100.0, 10.0, 1.0);
        assert!((e - 50.0).abs() < 1e-8);
    }

    #[test]
    fn test_emax_model_saturation() {
        let e = emax_model(1000.0, 100.0, 10.0, 1.0);
        assert!((e - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_emax_model_hill_coefficient() {
        let e_sharp = emax_model(8.0, 100.0, 10.0, 4.0);
        let e_flat = emax_model(8.0, 100.0, 10.0, 1.0);
        // Higher Hill coefficient gives sharper response below EC50
        assert!(e_sharp < e_flat);
    }
}
