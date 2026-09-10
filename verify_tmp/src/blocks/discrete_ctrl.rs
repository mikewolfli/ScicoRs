//! Discrete-time control system blocks.
//!
//! Provides Block implementations for unit delay, discrete integrator,
//! discrete filter, and discrete PID controller.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, PortDirection as PD, Scalar, SignalType, SignalValue, Time,
};
use crate::runtime::discrete::{FIRFilter, IIRFilter};

// ──────────────────────────────────────────────
// UnitDelay
// ──────────────────────────────────────────────

/// Unit delay: `y[n] = u[n-1]`.
#[derive(Debug, Clone)]
pub struct UnitDelay {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    prev: Scalar,
    initialized: bool,
}

impl UnitDelay {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "UnitDelay".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            prev: 0.0,
            initialized: false,
        }
    }
}

impl Block for UnitDelay {
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
        self.prev = 0.0;
        self.initialized = false;
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
        let y = if self.initialized { self.prev } else { 0.0 };
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(
                SignalType::Discrete,
                SignalValue::Scalar(y),
                self.current_time,
            ));
        }
        Ok(())
    }
    fn derivative(&self) -> Result<Vec<Scalar>, SimError> {
        Ok(Vec::new())
    }
    fn update(&mut self) -> Result<(), SimError> {
        self.prev = self
            .ports
            .get("u")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        self.initialized = true;
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
// DiscreteIntegratorBlock
// ──────────────────────────────────────────────

/// Discrete integrator: `y[n] = y[n-1] + dt * u[n]` (forward Euler).
#[derive(Debug, Clone)]
pub struct DiscreteIntegratorBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub dt: Scalar,
    pub initial: Scalar,
    pub min: Scalar,
    pub max: Scalar,
    state: Scalar,
}

impl DiscreteIntegratorBlock {
    pub fn new(id: &str, dt: Scalar, initial: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "DiscreteIntegrator".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            dt,
            initial,
            min: -1e30,
            max: 1e30,
            state: initial,
        }
    }

    pub fn reset(&mut self) {
        self.state = self.initial;
    }
}

impl Block for DiscreteIntegratorBlock {
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
        self.state = self.initial;
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(
                SignalType::Discrete,
                SignalValue::Scalar(self.state),
                self.current_time,
            ));
        }
        Ok(())
    }
    fn derivative(&self) -> Result<Vec<Scalar>, SimError> {
        Ok(Vec::new())
    }
    fn update(&mut self) -> Result<(), SimError> {
        let u = self
            .ports
            .get("u")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        self.state = (self.state + self.dt * u).clamp(self.min, self.max);
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
// DiscreteFilter
// ──────────────────────────────────────────────

/// Wraps FIR or IIR filter as a Block.
#[derive(Debug, Clone)]
pub enum DiscreteFilterKind {
    Fir(FIRFilter),
    Iir(IIRFilter),
}

/// Discrete filter block.
#[derive(Debug, Clone)]
pub struct DiscreteFilter {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    kind: DiscreteFilterKind,
}

impl DiscreteFilter {
    pub fn new_fir(id: &str, coefficients: &[Scalar]) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "FIRFilter".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            kind: DiscreteFilterKind::Fir(FIRFilter::new(coefficients)),
        }
    }

    pub fn new_iir(id: &str, b: &[Scalar], a: &[Scalar]) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "IIRFilter".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            kind: DiscreteFilterKind::Iir(IIRFilter::new(b, a)),
        }
    }
}

impl Block for DiscreteFilter {
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
        let u = self
            .ports
            .get("u")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let y = match &mut self.kind {
            DiscreteFilterKind::Fir(f) => f.step(u),
            DiscreteFilterKind::Iir(f) => f.step(u),
        };
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(
                SignalType::Discrete,
                SignalValue::Scalar(y),
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
// DiscretePID
// ──────────────────────────────────────────────

/// Discrete PID controller with trapezoidal integration.
#[derive(Debug, Clone)]
pub struct DiscretePID {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub kp: Scalar,
    pub ki: Scalar,
    pub kd: Scalar,
    pub dt: Scalar,
    pub min: Scalar,
    pub max: Scalar,
    integral: Scalar,
    prev_error: Scalar,
    initialized: bool,
}

impl DiscretePID {
    pub fn new(id: &str, kp: Scalar, ki: Scalar, kd: Scalar, dt: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("ref", PD::Input, SignalType::Discrete));
        ports.add(Port::new("meas", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "DiscretePID".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            kp,
            ki,
            kd,
            dt,
            min: -1e30,
            max: 1e30,
            integral: 0.0,
            prev_error: 0.0,
            initialized: false,
        }
    }
}

impl Block for DiscretePID {
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
        self.integral = 0.0;
        self.initialized = false;
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
        let ref_val = self
            .ports
            .get("ref")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let meas = self
            .ports
            .get("meas")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let error = ref_val - meas;

        if !self.initialized {
            self.prev_error = error;
            self.initialized = true;
        }

        let p_term = self.kp * error;
        self.integral += self.dt * error;
        let i_term = self.ki * self.integral;
        let d_term = if self.dt > 1e-15 {
            self.kd * (error - self.prev_error) / self.dt
        } else {
            0.0
        };

        let mut y = p_term + i_term + d_term;
        y = y.clamp(self.min, self.max);
        self.prev_error = error;

        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(
                SignalType::Discrete,
                SignalValue::Scalar(y),
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

    fn feed(block: &mut dyn Block, port: &str, v: Scalar) {
        if let Some(p) = block.ports_mut().get_mut(port) {
            p.write(Signal::new(
                SignalType::Discrete,
                SignalValue::Scalar(v),
                0.0,
            ));
        }
    }

    fn read_out(block: &dyn Block, port: &str) -> Scalar {
        block
            .ports()
            .get(port)
            .unwrap()
            .read()
            .unwrap()
            .as_scalar()
            .unwrap()
    }

    #[test]
    fn test_unit_delay() {
        let mut d = UnitDelay::new("ud1");
        d.init().unwrap();
        // First output is 0
        d.output().unwrap();
        assert!((read_out(&d, "y") - 0.0).abs() < EPSILON);
        // Feed input, then update
        feed(&mut d, "u", 42.0);
        d.update().unwrap();
        // Now output should be 42
        d.output().unwrap();
        assert!((read_out(&d, "y") - 42.0).abs() < EPSILON);
    }

    #[test]
    fn test_discrete_integrator() {
        let mut int = DiscreteIntegratorBlock::new("di1", 0.1, 0.0);
        int.init().unwrap();
        feed(&mut int, "u", 5.0);
        int.update().unwrap();
        int.output().unwrap();
        // y = 0 + 0.1 * 5 = 0.5
        assert!((read_out(&int, "y") - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_fir_filter_block() {
        // Moving average of 3: [1/3, 1/3, 1/3]
        let mut f = DiscreteFilter::new_fir("fir1", &[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]);
        f.init().unwrap();
        feed(&mut f, "u", 3.0);
        f.output().unwrap();
        assert!((read_out(&f, "y") - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_discrete_pid_proportional() {
        let mut pid = DiscretePID::new("dp1", 2.0, 0.0, 0.0, 0.1);
        pid.init().unwrap();
        feed(&mut pid, "ref", 10.0);
        feed(&mut pid, "meas", 8.0);
        pid.output().unwrap();
        assert!((read_out(&pid, "y") - 4.0).abs() < EPSILON);
    }
}
