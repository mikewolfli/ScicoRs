//! MOSFET DC/AC models (Shichman-Hodges Level 1).
//!
//! Provides the Shichman-Hodges (Level 1) MOSFET model for DC drain current,
//! small-signal parameters, and a `Block`-wrapped `MosfetBlock` for use
//! within the simulation engine.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, PortDirection as PD, Scalar, SignalType, SignalValue, Time,
};


// ──────────────────────────────────────────────
// 1. MOSFET Model Parameters
// ──────────────────────────────────────────────

/// Shichman-Hodges (Level 1) MOSFET model.
///
/// Supports both NMOS and PMOS devices with channel-length modulation,
/// body effect, and basic geometry scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MosfetModel {
    /// Threshold voltage (V) — VTO for NMOS, -VTO for PMOS.
    pub vto: Scalar,
    /// Transconductance parameter KP = μ*Cox (A/V²).
    pub kp: Scalar,
    /// Channel-length modulation parameter (1/V).
    pub lambda: Scalar,
    /// Body effect coefficient (V^0.5).
    pub gamma: Scalar,
    /// Surface potential (V) — usually 2*Φf.
    pub phi: Scalar,
    /// Channel width (m).
    pub w: Scalar,
    /// Channel length (m).
    pub l: Scalar,
    /// True for NMOS, false for PMOS.
    pub is_nmos: bool,
    /// Temperature (K).
    pub temp: Scalar,
}

impl MosfetModel {
    /// Create a new MOSFET model with default parameters (NMOS).
    pub fn new_nmos() -> Self {
        Self {
            vto: 0.7,
            kp: 1.0e-4,
            lambda: 0.02,
            gamma: 0.5,
            phi: 0.65,
            w: 1e-6,
            l: 1e-6,
            is_nmos: true,
            temp: 300.0,
        }
    }

    /// Create a new MOSFET model with default parameters (PMOS).
    pub fn new_pmos() -> Self {
        Self {
            vto: -0.7,
            kp: 4.0e-5,
            lambda: 0.03,
            gamma: 0.6,
            phi: 0.65,
            w: 2e-6,
            l: 1e-6,
            is_nmos: false,
            temp: 300.0,
        }
    }

    /// Effective voltage (Vgs - Vth), accounting for body effect.
    fn v_eff(&self, vgs: Scalar, vbs: Scalar) -> Scalar {
        let vth = self.threshold_voltage(vbs);
        if self.is_nmos { vgs - vth } else { vth - vgs }
    }

    /// Threshold voltage including body effect.
    pub fn threshold_voltage(&self, vbs: Scalar) -> Scalar {
        if self.is_nmos {
            let vth = self.vto + self.gamma * ((self.phi - vbs).sqrt() - self.phi.sqrt());
            vth.max(0.01) // Ensure positive for NMOS
        } else {
            let vth = self.vto - self.gamma * ((self.phi + vbs).sqrt() - self.phi.sqrt());
            vth.min(-0.01) // Ensure negative for PMOS
        }
    }

    /// Beta factor: β = KP * W/L.
    pub fn beta(&self) -> Scalar {
        self.kp * self.w / self.l
    }

}

impl MosfetModel {
    /// Saturation voltage: Vdsat = Veff.
    pub fn vdsat(&self, vgs: Scalar, vbs: Scalar) -> Scalar {
        self.v_eff(vgs, vbs).max(0.0)
    }
}

// ──────────────────────────────────────────────
// 2. DC Drain Current
// ──────────────────────────────────────────────

/// Compute MOSFET drain current using Shichman-Hodges Level 1 model.
///
/// Returns drain current Id (A). Positive = conventional drain-to-source.
///
/// # Regions
/// - **Cutoff**: Vgs < Vth → Id = 0
/// - **Triode (Linear)**: Vds < Vdsat → Id = β * (Veff - Vds/2) * Vds * (1 + λ*Vds)
/// - **Saturation**: Vds ≥ Vdsat → Id = β/2 * Veff² * (1 + λ*Vds)
pub fn mosfet_drain_current(model: &MosfetModel, vgs: Scalar, vds: Scalar, vbs: Scalar) -> Scalar {
    let veff = model.v_eff(vgs, vbs);
    if veff <= 0.0 {
        return 0.0; // Cutoff region
    }

    let beta = model.beta();
    let vdsat = veff;
    let lambda_effect = 1.0 + model.lambda * vds.abs();

    if vds.abs() < vdsat {
        // Triode (linear) region
        let id = beta * (veff - vds.abs() * 0.5) * vds.abs() * lambda_effect;
        if model.is_nmos { id } else { -id }
    } else {
        // Saturation region
        let id = 0.5 * beta * veff * veff * lambda_effect;
        if model.is_nmos { id } else { -id }
    }
}

/// Small-signal transconductance gm = ∂Id/∂Vgs.
pub fn mosfet_gm(model: &MosfetModel, vgs: Scalar, vds: Scalar, vbs: Scalar) -> Scalar {
    let id = mosfet_drain_current(model, vgs, vds, vbs);
    let veff = model.v_eff(vgs, vbs);
    if veff <= 0.0 || id.abs() < 1e-20 {
        return 0.0;
    }
    if vds.abs() < veff {
        // Triode: gm = β * Vds
        model.beta() * vds.abs()
    } else {
        // Saturation: gm = sqrt(2 * β * Id)
        (2.0 * model.beta() * id.abs()).sqrt()
    }
}

