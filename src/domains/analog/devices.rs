//! SPICE device element stamps and Block wrappers.
//!
//! Provides element stamps for MNA matrix assembly and corresponding
//! Block implementations for passive and active devices.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, PortDirection as PD, Scalar, SignalType, SignalValue, Time,
};

use super::mna::MnaMatrix;

// ──────────────────────────────────────────────
// 1. MNA Element Stamps
// ──────────────────────────────────────────────

/// Resistor stamp helper.
pub struct ResistorStamp {
    pub node_p: usize,
    pub node_n: usize,
    pub resistance: Scalar,
}

impl ResistorStamp {
    pub fn stamp(&self, mna: &mut MnaMatrix) {
        mna.stamp_resistor(self.node_p, self.node_n, self.resistance);
    }
}

/// Capacitor stamp (uses companion model for transient analysis).
pub struct CapacitorStamp {
    pub node_p: usize,
    pub node_n: usize,
    pub capacitance: Scalar,
    /// Previous voltage across capacitor (for companion model).
    pub prev_voltage: Scalar,
}

impl CapacitorStamp {
    /// Companion model conductance: G_eq = C / dt
    pub fn companion_conductance(&self, dt: Scalar) -> Scalar {
        if dt <= 0.0 {
            1e12
        } else {
            self.capacitance / dt
        }
    }

    /// Companion model current source: I_eq = -C/dt * V(t)
    pub fn companion_current(&self, dt: Scalar, v_prev: Scalar) -> Scalar {
        if dt <= 0.0 {
            0.0
        } else {
            -self.capacitance / dt * v_prev
        }
    }
}

/// Inductor stamp (uses companion model for transient analysis).
pub struct InductorStamp {
    pub node_p: usize,
    pub node_n: usize,
    pub inductance: Scalar,
    /// Previous current through inductor (for companion model).
    pub prev_current: Scalar,
}

impl InductorStamp {
    /// Companion model conductance: G_eq = dt / L
    pub fn companion_conductance(&self, dt: Scalar) -> Scalar {
        if self.inductance <= 0.0 {
            1e12
        } else {
            dt / self.inductance
        }
    }

    /// Companion model current source: I_eq = I(t) (historical term)
    pub fn companion_current(&self, i_prev: Scalar) -> Scalar {
        i_prev
    }
}

/// Diode stamp (non-linear, uses linearized companion model).
pub struct DiodeStamp {
    pub node_p: usize,
    pub node_n: usize,
    pub saturation_current: Scalar,
    pub emission_coefficient: Scalar,
    pub temperature: Scalar,
}

impl DiodeStamp {
    /// Thermal voltage at model temperature.
    pub fn v_t(&self) -> Scalar {
        1.380649e-23 * self.temperature / 1.602176634e-19
    }

    /// Diode current: Id = Is * (exp(Vd/(N*Vt)) - 1)
    pub fn current(&self, vd: Scalar) -> Scalar {
        let vt = self.v_t();
        let nvt = self.emission_coefficient * vt;
        if nvt <= 0.0 {
            return 0.0;
        }
        let arg = vd / nvt;
        if arg > 100.0 {
            self.saturation_current * arg.exp()
        } else {
            self.saturation_current * (arg.exp() - 1.0)
        }
    }

    /// Small-signal conductance: Gd = dId/dVd = Is/(N*Vt) * exp(Vd/(N*Vt))
    pub fn conductance(&self, vd: Scalar) -> Scalar {
        let vt = self.v_t();
        let nvt = self.emission_coefficient * vt;
        if nvt <= 0.0 {
            return 0.0;
        }
        let arg = vd / nvt;
        self.saturation_current / nvt * arg.exp()
    }

    /// Linearized companion model: I = Ieq + Geq * V
    pub fn companion(&self, vd: Scalar) -> (Scalar, Scalar) {
        let geq = self.conductance(vd);
        let ieq = self.current(vd) - geq * vd;
        (geq, ieq)
    }
}

/// MOSFET stamp for MNA (uses linearized small-signal model).
pub struct MosfetStamp {
    pub node_d: usize,
    pub node_g: usize,
    pub node_s: usize,
    pub gm: Scalar,
    pub gds: Scalar,
}

impl MosfetStamp {
    /// Stamp the linearized MOSFET small-signal model into MNA.
    pub fn stamp(&self, mna: &mut MnaMatrix) {
        // Gds between drain and source
        mna.stamp_conductance(self.node_d, self.node_s, self.gds);
        // VCCS: gm * Vgs, drain-to-source
        mna.stamp_vccs(self.node_d, self.node_s, self.node_g, self.node_s, self.gm);
    }
}

