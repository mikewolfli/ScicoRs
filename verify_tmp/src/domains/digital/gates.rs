//! Extended logic gate blocks.
//!
//! Provides additional gate types beyond the basic gates in
//! `blocks/logic.rs`: NAND, NOR, XNOR, buffer, tri-state buffer,
//! and a standalone NOT gate block.

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

// ──────────────────────────────────────────────
// LogicNand
// ──────────────────────────────────────────────

/// NAND gate: `y = !(u1 && u2)`.
#[derive(Debug, Clone)]
pub struct LogicNand {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl LogicNand {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Discrete));
        ports.add(Port::new("u2", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "LogicNand".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for LogicNand {
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
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        let u1 = self.ports.get("u1").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let u2 = self.ports.get("u2").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let y = from_bool(!(to_bool(u1) && to_bool(u2)));
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(y), self.current_time));
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
// LogicNor
// ──────────────────────────────────────────────

/// NOR gate: `y = !(u1 || u2)`.
#[derive(Debug, Clone)]
pub struct LogicNor {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl LogicNor {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Discrete));
        ports.add(Port::new("u2", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "LogicNor".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for LogicNor {
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
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        let u1 = self.ports.get("u1").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let u2 = self.ports.get("u2").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let y = from_bool(!(to_bool(u1) || to_bool(u2)));
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(y), self.current_time));
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
// LogicXnor
// ──────────────────────────────────────────────

/// XNOR gate: `y = !(u1 ^ u2)`.
#[derive(Debug, Clone)]
pub struct LogicXnor {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl LogicXnor {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Discrete));
        ports.add(Port::new("u2", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "LogicXnor".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for LogicXnor {
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
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        let u1 = self.ports.get("u1").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let u2 = self.ports.get("u2").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let y = from_bool(to_bool(u1) == to_bool(u2));
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(y), self.current_time));
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
// LogicBuffer
// ──────────────────────────────────────────────

/// Buffer: `y = u` (with configurable drive strength).
#[derive(Debug, Clone)]
pub struct LogicBuffer {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl LogicBuffer {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "LogicBuffer".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for LogicBuffer {
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
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        let u = self.ports.get("u").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(u), self.current_time));
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
// TriStateBuffer
// ──────────────────────────────────────────────

/// Tri-state buffer: `y = en ? u : Z` (high-impedance ≈ 0.5 output).
///
/// In high-impedance state, the output is set to 0.5 (undriven midpoint).
#[derive(Debug, Clone)]
pub struct TriStateBuffer {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl TriStateBuffer {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Discrete));
        ports.add(Port::new("en", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "TriStateBuffer".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for TriStateBuffer {
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
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        let u = self.ports.get("u").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let en = self.ports.get("en").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let y = if to_bool(en) { u } else { 0.5 }; // High-Z → midpoint
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(y), self.current_time));
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
// LogicNotBlock (standalone NOT gate)
// ──────────────────────────────────────────────

/// NOT gate: `y = !u`.
#[derive(Debug, Clone)]
pub struct LogicNotBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl LogicNotBlock {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "LogicNot".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for LogicNotBlock {
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
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        let u = self.ports.get("u").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let y = from_bool(!to_bool(u));
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(y), self.current_time));
        }
        Ok(())
    }
    fn derivative(&self) -> Result<Vec<Scalar>, SimError> { Ok(Vec::new()) }
    fn update(&mut self) -> Result<(), SimError> { Ok(()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), SimError> { self.status = ComponentStatus::Completed; Ok(()) }
    fn clone_block(&self) -> Box<dyn Block> { Box::new(self.clone()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nand_truth_table() {
        let mut gate = LogicNand::new("nand1");
        gate.init().unwrap();
        // Test all 4 combinations by writing directly to port
        let test_cases = [(0.0, 0.0, 1.0), (0.0, 1.0, 1.0), (1.0, 0.0, 1.0), (1.0, 1.0, 0.0)];
        for (u1, u2, expected) in &test_cases {
            if let Some(port) = gate.ports_mut().get_mut("u1") {
                port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(*u1), 0.0));
            }
            if let Some(port) = gate.ports_mut().get_mut("u2") {
                port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(*u2), 0.0));
            }
            gate.output().unwrap();
            let result = gate.ports().get("y").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
            assert!((result - expected).abs() < 0.01, "NAND({},{}) = {}, expected {}", u1, u2, result, expected);
        }
    }

    #[test]
    fn test_nor_truth_table() {
        let mut gate = LogicNor::new("nor1");
        gate.init().unwrap();
        let test_cases = [(0.0, 0.0, 1.0), (0.0, 1.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0)];
        for (u1, u2, expected) in &test_cases {
            if let Some(port) = gate.ports_mut().get_mut("u1") {
                port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(*u1), 0.0));
            }
            if let Some(port) = gate.ports_mut().get_mut("u2") {
                port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(*u2), 0.0));
            }
            gate.output().unwrap();
            let result = gate.ports().get("y").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
            assert!((result - expected).abs() < 0.01);
        }
    }

    #[test]
    fn test_xnor_truth_table() {
        let mut gate = LogicXnor::new("xnor1");
        gate.init().unwrap();
        let test_cases = [(0.0, 0.0, 1.0), (0.0, 1.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 1.0)];
        for (u1, u2, expected) in &test_cases {
            if let Some(port) = gate.ports_mut().get_mut("u1") {
                port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(*u1), 0.0));
            }
            if let Some(port) = gate.ports_mut().get_mut("u2") {
                port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(*u2), 0.0));
            }
            gate.output().unwrap();
            let result = gate.ports().get("y").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
            assert!((result - expected).abs() < 0.01);
        }
    }

    #[test]
    fn test_buffer_passes_input() {
        let mut buf = LogicBuffer::new("buf1");
        buf.init().unwrap();
        if let Some(port) = buf.ports_mut().get_mut("u") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.7), 0.0));
        }
        buf.output().unwrap();
        let y = buf.ports().get("y").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((y - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_tristate_enabled() {
        let mut buf = TriStateBuffer::new("tris1");
        buf.init().unwrap();
        if let Some(port) = buf.ports_mut().get_mut("u") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        if let Some(port) = buf.ports_mut().get_mut("en") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        buf.output().unwrap();
        let y = buf.ports().get("y").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((y - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_tristate_disabled() {
        let mut buf = TriStateBuffer::new("tris1");
        buf.init().unwrap();
        if let Some(port) = buf.ports_mut().get_mut("u") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        if let Some(port) = buf.ports_mut().get_mut("en") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        buf.output().unwrap();
        let y = buf.ports().get("y").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((y - 0.5).abs() < 0.01); // High-Z → midpoint
    }

    #[test]
    fn test_not_gate() {
        let mut gate = LogicNotBlock::new("not1");
        gate.init().unwrap();
        if let Some(port) = gate.ports_mut().get_mut("u") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        gate.output().unwrap();
        let y = gate.ports().get("y").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((y - 0.0).abs() < 0.01);

        if let Some(port) = gate.ports_mut().get_mut("u") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        gate.output().unwrap();
        let y = gate.ports().get("y").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((y - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_nand_creation() {
        let gate = LogicNand::new("nand1");
        assert_eq!(gate.id(), "nand1");
        assert_eq!(gate.block_type(), "LogicNand");
        assert!(gate.ports().get("u1").is_some());
        assert!(gate.ports().get("u2").is_some());
        assert!(gate.ports().get("y").is_some());
    }
}
