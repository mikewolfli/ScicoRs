//! Chemical reaction kinetics models.
//!
//! Provides Arrhenius rate law, power-law kinetics, reversible reactions,
//! equilibrium constants, half-life calculations, and a ReactionKinetics
//! struct for multi-species, multi-reaction systems.

use crate::core::types::Scalar;

/// Arrhenius rate constant: k = A·exp(-Ea/(R·T)).
pub fn arrhenius_rate(a: Scalar, ea: Scalar, t: Scalar) -> Scalar {
    if t <= 0.0 {
        return 0.0;
    }
    a * (-ea / (crate::domains::chemical::physics::R * t)).exp()
}

/// Power-law reaction rate: r = k·[A]^α·[B]^β.
pub fn reaction_rate(k: Scalar, c_a: Scalar, c_b: Scalar, alpha: Scalar, beta: Scalar) -> Scalar {
    k * c_a.powf(alpha) * c_b.powf(beta)
}

/// Reversible reaction rate: r = k_f·[A] - k_r·[B].
pub fn reversible_rate(k_f: Scalar, k_r: Scalar, c_a: Scalar, c_b: Scalar) -> Scalar {
    k_f * c_a - k_r * c_b
}

/// Equilibrium constant from Gibbs free energy: K = exp(-ΔG/(R·T)).
pub fn equilibrium_constant(delta_g: Scalar, t: Scalar) -> Scalar {
    if t <= 0.0 {
        return 0.0;
    }
    (-delta_g / (crate::domains::chemical::physics::R * t)).exp()
}

/// Half-life for first-order reaction: t₁/₂ = ln(2)/k.
pub fn half_life_first_order(k: Scalar) -> Scalar {
    if k <= 0.0 {
        return f64::INFINITY;
    }
    (2.0_f64.ln()) / k
}

/// A multi-species, multi-reaction kinetic system.
///
/// Models a set of chemical reactions with their rate constants
/// and stoichiometric coefficients.
pub struct ReactionKinetics {
    /// Rate constants for each reaction (forward).
    pub rate_constants: Vec<Scalar>,
    /// Stoichiometric matrix: [reaction_index][species_index].
    pub stoichiometry: Vec<Vec<Scalar>>,
    /// Number of chemical species.
    pub species_count: usize,
    /// Number of reactions.
    pub reaction_count: usize,
}

impl ReactionKinetics {
    /// Create a new reaction kinetics system.
    ///
    /// # Panics
    ///
    /// Panics if the stoichiometry matrix is inconsistent with the
    /// rate constants vector (each reaction must have one rate constant)
    /// or if rows have inconsistent lengths.
    pub fn new(rate_constants: Vec<Scalar>, stoichiometry: Vec<Vec<Scalar>>) -> Self {
        let reaction_count = rate_constants.len();
        assert_eq!(
            stoichiometry.len(),
            reaction_count,
            "each reaction must have a stoichiometry row"
        );

        let species_count = if reaction_count > 0 {
            stoichiometry[0].len()
        } else {
            0
        };

        for (i, row) in stoichiometry.iter().enumerate() {
            assert_eq!(
                row.len(),
                species_count,
                "stoichiometry row {i} has inconsistent length"
            );
        }

        Self {
            rate_constants,
            stoichiometry,
            species_count,
            reaction_count,
        }
    }

    /// Compute the concentration derivatives dC/dt for each species.
    ///
    /// For each reaction j: r_j = k_j * Π_i C_i^{ν_{ij}} (for ν_{ij} < 0, reactants)
    /// dC_i/dt = Σ_j ν_{ij} * r_j
    pub fn concentration_derivatives(&self, concentrations: &[Scalar], _t: Scalar) -> Vec<Scalar> {
        assert_eq!(
            concentrations.len(),
            self.species_count,
            "concentration vector length must match species_count"
        );

        let rates = self.reaction_rates(concentrations, 0.0);
        let mut derivatives = vec![0.0; self.species_count];
        for (j, rate) in rates.iter().enumerate() {
            for i in 0..self.species_count {
                derivatives[i] += self.stoichiometry[j][i] * rate;
            }
        }
        derivatives
    }

