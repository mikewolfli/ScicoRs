//! Combinational logic blocks.
//!
//! Provides Block implementations for adder, multiplier, decoder,
//! and ALU (Arithmetic Logic Unit).

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
// AdderBlock
// ──────────────────────────────────────────────

/// Ripple-carry adder with configurable width.
///
/// Ports:
/// - `a` (input): Bus input A (as scalar, integer value)
/// - `b` (input): Bus input B (as scalar, integer value)
/// - `cin` (input): Carry-in
/// - `sum` (output): Sum (as scalar integer)
/// - `cout` (output): Carry-out
#[derive(Debug, Clone)]
pub struct AdderBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub width: usize,
}

impl AdderBlock {
    pub fn new(id: &str, width: usize) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("a", PD::Input, SignalType::Discrete));
        ports.add(Port::new("b", PD::Input, SignalType::Discrete));
        ports.add(Port::new("cin", PD::Input, SignalType::Discrete));
        ports.add(Port::new("sum", PD::Output, SignalType::Discrete));
        ports.add(Port::new("cout", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "Adder".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            width,
        }
    }
}

impl Block for AdderBlock {
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
        let a_val = self.ports.get("a").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        let b_val = self.ports.get("b").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        let cin = to_bool(self.ports.get("cin").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));

        let mask = (1u64 << self.width) - 1;
        let sum_val = a_val.wrapping_add(b_val).wrapping_add(if cin { 1 } else { 0 });
        let cout = (sum_val >> self.width) != 0;
        let sum_truncated = sum_val & mask;

        if let Some(port) = self.ports.get_mut("sum") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(sum_truncated as Scalar), self.current_time));
        }
        if let Some(port) = self.ports.get_mut("cout") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(cout)), self.current_time));
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
// MultiplierBlock
// ──────────────────────────────────────────────

/// Unsigned integer multiplier.
///
/// Ports: `a`, `b` (inputs), `product` (output, 2*width bits).
#[derive(Debug, Clone)]
pub struct MultiplierBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub width: usize,
}

impl MultiplierBlock {
    pub fn new(id: &str, width: usize) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("a", PD::Input, SignalType::Discrete));
        ports.add(Port::new("b", PD::Input, SignalType::Discrete));
        ports.add(Port::new("product", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "Multiplier".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            width,
        }
    }
}

impl Block for MultiplierBlock {
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
        let a_val = self.ports.get("a").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        let b_val = self.ports.get("b").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        let product = a_val.wrapping_mul(b_val);

        if let Some(port) = self.ports.get_mut("product") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(product as Scalar), self.current_time));
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
// DecoderBlock
// ──────────────────────────────────────────────

/// Binary decoder: selects one of 2^n outputs based on n-bit input.
///
/// Ports: `in` (input, scalar), `out` (output, scalar = selected line index).
#[derive(Debug, Clone)]
pub struct DecoderBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub input_width: usize,
}

impl DecoderBlock {
    pub fn new(id: &str, input_width: usize) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("in", PD::Input, SignalType::Discrete));
        ports.add(Port::new("out", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "Decoder".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            input_width,
        }
    }
}

impl Block for DecoderBlock {
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
        let in_val = self.ports.get("in").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        let max_idx = (1u64 << self.input_width).saturating_sub(1);
        let out_val = if in_val <= max_idx { 1u64 << in_val } else { 0 };

        if let Some(port) = self.ports.get_mut("out") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(out_val as Scalar), self.current_time));
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
// ALU Operations
// ──────────────────────────────────────────────

/// ALU operation codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ALUOp {
    Add = 0,
    Sub = 1,
    And = 2,
    Or = 3,
    Xor = 4,
    Not = 5,
    Shl = 6,
    Shr = 7,
}

impl ALUOp {
    pub fn from_u64(v: u64) -> Self {
        match v {
            0 => Self::Add,
            1 => Self::Sub,
            2 => Self::And,
            3 => Self::Or,
            4 => Self::Xor,
            5 => Self::Not,
            6 => Self::Shl,
            7 => Self::Shr,
            _ => Self::Add,
        }
    }
}

