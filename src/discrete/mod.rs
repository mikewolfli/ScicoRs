//! Discrete and Multi-Rate Systems
//!
//! Provides support for multi-rate sampling, discrete-time blocks,
//! digital filters, clocks, counters, timers, and related primitives.

use crate::core::block::BlockId;
use crate::core::block::{Block, BlockError, SimpleBlock};
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortDirection, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{ComponentStatus, Scalar, SignalType, SignalValue, Time};

/// Describes a clock signal for multi-rate systems.
#[derive(Debug, Clone)]
pub struct ClockDef {
    /// Name of the clock domain.
    pub name: String,
    /// Sample period in seconds.
    pub period: Time,
    /// Phase offset in seconds.
    pub phase: Time,
    /// Whether this clock is active.
    pub active: bool,
}

impl ClockDef {
    pub fn new(name: &str, period: Time) -> Self {
        Self {
            name: name.to_string(),
            period,
            phase: 0.0,
            active: true,
        }
    }

    /// Check if this clock triggers at the given time.
    pub fn triggers_at(&self, t: Time) -> bool {
        if !self.active || self.period <= 0.0 {
            return false;
        }
        let adjusted = t - self.phase;
        if adjusted < 0.0 {
            return false;
        }
        let remainder = adjusted % self.period;
        remainder.abs() < 1e-12 || (self.period - remainder).abs() < 1e-12
    }

    /// Get the next trigger time after `t`.
    pub fn next_trigger(&self, t: Time) -> Time {
        if !self.active || self.period <= 0.0 {
            return Time::MAX;
        }
        let adjusted = t - self.phase;
        let periods = (adjusted / self.period).ceil();
        self.phase + periods * self.period
    }
}

/// A sample-and-hold block that captures an input signal at a clock edge.
#[derive(Debug)]
pub struct SampleAndHold {
    inner: SimpleBlock,
    clock: ClockDef,
    held_value: SignalValue,
}

impl SampleAndHold {
    pub fn new(id: &str, clock: ClockDef) -> Self {
        let mut inner = SimpleBlock::new(id, "SampleAndHold");
        inner.add_port(Port::new(
            "in",
            PortDirection::Input,
            SignalType::Continuous,
        ));
        inner.add_port(Port::new(
            "out",
            PortDirection::Output,
            SignalType::Discrete,
        ));
        Self {
            inner,
            clock,
            held_value: SignalValue::None,
        }
    }
}

impl Block for SampleAndHold {
    fn id(&self) -> &BlockId {
        self.inner.id()
    }
    fn block_type(&self) -> &str {
        self.inner.block_type()
    }
    fn ports(&self) -> &PortSet {
        self.inner.ports()
    }
    fn ports_mut(&mut self) -> &mut PortSet {
        self.inner.ports_mut()
    }
    fn params(&self) -> &ParameterSet {
        self.inner.params()
    }
    fn params_mut(&mut self) -> &mut ParameterSet {
        self.inner.params_mut()
    }
    fn status(&self) -> ComponentStatus {
        self.inner.status()
    }
    fn set_status(&mut self, s: ComponentStatus) {
        self.inner.set_status(s);
    }
    fn set_time(&mut self, t: Time) {
        self.inner.set_time(t);
    }
    fn time(&self) -> Time {
        self.inner.time()
    }

    fn init(&mut self) -> Result<(), BlockError> {
        self.held_value = SignalValue::None;
        self.inner.init()
    }

    fn output(&mut self) -> Result<(), BlockError> {
        if self.clock.triggers_at(self.time())
            && let Some(port) = self.inner.ports().get("in")
            && let Some(signal) = port.read()
        {
            self.held_value = signal.value.clone();
        }
        let t = self.time();
        if let Some(out) = self.inner.ports_mut().get_mut("out") {
            out.write(Signal::new(
                SignalType::Discrete,
                self.held_value.clone(),
                t,
            ));
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> {
        Ok(Vec::new())
    }
    fn update(&mut self) -> Result<(), BlockError> {
        Ok(())
    }
    fn zero_crossings(&self) -> Vec<Scalar> {
        Vec::new()
    }
    fn terminate(&mut self) -> Result<(), BlockError> {
        self.inner.terminate()
    }
}

/// A simple counter block.
#[derive(Debug)]
pub struct Counter {
    inner: SimpleBlock,
    count: i64,
    max_count: i64,
}

impl Counter {
    pub fn new(id: &str, max_count: i64) -> Self {
        let mut inner = SimpleBlock::new(id, "Counter");
        inner.add_port(Port::new(
            "trigger",
            PortDirection::Input,
            SignalType::Event,
        ));
        inner.add_port(Port::new(
            "out",
            PortDirection::Output,
            SignalType::Discrete,
        ));
        inner.add_port(Port::new("carry", PortDirection::Output, SignalType::Event));
        Self {
            inner,
            count: 0,
            max_count,
        }
    }
}

impl Block for Counter {
    fn id(&self) -> &BlockId {
        self.inner.id()
    }
    fn block_type(&self) -> &str {
        self.inner.block_type()
    }
    fn ports(&self) -> &PortSet {
        self.inner.ports()
    }
    fn ports_mut(&mut self) -> &mut PortSet {
        self.inner.ports_mut()
    }
    fn params(&self) -> &ParameterSet {
        self.inner.params()
    }
    fn params_mut(&mut self) -> &mut ParameterSet {
        self.inner.params_mut()
    }
    fn status(&self) -> ComponentStatus {
        self.inner.status()
    }
    fn set_status(&mut self, s: ComponentStatus) {
        self.inner.set_status(s);
    }
    fn set_time(&mut self, t: Time) {
        self.inner.set_time(t);
    }
    fn time(&self) -> Time {
        self.inner.time()
    }

    fn init(&mut self) -> Result<(), BlockError> {
        self.count = 0;
        self.inner.init()
    }

    fn output(&mut self) -> Result<(), BlockError> {
        // Output current count value
        let t = self.time();
        if let Some(out) = self.inner.ports_mut().get_mut("out") {
            out.write(Signal::new(
                SignalType::Discrete,
                SignalValue::Integer(self.count),
                t,
            ));
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), BlockError> {
        // Check for trigger
        if let Some(port) = self.inner.ports().get("trigger")
            && port.read().is_some()
        {
            self.count += 1;
            if self.max_count > 0 && self.count >= self.max_count {
                let t = self.time();
                if let Some(carry) = self.inner.ports_mut().get_mut("carry") {
                    carry.write(Signal::new(
                        SignalType::Event,
                        SignalValue::Integer(self.count),
                        t,
                    ));
                }
                self.count = 0;
            }
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> {
        Ok(Vec::new())
    }
    fn zero_crossings(&self) -> Vec<Scalar> {
        Vec::new()
    }
    fn terminate(&mut self) -> Result<(), BlockError> {
        self.inner.terminate()
    }
}