    /// Compute the rate r_j of each reaction (mol/(m³·s)).
    ///
    /// For each reaction j: r_j = k_j · Πᵢ Cᵢ^{|νᵢⱼ|} over reactants (νᵢⱼ < 0).
    /// This is the per-reaction rate needed for heat generation.
    pub fn reaction_rates(&self, concentrations: &[Scalar], _t: Scalar) -> Vec<Scalar> {
        assert_eq!(
            concentrations.len(),
            self.species_count,
            "concentration vector length must match species_count"
        );
        let mut rates = vec![0.0; self.reaction_count];
        for j in 0..self.reaction_count {
            let mut rate = self.rate_constants[j];
            for i in 0..self.species_count {
                let nu = self.stoichiometry[j][i];
                if nu < 0.0 {
                    rate *= concentrations[i].powf(-nu);
                }
            }
            rates[j] = rate;
        }
        rates
    }

    /// Advance concentrations by one Euler step of size `dt`.
    pub fn step(&self, concentrations: &mut [Scalar], dt: Scalar, t: Scalar) {
        let derivs = self.concentration_derivatives(concentrations, t);
        for i in 0..self.species_count {
            let new_val = concentrations[i] + derivs[i] * dt;
            concentrations[i] = new_val.max(0.0); // prevent negative concentrations
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrhenius_rate_high_t() {
        let k = arrhenius_rate(1e12, 100_000.0, 1000.0);
        assert!(k > 0.0);
        assert!(k < 1e12);
    }

    #[test]
    fn test_arrhenius_rate_zero_t() {
        assert_eq!(arrhenius_rate(1e12, 100_000.0, 0.0), 0.0);
    }

    #[test]
    fn test_arrhenius_increases_with_t() {
        let k1 = arrhenius_rate(1e12, 50_000.0, 300.0);
        let k2 = arrhenius_rate(1e12, 50_000.0, 600.0);
        assert!(k2 > k1);
    }

    #[test]
    fn test_reaction_rate() {
        let r = reaction_rate(0.1, 2.0, 1.5, 1.0, 1.0);
        assert!((r - 0.3).abs() < 1e-12);
    }

    #[test]
    fn test_reversible_rate_equilibrium() {
        // At equilibrium with K_eq = k_f/k_r = c_b/c_a
        let r = reversible_rate(1.0, 2.0, 4.0, 2.0);
        assert!((r).abs() < 1e-12);
    }

    #[test]
    fn test_equilibrium_constant() {
        let k = equilibrium_constant(-5000.0, 300.0);
        assert!(k > 1.0); // negative ΔG → K > 1
    }

    #[test]
    fn test_half_life_first_order() {
        let t_half = half_life_first_order(0.693147);
        assert!((t_half - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_half_life_zero_k() {
        assert_eq!(half_life_first_order(0.0), f64::INFINITY);
    }

    #[test]
    fn test_reaction_kinetics_simple_a_to_b() {
        // A -> B, k=0.1
        let kinetics = ReactionKinetics::new(vec![0.1], vec![vec![-1.0, 1.0]]);
        let derivs = kinetics.concentration_derivatives(&[1.0, 0.0], 0.0);
        assert!((derivs[0] - (-0.1)).abs() < 1e-12);
        assert!((derivs[1] - 0.1).abs() < 1e-12);
    }

    #[test]
    fn test_reaction_kinetics_step() {
        let kinetics = ReactionKinetics::new(vec![0.1], vec![vec![-1.0, 1.0]]);
        let mut conc = vec![1.0, 0.0];
        kinetics.step(&mut conc, 0.1, 0.0);
        assert!((conc[0] - 0.99).abs() < 1e-10);
        assert!((conc[1] - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_reaction_kinetics_second_order() {
        // A + B -> C, k=0.5
        let kinetics = ReactionKinetics::new(vec![0.5], vec![vec![-1.0, -1.0, 1.0]]);
        let derivs = kinetics.concentration_derivatives(&[2.0, 3.0, 0.0], 0.0);
        // rate = 0.5 * 2^1 * 3^1 = 3.0
        assert!((derivs[0] - (-3.0)).abs() < 1e-12);
        assert!((derivs[1] - (-3.0)).abs() < 1e-12);
        assert!((derivs[2] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn test_reaction_kinetics_no_negative_concentrations() {
        // Verify step doesn't produce negative concentrations
        let kinetics = ReactionKinetics::new(vec![10.0], vec![vec![-1.0, 1.0]]);
        let mut conc = vec![0.01, 0.0];
        kinetics.step(&mut conc, 1.0, 0.0);
        assert!(conc[0] >= 0.0);
    }
}
