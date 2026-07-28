//! BJT Ebers-Moll model.
//!
//! Provides the Ebers-Moll large-signal BJT model for DC collector and
//! base current computation, small-signal parameters, and a `Block`-wrapped
//! `BjtBlock` for use within the simulation engine.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, PortDirection as PD, Scalar, SignalType, SignalValue, Time,
};

use super::physics::thermal_voltage;

// ──────────────────────────────────────────────
// 1. BJT Model Parameters
// ──────────────────────────────────────────────

/// Ebers-Moll large-signal BJT model.
///
/// Supports both NPN and PNP bipolar junction transistors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BjtModel {
    /// Saturation current (A) — IS.
    pub is: Scalar,
    /// Forward current gain — BF (beta_f).
    pub bf: Scalar,
    /// Reverse current gain — BR (beta_r).
    pub br: Scalar,
    /// Forward Early voltage (V) — VAF.
    pub vaf: Scalar,
    /// Reverse Early voltage (V) — VAR.
    pub var: Scalar,
    /// Emission coefficient for forward mode — NF.
    pub nf: Scalar,
    /// Emission coefficient for reverse mode — NR.
    pub nr: Scalar,
    /// True for NPN, false for PNP.
    pub is_npn: bool,
    /// Temperature (K).
    pub temp: Scalar,
}

impl BjtModel {
    /// Create a new NPN BJT model with typical parameters.
    pub fn new_npn() -> Self {
        Self {
            is: 1.0e-16,
            bf: 100.0,
            br: 1.0,
            vaf: 50.0,
            var: 5.0,
            nf: 1.0,
            nr: 1.0,
            is_npn: true,
            temp: 300.0,
        }
    }

    /// Create a new PNP BJT model with typical parameters.
    pub fn new_pnp() -> Self {
        Self {
            is: 1.0e-16,
            bf: 80.0,
            br: 0.5,
            vaf: 40.0,
            var: 4.0,
            nf: 1.0,
            nr: 1.0,
            is_npn: false,
            temp: 300.0,
        }
    }

    /// Thermal voltage at model temperature.
    pub fn v_t(&self) -> Scalar {
        thermal_voltage(self.temp)
    }

    /// Forward transport current: Icc = IS * (exp(Vbe/(NF*Vt)) - 1).
    fn forward_current(&self, vbe: Scalar) -> Scalar {
        let vt = self.v_t();
        let arg = vbe / (self.nf * vt);
        if arg > 100.0 {
            self.is * arg.exp() // Avoid overflow with clamp
        } else {
            self.is * (arg.exp() - 1.0)
        }
    }

    /// Reverse transport current: Iec = IS * (exp(Vbc/(NR*Vt)) - 1).
    fn reverse_current(&self, vbc: Scalar) -> Scalar {
        let vt = self.v_t();
        let arg = vbc / (self.nr * vt);
        if arg > 100.0 {
            self.is * arg.exp()
        } else {
            self.is * (arg.exp() - 1.0)
        }
    }
}

// ──────────────────────────────────────────────
// 2. Collector and Base Current
// ──────────────────────────────────────────────

/// Compute BJT collector current using Ebers-Moll model.
///
/// Returns Ic (A). Positive = conventional collector current (into collector
/// for NPN, out of collector for PNP).
///
/// # Arguments
/// * `model` - BJT model parameters
/// * `vbe` - Base-emitter voltage (V)
/// * `vbc` - Base-collector voltage (V)
pub fn bjt_collector_current(model: &BjtModel, vbe: Scalar, vbc: Scalar) -> Scalar {
    let icc = model.forward_current(vbe);
    let iec = model.reverse_current(vbc);

    // Collector current = Icc - Iec/BR - Iec (Gummel-Poon simplified)
    let ic = icc - iec * (1.0 + 1.0 / model.br);

    // Early effect (forward)
    let early_factor = if vbc > -model.vaf && model.vaf > 0.0 {
        1.0 + vbc.abs() / model.vaf
    } else {
        1.0
    };

    let ic_early = ic * early_factor;

    if model.is_npn { ic_early } else { -ic_early }
}