/// BJT stamp for MNA.
pub struct BjtStamp {
    pub node_c: usize,
    pub node_b: usize,
    pub node_e: usize,
    pub gm: Scalar,
    pub go: Scalar,  // output conductance
    pub gpi: Scalar, // base-emitter conductance
}

impl BjtStamp {
    pub fn stamp(&self, mna: &mut MnaMatrix) {
        // Gpi between base and emitter
        mna.stamp_conductance(self.node_b, self.node_e, self.gpi);
        // VCCS: gm * Vbe, collector-to-emitter
        mna.stamp_vccs(self.node_c, self.node_e, self.node_b, self.node_e, self.gm);
        // Go between collector and emitter
        mna.stamp_conductance(self.node_c, self.node_e, self.go);
    }
}

/// Op-amp stamp (ideal).
pub struct OpAmpStamp {
    pub node_p: usize, // non-inverting input
    pub node_n: usize, // inverting input
    pub node_o: usize, // output
}

impl OpAmpStamp {
    /// For an ideal op-amp, the output voltage is A*(Vp - Vn).
    /// We model this as a VCVS between node_o and ground.
    pub fn stamp(&self, mna: &mut MnaMatrix, gain: Scalar, vsrc_idx: usize) {
        mna.stamp_vcvs(self.node_o, 0, self.node_p, self.node_n, gain, vsrc_idx);
    }
}

/// Current source stamp.
pub struct CurrentSourceStamp {
    pub node_p: usize,
    pub node_n: usize,
    pub current: Scalar,
}

impl CurrentSourceStamp {
    pub fn stamp(&self, mna: &mut MnaMatrix) {
        mna.stamp_current_source(self.node_p, self.node_n, self.current);
    }
}

/// Voltage source stamp.
pub struct VoltageSourceStamp {
    pub node_p: usize,
    pub node_n: usize,
    pub voltage: Scalar,
    pub vsrc_idx: usize,
}

impl VoltageSourceStamp {
    pub fn stamp(&self, mna: &mut MnaMatrix) {
        mna.stamp_voltage_source(self.node_p, self.node_n, self.voltage, self.vsrc_idx);
    }
}

// ──────────────────────────────────────────────
// 2. Block Wrappers
// ──────────────────────────────────────────────

/// Resistor block for use in the simulation engine.
#[derive(Debug, Clone)]
pub struct ResistorBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub resistance: Scalar,
}

impl ResistorBlock {
    pub fn new(id: &str, resistance: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("p", PD::Input, SignalType::Continuous));
        ports.add(Port::new("n", PD::Input, SignalType::Continuous));
        ports.add(Port::new("i", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Resistor".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            resistance,
        }
    }
}

impl Block for ResistorBlock {
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
        let vp = self
            .ports
            .get("p")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let vn = self
            .ports
            .get("n")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let i = if self.resistance > 0.0 {
            (vp - vn) / self.resistance
        } else {
            0.0
        };
        if let Some(port) = self.ports.get_mut("i") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(i),
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

/// Capacitor block for use in the simulation engine.
#[derive(Debug, Clone)]
pub struct CapacitorBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub capacitance: Scalar,
    prev_voltage: Scalar,
}

impl CapacitorBlock {
    pub fn new(id: &str, capacitance: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("p", PD::Input, SignalType::Continuous));
        ports.add(Port::new("n", PD::Input, SignalType::Continuous));
        ports.add(Port::new("i", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Capacitor".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            capacitance,
            prev_voltage: 0.0,
        }
    }
}

impl Block for CapacitorBlock {
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
        self.prev_voltage = 0.0;
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        let vp = self
            .ports
            .get("p")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let vn = self
            .ports
            .get("n")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let v = vp - vn;
        // I = C * dV/dt ≈ C * (V - V_prev) / dt
        // For now, output the voltage across the capacitor
        if let Some(port) = self.ports.get_mut("i") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(v),
                self.current_time,
            ));
        }
        Ok(())
    }
    fn derivative(&self) -> Result<Vec<Scalar>, SimError> {
        Ok(Vec::new())
    }
    fn update(&mut self) -> Result<(), SimError> {
        let vp = self
            .ports
            .get("p")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let vn = self
            .ports
            .get("n")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        self.prev_voltage = vp - vn;
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

/// Inductor block for use in the simulation engine.
#[derive(Debug, Clone)]
pub struct InductorBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub inductance: Scalar,
    prev_current: Scalar,
}

impl InductorBlock {
    pub fn new(id: &str, inductance: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("p", PD::Input, SignalType::Continuous));
        ports.add(Port::new("n", PD::Input, SignalType::Continuous));
        ports.add(Port::new("i", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Inductor".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            inductance,
            prev_current: 0.0,
        }
    }
}

