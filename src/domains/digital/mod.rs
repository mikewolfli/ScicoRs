//! Digital Logic & RTL Simulation (Phase 15).
//!
//! Provides extended logic gates, sequential elements, combinational logic
//! blocks, a simple CPU model, and timing analysis utilities.
//!
//! # Modules
//!
//! - **gates**: Extended logic gate blocks (NAND, NOR, XNOR, buffer, tri-state)
//! - **sequential**: D FF, JK FF, T FF, latch, shift register
//! - **combinational**: Adder, subtractor, multiplier, decoder, ALU
//! - **cpu**: Simple CPU model with register file, pipeline, RISC instruction set
//! - **timing**: Setup/hold timing analysis, clock jitter, propagation delay

pub mod gates;
pub mod sequential;
pub mod combinational;
pub mod cpu;
pub mod timing;

pub use gates::{
    LogicBuffer, LogicNand, LogicNor, LogicNotBlock, LogicXnor, TriStateBuffer,
};
pub use sequential::{
    DFlipFlopBlock, JKFlipFlopBlock, LatchBlock, ShiftRegisterBlock, TFlipFlopBlock,
};
pub use combinational::{ALUBlock, ALUOp, AdderBlock, DecoderBlock, MultiplierBlock};
pub use cpu::{PipelineStages, SimpleCpu, CpuInstruction, CpuProgram};
pub use timing::{GateConnection, TimingAnalyzer};
