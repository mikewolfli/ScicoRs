//! Package parasitics: bond wire, BGA, leadframe models.

use crate::core::types::Scalar;

/// Bond wire self-inductance (nH).
///
/// L ≈ 2·l·[ln(4·l/d) - 0.75], l in mm, d in mm.
pub fn bond_wire_inductance(length_mm: Scalar, diameter_mm: Scalar) -> Scalar {
    if length_mm <= 0.0 || diameter_mm <= 0.0 { return 0.0; }
    let ratio = 4.0 * length_mm / diameter_mm;
    2.0 * length_mm * (f64::ln(ratio) - 0.75)
}

/// BGA solder ball capacitance (pF).
pub fn bga_ball_capacitance(ball_diameter: Scalar, ball_pitch: Scalar, dielectric_er: Scalar, height: Scalar) -> Scalar {
    if ball_pitch <= 0.0 || height <= 0.0 { return 0.0; }
    let r = ball_diameter / 2.0;
    let c_self = 4.0 * std::f64::consts::PI * 8.854e-12 * dielectric_er * r;
    let c_mutual = c_self * r / ball_pitch;
    (c_self + c_mutual) * 1e12 // pF
}

/// Package parasitics model.
#[derive(Debug, Clone)]
pub struct PackageParasitics {
    pub r_bond: Vec<Scalar>,
    pub l_bond: Vec<Scalar>,
    pub c_pad: Vec<Scalar>,
    pub c_coupling: Vec<(usize, usize, Scalar)>,
}

impl PackageParasitics {
    pub fn new() -> Self { Self { r_bond: Vec::new(), l_bond: Vec::new(), c_pad: Vec::new(), c_coupling: Vec::new() } }

    pub fn total_pin_c(&self, pin: usize) -> Scalar {
        let mut c = *self.c_pad.get(pin).unwrap_or(&0.0);
        for &(i, j, cc) in &self.c_coupling {
            if i == pin || j == pin { c += cc; }
        }
        c
    }

    pub fn mutual_inductance(&self, pin_i: usize, pin_j: usize, k: Scalar) -> Scalar {
        let li = self.l_bond.get(pin_i).copied().unwrap_or(0.0);
        let lj = self.l_bond.get(pin_j).copied().unwrap_or(0.0);
        k * f64::sqrt(li * lj)
    }
}

impl Default for PackageParasitics { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bond_wire_inductance() {
        let l = bond_wire_inductance(1.0, 0.025);
        assert!(l > 0.0); // ~nH range for 1mm wire
    }

    #[test]
    fn test_bga_ball_capacitance() {
        let c = bga_ball_capacitance(0.5e-3, 0.8e-3, 4.5, 0.3e-3);
        assert!(c > 0.01 && c < 1.0); // pF range
    }

    #[test]
    fn test_package_total_pin_c() {
        let pkg = PackageParasitics { r_bond: vec![0.1], l_bond: vec![1.0], c_pad: vec![0.5], c_coupling: vec![(0, 1, 0.1)] };
        assert!((pkg.total_pin_c(0) - 0.6).abs() < 1e-10);
    }
}
