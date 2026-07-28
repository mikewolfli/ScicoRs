//! Sequential logic elements.
//!
//! Provides Block implementations for D flip-flop, JK flip-flop,
//! T flip-flop, level-sensitive latch, and shift register.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, PortDirection as PD, Scalar, SignalType, SignalValue, Time,
};

// ──────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────

fn to_bool(v: Scalar) -> bool {
    v >= 0.5
}

fn from_bool(b: bool) -> Scalar {
    if b { 1.0 } else { 0.0 }
}

fn detect_rising_edge(prev: bool, curr: bool) -> bool {
    !prev && curr
}

// ──────────────────────────────────────────────
// DFlipFlopBlock
// ──────────────────────────────────────────────

/// Rising-edge triggered D flip-flop.
///
/// Ports:
/// - `d` (input): Data input
/// - `clk` (input): Clock
/// - `rst` (input): Synchronous reset
/// - `q` (output): Stored value
/// - `qn` (output): Inverted stored value
#[derive(Debug, Clone)]
pub struct DFlipFlopBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    q: bool,
    prev_clk: bool,
    pub initial_q: bool,
}

impl DFlipFlopBlock {
    pub fn new(id: &str, initial_q: bool) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("d", PD::Input, SignalType::Discrete));
        ports.add(Port::new("clk", PD::Input, SignalType::Discrete));
        ports.add(Port::new("rst", PD::Input, SignalType::Discrete));
        ports.add(Port::new("q", PD::Output, SignalType::Discrete));
        ports.add(Port::new("qn", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "DFlipFlop".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            q: initial_q,
            prev_clk: false,
            initial_q,
        }
    }

    pub fn reset(&mut self) {
        self.q = self.initial_q;
        self.prev_clk = false;
    }
}

impl Block for DFlipFlopBlock {
    fn id(&self) -> &BlockId { &self.id }
    fn block_type(&self) -> &str { &self.block_type }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), SimError> {
        self.q = self.initial_q;
        self.prev_clk = false;
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        if let Some(port) = self.ports.get_mut("q") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(self.q)), self.current_time));
        }
        if let Some(port) = self.ports.get_mut("qn") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(!self.q)), self.current_time));
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), SimError> {
        let clk = to_bool(self.ports.get("clk").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        let rst = to_bool(self.ports.get("rst").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        let d = to_bool(self.ports.get("d").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));

        if rst {
            self.q = false;
        } else if detect_rising_edge(self.prev_clk, clk) {
            self.q = d;
        }
        self.prev_clk = clk;
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, SimError> { Ok(Vec::new()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), SimError> { self.status = ComponentStatus::Completed; Ok(()) }
    fn clone_block(&self) -> Box<dyn Block> { Box::new(self.clone()) }
}

// ──────────────────────────────────────────────
// JKFlipFlopBlock
// ──────────────────────────────────────────────

/// Rising-edge triggered JK flip-flop.
///
/// Ports: `j`, `k`, `clk`, `rst` → `q`, `qn`
/// - J=0,K=0: hold; J=0,K=1: reset; J=1,K=0: set; J=1,K=1: toggle
#[derive(Debug, Clone)]
pub struct JKFlipFlopBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    q: bool,
    prev_clk: bool,
    pub initial_q: bool,
}

impl JKFlipFlopBlock {
    pub fn new(id: &str, initial_q: bool) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("j", PD::Input, SignalType::Discrete));
        ports.add(Port::new("k", PD::Input, SignalType::Discrete));
        ports.add(Port::new("clk", PD::Input, SignalType::Discrete));
        ports.add(Port::new("rst", PD::Input, SignalType::Discrete));
        ports.add(Port::new("q", PD::Output, SignalType::Discrete));
        ports.add(Port::new("qn", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "JKFlipFlop".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            q: initial_q,
            prev_clk: false,
            initial_q,
        }
    }
}

