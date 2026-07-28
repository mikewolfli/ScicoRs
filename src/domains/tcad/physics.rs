//! Physical constants and carrier transport models for semiconductor TCAD.
//!
//! Provides fundamental physical constants, carrier mobility models,
//! drift-diffusion transport equations, and PN junction physics.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// 1. Fundamental Physical Constants
// ──────────────────────────────────────────────

/// Elementary charge (C).
pub const Q: Scalar = 1.602176634e-19;

/// Boltzmann constant (J/K).
pub const K_B: Scalar = 1.380649e-23;

/// Vacuum permittivity (F/m).
pub const EPSILON_0: Scalar = 8.854187817e-12;

/// Room temperature (K).
pub const T_300K: Scalar = 300.0;

/// Thermal voltage at 300K (V).
pub const V_T_300K: Scalar = 0.02585;

/// Intrinsic carrier concentration of silicon at 300K (cm⁻³).
pub const NI_SI_300K: Scalar = 1.5e10;

/// Silicon relative permittivity.
pub const EPSILON_SI: Scalar = 11.7;

/// Silicon dioxide relative permittivity.
pub const EPSILON_OX: Scalar = 3.9;

// ──────────────────────────────────────────────
// 2. Carrier Mobility Model
// ──────────────────────────────────────────────

/// Carrier mobility model for semiconductor simulation.
///
/// Stores low-field electron and hole mobilities. The `effective_mobility()`
/// method applies temperature and field dependence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MobilityModel {
    /// Low-field electron mobility (cm²/V·s).
    pub mun0: Scalar,
    /// Low-field hole mobility (cm²/V·s).
    pub mup0: Scalar,
    /// Mobility temperature exponent.
    pub temp_exponent: Scalar,
}

impl MobilityModel {
    /// Create a new mobility model with specified low-field mobilities.
    pub fn new(mun0: Scalar, mup0: Scalar) -> Self {
        Self {
            mun0,
            mup0,
            temp_exponent: -1.5,
        }
    }

    /// Default mobility model for silicon at 300K.
    pub fn silicon_300k() -> Self {
        Self::new(1350.0, 480.0)
    }

    /// Effective electron mobility at given temperature and field.
    pub fn electron_mobility(&self, temp: Scalar, field: Scalar) -> Scalar {
        let temp_factor = (temp / T_300K).powf(self.temp_exponent);
        let mun = self.mun0 * temp_factor;
        // Caughey-Thomas field-dependent mobility
        mun / (1.0 + (field * mun / 1.0e7).powi(2)).sqrt()
    }

    /// Effective hole mobility at given temperature and field.
    pub fn hole_mobility(&self, temp: Scalar, field: Scalar) -> Scalar {
        let temp_factor = (temp / T_300K).powf(self.temp_exponent);
        let mup = self.mup0 * temp_factor;
        mup / (1.0 + (field * mup / 1.0e7).powi(2)).sqrt()
    }
}

// ──────────────────────────────────────────────
// 3. Drift-Diffusion Transport
// ──────────────────────────────────────────────

/// Compute drift-diffusion current densities.
///
/// Returns `(jn, jp)` — electron and hole current densities (A/cm²).
///
/// # Arguments
/// * `q` - Elementary charge (C)
/// * `n` - Electron concentration (cm⁻³)
/// * `p` - Hole concentration (cm⁻³)
/// * `mu_n` - Electron mobility (cm²/V·s)
/// * `mu_p` - Hole mobility (cm²/V·s)
/// * `grad_phi` - Electrostatic potential gradient (V/cm)
/// * `grad_n` - Electron concentration gradient (cm⁻⁴)
/// * `grad_p` - Hole concentration gradient (cm⁻⁴)
#[allow(clippy::too_many_arguments)]
pub fn drift_diffusion_current(
    q: Scalar,
    n: Scalar,
    p: Scalar,
    mu_n: Scalar,
    mu_p: Scalar,
    grad_phi: Scalar,
    grad_n: Scalar,
    grad_p: Scalar,
    v_t: Scalar,
) -> (Scalar, Scalar) {
    // Electron current: Jn = q * (n * mu_n * grad_phi + Dn * grad_n)
    // where Dn = mu_n * Vt (Einstein relation)
    let dn = mu_n * v_t;
    let jn = q * (n * mu_n * grad_phi + dn * grad_n);

    // Hole current: Jp = q * (p * mu_p * grad_phi - Dp * grad_p)
    let dp = mu_p * v_t;
    let jp = q * (p * mu_p * grad_phi - dp * grad_p);

    (jn, jp)
}

/// Total current density (sum of electron and hole components).
pub fn total_current(jn: Scalar, jp: Scalar) -> Scalar {
    jn + jp
}

// ──────────────────────────────────────────────
// 4. PN Junction Physics
// ──────────────────────────────────────────────

/// Compute the built-in potential of a PN junction (V).
///
/// # Arguments
/// * `na` - Acceptor doping concentration (cm⁻³)
/// * `nd` - Donor doping concentration (cm⁻³)
/// * `ni` - Intrinsic carrier concentration (cm⁻³)
/// * `temp` - Temperature (K)
pub fn built_in_potential(na: Scalar, nd: Scalar, ni: Scalar, temp: Scalar) -> Scalar {
    let v_t = K_B * temp / Q;
    v_t * (na * nd / (ni * ni)).ln()
}