/// Small-signal output conductance gds = ∂Id/∂Vds.
pub fn mosfet_gds(model: &MosfetModel, vgs: Scalar, vds: Scalar, vbs: Scalar) -> Scalar {
    let id = mosfet_drain_current(model, vgs, vds, vbs);
    let veff = model.v_eff(vgs, vbs);
    if veff <= 0.0 || id.abs() < 1e-20 {
        return 0.0;
    }
    let beta = model.beta();
    if vds.abs() < veff {
        // Triode: gds = β * (Veff - Vds) + correction
        beta * (veff - vds.abs()) + model.lambda * id.abs()
    } else {
        // Saturation: gds = λ * Id
        model.lambda * id.abs()
    }
}

// ──────────────────────────────────────────────
// 3. Block Wrapper
// ──────────────────────────────────────────────

/// MOSFET block for use in the simulation engine.
///
/// Ports:
/// - `g` (input): Gate voltage
/// - `d` (input): Drain voltage
/// - `s` (input): Source voltage
/// - `b` (input): Bulk voltage
/// - `id` (output): Drain current
#[derive(Debug, Clone)]
pub struct MosfetBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    /// The MOSFET model parameters.
    pub model: MosfetModel,
}

impl MosfetBlock {
    /// Create a new MOSFET block with the given model.
    pub fn new(id: &str, model: MosfetModel) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("g", PD::Input, SignalType::Continuous));
        ports.add(Port::new("d", PD::Input, SignalType::Continuous));
        ports.add(Port::new("s", PD::Input, SignalType::Continuous));
        ports.add(Port::new("b", PD::Input, SignalType::Continuous));
        ports.add(Port::new("id", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "MosfetBlock".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            model,
        }
    }

    /// Read a port voltage.
    fn read_voltage(&self, name: &str) -> Scalar {
        self.ports
            .get(name)
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0)
    }
}

impl Block for MosfetBlock {
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
        let vg = self.read_voltage("g");
        let vd = self.read_voltage("d");
        let vs = self.read_voltage("s");
        let vb = self.read_voltage("b");

        let vgs = vg - vs;
        let vds = vd - vs;
        let vbs = vb - vs;

        let id = mosfet_drain_current(&self.model, vgs, vds, vbs);

        if let Some(port) = self.ports.get_mut("id") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(id),
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
    fn test_mosfet_model_default_nmos() {
        let model = MosfetModel::new_nmos();
        assert!((model.vto - 0.7).abs() < 0.01);
        assert!(model.is_nmos);
    }

    #[test]
    fn test_mosfet_model_default_pmos() {
        let model = MosfetModel::new_pmos();
        assert!((model.vto + 0.7).abs() < 0.01);
        assert!(!model.is_nmos);
    }

    #[test]
    fn test_mosfet_cutoff_region() {
        let model = MosfetModel::new_nmos();
        let id = mosfet_drain_current(&model, 0.0, 1.0, 0.0);
        assert!((id).abs() < 1e-20);
    }

    #[test]
    fn test_mosfet_saturation_region() {
        let model = MosfetModel::new_nmos();
        let id = mosfet_drain_current(&model, 1.5, 2.0, 0.0);
        // Should be in saturation: Vgs > Vth, Vds > Vdsat
        assert!(id > 0.0);
    }

    #[test]
    fn test_mosfet_triode_region() {
        let model = MosfetModel::new_nmos();
        let id = mosfet_drain_current(&model, 1.5, 0.1, 0.0);
        // Should be in triode: Vgs > Vth, Vds < Vdsat
        assert!(id > 0.0);
        // Triode current should be less than saturation at same Vgs
        let id_sat = mosfet_drain_current(&model, 1.5, 2.0, 0.0);
        assert!(id < id_sat);
    }

    #[test]
    fn test_mosfet_gm_saturation() {
        let model = MosfetModel::new_nmos();
        let gm = mosfet_gm(&model, 1.5, 2.0, 0.0);
        assert!(gm > 0.0);
    }

    #[test]
    fn test_mosfet_gds_saturation() {
        let model = MosfetModel::new_nmos();
        let gds = mosfet_gds(&model, 1.5, 2.0, 0.0);
        assert!(gds > 0.0);
    }

    #[test]
    fn test_mosfet_block_creation() {
        let model = MosfetModel::new_nmos();
        let block = MosfetBlock::new("m1", model);
        assert_eq!(block.id(), "m1");
        assert_eq!(block.block_type(), "MosfetBlock");
    }

    #[test]
    fn test_mosfet_block_ports() {
        let model = MosfetModel::new_nmos();
        let block = MosfetBlock::new("m1", model);
        assert!(block.ports().get("g").is_some());
        assert!(block.ports().get("d").is_some());
        assert!(block.ports().get("s").is_some());
        assert!(block.ports().get("b").is_some());
        assert!(block.ports().get("id").is_some());
    }

    #[test]
    fn test_mosfet_pmos_negative_current() {
        let model = MosfetModel::new_pmos();
        // PMOS: Vgs < Vth (more negative than -0.7), Vds < 0
        let id = mosfet_drain_current(&model, -1.5, -1.0, 0.0);
        assert!(id < 0.0); // PMOS current flows opposite direction
    }

    #[test]
    fn test_mosfet_threshold_body_effect() {
        let model = MosfetModel::new_nmos();
        let vth0 = model.threshold_voltage(0.0);
        let vth_neg = model.threshold_voltage(-1.0);
        // Body effect increases Vth when Vbs is negative (for NMOS)
        assert!(vth_neg > vth0);
    }

    #[test]
    fn test_mosfet_beta_scaling() {
        let model = MosfetModel::new_nmos();
        let beta = model.beta();
        // KP * W/L = 1e-4 * 1/1 = 1e-4
        assert!((beta - 1.0e-4).abs() < 1e-6);
    }
}
