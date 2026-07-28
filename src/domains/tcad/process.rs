//! Basic semiconductor process simulation.
//!
//! Provides simplified models for key semiconductor fabrication processes:
//! diffusion, ion implantation, and thermal oxidation.

use super::physics::{K_B, Q};
use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// 1. Oxidation Ambient
// ──────────────────────────────────────────────

/// Ambient type for thermal oxidation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OxidationAmbient {
    /// Dry O₂ ambient.
    DryOxygen,
    /// Wet H₂O ambient (steam).
    WetVapor,
}

// ──────────────────────────────────────────────
// 2. Diffusion
// ──────────────────────────────────────────────

/// Compute diffusion profile using Gaussian distribution.
///
/// Returns the concentration at depth `x` (cm⁻³) given implant dose
/// and diffusion conditions.
///
/// # Arguments
/// * `dose` - Implant dose (cm⁻²)
/// * `diffusivity` - Diffusion coefficient (cm²/s)
/// * `time` - Diffusion time (s)
/// * `x` - Depth (cm)
pub fn diffusion_profile(dose: Scalar, diffusivity: Scalar, time: Scalar, x: Scalar) -> Scalar {
    if diffusivity <= 0.0 || time <= 0.0 {
        // No diffusion — return a thin layer approximation
        return if x.abs() < 1e-7 { dose / 1e-7 } else { 0.0 };
    }

    let diffusion_length = (4.0 * diffusivity * time).sqrt();
    if diffusion_length < 1e-20 {
        return if x.abs() < 1e-7 { dose / 1e-7 } else { 0.0 };
    }

    let peak = dose / (diffusion_length * (std::f64::consts::PI).sqrt());
    peak * (-x * x / (4.0 * diffusivity * time)).exp()
}

/// Compute diffusion coefficient using Arrhenius relationship.
///
/// # Arguments
/// * `d0` - Pre-exponential factor (cm²/s)
/// * `ea` - Activation energy (eV)
/// * `temp` - Temperature (K)
pub fn diffusivity_arrhenius(d0: Scalar, ea: Scalar, temp: Scalar) -> Scalar {
    if temp <= 0.0 {
        return 0.0;
    }
    d0 * (-ea * Q / (K_B * temp)).exp()
}

/// Boron diffusion coefficient in silicon.
pub fn boron_diffusivity(temp: Scalar) -> Scalar {
    // D0 = 0.76 cm²/s, Ea = 3.46 eV
    diffusivity_arrhenius(0.76, 3.46, temp)
}

/// Phosphorus diffusion coefficient in silicon.
pub fn phosphorus_diffusivity(temp: Scalar) -> Scalar {
    // D0 = 3.85 cm²/s, Ea = 3.66 eV
    diffusivity_arrhenius(3.85, 3.66, temp)
}

/// Arsenic diffusion coefficient in silicon.
pub fn arsenic_diffusivity(temp: Scalar) -> Scalar {
    // D0 = 0.32 cm²/s, Ea = 3.56 eV
    diffusivity_arrhenius(0.32, 3.56, temp)
}

// ──────────────────────────────────────────────
// 3. Ion Implantation
// ──────────────────────────────────────────────

/// Compute ion implantation projected range using simplified LSS theory (cm).
///
/// # Arguments
/// * `energy` - Implant energy (keV)
/// * `mass_ion` - Ion mass (amu)
/// * `mass_target` - Target atom mass (amu)
pub fn implant_range(energy: Scalar, mass_ion: Scalar, mass_target: Scalar) -> Scalar {
    if energy <= 0.0 {
        return 0.0;
    }

    // Simplified range calculation
    // Rp ≈ 0.01 * sqrt(E) * (M_target / M_ion)^0.3 (μm)
    let range_um = 0.01 * energy.sqrt() * (mass_target / mass_ion).powf(0.3);
    range_um * 1e-4 // Convert μm to cm
}

/// Compute implant straggle (standard deviation of implant profile) in cm.
pub fn implant_straggle(energy: Scalar, mass_ion: Scalar, mass_target: Scalar) -> Scalar {
    if energy <= 0.0 {
        return 0.0;
    }

    // Simplified straggle: ΔRp ≈ 0.3 * Rp
    let rp = implant_range(energy, mass_ion, mass_target);
    0.3 * rp
}

/// Gaussian implant profile at depth x (cm⁻³).
///
/// # Arguments
/// * `dose` - Implant dose (cm⁻²)
/// * `rp` - Projected range (cm)
/// * `delta_rp` - Straggle (cm)
/// * `x` - Depth (cm)
pub fn implant_profile(dose: Scalar, rp: Scalar, delta_rp: Scalar, x: Scalar) -> Scalar {
    if delta_rp <= 0.0 {
        return if (x - rp).abs() < 1e-7 {
            dose / 1e-7
        } else {
            0.0
        };
    }

    let peak = dose / (delta_rp * (2.0 * std::f64::consts::PI).sqrt());
    peak * (-0.5 * ((x - rp) / delta_rp).powi(2)).exp()
}

// ──────────────────────────────────────────────
// 4. Thermal Oxidation
// ──────────────────────────────────────────────