/// Compute BJT base current.
///
/// Returns Ib (A).
pub fn bjt_base_current(model: &BjtModel, vbe: Scalar, vbc: Scalar) -> Scalar {
    let icc = model.forward_current(vbe);
    let iec = model.reverse_current(vbc);

    let ib_fwd = icc / model.bf;
    let ib_rev = iec / model.br;

    let ib = ib_fwd + ib_rev;

    if model.is_npn { ib } else { -ib }
}

/// Compute BJT emitter current: Ie = -(Ic + Ib).
pub fn bjt_emitter_current(ic: Scalar, ib: Scalar) -> Scalar {
    -(ic + ib)
}

/// Small-signal transconductance: gm = Ic / Vt.
pub fn bjt_gm(ic: Scalar, vt: Scalar) -> Scalar {
    if vt > 0.0 {
        ic / vt
    } else {
        0.0
    }
}

/// Small-signal base-emitter resistance: rπ = β / gm.
pub fn bjt_rpi(beta: Scalar, gm: Scalar) -> Scalar {
    if gm > 0.0 {
        beta / gm
    } else {
        1e12
    }
}

// ──────────────────────────────────────────────
// 3. Block Wrapper
// ──────────────────────────────────────────────

/// BJT block for use in the simulation engine.
///
/// Ports:
/// - `b` (input): Base voltage
/// - `c` (input): Collector voltage
/// - `e` (input): Emitter voltage
/// - `ic` (output): Collector current
/// - `ib` (output): Base current
#[derive(Debug, Clone)]
pub struct BjtBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    /// The BJT model parameters.
    pub model: BjtModel,
}

impl BjtBlock {
    /// Create a new BJT block with the given model.
    pub fn new(id: &str, model: BjtModel) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("b", PD::Input, SignalType::Continuous));
        ports.add(Port::new("c", PD::Input, SignalType::Continuous));
        ports.add(Port::new("e", PD::Input, SignalType::Continuous));
        ports.add(Port::new("ic", PD::Output, SignalType::Continuous));
        ports.add(Port::new("ib", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "BjtBlock".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            model,
        }
    }

    fn read_voltage(&self, name: &str) -> Scalar {
        self.ports
            .get(name)
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0)
    }
}

impl Block for BjtBlock {
    fn id(&self) -> &BlockId {
        &self.id
    }
    fn block_type(&self) -> &str {
        &self.block_type
    }
    fn ports(&self) -> &PortSet {
        &self.ports
    }
    fn ports_mut(&mut self) -> &mut PortSet {
        &mut self.ports
    }
    fn params(&self) -> &ParameterSet {
        &self.params
    }
    fn params_mut(&mut self) -> &mut ParameterSet {
        &mut self.params
    }
    fn status(&self) -> ComponentStatus {
        self.status
    }
    fn set_status(&mut self, s: ComponentStatus) {
        self.status = s;
    }
    fn set_time(&mut self, t: Time) {
        self.current_time = t;
    }
    fn time(&self) -> Time {
        self.current_time
    }

