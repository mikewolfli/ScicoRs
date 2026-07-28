//! IV/CV curve computation utilities for semiconductor devices.
//!
//! Provides functions to compute IV (current-voltage) and CV (capacitance-voltage)
//! characteristic curves for MOSFET and BJT devices.

use crate::core::types::Scalar;
use super::mosfet::{MosfetModel, mosfet_drain_current};
use super::bjt::{BjtModel, bjt_collector_current};

// ──────────────────────────────────────────────
// 1. MOSFET IV Curves
// ──────────────────────────────────────────────

/// Compute MOSFET output IV curve (Id vs Vds) at multiple Vgs values.
///
/// Returns a vector of `(vds, vgs, id)` tuples forming the curve family.
///
/// # Arguments
/// * `model` - The MOSFET model
/// * `vgs_range` - (start, stop, num_steps) for gate voltage
/// * `vds_range` - (start, stop, num_steps) for drain voltage
/// * `vbs` - Bulk-source voltage
pub fn mosfet_iv_curve(
    model: &MosfetModel,
    vgs_range: (Scalar, Scalar, usize),
    vds_range: (Scalar, Scalar, usize),
    vbs: Scalar,
) -> Vec<(Scalar, Scalar, Scalar)> {
    let mut points = Vec::new();
    let (vgs_start, vgs_stop, vgs_steps) = vgs_range;
    let (vds_start, vds_stop, vds_steps) = vds_range;

    for i in 0..vgs_steps {
        let vgs = if vgs_steps > 1 {
            vgs_start + (vgs_stop - vgs_start) * i as Scalar / (vgs_steps - 1) as Scalar
        } else {
            vgs_start
        };
        for j in 0..vds_steps {
            let vds = if vds_steps > 1 {
                vds_start + (vds_stop - vds_start) * j as Scalar / (vds_steps - 1) as Scalar
            } else {
                vds_start
            };
            let id = mosfet_drain_current(model, vgs, vds, vbs);
            points.push((vds, vgs, id));
        }
    }
    points
}

/// Compute MOSFET transfer curve (Id vs Vgs) at a fixed Vds.
///
/// Returns a vector of `(vgs, id)` pairs.
///
/// # Arguments
/// * `model` - The MOSFET model
/// * `vds` - Fixed drain-source voltage
/// * `vgs_range` - (start, stop, num_steps) for gate voltage
/// * `vbs` - Bulk-source voltage
pub fn mosfet_transfer_curve(
    model: &MosfetModel,
    vds: Scalar,
    vgs_range: (Scalar, Scalar, usize),
    vbs: Scalar,
) -> Vec<(Scalar, Scalar)> {
    let (start, stop, steps) = vgs_range;
    let mut points = Vec::with_capacity(steps);

    for i in 0..steps {
        let vgs = if steps > 1 {
            start + (stop - start) * i as Scalar / (steps - 1) as Scalar
        } else {
            start
        };
        let id = mosfet_drain_current(model, vgs, vds, vbs);
        points.push((vgs, id));
    }
    points
}

/// Compute MOSFET CV curve (Cgg vs Vgs) — gate capacitance.
///
/// Returns a vector of `(vgs, cgg)` pairs where Cgg is the small-signal
/// gate capacitance normalized to oxide capacitance.
///
/// # Arguments
/// * `model` - The MOSFET model
/// * `vgs_range` - (start, stop, num_steps) for gate voltage
pub fn mosfet_cv_curve(
    model: &MosfetModel,
    vgs_range: (Scalar, Scalar, usize),
) -> Vec<(Scalar, Scalar)> {
    let (start, stop, steps) = vgs_range;
    let mut points = Vec::with_capacity(steps);

    for i in 0..steps {
        let vgs = if steps > 1 {
            start + (stop - start) * i as Scalar / (steps - 1) as Scalar
        } else {
            start
        };

        // Simplified capacitance model:
        // - Below threshold: Cgg = Cox (accumulation/depletion, simplified)
        // - Above threshold: Cgg = 2/3 * Cox (saturation region)
        let vth = model.threshold_voltage(0.0);
        let cox_ratio = if vgs > vth { 0.667 } else { 1.0 };

        points.push((vgs, cox_ratio));
    }
    points
}

// ──────────────────────────────────────────────
// 2. BJT IV Curves
// ──────────────────────────────────────────────

