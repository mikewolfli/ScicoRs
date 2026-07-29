//! NBTI and HCI reliability degradation models.

use crate::core::types::Scalar;

/// NBTI degradation (threshold voltage shift).
pub fn nbti_degradation(vgs: Scalar, temp: Scalar, time: Scalar) -> Scalar {
    let k = 8.617e-5;
    let ea = 0.15;
    let gamma = 0.3;
    let dvth0 = 0.05;
    dvth0 * (vgs / 1.8).powf(gamma) * (-ea / (k * temp)).exp() * (time / 3600.0).powf(0.25)
}

/// HCI degradation (saturation current degradation).
pub fn hci_degradation(vds: Scalar, ids: Scalar, _time: Scalar) -> Scalar {
    let a = 1e-9;
    a * (ids / 1e-3) * (vds / 1.0).powf(2.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_nbti() { let d = nbti_degradation(1.8, 400.0, 1000.0); assert!(d > 0.0); }
    #[test]
    fn test_hci() { let d = hci_degradation(1.8, 1e-3, 1000.0); assert!(d >= 0.0); }
}
