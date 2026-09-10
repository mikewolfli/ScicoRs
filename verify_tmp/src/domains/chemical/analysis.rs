//! Chemical process analysis tools.
//!
//! Provides conversion, yield, selectivity, and enthalpy calculations
//! for chemical reaction analysis.

use crate::core::types::Scalar;

/// Calculate conversion of a reactant.
///
/// X = (C_in - C_out) / C_in
///
/// Returns 0.0 if inlet concentration is zero.
pub fn conversion(c_in: Scalar, c_out: Scalar) -> Scalar {
    if c_in <= 0.0 {
        return 0.0;
    }
    ((c_in - c_out) / c_in).clamp(0.0, 1.0)
}

/// Calculate yield of a product relative to a reactant.
///
/// Y = Product_moles / (Reactant_moles * Stoichiometric_coeff)
///
/// where stoichiometric_coeff = |ν_reactant / ν_product|.
pub fn yield_ratio(
    product_moles: Scalar,
    reactant_moles: Scalar,
    stoichiometric_coeff: Scalar,
) -> Scalar {
    if reactant_moles <= 0.0 || stoichiometric_coeff <= 0.0 {
        return 0.0;
    }
    (product_moles / (reactant_moles * stoichiometric_coeff)).clamp(0.0, 1.0)
}

/// Calculate selectivity of desired product over total products.
///
/// S = Desired_product / Total_products
pub fn selectivity(desired_product: Scalar, total_products: Scalar) -> Scalar {
    if total_products != 0.0 {
        (desired_product / total_products).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Calculate standard reaction enthalpy from formation enthalpies.
///
/// ΔH_rxn = Σ ν_i · ΔH_f,i
///
/// where ν_i are stoichiometric coefficients (negative for reactants,
/// positive for products) and ΔH_f,i are standard formation enthalpies.
pub fn reaction_enthalpy(formation_enthalpies: &[Scalar], stoichiometry: &[Scalar]) -> Scalar {
    assert_eq!(
        formation_enthalpies.len(),
        stoichiometry.len(),
        "formation_enthalpies and stoichiometry must have the same length"
    );

    formation_enthalpies
        .iter()
        .zip(stoichiometry.iter())
        .map(|(h, s)| h * s)
        .sum()
}

/// Calculate the extent of reaction from concentration changes.
///
/// ξ = (C_i - C_i0) / ν_i
///
/// where ν_i is the stoichiometric coefficient for species i.
pub fn reaction_extent(
    initial_conc: &[Scalar],
    current_conc: &[Scalar],
    stoichiometry: &[Scalar],
) -> Result<Scalar, String> {
    let n = initial_conc.len();
    if current_conc.len() != n || stoichiometry.len() != n {
        return Err("All input slices must have the same length".to_string());
    }

    let mut extent_sum = 0.0;
    let mut weight_sum = 0.0;

    for i in 0..n {
        if stoichiometry[i].abs() > 1e-15 {
            let xi_i = (current_conc[i] - initial_conc[i]) / stoichiometry[i];
            extent_sum += xi_i * stoichiometry[i].abs();
            weight_sum += stoichiometry[i].abs();
        }
    }

    if weight_sum < 1e-15 {
        return Err("No non-zero stoichiometric coefficients".to_string());
    }

    Ok(extent_sum / weight_sum)
}

/// Calculate the equilibrium conversion for a reversible first-order reaction.
///
/// For A ⇌ B with equilibrium constant K_eq,
/// X_eq = K_eq / (1 + K_eq)
pub fn equilibrium_conversion(k_eq: Scalar) -> Scalar {
    if k_eq < 0.0 {
        return 0.0;
    }
    (k_eq / (1.0 + k_eq)).clamp(0.0, 1.0)
}

/// Calculate space-time (τ) for a flow reactor.
///
/// τ = V / Q
pub fn space_time(volume: Scalar, flow_rate: Scalar) -> Scalar {
    if flow_rate <= 0.0 {
        return f64::INFINITY;
    }
    volume / flow_rate
}

/// Calculate Damköhler number (Da) for a first-order reaction.
///
/// Da = k · τ
pub fn damkohler_number(k: Scalar, residence_time: Scalar) -> Scalar {
    k * residence_time
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversion() {
        assert!((conversion(1.0, 0.3) - 0.7).abs() < 1e-12);
    }

    #[test]
    fn test_conversion_zero_inlet() {
        assert_eq!(conversion(0.0, 0.5), 0.0);
    }

    #[test]
    fn test_conversion_clamps() {
        // Should not exceed 1.0 even if c_out < 0 (unphysical)
        assert_eq!(conversion(1.0, -0.5), 1.0);
    }

    #[test]
    fn test_yield_ratio() {
        // 2 moles product from 1 mole reactant with coeff 2
        let y = yield_ratio(2.0, 1.0, 2.0);
        assert!((y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_yield_ratio_zero_reactant() {
        assert_eq!(yield_ratio(1.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn test_selectivity() {
        let s = selectivity(0.8, 1.0);
        assert!((s - 0.8).abs() < 1e-12);
    }

    #[test]
    fn test_selectivity_zero_total() {
        assert_eq!(selectivity(1.0, 0.0), 0.0);
    }

    #[test]
    fn test_reaction_enthalpy() {
        // Formation enthalpies [A, B, C] with stoichiometry [-1, 1, 1]
        // A -> B + C
        let enthalpies = vec![-100.0, -50.0, -30.0];
        let stoichiometry = vec![-1.0, 1.0, 1.0];
        let delta_h = reaction_enthalpy(&enthalpies, &stoichiometry);
        // ΔH = (-1)(-100) + (1)(-50) + (1)(-30) = 100 - 50 - 30 = 20
        assert!((delta_h - 20.0).abs() < 1e-12);
    }

    #[test]
    fn test_reaction_extent() {
        // A -> B, initial [1.0, 0.0], current [0.7, 0.3], stoichiometry [-1, 1]
        let xi = reaction_extent(&[1.0, 0.0], &[0.7, 0.3], &[-1.0, 1.0]).unwrap();
        assert!((xi - 0.3).abs() < 1e-12);
    }

    #[test]
    fn test_equilibrium_conversion() {
        let x = equilibrium_conversion(4.0);
        assert!((x - 0.8).abs() < 1e-12);
    }

    #[test]
    fn test_space_time() {
        let tau = space_time(10.0, 2.0);
        assert!((tau - 5.0).abs() < 1e-12);
        assert_eq!(space_time(10.0, 0.0), f64::INFINITY);
    }

    #[test]
    fn test_damkohler_number() {
        let da = damkohler_number(0.5, 10.0);
        assert!((da - 5.0).abs() < 1e-12);
    }
}