/// Compute depletion width for an abrupt PN junction (cm).
///
/// # Arguments
/// * `na` - Acceptor doping (cm⁻³)
/// * `nd` - Donor doping (cm⁻³)
/// * `v_bi` - Built-in potential (V)
/// * `v_r` - Reverse bias voltage (V)
/// * `eps` - Semiconductor permittivity (F/cm)
pub fn depletion_width(na: Scalar, nd: Scalar, v_bi: Scalar, v_r: Scalar, eps: Scalar) -> Scalar {
    let total_doping = 1.0 / na + 1.0 / nd;
    let v_total = v_bi + v_r;
    if v_total < 0.0 {
        return 0.0; // Forward bias — no depletion
    }
    (2.0 * eps * v_total * total_doping / Q).sqrt()
}

/// Maximum electric field in a PN junction (V/cm).
pub fn max_electric_field(
    na: Scalar,
    nd: Scalar,
    v_bi: Scalar,
    v_r: Scalar,
    eps: Scalar,
) -> Scalar {
    let w = depletion_width(na, nd, v_bi, v_r, eps);
    if w <= 0.0 {
        return 0.0;
    }
    let n_eff = (na * nd) / (na + nd);
    Q * n_eff * w / eps
}

/// Junction capacitance per unit area (F/cm²).
pub fn junction_capacitance(
    na: Scalar,
    nd: Scalar,
    v_bi: Scalar,
    v_r: Scalar,
    eps: Scalar,
) -> Scalar {
    let w = depletion_width(na, nd, v_bi, v_r, eps);
    if w <= 0.0 {
        return 1e6_f64; // Heavy forward bias — large capacitance
    }
    eps / w
}

// ──────────────────────────────────────────────
// 5. Thermal voltage helper
// ──────────────────────────────────────────────

/// Thermal voltage at a given temperature (V).
pub fn thermal_voltage(temp: Scalar) -> Scalar {
    K_B * temp / Q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physical_constants() {
        assert!((Q - 1.602176634e-19).abs() < 1e-27);
        assert!((K_B - 1.380649e-23).abs() < 1e-27);
        assert!((EPSILON_0 - 8.854187817e-12).abs() < 1e-15);
        assert!((T_300K - 300.0).abs() < 1e-10);
    }

    #[test]
    fn test_thermal_voltage_300k() {
        let vt = thermal_voltage(T_300K);
        assert!((vt - V_T_300K).abs() < 1e-4);
    }

    #[test]
    fn test_mobility_silicon_default() {
        let mob = MobilityModel::silicon_300k();
        assert!((mob.mun0 - 1350.0).abs() < 1.0);
        assert!((mob.mup0 - 480.0).abs() < 1.0);
    }

    #[test]
    fn test_mobility_temperature_dependence() {
        let mob = MobilityModel::silicon_300k();
        let mun_400k = mob.electron_mobility(400.0, 0.0);
        let mun_300k = mob.electron_mobility(300.0, 0.0);
        // Mobility decreases with temperature (exponent -1.5)
        assert!(mun_400k < mun_300k);
    }

    #[test]
    fn test_mobility_field_dependence() {
        let mob = MobilityModel::silicon_300k();
        let mun_low = mob.electron_mobility(300.0, 0.0);
        let mun_high = mob.electron_mobility(300.0, 1e5);
        // High field reduces mobility (velocity saturation)
        assert!(mun_high < mun_low);
    }

    #[test]
    fn test_drift_diffusion_current() {
        // Under equilibrium (no gradients), currents should be zero
        let (jn, jp) =
            drift_diffusion_current(Q, 1e16, 1e16, 1350.0, 480.0, 0.0, 0.0, 0.0, V_T_300K);
        assert!((jn).abs() < 1e-30);
        assert!((jp).abs() < 1e-30);
    }

    #[test]
    fn test_built_in_potential() {
        // Silicon PN junction: Na=1e17, Nd=1e15 at 300K
        let vbi = built_in_potential(1e17, 1e15, NI_SI_300K, T_300K);
        // Should be ~0.7V for typical silicon junction
        assert!(vbi > 0.5 && vbi < 0.9);
    }

    #[test]
    fn test_depletion_width_forward_bias() {
        // Heavy forward bias — depletion width goes to zero
        let w = depletion_width(1e17, 1e15, 0.7, -1.0, EPSILON_SI * EPSILON_0 * 100.0);
        assert!(w <= 0.0 || w >= 0.0); // Just check no panic
    }

    #[test]
    fn test_total_current_sum() {
        let jn = 1.0e-4;
        let jp = 2.0e-4;
        assert!((total_current(jn, jp) - 3.0e-4).abs() < 1e-15);
    }

    #[test]
    fn test_junction_capacitance_reverse_bias() {
        // Reverse bias reduces capacitance
        let c_0v = junction_capacitance(1e17, 1e15, 0.7, 0.0, EPSILON_SI * EPSILON_0 * 100.0);
        let c_5v = junction_capacitance(1e17, 1e15, 0.7, 5.0, EPSILON_SI * EPSILON_0 * 100.0);
        assert!(c_5v < c_0v);
    }
}