impl Block for InductorBlock {
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
        self.prev_current = 0.0;
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        if let Some(port) = self.ports.get_mut("i") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(self.prev_current),
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

/// Diode block for use in the simulation engine.
#[derive(Debug, Clone)]
pub struct DiodeBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub saturation_current: Scalar,
    pub emission_coefficient: Scalar,
    pub temperature: Scalar,
}

impl DiodeBlock {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("p", PD::Input, SignalType::Continuous));
        ports.add(Port::new("n", PD::Input, SignalType::Continuous));
        ports.add(Port::new("i", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Diode".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            saturation_current: 1.0e-14,
            emission_coefficient: 1.0,
            temperature: 300.0,
        }
    }

    fn v_t(&self) -> Scalar {
        1.380649e-23 * self.temperature / 1.602176634e-19
    }

    fn diode_current(&self, vd: Scalar) -> Scalar {
        let nvt = self.emission_coefficient * self.v_t();
        if nvt <= 0.0 {
            return 0.0;
        }
        let arg = vd / nvt;
        if arg > 100.0 {
            self.saturation_current * arg.exp()
        } else {
            self.saturation_current * (arg.exp() - 1.0)
        }
    }
}

impl Block for DiodeBlock {
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
        let vp = self
            .ports
            .get("p")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let vn = self
            .ports
            .get("n")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let vd = vp - vn;
        let id = self.diode_current(vd);
        if let Some(port) = self.ports.get_mut("i") {
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
    fn test_resistor_block() {
        let r = ResistorBlock::new("r1", 1000.0);
        assert_eq!(r.id(), "r1");
        assert!((r.resistance - 1000.0).abs() < 1e-10);
    }

    #[test]
    fn test_capacitor_block() {
        let c = CapacitorBlock::new("c1", 1e-6);
        assert_eq!(c.id(), "c1");
        assert!((c.capacitance - 1e-6).abs() < 1e-12);
    }

    #[test]
    fn test_diode_stamp_current() {
        let ds = DiodeStamp {
            node_p: 1,
            node_n: 0,
            saturation_current: 1e-14,
            emission_coefficient: 1.0,
            temperature: 300.0,
        };
        // Forward bias: Vd = 0.6V
        let i_fwd = ds.current(0.6);
        assert!(i_fwd > 0.0);
        // Reverse bias: Vd = -5V
        let i_rev = ds.current(-5.0);
        assert!((i_rev + 1e-14).abs() < 1e-15);
    }

    #[test]
    fn test_diode_stamp_conductance() {
        let ds = DiodeStamp {
            node_p: 1,
            node_n: 0,
            saturation_current: 1e-14,
            emission_coefficient: 1.0,
            temperature: 300.0,
        };
        let g = ds.conductance(0.6);
        assert!(g > 0.0);
    }

    #[test]
    fn test_capacitor_companion() {
        let cs = CapacitorStamp {
            node_p: 1,
            node_n: 0,
            capacitance: 1e-6,
            prev_voltage: 1.0,
        };
        let geq = cs.companion_conductance(1e-6);
        assert!((geq - 1.0).abs() < 1e-6);
        let ieq = cs.companion_current(1e-6, 1.0);
        assert!((ieq + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_inductor_companion() {
        let ls = InductorStamp {
            node_p: 1,
            node_n: 0,
            inductance: 1e-3,
            prev_current: 0.5,
        };
        let geq = ls.companion_conductance(1e-6);
        assert!((geq - 1e-3).abs() < 1e-6);
    }

    #[test]
    fn test_diode_block() {
        let d = DiodeBlock::new("d1");
        assert_eq!(d.id(), "d1");
    }

    #[test]
    fn test_mosfet_stamp_creation() {
        let ms = MosfetStamp {
            node_d: 1,
            node_g: 2,
            node_s: 0,
            gm: 0.001,
            gds: 1e-5,
        };
        let mut mna = MnaMatrix::new(2, 0);
        ms.stamp(&mut mna);
        // Add gate resistor to ground to prevent singular matrix
        mna.stamp_resistor(2, 0, 1e6);
        let sol = mna.solve().unwrap();
        assert_eq!(sol.node_voltages.len(), 2);
    }

    #[test]
    fn test_opamp_stamp() {
        let op = OpAmpStamp {
            node_p: 1,
            node_n: 2,
            node_o: 3,
        };
        let mut mna = MnaMatrix::new(3, 1);
        op.stamp(&mut mna, 100000.0, 0);
        // Add feedback resistor: node_o to node_n
        mna.stamp_resistor(3, 2, 1000.0);
        // Input resistor: node_1 to ground
        mna.stamp_resistor(1, 0, 1000.0);
        let sol = mna.solve().unwrap();
        assert_eq!(sol.node_voltages.len(), 3);
    }
}
