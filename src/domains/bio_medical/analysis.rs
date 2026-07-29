//! Biomedical analysis tools: cardiac output, BSA, eGFR, perfusion pressure.

use crate::core::types::Scalar;

/// Cardiac output: CO = HR · SV
///
/// * `heart_rate` — beats per minute (bpm)
/// * `stroke_volume` — stroke volume (L/beat)
/// Returns cardiac output in L/min.
pub fn cardiac_output(heart_rate: Scalar, stroke_volume: Scalar) -> Scalar {
    heart_rate * stroke_volume
}

/// Body surface area (Mosteller formula): BSA = √(weight·height / 3600)
///
/// * `weight_kg` — body weight (kg)
/// * `height_cm` — height (cm)
/// Returns BSA in m².
pub fn body_surface_area(weight_kg: Scalar, height_cm: Scalar) -> Scalar {
    (weight_kg * height_cm / 3600.0).sqrt()
}

/// Estimated glomerular filtration rate (CKD-EPI 2021 equation).
///
/// * `creatinine` — serum creatinine (mg/dL)
/// * `age` — age (years)
/// * `is_male` — true for male, false for female
/// * `is_black` — true for Black race
/// Returns eGFR in mL/min/1.73m².
pub fn egfr_ckd_epi(creatinine: Scalar, age: Scalar, is_male: bool, is_black: bool) -> Scalar {
    let (kappa, alpha): (Scalar, Scalar) = if is_male {
        (0.9, -0.411)
    } else {
        (0.7, -0.329)
    };

    let scr_over_kappa = creatinine / kappa;
    let min_term = scr_over_kappa.min(1.0).powf(alpha);
    let max_term = scr_over_kappa.max(1.0).powf(-1.209);
    let age_factor = 0.993_f64.powf(age);

    let mut egfr = 141.0 * min_term * max_term * age_factor;

    if !is_male {
        egfr *= 1.018;
    }
    if is_black {
        egfr *= 1.159;
    }

    egfr
}

/// Perfusion pressure: PP = MAP - CVP
///
/// * `map` — mean arterial pressure (mmHg)
/// * `cvp` — central venous pressure (mmHg)
/// Returns perfusion pressure in mmHg.
pub fn perfusion_pressure(map: Scalar, cvp: Scalar) -> Scalar {
    map - cvp
}

#[cfg(test)]
mod tests {
    #![allow(clippy::collapsible_if, clippy::doc_lazy_continuation)]
    use super::*;

    #[test]
    fn test_cardiac_output_typical() {
        let co = cardiac_output(70.0, 0.07);
        assert!((co - 4.9).abs() < 1e-10);
    }

    #[test]
    fn test_cardiac_output_zero_hr() {
        let co = cardiac_output(0.0, 0.07);
        assert!((co).abs() < 1e-10);
    }

    #[test]
    fn test_body_surface_area_typical() {
        let bsa = body_surface_area(70.0, 175.0);
        let expected = Scalar::sqrt(70.0 * 175.0 / 3600.0);
        assert!((bsa - expected).abs() < 1e-10);
        // Typical adult BSA ~1.7-2.0 m²
        assert!(bsa > 1.5 && bsa < 2.0);
    }

    #[test]
    fn test_egfr_ckd_epi_young_male() {
        let egfr = egfr_ckd_epi(0.9, 30.0, true, false);
        assert!(egfr > 80.0 && egfr < 150.0);
    }

    #[test]
    fn test_egfr_ckd_epi_young_female() {
        let egfr = egfr_ckd_epi(0.7, 30.0, false, false);
        assert!(egfr > 80.0 && egfr < 150.0);
    }

    #[test]
    fn test_egfr_ckd_epi_elevated_creatinine() {
        let egfr_normal = egfr_ckd_epi(0.9, 50.0, true, false);
        let egfr_high = egfr_ckd_epi(2.0, 50.0, true, false);
        assert!(egfr_high < egfr_normal);
    }

    #[test]
    fn test_egfr_ckd_epi_age_decline() {
        let egfr_young = egfr_ckd_epi(0.9, 30.0, true, false);
        let egfr_old = egfr_ckd_epi(0.9, 80.0, true, false);
        assert!(egfr_old < egfr_young);
    }

    #[test]
    fn test_egfr_ckd_epi_race_adjustment() {
        let egfr_ref = egfr_ckd_epi(0.9, 40.0, true, false);
        let egfr_black = egfr_ckd_epi(0.9, 40.0, true, true);
        assert!((egfr_black - egfr_ref * 1.159).abs() < 1e-6);
    }

    #[test]
    fn test_perfusion_pressure() {
        let pp = perfusion_pressure(93.0, 8.0);
        assert!((pp - 85.0).abs() < 1e-10);
    }

    #[test]
    fn test_perfusion_pressure_zero_cvp() {
        let pp = perfusion_pressure(93.0, 0.0);
        assert!((pp - 93.0).abs() < 1e-10);
    }
}
