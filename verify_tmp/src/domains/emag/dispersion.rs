//! Dispersive material models for FDTD simulation.
//!
//! Implements Drude, Debye, and Lorentz dispersion models for
//! frequency-dependent materials in electromagnetic simulation.

use crate::core::types::Scalar;
use num_complex::Complex64;

/// Dispersion model types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DispersionModel {
    Drude { wp: Scalar, gamma: Scalar },
    Debye { eps_s: Scalar, eps_inf: Scalar, tau: Scalar },
    Lorentz { eps_s: Scalar, eps_inf: Scalar, wp: Scalar, gamma: Scalar },
}

impl DispersionModel {
    /// Frequency-dependent permittivity ε(ω).
    pub fn epsilon(&self, omega: Scalar) -> Complex64 {
        match *self {
            Self::Drude { wp, gamma } => {
                let w2 = omega * omega;
                let g2 = gamma * gamma;
                Complex64::new(1.0 - wp * wp / (w2 + g2), -wp * wp * gamma / (omega * (w2 + g2)))
            }
            Self::Debye { eps_s, eps_inf, tau } => {
                let tau_omega = tau * omega;
                Complex64::new(eps_inf + (eps_s - eps_inf) / (1.0 + tau_omega * tau_omega),
                    -(eps_s - eps_inf) * tau_omega / (1.0 + tau_omega * tau_omega))
            }
            Self::Lorentz { eps_s, eps_inf, wp, gamma } => {
                let w2 = omega * omega;
                let num = (eps_s - eps_inf) * wp * wp;
                let denom_r = wp * wp - w2;
                let denom_i = gamma * omega;
                let denom = denom_r * denom_r + denom_i * denom_i;
                Complex64::new(eps_inf + num * denom_r / denom, -num * denom_i / denom)
            }
        }
    }

    /// Plasma frequency (rad/s).
    pub fn plasma_frequency(&self) -> Scalar {
        match *self {
            Self::Drude { wp, .. } => wp,
            Self::Lorentz { wp, .. } => wp,
            Self::Debye { .. } => 0.0,
        }
    }
}

/// Drude model for metals (e.g., gold, silver).
pub fn drude_gold() -> DispersionModel {
    DispersionModel::Drude { wp: 1.37e16, gamma: 4.05e13 }
}

pub fn drude_silver() -> DispersionModel {
    DispersionModel::Drude { wp: 1.39e16, gamma: 3.21e13 }
}

/// Debye model for polar liquids (e.g., water).
pub fn debye_water() -> DispersionModel {
    DispersionModel::Debye { eps_s: 80.0, eps_inf: 4.0, tau: 8.5e-12 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drude_epsilon() {
        let gold = drude_gold();
        let eps = gold.epsilon(1e15);
        assert!(eps.re < 1.0, "Drude Re(ε) < 1 below plasma freq");
        // Drude metals have negative Im(ε) in physics convention (e^(iωt) sign)
        assert!(eps.im.is_finite(), "Drude Im(ε) should be finite");
    }

    #[test]
    fn test_debye_water() {
        let water = debye_water();
        let eps = water.epsilon(1e10);
        assert!((eps.re - 80.0).abs() < 1.0, "Debye water Re(ε) ≈ 80 at DC");
    }

    #[test]
    fn test_lorentz_epsilon() {
        let lt = DispersionModel::Lorentz { eps_s: 3.0, eps_inf: 1.0, wp: 1e15, gamma: 1e14 };
        let eps = lt.epsilon(1e15);
        assert!(eps.re.is_finite());
        assert!(eps.im.is_finite());
    }
}