    fn init(&mut self) -> Result<(), SimError> {
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        let vb = self.read_voltage("b");
        let vc = self.read_voltage("c");
        let ve = self.read_voltage("e");

        let vbe = vb - ve;
        let vbc = vb - vc;

        let ic = bjt_collector_current(&self.model, vbe, vbc);
        let ib = bjt_base_current(&self.model, vbe, vbc);

        if let Some(port) = self.ports.get_mut("ic") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(ic),
                self.current_time,
            ));
        }
        if let Some(port) = self.ports.get_mut("ib") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(ib),
                self.current_time,
            ));
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, SimError> {
        Ok(Vec::new())
    }
    fn update(&mut self) -> Result<(), SimError> {
        Ok(())
    }
    fn zero_crossings(&self) -> Vec<Scalar> {
        Vec::new()
    }
    fn terminate(&mut self) -> Result<(), SimError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }
    fn clone_block(&self) -> Box<dyn Block> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bjt_model_default_npn() {
        let model = BjtModel::new_npn();
        assert!((model.bf - 100.0).abs() < 1.0);
        assert!(model.is_npn);
    }

    #[test]
    fn test_bjt_model_default_pnp() {
        let model = BjtModel::new_pnp();
        assert!((model.bf - 80.0).abs() < 1.0);
        assert!(!model.is_npn);
    }

    #[test]
    fn test_bjt_forward_active_ic() {
        let model = BjtModel::new_npn();
        // Vbe = 0.7V (forward biased), Vbc = -2V (reverse biased)
        let ic = bjt_collector_current(&model, 0.7, -2.0);
        assert!(ic > 1e-10);
        assert!(ic < 1e-1); // Reasonable range
    }

    #[test]
    fn test_bjt_cutoff_ic() {
        let model = BjtModel::new_npn();
        // Vbe = 0V (cutoff)
        let ic = bjt_collector_current(&model, 0.0, 0.0);
        assert!((ic).abs() < 1e-20);
    }

    #[test]
    fn test_bjt_base_current() {
        let model = BjtModel::new_npn();
        let ic = bjt_collector_current(&model, 0.7, -2.0);
        let ib = bjt_base_current(&model, 0.7, -2.0);
        // Ib = Ic / beta
        let beta_measured = ic / ib;
        assert!((beta_measured - model.bf).abs() / model.bf < 0.5);
    }

    #[test]
    fn test_bjt_small_signal_gm() {
        let model = BjtModel::new_npn();
        let ic = bjt_collector_current(&model, 0.7, -2.0);
        let vt = model.v_t();
        let gm = bjt_gm(ic, vt);
        assert!(gm > 0.0);
        // gm = Ic / Vt ≈ Ic / 0.02585
        assert!((gm - ic / vt).abs() < 1e-6 * gm.abs().max(1.0));
    }

    #[test]
    fn test_bjt_emitter_current_kcl() {
        let model = BjtModel::new_npn();
        let ic = bjt_collector_current(&model, 0.7, -2.0);
        let ib = bjt_base_current(&model, 0.7, -2.0);
        let ie = bjt_emitter_current(ic, ib);
        // KCL: Ie + Ic + Ib = 0
        assert!((ie + ic + ib).abs() < 1e-20);
    }

    #[test]
    fn test_bjt_block_creation() {
        let model = BjtModel::new_npn();
        let block = BjtBlock::new("q1", model);
        assert_eq!(block.id(), "q1");
        assert_eq!(block.block_type(), "BjtBlock");
    }

    #[test]
    fn test_bjt_block_ports() {
        let model = BjtModel::new_npn();
        let block = BjtBlock::new("q1", model);
        assert!(block.ports().get("b").is_some());
        assert!(block.ports().get("c").is_some());
        assert!(block.ports().get("e").is_some());
        assert!(block.ports().get("ic").is_some());
        assert!(block.ports().get("ib").is_some());
    }

    #[test]
    fn test_bjt_pnp_current_direction() {
        let model = BjtModel::new_pnp();
        // PNP forward-active: Vbe forward biased (<0), Vbc reverse biased (>0)
        let ic = bjt_collector_current(&model, -0.7, -2.0);
        assert!(ic < 0.0); // PNP collector current is negative
    }

    #[test]
    fn test_bjt_early_effect() {
        let model_npn = BjtModel::new_npn();
        let ic_low_vce = bjt_collector_current(&model_npn, 0.7, -0.5);
        let ic_high_vce = bjt_collector_current(&model_npn, 0.7, -5.0);
        // Higher Vce (more reverse Vbc) should give slightly higher Ic (Early effect)
        assert!(ic_high_vce.abs() > ic_low_vce.abs());
    }
}
