//! Signal source blocks.
//!
//! Provides Block implementations for common signal generators:
//! constant, sine, square, step, pulse, and noise sources.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, PortDirection as PD, Scalar, SignalType, SignalValue, Time,
};
use std::f64::consts::TAU;

// ──────────────────────────────────────────────
// ConstantSource
// ──────────────────────────────────────────────

/// Emits a constant signal value on every step.
#[derive(Debug, Clone)]
pub struct ConstantSource {
    id: BlockId,
    block_type: String,
    value: SignalValue,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl ConstantSource {
    /// Create a new constant source.
    pub fn new(id: &str, value: SignalValue) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("out", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "ConstantSource".to_string(),
            value,
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }

    /// Create a scalar constant source.
    pub fn scalar(id: &str, value: Scalar) -> Self {
        Self::new(id, SignalValue::Scalar(value))
    }
}

impl Block for ConstantSource {
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
        if let Some(port) = self.ports.get_mut("out") {
            port.write(Signal::new(
                SignalType::Continuous,
                self.value.clone(),
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

// ──────────────────────────────────────────────
// SineSource
// ──────────────────────────────────────────────

/// Sinusoidal signal source: `y(t) = offset + amplitude * sin(2π·f·t + phase)`.
#[derive(Debug, Clone)]
pub struct SineSource {
    id: BlockId,
    block_type: String,
    pub amplitude: Scalar,
    pub frequency: Scalar,
    pub phase: Scalar,
    pub offset: Scalar,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl SineSource {
    pub fn new(
        id: &str,
        amplitude: Scalar,
        frequency: Scalar,
        phase: Scalar,
        offset: Scalar,
    ) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("out", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "SineSource".to_string(),
            amplitude,
            frequency,
            phase,
            offset,
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }

    /// Compute the output value at time t.
    pub fn compute(&self, t: Time) -> Scalar {
        self.offset + self.amplitude * (TAU * self.frequency * t + self.phase).sin()
    }
}

impl Block for SineSource {
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
        let val = self.compute(self.current_time);
        if let Some(port) = self.ports.get_mut("out") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(val),
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

// ──────────────────────────────────────────────
// SquareSource
// ──────────────────────────────────────────────

/// Square wave source with configurable duty cycle.
#[derive(Debug, Clone)]
pub struct SquareSource {
    id: BlockId,
    block_type: String,
    pub amplitude: Scalar,
    pub frequency: Scalar,
    pub duty_cycle: Scalar,
    pub offset: Scalar,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl SquareSource {
    pub fn new(
        id: &str,
        amplitude: Scalar,
        frequency: Scalar,
        duty_cycle: Scalar,
        offset: Scalar,
    ) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("out", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "SquareSource".to_string(),
            amplitude,
            frequency,
            duty_cycle: duty_cycle.clamp(0.0, 1.0),
            offset,
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }

    pub fn compute(&self, t: Time) -> Scalar {
        if self.frequency <= 0.0 {
            return self.offset;
        }
        let period = 1.0 / self.frequency;
        let phase_in_period = (t % period) / period;
        if phase_in_period < self.duty_cycle {
            self.offset + self.amplitude
        } else {
            self.offset - self.amplitude
        }
    }
}

impl Block for SquareSource {
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
        let val = self.compute(self.current_time);
        if let Some(port) = self.ports.get_mut("out") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(val),
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

// ──────────────────────────────────────────────
// StepSource
// ──────────────────────────────────────────────

/// Step signal: transitions from `initial` to `final_val` at `step_time`.
#[derive(Debug, Clone)]
pub struct StepSource {
    id: BlockId,
    block_type: String,
    pub initial: Scalar,
    pub final_val: Scalar,
    pub step_time: Time,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl StepSource {
    pub fn new(id: &str, initial: Scalar, final_val: Scalar, step_time: Time) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("out", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "StepSource".to_string(),
            initial,
            final_val,
            step_time,
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for StepSource {
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
        let val = if self.current_time >= self.step_time {
            self.final_val
        } else {
            self.initial
        };
        if let Some(port) = self.ports.get_mut("out") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(val),
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
        let diff = self.final_val - self.initial;
        if diff.abs() < 1e-15 {
            return Vec::new();
        }
        // Zero-crossing at step_time
        vec![self.current_time - self.step_time]
    }
    fn terminate(&mut self) -> Result<(), SimError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }
    fn clone_block(&self) -> Box<dyn Block> {
        Box::new(self.clone())
    }
}

// ──────────────────────────────────────────────
// PulseSource
// ──────────────────────────────────────────────

/// Single or periodic pulse generator.
#[derive(Debug, Clone)]
pub struct PulseSource {
    id: BlockId,
    block_type: String,
    pub amplitude: Scalar,
    pub width: Time,
    pub period: Option<Time>,
    pub delay: Time,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl PulseSource {
    pub fn new(
        id: &str,
        amplitude: Scalar,
        width: Time,
        period: Option<Time>,
        delay: Time,
    ) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("out", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "PulseSource".to_string(),
            amplitude,
            width,
            period,
            delay,
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }

    pub fn compute(&self, t: Time) -> Scalar {
        if t < self.delay {
            return 0.0;
        }
        let elapsed = t - self.delay;
        let in_first_pulse = elapsed < self.width;
        match self.period {
            None => {
                if in_first_pulse {
                    self.amplitude
                } else {
                    0.0
                }
            }
            Some(p) if p > 0.0 => {
                let cycle_pos = elapsed % p;
                if cycle_pos < self.width {
                    self.amplitude
                } else {
                    0.0
                }
            }
            Some(_) => 0.0,
        }
    }
}

impl Block for PulseSource {
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
        let val = self.compute(self.current_time);
        if let Some(port) = self.ports.get_mut("out") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(val),
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
        // Crossing at rising and falling edges
        if self.amplitude.abs() < 1e-15 {
            return Vec::new();
        }
        vec![
            self.current_time - self.delay,
            self.current_time - (self.delay + self.width),
        ]
    }
    fn terminate(&mut self) -> Result<(), SimError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }
    fn clone_block(&self) -> Box<dyn Block> {
        Box::new(self.clone())
    }
}

// ──────────────────────────────────────────────
// NoiseSource
// ──────────────────────────────────────────────

/// Type of noise distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoiseType {
    Gaussian,
    Uniform,
    PseudoRandom,
}

/// Random signal generator.
#[derive(Debug, Clone)]
pub struct NoiseSource {
    id: BlockId,
    block_type: String,
    pub mean: Scalar,
    pub std_dev: Scalar,
    pub noise_type: NoiseType,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    index: u64,
    // Simple LCG for deterministic pseudo-random sequences
    state: u64,
}

impl NoiseSource {
    pub fn new(
        id: &str,
        mean: Scalar,
        std_dev: Scalar,
        noise_type: NoiseType,
        seed: Option<u64>,
    ) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("out", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "NoiseSource".to_string(),
            mean,
            std_dev,
            noise_type,
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            index: 0,
            state: seed.unwrap_or(12345),
        }
    }

    fn next_uniform(&mut self) -> Scalar {
        // Linear congruential generator
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 11) as Scalar * (1.0 / (1u64 << 53) as Scalar)
    }

    fn next_gaussian(&mut self) -> Scalar {
        // Box-Muller transform
        let u1 = self.next_uniform().max(1e-15);
        let u2 = self.next_uniform();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }

    fn compute_next(&mut self) -> Scalar {
        self.index += 1;
        match self.noise_type {
            NoiseType::Uniform => self.mean + self.std_dev * (2.0 * self.next_uniform() - 1.0),
            NoiseType::Gaussian => self.mean + self.std_dev * self.next_gaussian(),
            NoiseType::PseudoRandom => {
                // Deterministic sequence based on index
                let phase = (self.index as Scalar * 1.618033988749895).fract();
                self.mean + self.std_dev * (4.0 * phase - 2.0)
            }
        }
    }
}

impl Block for NoiseSource {
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
        let val = self.compute_next();
        if let Some(port) = self.ports.get_mut("out") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(val),
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

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::EPSILON;

    #[test]
    fn test_constant_source() {
        let mut src = ConstantSource::scalar("c1", 42.0);
        src.init().unwrap();
        src.output().unwrap();
        let sig = src.ports().get("out").unwrap().read().unwrap();
        assert_eq!(sig.as_scalar(), Some(42.0));
    }

    #[test]
    fn test_sine_source() {
        let mut src = SineSource::new("s1", 1.0, 1.0, 0.0, 0.0);
        src.init().unwrap();
        src.set_time(0.0);
        src.output().unwrap();
        let v0 = src
            .ports()
            .get("out")
            .unwrap()
            .read()
            .unwrap()
            .as_scalar()
            .unwrap();
        assert!((v0 - 0.0).abs() < EPSILON);

        src.set_time(0.25);
        src.output().unwrap();
        let v1 = src
            .ports()
            .get("out")
            .unwrap()
            .read()
            .unwrap()
            .as_scalar()
            .unwrap();
        assert!((v1 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_square_source() {
        let mut src = SquareSource::new("sq1", 1.0, 1.0, 0.5, 0.0);
        src.init().unwrap();
        src.set_time(0.0);
        src.output().unwrap();
        let v = src
            .ports()
            .get("out")
            .unwrap()
            .read()
            .unwrap()
            .as_scalar()
            .unwrap();
        assert!((v - 1.0).abs() < EPSILON);

        src.set_time(0.6);
        src.output().unwrap();
        let v2 = src
            .ports()
            .get("out")
            .unwrap()
            .read()
            .unwrap()
            .as_scalar()
            .unwrap();
        assert!((v2 - (-1.0)).abs() < EPSILON);
    }

    #[test]
    fn test_step_source() {
        let mut src = StepSource::new("st1", 0.0, 1.0, 0.5);
        src.init().unwrap();
        src.set_time(0.0);
        src.output().unwrap();
        assert!(
            (src.ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap()
                - 0.0)
                .abs()
                < EPSILON
        );

        src.set_time(1.0);
        src.output().unwrap();
        assert!(
            (src.ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap()
                - 1.0)
                .abs()
                < EPSILON
        );
    }

    #[test]
    fn test_pulse_source_single() {
        let mut src = PulseSource::new("p1", 1.0, 0.1, None, 0.0);
        src.init().unwrap();
        src.set_time(0.05);
        src.output().unwrap();
        assert!(
            (src.ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap()
                - 1.0)
                .abs()
                < EPSILON
        );

        src.set_time(0.2);
        src.output().unwrap();
        assert!(
            (src.ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap()
                - 0.0)
                .abs()
                < EPSILON
        );
    }

    #[test]
    fn test_pulse_source_periodic() {
        let mut src = PulseSource::new("p2", 1.0, 0.05, Some(0.2), 0.0);
        src.init().unwrap();
        // At t=0.25, should be in second pulse (0.2..0.25)
        src.set_time(0.25);
        src.output().unwrap();
        assert!(
            (src.ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap()
                - 1.0)
                .abs()
                < EPSILON
        );

        // At t=0.3, between pulses
        src.set_time(0.3);
        src.output().unwrap();
        assert!(
            (src.ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap()
                - 0.0)
                .abs()
                < EPSILON
        );
    }

    #[test]
    fn test_noise_source_uniform() {
        let mut src = NoiseSource::new("n1", 0.0, 1.0, NoiseType::Uniform, Some(42));
        src.init().unwrap();
        let mut sum = 0.0;
        for _ in 0..100 {
            src.output().unwrap();
            let v = src
                .ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap();
            sum += v;
        }
        let mean = sum / 100.0;
        // Uniform distribution with std_dev=1 should have mean near 0
        assert!(mean.abs() < 1.0, "uniform noise mean = {}", mean);
    }

    #[test]
    fn test_noise_source_gaussian() {
        let mut src = NoiseSource::new("n2", 0.0, 1.0, NoiseType::Gaussian, Some(42));
        src.init().unwrap();
        let mut sum = 0.0;
        for _ in 0..500 {
            src.output().unwrap();
            let v = src
                .ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap();
            sum += v;
        }
        let mean = sum / 500.0;
        assert!(mean.abs() < 0.3, "gaussian noise mean = {}", mean);
    }

    #[test]
    fn test_noise_source_deterministic() {
        let mut src1 = NoiseSource::new("d1", 0.0, 1.0, NoiseType::PseudoRandom, Some(123));
        let mut src2 = NoiseSource::new("d2", 0.0, 1.0, NoiseType::PseudoRandom, Some(123));
        src1.init().unwrap();
        src2.init().unwrap();
        for _ in 0..10 {
            src1.output().unwrap();
            src2.output().unwrap();
            let v1 = src1
                .ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap();
            let v2 = src2
                .ports()
                .get("out")
                .unwrap()
                .read()
                .unwrap()
                .as_scalar()
                .unwrap();
            assert!((v1 - v2).abs() < EPSILON, "deterministic noise mismatch");
        }
    }

    #[test]
    fn test_constant_source_clone() {
        let src = ConstantSource::scalar("c1", 99.0);
        let cloned = src.clone_block();
        assert_eq!(cloned.id(), "c1");
    }
}
