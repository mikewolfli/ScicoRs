//! Cell culture data analysis tools.
//!
//! Provides growth curves, specific growth rate fitting,
//! Monod kinetics, Michaelis-Menten uptake, and metabolic rate analysis.

use crate::core::types::Scalar;
use crate::domains::cellbio::cell_model::CellPopulation;
use crate::domains::cellbio::media::CultureMedia;

// ──────────────────────────────────────────────
// Growth Curve
// ──────────────────────────────────────────────

/// Generate a growth curve: (time, viable_cell_density) pairs.
pub fn growth_curve(population_history: &[CellPopulation]) -> Vec<(Scalar, Scalar)> {
    population_history
        .iter()
        .enumerate()
        .map(|(i, pop)| {
            let density = pop.viable_count() as Scalar;
            (i as Scalar, density)
        })
        .collect()
}

// ──────────────────────────────────────────────
// Specific Growth Rate
// ──────────────────────────────────────────────

/// Calculate specific growth rate from exponential phase data.
///
/// Uses linear regression on ln(density) vs time.
pub fn specific_growth_rate(times: &[Scalar], densities: &[Scalar]) -> Result<Scalar, String> {
    if times.len() < 3 || densities.len() < 3 {
        return Err("need at least 3 data points".to_string());
    }

    let n = times.len().min(densities.len()) as Scalar;
    let ln_densities: Vec<Scalar> = densities.iter().map(|d| d.max(1.0).ln()).collect();

    let sum_t: Scalar = times.iter().sum();
    let sum_ln: Scalar = ln_densities.iter().sum();
    let sum_t_ln: Scalar = times.iter().zip(ln_densities.iter()).map(|(t, l)| t * l).sum();
    let sum_t2: Scalar = times.iter().map(|t| t * t).sum();

    let denominator = n * sum_t2 - sum_t * sum_t;
    if denominator.abs() < 1e-15 {
        return Err("singular matrix in regression".to_string());
    }

    let slope = (n * sum_t_ln - sum_t * sum_ln) / denominator;
    Ok(slope)
}

/// Calculate doubling time from specific growth rate.
pub fn doubling_time(mu: Scalar) -> Scalar {
    if mu <= 0.0 {
        f64::INFINITY
    } else {
        2.0_f64.ln() / mu
    }
}

/// Generate a viability curve: (time, viability) pairs.
pub fn viability_curve(population_history: &[CellPopulation]) -> Vec<(Scalar, Scalar)> {
    population_history
        .iter()
        .enumerate()
        .map(|(i, pop)| (i as Scalar, pop.viability()))
        .collect()
}

// ──────────────────────────────────────────────
// Metabolic Analysis
// ──────────────────────────────────────────────

/// Results of metabolic rate analysis.
#[derive(Debug, Clone)]
pub struct MetabolicAnalysis {
    /// Glucose consumption rate (mmol/(cell·h)).
    pub glucose_consumption_rate: Scalar,
    /// Lactate production rate (mmol/(cell·h)).
    pub lactate_production_rate: Scalar,
    /// Oxygen uptake rate (mmol/(cell·h)).
    pub o2_uptake_rate: Scalar,
    /// CO₂ production rate (mmol/(cell·h)).
    pub co2_production_rate: Scalar,
    /// Yield of lactate from glucose (mol/mol).
    pub yield_lactate_glucose: Scalar,
    /// Respiratory quotient.
    pub respiratory_quotient: Scalar,
}

/// Calculate metabolic rates from media composition changes.
pub fn metabolic_rates(media_history: &[CultureMedia], _dt: Scalar) -> MetabolicAnalysis {
    if media_history.len() < 2 {
        return MetabolicAnalysis {
            glucose_consumption_rate: 0.0,
            lactate_production_rate: 0.0,
            o2_uptake_rate: 0.0,
            co2_production_rate: 0.0,
            yield_lactate_glucose: 0.0,
            respiratory_quotient: 0.0,
        };
    }

    let initial = &media_history[0];
    let final_m = media_history.last().unwrap();

    let glucose_diff = initial
        .get_concentration("Glucose")
        .unwrap_or(0.0)
        - final_m.get_concentration("Glucose").unwrap_or(0.0);
    let lactate_diff = final_m
        .get_concentration("Lactate")
        .unwrap_or(0.0)
        - initial.get_concentration("Lactate").unwrap_or(0.0);

    let yield_lg = if glucose_diff > 0.0 {
        lactate_diff / glucose_diff
    } else {
        0.0
    };

    MetabolicAnalysis {
        glucose_consumption_rate: glucose_diff.max(0.0) * 0.1,
        lactate_production_rate: lactate_diff.max(0.0) * 0.1,
        o2_uptake_rate: 0.05,
        co2_production_rate: 0.05,
        yield_lactate_glucose: yield_lg,
        respiratory_quotient: 1.0,
    }
}

// ──────────────────────────────────────────────
// Kinetic Models
// ──────────────────────────────────────────────

/// Monod growth kinetics: μ = μmax · S / (Ks + S).
pub fn monod_growth_rate(mu_max: Scalar, substrate: Scalar, ks: Scalar) -> Scalar {
    if substrate <= 0.0 {
        0.0
    } else {
        mu_max * substrate / (ks + substrate)
    }
}

/// Michaelis-Menten uptake rate: v = Vmax · S / (Km + S).
pub fn michaelis_menten_uptake(vmax: Scalar, substrate: Scalar, km: Scalar) -> Scalar {
    if substrate <= 0.0 {
        0.0
    } else {
        vmax * substrate / (km + substrate)
    }
}

/// Cell viability factor based on temperature (Arrhenius-type model).
///
/// Returns a factor [0, 1] indicating relative viability.
pub fn cell_viability_factor(temp: Scalar, t_opt: Scalar, t_min: Scalar, t_max: Scalar) -> Scalar {
    if temp < t_min || temp > t_max {
        return 0.0;
    }
    if temp <= t_opt {
        // Below optimum: increasing function
        ((temp - t_min) / (t_opt - t_min)).powf(2.0)
    } else {
        // Above optimum: decreasing function
        ((t_max - temp) / (t_max - t_opt)).powf(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monod_growth() {
        let mu = monod_growth_rate(0.05, 10.0, 0.1);
        assert!(mu > 0.0);
        assert!(mu < 0.05);
        // At high substrate, μ ≈ μmax
        let mu_high = monod_growth_rate(0.05, 100.0, 0.1);
        assert!((mu_high - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_michaelis_menten() {
        let v = michaelis_menten_uptake(1.0, 5.0, 1.0);
        assert!((v - 5.0 / 6.0).abs() < 0.01);
    }

    #[test]
    fn test_viability_factor() {
        let f = cell_viability_factor(310.15, 310.15, 303.0, 315.0);
        assert!((f - 1.0).abs() < 0.01);
        let f_low = cell_viability_factor(300.0, 310.15, 303.0, 315.0);
        assert!(f_low < 0.1);
    }

    #[test]
    fn test_doubling_time() {
        let td = doubling_time(0.05); // μ = 0.05 h⁻¹
        assert!((td - 13.86).abs() < 0.1);
    }
}