// ──────────────────────────────────────────────
// ALUBlock
// ──────────────────────────────────────────────

/// Arithmetic Logic Unit.
///
/// Ports:
/// - `a` (input): Operand A
/// - `b` (input): Operand B
/// - `opcode` (input): Operation code (0-7)
/// - `result` (output): Result
/// - `zero` (output): Zero flag
/// - `carry` (output): Carry/borrow flag
#[derive(Debug, Clone)]
pub struct ALUBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub width: usize,
}

impl ALUBlock {
    pub fn new(id: &str, width: usize) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("a", PD::Input, SignalType::Discrete));
        ports.add(Port::new("b", PD::Input, SignalType::Discrete));
        ports.add(Port::new("opcode", PD::Input, SignalType::Discrete));
        ports.add(Port::new("result", PD::Output, SignalType::Discrete));
        ports.add(Port::new("zero", PD::Output, SignalType::Discrete));
        ports.add(Port::new("carry", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "ALU".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            width,
        }
    }
}

impl Block for ALUBlock {
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
        if self.width == 0 {
            // Degenerate zero-bit ALU: output zero.
            if let Some(port) = self.ports.get_mut("result") {
                port.write(Signal::new(
                    SignalType::Discrete,
                    SignalValue::Scalar(0.0),
                    self.current_time,
                ));
            }
            return Ok(());
        }
        let a_val = self.ports.get("a").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        let b_val = self.ports.get("b").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        let op_val = self.ports.get("opcode").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        let op = ALUOp::from_u64(op_val);

        let mask = (1u64 << self.width) - 1;
        let (result, carry) = match op {
            ALUOp::Add => {
                let r = a_val.wrapping_add(b_val);
                let c = (r >> self.width) != 0;
                (r & mask, c)
            }
            ALUOp::Sub => {
                let r = a_val.wrapping_sub(b_val);
                let c = b_val > a_val;
                (r & mask, c)
            }
            ALUOp::And => (a_val & b_val, false),
            ALUOp::Or => (a_val | b_val, false),
            ALUOp::Xor => (a_val ^ b_val, false),
            ALUOp::Not => (!a_val & mask, false),
            ALUOp::Shl => {
                let shift = (b_val % self.width as u64) as u32;
                ((a_val << shift) & mask, false)
            }
            ALUOp::Shr => {
                let shift = (b_val % self.width as u64) as u32;
                (a_val >> shift, false)
            }
        };

        let zero = result == 0;

