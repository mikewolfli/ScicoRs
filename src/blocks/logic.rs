//! Logic operation blocks.
//!
//! Provides Block implementations for boolean logic gates, comparators,
//! multiplexers, saturation, and switches.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, PortDirection as PD, Scalar, SignalType, SignalValue, Time,
};

// ──────────────────────────────────────────────
// LogicAnd
// ──────────────────────────────────────────────

/// Boolean AND gate: `y = u1 && u2`. Inputs thresholded at 0.5.
#[derive(Debug, Clone)]
pub struct LogicAnd {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl LogicAnd {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Discrete));
        ports.add(Port::new("u2", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "LogicAnd".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

fn to_bool(v: Scalar) -> bool {
    v >= 0.5
}
fn from_bool(b: bool) -> Scalar {
    if b { 1.0 } else { 0.0 }
}

impl Block for LogicAnd {
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
        let u1 = self
            .ports
            .get("u1")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let u2 = self
            .ports
            .get("u2")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let y = from_bool(to_bool(u1) && to_bool(u2));
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
// LogicOr
// ──────────────────────────────────────────────

/// Boolean OR gate: `y = u1 || u2`.
#[derive(Debug, Clone)]
pub struct LogicOr {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl LogicOr {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Discrete));
        ports.add(Port::new("u2", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "LogicOr".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for LogicOr {
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
        let u1 = self
            .ports
            .get("u1")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let u2 = self
            .ports
            .get("u2")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let y = from_bool(to_bool(u1) || to_bool(u2));
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
// LogicNot
// ──────────────────────────────────────────────

/// Boolean NOT gate: `y = !u`.
#[derive(Debug, Clone)]
pub struct LogicNot {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl LogicNot {
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

impl Block for LogicNot {
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
        let y = from_bool(!to_bool(u));
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
// LogicXor
// ──────────────────────────────────────────────

/// Boolean XOR gate: `y = u1 ^ u2`.
#[derive(Debug, Clone)]
pub struct LogicXor {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl LogicXor {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Discrete));
        ports.add(Port::new("u2", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "LogicXor".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for LogicXor {
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
        let u1 = self
            .ports
            .get("u1")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let u2 = self
            .ports
            .get("u2")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let y = from_bool(to_bool(u1) != to_bool(u2));
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
// Comparator
// ──────────────────────────────────────────────

/// Compares two inputs: `y = u1 > u2 ? 1.0 : 0.0`.
#[derive(Debug, Clone)]
pub struct Comparator {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub hysteresis: Scalar,
}

impl Comparator {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Continuous));
        ports.add(Port::new("u2", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Discrete));
        Self {
            id: id.to_string(),
            block_type: "Comparator".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            hysteresis: 0.0,
        }
    }
}

impl Block for Comparator {
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
        let u1 = self
            .ports
            .get("u1")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let u2 = self
            .ports
            .get("u2")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let y = if u1 > u2 + self.hysteresis { 1.0 } else { 0.0 };
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
// Multiplexer (2-to-1)
// ──────────────────────────────────────────────

/// 2-to-1 multiplexer: `y = sel == 0 ? u0 : u1`.
#[derive(Debug, Clone)]
pub struct Multiplexer {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl Multiplexer {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u0", PD::Input, SignalType::Continuous));
        ports.add(Port::new("u1", PD::Input, SignalType::Continuous));
        ports.add(Port::new("sel", PD::Input, SignalType::Discrete));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Multiplexer".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for Multiplexer {
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
        let u0 = self
            .ports
            .get("u0")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let u1 = self
            .ports
            .get("u1")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let sel = self
            .ports
            .get("sel")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let y = if to_bool(sel) { u1 } else { u0 };
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(
                SignalType::Continuous,
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
// Saturation
// ──────────────────────────────────────────────

/// Limits the input to a range: `y = clamp(u, min, max)`.
#[derive(Debug, Clone)]
pub struct Saturation {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub min: Scalar,
    pub max: Scalar,
}

impl Saturation {
    pub fn new(id: &str, min: Scalar, max: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Saturation".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            min,
            max,
        }
    }
}

impl Block for Saturation {
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
        let y = u.clamp(self.min, self.max);
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(
                SignalType::Continuous,
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
// Switch
// ──────────────────────────────────────────────

/// Selects between two inputs based on control: `y = control > threshold ? u2 : u1`.
#[derive(Debug, Clone)]
pub struct Switch {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub threshold: Scalar,
}

impl Switch {
    pub fn new(id: &str, threshold: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Continuous));
        ports.add(Port::new("u2", PD::Input, SignalType::Continuous));
        ports.add(Port::new("control", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Switch".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            threshold,
        }
    }
}

impl Block for Switch {
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
        let u1 = self
            .ports
            .get("u1")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let u2 = self
            .ports
            .get("u2")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let ctrl = self
            .ports
            .get("control")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        let y = if ctrl > self.threshold { u2 } else { u1 };
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(
                SignalType::Continuous,
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
                SignalType::Continuous,
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
    fn test_logic_and() {
        let mut g = LogicAnd::new("and1");
        g.init().unwrap();
        feed(&mut g, "u1", 1.0);
        feed(&mut g, "u2", 1.0);
        g.output().unwrap();
        assert!((read_out(&g, "y") - 1.0).abs() < EPSILON);
        feed(&mut g, "u1", 1.0);
        feed(&mut g, "u2", 0.0);
        g.output().unwrap();
        assert!((read_out(&g, "y") - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_logic_or() {
        let mut g = LogicOr::new("or1");
        g.init().unwrap();
        feed(&mut g, "u1", 0.0);
        feed(&mut g, "u2", 1.0);
        g.output().unwrap();
        assert!((read_out(&g, "y") - 1.0).abs() < EPSILON);
        feed(&mut g, "u1", 0.0);
        feed(&mut g, "u2", 0.0);
        g.output().unwrap();
        assert!((read_out(&g, "y") - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_logic_not() {
        let mut g = LogicNot::new("not1");
        g.init().unwrap();
        feed(&mut g, "u", 1.0);
        g.output().unwrap();
        assert!((read_out(&g, "y") - 0.0).abs() < EPSILON);
        feed(&mut g, "u", 0.0);
        g.output().unwrap();
        assert!((read_out(&g, "y") - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_logic_xor() {
        let mut g = LogicXor::new("xor1");
        g.init().unwrap();
        feed(&mut g, "u1", 1.0);
        feed(&mut g, "u2", 0.0);
        g.output().unwrap();
        assert!((read_out(&g, "y") - 1.0).abs() < EPSILON);
        feed(&mut g, "u1", 1.0);
        feed(&mut g, "u2", 1.0);
        g.output().unwrap();
        assert!((read_out(&g, "y") - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_comparator() {
        let mut c = Comparator::new("cmp1");
        c.init().unwrap();
        feed(&mut c, "u1", 5.0);
        feed(&mut c, "u2", 3.0);
        c.output().unwrap();
        assert!((read_out(&c, "y") - 1.0).abs() < EPSILON);
        feed(&mut c, "u1", 1.0);
        feed(&mut c, "u2", 3.0);
        c.output().unwrap();
        assert!((read_out(&c, "y") - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_multiplexer() {
        let mut m = Multiplexer::new("mux1");
        m.init().unwrap();
        feed(&mut m, "u0", 10.0);
        feed(&mut m, "u1", 20.0);
        feed(&mut m, "sel", 0.0);
        m.output().unwrap();
        assert!((read_out(&m, "y") - 10.0).abs() < EPSILON);
        feed(&mut m, "sel", 1.0);
        m.output().unwrap();
        assert!((read_out(&m, "y") - 20.0).abs() < EPSILON);
    }

    #[test]
    fn test_saturation() {
        let mut s = Saturation::new("sat1", -1.0, 1.0);
        s.init().unwrap();
        feed(&mut s, "u", 5.0);
        s.output().unwrap();
        assert!((read_out(&s, "y") - 1.0).abs() < EPSILON);
        feed(&mut s, "u", -5.0);
        s.output().unwrap();
        assert!((read_out(&s, "y") - (-1.0)).abs() < EPSILON);
        feed(&mut s, "u", 0.5);
        s.output().unwrap();
        assert!((read_out(&s, "y") - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_switch() {
        let mut sw = Switch::new("sw1", 0.5);
        sw.init().unwrap();
        feed(&mut sw, "u1", 10.0);
        feed(&mut sw, "u2", 20.0);
        feed(&mut sw, "control", 0.0);
        sw.output().unwrap();
        assert!((read_out(&sw, "y") - 10.0).abs() < EPSILON);
        feed(&mut sw, "control", 1.0);
        sw.output().unwrap();
        assert!((read_out(&sw, "y") - 20.0).abs() < EPSILON);
    }
}