/// Compute BJT output IV curve (Ic vs Vce) at multiple Ib or Vbe values.
///
/// Returns a vector of `(vce, vbe_or_ib, ic)` tuples.
///
/// # Arguments
/// * `model` - The BJT model
/// * `vbe_list` - List of base-emitter voltages to sweep
/// * `vce_range` - (start, stop, num_steps) for collector-emitter voltage
pub fn bjt_iv_curve(
    model: &BjtModel,
    vbe_list: &[Scalar],
    vce_range: (Scalar, Scalar, usize),
) -> Vec<(Scalar, Scalar, Scalar)> {
    let (start, stop, steps) = vce_range;
    let mut points = Vec::new();

    for &vbe in vbe_list {
        for i in 0..steps {
            let vce = if steps > 1 {
                start + (stop - start) * i as Scalar / (steps - 1) as Scalar
            } else {
                start
            };
            // Vbc = Vb - Vc = Vbe + Ve - Vce - Ve = Vbe - Vce
            let vbc = vbe - vce;
            let ic = bjt_collector_current(model, vbe, vbc);
            points.push((vce, vbe, ic));
        }
    }
    points
}

/// Compute BJT transfer curve (Ic vs Vbe) at a fixed Vce.
pub fn bjt_transfer_curve(
    model: &BjtModel,
    vbe_range: (Scalar, Scalar, usize),
    vce: Scalar,
) -> Vec<(Scalar, Scalar)> {
    let (start, stop, steps) = vbe_range;
    let mut points = Vec::with_capacity(steps);

    for i in 0..steps {
        let vbe = if steps > 1 {
            start + (stop - start) * i as Scalar / (steps - 1) as Scalar
        } else {
            start
        };
        let vbc = vbe - vce;
        let ic = bjt_collector_current(model, vbe, vbc);
        points.push((vbe, ic));
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mosfet_iv_curve_basic() {
        let model = MosfetModel::new_nmos();
        let curve = mosfet_iv_curve(&model, (0.0, 2.0, 3), (0.0, 2.0, 5), 0.0);
        // 3 * 5 = 15 points
        assert_eq!(curve.len(), 15);
    }

    #[test]
    fn test_mosfet_iv_curve_saturation() {
        let model = MosfetModel::new_nmos();
        // Vgs=2V, Vds=2V → saturation
        let id_sat = mosfet_drain_current(&model, 2.0, 2.0, 0.0);
        // Vgs=2V, Vds=0.1V → triode
        let id_tri = mosfet_drain_current(&model, 2.0, 0.1, 0.0);
        assert!(id_sat > id_tri);
    }

    #[test]
    fn test_mosfet_transfer_curve() {
        let model = MosfetModel::new_nmos();
        let curve = mosfet_transfer_curve(&model, 2.0, (0.0, 2.0, 10), 0.0);
        assert_eq!(curve.len(), 10);
        // Id should increase with Vgs
        for i in 1..curve.len() {
            if curve[i].1 > 1e-15 && curve[i - 1].1 > 1e-15 {
                assert!(curve[i].1 >= curve[i - 1].1 * 0.999);
            }
        }
    }

    #[test]
    fn test_mosfet_cv_curve() {
        let model = MosfetModel::new_nmos();
        let curve = mosfet_cv_curve(&model, (-1.0, 2.0, 10));
        assert_eq!(curve.len(), 10);
        // Cgg should transition from 1.0 to 0.667
        let above_vth = curve.iter().any(|(_, c)| (*c - 0.667).abs() < 0.01);
        assert!(above_vth);
    }

    #[test]
    fn test_bjt_iv_curve_basic() {
        let model = BjtModel::new_npn();
        let curve = bjt_iv_curve(&model, &[0.6, 0.7], (0.0, 3.0, 10));
        // 2 * 10 = 20 points
        assert_eq!(curve.len(), 20);
    }

    #[test]
    fn test_bjt_transfer_curve() {
        let model = BjtModel::new_npn();
        let curve = bjt_transfer_curve(&model, (0.4, 0.8, 20), 2.0);
        assert_eq!(curve.len(), 20);
        // Ic increases exponentially with Vbe
        assert!(curve.last().unwrap().1 > curve.first().unwrap().1);
    }

    #[test]
    fn test_bjt_iv_curve_saturation_region() {
        let model = BjtModel::new_npn();
        let curve = bjt_iv_curve(&model, &[0.7], (0.0, 3.0, 5));
        // At Vce=3V (forward active), Ic should be higher than at Vce=0V
        let ic_sat = curve.last().unwrap().2;
        let ic_linear = curve.first().unwrap().2;
        assert!(ic_sat.abs() >= ic_linear.abs());
    }
}