        if let Some(port) = self.ports.get_mut("result") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(result as Scalar), self.current_time));
        }
        if let Some(port) = self.ports.get_mut("zero") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(zero)), self.current_time));
        }
        if let Some(port) = self.ports.get_mut("carry") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(from_bool(carry)), self.current_time));
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
    fn test_adder_8bit() {
        let mut adder = AdderBlock::new("add1", 8);
        adder.init().unwrap();

        if let Some(port) = adder.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(5.0), 0.0));
        }
        if let Some(port) = adder.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(3.0), 0.0));
        }
        adder.output().unwrap();
        let sum = adder.ports().get("sum").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((sum - 8.0).abs() < 0.01);
    }

    #[test]
    fn test_adder_with_carry() {
        let mut adder = AdderBlock::new("add1", 4);
        adder.init().unwrap();

        if let Some(port) = adder.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(15.0), 0.0));
        }
        if let Some(port) = adder.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        adder.output().unwrap();
        let sum = adder.ports().get("sum").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        let cout = to_bool(adder.ports().get("cout").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        assert!((sum - 0.0).abs() < 0.01); // 16 mod 16 = 0
        assert!(cout); // Carry out
    }

    #[test]
    fn test_multiplier() {
        let mut mul = MultiplierBlock::new("mul1", 8);
        mul.init().unwrap();

        if let Some(port) = mul.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(6.0), 0.0));
        }
        if let Some(port) = mul.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(7.0), 0.0));
        }
        mul.output().unwrap();
        let prod = mul.ports().get("product").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((prod - 42.0).abs() < 0.01);
    }

    #[test]
    fn test_decoder_2to4() {
        let mut dec = DecoderBlock::new("dec1", 2);
        dec.init().unwrap();

        if let Some(port) = dec.ports_mut().get_mut("in") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        dec.output().unwrap();
        let out = dec.ports().get("out").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        assert_eq!(out, 2); // 1 << 1 = 2
    }

    #[test]
    fn test_alu_add() {
        let mut alu = ALUBlock::new("alu1", 8);
        alu.init().unwrap();

        if let Some(port) = alu.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(10.0), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(20.0), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("opcode") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        alu.output().unwrap();
        let result = alu.ports().get("result").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((result - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_alu_sub() {
        let mut alu = ALUBlock::new("alu1", 8);
        alu.init().unwrap();

        if let Some(port) = alu.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(20.0), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(5.0), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("opcode") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        alu.output().unwrap();
        let result = alu.ports().get("result").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((result - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_alu_and() {
        let mut alu = ALUBlock::new("alu1", 8);
        alu.init().unwrap();

        if let Some(port) = alu.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0xFFu64 as Scalar), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0x0Fu64 as Scalar), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("opcode") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(2.0), 0.0));
        }
        alu.output().unwrap();
        let result = alu.ports().get("result").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        assert_eq!(result, 0x0F);
    }

    #[test]
    fn test_alu_or() {
        let mut alu = ALUBlock::new("alu1", 8);
        alu.init().unwrap();

        if let Some(port) = alu.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0xF0u64 as Scalar), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0x0Fu64 as Scalar), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("opcode") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(3.0), 0.0));
        }
        alu.output().unwrap();
        let result = alu.ports().get("result").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        assert_eq!(result, 0xFF);
    }

    #[test]
    fn test_alu_xor() {
        let mut alu = ALUBlock::new("alu1", 8);
        alu.init().unwrap();

        if let Some(port) = alu.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0xFFu64 as Scalar), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0xFFu64 as Scalar), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("opcode") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(4.0), 0.0));
        }
        alu.output().unwrap();
        let result = alu.ports().get("result").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((result - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_alu_not() {
        let mut alu = ALUBlock::new("alu1", 8);
        alu.init().unwrap();

        if let Some(port) = alu.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0x00u64 as Scalar), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("opcode") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(5.0), 0.0));
        }
        alu.output().unwrap();
        let result = alu.ports().get("result").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0) as u64;
        assert_eq!(result, 0xFF);
    }

    #[test]
    fn test_alu_shift_left() {
        let mut alu = ALUBlock::new("alu1", 8);
        alu.init().unwrap();

        if let Some(port) = alu.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(1.0), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(2.0), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("opcode") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(6.0), 0.0));
        }
        alu.output().unwrap();
        let result = alu.ports().get("result").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0);
        assert!((result - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_alu_zero_flag() {
        let mut alu = ALUBlock::new("alu1", 8);
        alu.init().unwrap();

        if let Some(port) = alu.ports_mut().get_mut("a") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("b") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        if let Some(port) = alu.ports_mut().get_mut("opcode") {
            port.write(Signal::new(SignalType::Discrete, SignalValue::Scalar(0.0), 0.0));
        }
        alu.output().unwrap();
        let zero = to_bool(alu.ports().get("zero").and_then(|p| p.read()).and_then(|s| s.as_scalar()).unwrap_or(0.0));
        assert!(zero);
    }

    #[test]
    fn test_aluop_from_u64() {
        assert_eq!(ALUOp::from_u64(0), ALUOp::Add);
        assert_eq!(ALUOp::from_u64(1), ALUOp::Sub);
        assert_eq!(ALUOp::from_u64(7), ALUOp::Shr);
        assert_eq!(ALUOp::from_u64(99), ALUOp::Add); // default
    }
}