impl Block for JKFlipFlopBlock {
    fn id(&self) -> &BlockId { &self.id }
    fn block_type(&self) -> &str { &self.block_type }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), SimError> {
        self.q = self.initial_q;
        self.prev_clk = false;
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        if let Some(port) = self.ports.get_mut("q") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(self.q)), self.current_time));
        }
        if let Some(port) = self.ports.get_mut("qn") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(!self.q)), self.current_time));
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), SimError> {
        let clk = to_bool(self.ports.get("clk").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        let rst = to_bool(self.ports.get("rst").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        let j = to_bool(self.ports.get("j").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        let k = to_bool(self.ports.get("k").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));

        if rst {
            self.q = false;
        } else if detect_rising_edge(self.prev_clk, clk) {
            match (j, k) {
                (false, false) => {} // hold
                (false, true) => self.q = false,
                (true, false) => self.q = true,
                (true, true) => self.q = !self.q,
            }
        }
        self.prev_clk = clk;
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, SimError> { Ok(Vec::new()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), SimError> { self.status = ComponentStatus::Completed; Ok(()) }
    fn clone_block(&self) -> Box<dyn Block> { Box::new(self.clone()) }
}

// ──────────────────────────────────────────────
// TFlipFlopBlock
// ──────────────────────────────────────────────

/// Toggle flip-flop: `q_next = t ? !q : q` on rising clock edge.
#[derive(Debug, Clone)]
pub struct TFlipFlopBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    q: bool,
    prev_clk: bool,
    pub initial_q: bool,
}

impl TFlipFlopBlock {
    pub fn new(id: &str, initial_q: bool) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("t", PD::Input, SignalType::Discrete));
        ports.add(Port::new("clk", PD::Input, SignalType::Discrete));
        ports.add(Port::new("rst", PD::Input, SignalType::Discrete));
        ports.add(Port::new("q", PD::Output, SignalType::Discrete));
        ports.add(Port::new("qn", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "TFlipFlop".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            q: initial_q,
            prev_clk: false,
            initial_q,
        }
    }
}

impl Block for TFlipFlopBlock {
    fn id(&self) -> &BlockId { &self.id }
    fn block_type(&self) -> &str { &self.block_type }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), SimError> {
        self.q = self.initial_q;
        self.prev_clk = false;
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        if let Some(port) = self.ports.get_mut("q") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(self.q)), self.current_time));
        }
        if let Some(port) = self.ports.get_mut("qn") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(!self.q)), self.current_time));
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), SimError> {
        let clk = to_bool(self.ports.get("clk").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        let rst = to_bool(self.ports.get("rst").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        let t = to_bool(self.ports.get("t").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));

        if rst {
            self.q = false;
        } else if detect_rising_edge(self.prev_clk, clk) && t {
            self.q = !self.q;
        }
        self.prev_clk = clk;
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, SimError> { Ok(Vec::new()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), SimError> { self.status = ComponentStatus::Completed; Ok(()) }
    fn clone_block(&self) -> Box<dyn Block> { Box::new(self.clone()) }
}

// ──────────────────────────────────────────────
// LatchBlock
// ──────────────────────────────────────────────

/// Level-sensitive transparent latch.
///
/// When `en` is high, `q` follows `d`. When `en` goes low, `q` holds.
#[derive(Debug, Clone)]
pub struct LatchBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    q: bool,
    pub initial_q: bool,
}

impl LatchBlock {
    pub fn new(id: &str, initial_q: bool) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("d", PD::Input, SignalType::Discrete));
        ports.add(Port::new("en", PD::Input, SignalType::Discrete));
        ports.add(Port::new("q", PD::Output, SignalType::Discrete));
        ports.add(Port::new("qn", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "Latch".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            q: initial_q,
            initial_q,
        }
    }
}

impl Block for LatchBlock {
    fn id(&self) -> &BlockId { &self.id }
    fn block_type(&self) -> &str { &self.block_type }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), SimError> {
        self.q = self.initial_q;
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        let en = to_bool(self.ports.get("en").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        let d = to_bool(self.ports.get("d").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));

        if en {
            self.q = d;
        }

        if let Some(port) = self.ports.get_mut("q") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(self.q)), self.current_time));
        }
        if let Some(port) = self.ports.get_mut("qn") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(!self.q)), self.current_time));
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, SimError> { Ok(Vec::new()) }
    fn update(&mut self) -> Result<(), SimError> { Ok(()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), SimError> { self.status = ComponentStatus::Completed; Ok(()) }
    fn clone_block(&self) -> Box<dyn Block> { Box::new(self.clone()) }
}

// ──────────────────────────────────────────────
// ShiftRegisterBlock
// ──────────────────────────────────────────────

/// Parallel-out shift register.
///
/// On each rising clock edge, shifts data in from `din` and shifts
/// the internal register, outputting all bits via `dout`.
#[derive(Debug, Clone)]
pub struct ShiftRegisterBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub width: usize,
    reg: Vec<bool>,
    prev_clk: bool,
}

impl ShiftRegisterBlock {
    pub fn new(id: &str, width: usize, initial: Option<Vec<bool>>) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("din", PD::Input, SignalType::Discrete));
        ports.add(Port::new("clk", PD::Input, SignalType::Discrete));
        ports.add(Port::new("dout", PD::Output, SignalType::Discrete));
        let reg = initial.unwrap_or_else(|| vec![false; width]);
        Self {
            id: id.to_string(),
            block_type: "ShiftRegister".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            width,
            reg,
            prev_clk: false,
        }
    }
}

