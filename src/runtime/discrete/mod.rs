//! Discrete-time simulation primitives.
//!
//! This module provides components for building discrete-time (sampled-data)
//! simulations within the kernel, including digital filters, integrators,
//! sample-and-hold blocks, counters, timers, PLC logic primitives, and
//! timing analysis utilities.

pub mod counter;
pub mod digital_filter;
pub mod digital_timing;
pub mod discrete_integrator;
pub mod plc_logic;
pub mod sample_hold;

pub use counter::{Counter, CounterDirection, Timer};
pub use digital_filter::{FIRFilter, IIRFilter, MovingAverage};
pub use digital_timing::{HazardType, TimingAnalysis};
pub use discrete_integrator::{DiscreteIntegrator, IntegrationMethod};
pub use plc_logic::{
    and_gate, nand_gate, nor_gate, not_gate, or_gate, xor_gate, DFlipFlop, EdgeDetector,
    RSFlipFlop,
};
pub use sample_hold::{linear_interpolate, resample, SampleHold};