/// Compute oxide thickness using Deal-Grove model.
///
/// Returns oxide thickness (cm).
///
/// # Arguments
/// * `time` - Oxidation time (s)
/// * `temp` - Temperature (K)
/// * `ambient` - Oxidation ambient (dry O₂ or wet H₂O)
pub fn oxide_thickness(time: Scalar, temp: Scalar, ambient: OxidationAmbient) -> Scalar {
    if time <= 0.0 || temp <= 0.0 {
        return 0.0;
    }

    // Deal-Grove parameters for <100> silicon
    let (a, b) = match ambient {
        OxidationAmbient::DryOxygen => {
            // B/A = 3.71e6 * exp(-2.0 eV/kT) μm/hr, B = 7.72e2 * exp(-1.23 eV/kT) μm²/hr
            let ba = 3.71e6 * (-2.0 * Q / (K_B * temp)).exp(); // μm/hr
            let b_val = 7.72e2 * (-1.23 * Q / (K_B * temp)).exp(); // μm²/hr
            (b_val / ba, b_val) // A = B / (B/A), B
        }
        OxidationAmbient::WetVapor => {
            // B/A = 8.95e7 * exp(-2.05 eV/kT) μm/hr, B = 3.86e2 * exp(-0.78 eV/kT) μm²/hr
            let ba = 8.95e7 * (-2.05 * Q / (K_B * temp)).exp();
            let b_val = 3.86e2 * (-0.78 * Q / (K_B * temp)).exp();
            (b_val / ba, b_val)
        }
    };

    let time_hours = time / 3600.0; // Convert seconds to hours

    // Deal-Grove: xox² + A*xox = B*t
    // xox = (A/2) * (sqrt(1 + 4*B*t/A²) - 1)
    if a <= 0.0 {
        return (b * time_hours).sqrt() * 1e-4; // cm
    }

    let term = 1.0 + 4.0 * b * time_hours / (a * a);
    let xox_um = 0.5 * a * (term.sqrt() - 1.0);
    xox_um.max(0.0) * 1e-4 // Convert μm to cm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diffusion_profile_peak() {
        // At x=0, profile should be at maximum
        let c = diffusion_profile(1e15, 1e-12, 3600.0, 0.0);
        let c_offset = diffusion_profile(1e15, 1e-12, 3600.0, 1e-4);
        assert!(c > c_offset);
    }

    #[test]
    fn test_diffusion_dose_conservation() {
        // The Gaussian diffusion profile from x=0 to ∞ integrates to dose/2
        // (the distribution is symmetric around x=0). Use trapezoidal rule.
        let dose = 1e15;
        let diffusivity = 1e-12;
        let time = 3600.0;
        let n_steps = 4000;
        let dx = 5e-7;
        let mut integral = 0.0;
        for i in 0..n_steps {
            let x_left = i as Scalar * dx;
            let x_right = (i + 1) as Scalar * dx;
            let c_left = diffusion_profile(dose, diffusivity, time, x_left);
            let c_right = diffusion_profile(dose, diffusivity, time, x_right);
            integral += 0.5 * (c_left + c_right) * dx;
        }
        // Integral from 0 to ∞ should be ≈ dose/2
        let expected = dose / 2.0;
        let ratio = integral / expected;
        assert!(
            (ratio - 1.0).abs() < 0.1,
            "integral/(dose/2) = {}, expected ~1.0",
            ratio
        );
    }

    #[test]
    fn test_arrhenius_diffusivity() {
        let d = diffusivity_arrhenius(1.0, 1.0, 300.0);
        assert!(d > 0.0);
        // Higher temperature → higher diffusivity
        let d_hot = diffusivity_arrhenius(1.0, 1.0, 1000.0);
        assert!(d_hot > d);
    }

    #[test]
    fn test_boron_diffusivity() {
        let d = boron_diffusivity(1273.0); // 1000°C
        assert!(d > 1e-20);
        assert!(d < 1.0);
    }

    #[test]
    fn test_implant_range_increases_with_energy() {
        let r1 = implant_range(10.0, 11.0, 28.0); // Boron into Si
        let r2 = implant_range(100.0, 11.0, 28.0);
        assert!(r2 > r1);
    }

    #[test]
    fn test_implant_profile_gaussian() {
        let dose = 1e15;
        let rp = implant_range(50.0, 11.0, 28.0);
        let drp = implant_straggle(50.0, 11.0, 28.0);
        let c_peak = implant_profile(dose, rp, drp, rp);
        let c_off = implant_profile(dose, rp, drp, rp + 5.0 * drp);
        assert!(c_peak > c_off);
    }

    #[test]
    fn test_oxide_thickness_dry() {
        // 1 hour dry oxidation at 1000°C
        let tox = oxide_thickness(3600.0, 1273.0, OxidationAmbient::DryOxygen);
        assert!(tox > 0.0);
        assert!(tox < 1e-4); // Less than 1 μm
    }

    #[test]
    fn test_oxide_thickness_wet_faster_than_dry() {
        let tox_dry = oxide_thickness(3600.0, 1273.0, OxidationAmbient::DryOxygen);
        let tox_wet = oxide_thickness(3600.0, 1273.0, OxidationAmbient::WetVapor);
        assert!(tox_wet > tox_dry);
    }

    #[test]
    fn test_oxide_thickness_zero_time() {
        let tox = oxide_thickness(0.0, 1273.0, OxidationAmbient::DryOxygen);
        assert!((tox).abs() < 1e-20);
    }

    #[test]
    fn test_diffusion_no_diffusivity() {
        let c = diffusion_profile(1e15, 0.0, 3600.0, 0.0);
        assert!(c > 0.0); // Should return thin layer approximation
    }
}