impl Block for ShiftRegisterBlock {
    fn id(&self) -> &BlockId { &self.id }
    fn block_type(&self) -> &str { &self.block_type }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), SimError> {
        for b in &mut self.reg {
            *b = false;
        }
        self.prev_clk = false;
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        // Output the last bit as serial dout
        let last_bit = self.reg.last().copied().unwrap_or(false);
        if let Some(port) = self.ports.get_mut("dout") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(last_bit)), self.current_time));
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), SimError> {
        let clk = to_bool(self.ports.get("clk").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        let din = to_bool(self.ports.get("din").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));

        if detect_rising_edge(self.prev_clk, clk) && !self.reg.is_empty() {
            // Shift right
            for i in (1..self.reg.len()).rev() {
                self.reg[i] = self.reg[i - 1];
            }
            self.reg[0] = din;
        }
        self.prev_clk = clk;
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, SimError> { Ok(Vec::new()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), SimError> { self.status = ComponentStatus::Completed; Ok(()) }
    fn clone_block(&self) -> Box<dyn Block> { Box::new(self.clone()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dff_initial_state() {
        let mut dff = DFlipFlopBlock::new("dff1", true);
        dff.init().unwrap();
        dff.output().unwrap();
        let q = dff.ports().get("q").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((q - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_dff_rising_edge() {
        let mut dff = DFlipFlopBlock::new("dff1", false);
        dff.init().unwrap();

        // Set D=1, clk=0
        if let Some(port) = dff.ports_mut().get_mut("d") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        if let Some(port) = dff.ports_mut().get_mut("clk") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        dff.update().unwrap();
        dff.output().unwrap();
        let q = dff.ports().get("q").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((q - 0.0).abs() < 0.01, "Before clock edge, q should be 0");

        // Rising edge: clk=0→1
        if let Some(port) = dff.ports_mut().get_mut("clk") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        dff.update().unwrap();
        dff.output().unwrap();
        let q = dff.ports().get("q").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((q - 1.0).abs() < 0.01, "After rising edge, q should be 1 (D=1)");
    }

    #[test]
    fn test_dff_reset() {
        let mut dff = DFlipFlopBlock::new("dff1", true);
        dff.init().unwrap();

        if let Some(port) = dff.ports_mut().get_mut("rst") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        if let Some(port) = dff.ports_mut().get_mut("clk") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        dff.update().unwrap();
        dff.output().unwrap();
        let q = dff.ports().get("q").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((q - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_jkff_toggle_mode() {
        let mut jkff = JKFlipFlopBlock::new("jk1", false);
        jkff.init().unwrap();

        // Set J=1, K=1 (toggle mode)
        if let Some(port) = jkff.ports_mut().get_mut("j") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        if let Some(port) = jkff.ports_mut().get_mut("k") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }

        // Clock rising edge 0→1
        if let Some(port) = jkff.ports_mut().get_mut("clk") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        jkff.update().unwrap();
        if let Some(port) = jkff.ports_mut().get_mut("clk") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        jkff.update().unwrap();
        jkff.output().unwrap();
        let q = jkff.ports().get("q").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((q - 1.0).abs() < 0.01, "JK toggle: q should be 1");

        // Second clock edge
        if let Some(port) = jkff.ports_mut().get_mut("clk") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        jkff.update().unwrap();
        if let Some(port) = jkff.ports_mut().get_mut("clk") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        jkff.update().unwrap();
        jkff.output().unwrap();
        let q = jkff.ports().get("q").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((q - 0.0).abs() < 0.01, "JK toggle: q should be 0");
    }

    #[test]
    fn test_tff_toggle() {
        let mut tff = TFlipFlopBlock::new("tff1", false);
        tff.init().unwrap();

        if let Some(port) = tff.ports_mut().get_mut("t") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }

        // Rising edge
        if let Some(port) = tff.ports_mut().get_mut("clk") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        tff.update().unwrap();
        if let Some(port) = tff.ports_mut().get_mut("clk") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        tff.update().unwrap();
        tff.output().unwrap();
        let q = tff.ports().get("q").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((q - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_latch_transparent() {
        let mut latch = LatchBlock::new("latch1", false);
        latch.init().unwrap();

        // Enable high, D=1 → Q should be 1
        if let Some(port) = latch.ports_mut().get_mut("en") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        if let Some(port) = latch.ports_mut().get_mut("d") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        latch.output().unwrap();
        let q = latch.ports().get("q").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((q - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_shift_register() {
        let mut sr = ShiftRegisterBlock::new("sr1", 4, None);
        sr.init().unwrap();

        // Clock in 1,0,1,0
        for &bit in &[1.0, 0.0, 1.0, 0.0] {
            if let Some(port) = sr.ports_mut().get_mut("din") {
                port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(bit), 0.0));
            }
            // Rising edge
            if let Some(port) = sr.ports_mut().get_mut("clk") {
                port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
            }
            sr.update().unwrap();
            if let Some(port) = sr.ports_mut().get_mut("clk") {
                port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
            }
            sr.update().unwrap();
        }
        sr.output().unwrap();
        let dout = sr.ports().get("dout").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        // After clocking in 1,0,1,0, the first bit (1) has shifted to position dout[3]
        assert!((dout - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_dff_ports() {
        let dff = DFlipFlopBlock::new("dff1", false);
        assert!(dff.ports().get("d").is_some());
        assert!(dff.ports().get("clk").is_some());
        assert!(dff.ports().get("rst").is_some());
        assert!(dff.ports().get("q").is_some());
        assert!(dff.ports().get("qn").is_some());
    }
}
