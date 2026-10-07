// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Math operation blocks.
//!
//! Provides Block implementations for arithmetic, trigonometric,
//! and matrix operations.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::{Parameter, ParameterSet, all_finite};
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, PortDirection as PD, Scalar, SignalType, SignalValue, Time,
};

// ──────────────────────────────────────────────
// Adder
// ──────────────────────────────────────────────

/// Weighted sum: `y = k1*u1 + k2*u2 + bias`.
#[derive(Debug, Clone)]
pub struct Adder {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub k1: Scalar,
    pub k2: Scalar,
    pub bias: Scalar,
}

impl Adder {
    pub fn new(id: &str, k1: Scalar, k2: Scalar, bias: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Continuous));
        ports.add(Port::new("u2", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Adder".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            k1,
            k2,
            bias,
        }
    }
}

impl Block for Adder {
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
        let y = self.k1 * u1 + self.k2 * u2 + self.bias;
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
    fn configuration(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_tunable(
                "bias",
                SignalValue::Scalar(self.bias),
                "block configuration",
            ),
            Parameter::new_tunable("k1", SignalValue::Scalar(self.k1), "block configuration"),
            Parameter::new_tunable("k2", SignalValue::Scalar(self.k2), "block configuration"),
        ]
    }
    fn apply_configuration(&mut self, params: &[Parameter]) -> usize {
        let mut applied = 0;
        for p in params {
            if let SignalValue::Scalar(v) = p.value {
                // A non-finite value from a hand-edited file is rejected
                // rather than stored: it would propagate into every
                // downstream computation as NaN.
                if !v.is_finite() {
                    continue;
                }
                match p.name.as_str() {
                    "bias" => {
                        self.bias = v;
                        applied += 1;
                    }
                    "k1" => {
                        self.k1 = v;
                        applied += 1;
                    }
                    "k2" => {
                        self.k2 = v;
                        applied += 1;
                    }
                    _ => {}
                }
            }
        }
        applied
    }
}

// ──────────────────────────────────────────────
// Subtractor
// ──────────────────────────────────────────────

/// Difference: `y = u1 - u2`.
#[derive(Debug, Clone)]
pub struct Subtractor {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl Subtractor {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Continuous));
        ports.add(Port::new("u2", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Subtractor".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for Subtractor {
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
        let y = u1 - u2;
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
// Multiplier
// ──────────────────────────────────────────────

/// Product: `y = u1 * u2`.
#[derive(Debug, Clone)]
pub struct Multiplier {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl Multiplier {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Continuous));
        ports.add(Port::new("u2", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Multiplier".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        }
    }
}

impl Block for Multiplier {
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
        let y = u1 * u2;
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
// Divider
// ──────────────────────────────────────────────

/// Division: `y = u1 / u2` with zero-guard.
#[derive(Debug, Clone)]
pub struct Divider {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub epsilon: Scalar,
}

impl Divider {
    pub fn new(id: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Continuous));
        ports.add(Port::new("u2", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Divider".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            epsilon: 1e-15,
        }
    }
}

impl Block for Divider {
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
        let y = if u2.abs() < self.epsilon {
            u1 / self.epsilon
        } else {
            u1 / u2
        };
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
    fn configuration(&self) -> Vec<Parameter> {
        vec![Parameter::new_tunable(
            "epsilon",
            SignalValue::Scalar(self.epsilon),
            "block configuration",
        )]
    }
    fn apply_configuration(&mut self, params: &[Parameter]) -> usize {
        let mut applied = 0;
        for p in params {
            if let SignalValue::Scalar(v) = p.value {
                // A non-finite value from a hand-edited file is rejected
                // rather than stored: it would propagate into every
                // downstream computation as NaN.
                if !v.is_finite() {
                    continue;
                }
                match p.name.as_str() {
                    "epsilon" => {
                        self.epsilon = v;
                        applied += 1;
                    }
                    _ => {}
                }
            }
        }
        applied
    }
}

// ──────────────────────────────────────────────
// Gain
// ──────────────────────────────────────────────

/// Amplifier: `y = k * u`.
#[derive(Debug, Clone)]
pub struct Gain {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub k: Scalar,
}

impl Gain {
    pub fn new(id: &str, k: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Gain".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            k,
        }
    }
}

impl Block for Gain {
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
        let y = self.k * u;
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

    /// The gain lives in the typed `k` field, so expose it explicitly (and read
    /// it back on load) rather than relying only on the mirrored parameter.
    fn configuration(&self) -> Vec<Parameter> {
        vec![Parameter::new_tunable(
            "k",
            SignalValue::Scalar(self.k),
            "gain factor",
        )]
    }
    fn apply_configuration(&mut self, params: &[Parameter]) -> usize {
        let mut applied = 0;
        for p in params {
            if let ("k", SignalValue::Scalar(v)) = (p.name.as_str(), &p.value) {
                if v.is_finite() {
                    self.k = *v;
                    applied += 1;
                }
            }
        }
        applied
    }
}

// ──────────────────────────────────────────────
// TrigFunction
// ──────────────────────────────────────────────

/// Named trigonometric operations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrigOp {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Exp,
    Log,
    Log10,
}

/// Computes a trigonometric or transcendental function of the input: `y = f(u)`.
#[derive(Debug, Clone)]
pub struct TrigFunction {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub op: TrigOp,
}

impl TrigFunction {
    pub fn new(id: &str, op: TrigOp) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: format!("Trig_{:?}", op),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            op,
        }
    }

    fn compute(&self, u: Scalar) -> Scalar {
        match self.op {
            TrigOp::Sin => u.sin(),
            TrigOp::Cos => u.cos(),
            TrigOp::Tan => u.tan(),
            TrigOp::Asin => u.asin(),
            TrigOp::Acos => u.acos(),
            TrigOp::Atan => u.atan(),
            TrigOp::Exp => u.exp(),
            TrigOp::Log => {
                if u > 0.0 {
                    u.ln()
                } else {
                    f64::NEG_INFINITY
                }
            }
            TrigOp::Log10 => {
                if u > 0.0 {
                    u.log10()
                } else {
                    f64::NEG_INFINITY
                }
            }
        }
    }
}

impl Block for TrigFunction {
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
        let y = self.compute(u);
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
// MatrixMultiply
// ──────────────────────────────────────────────

/// 2x2 matrix multiplication: `y = A * u` (A is 2x2, u is 2-element vector).
#[derive(Debug, Clone)]
pub struct MatrixMultiply {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub a: [[Scalar; 2]; 2],
}

impl MatrixMultiply {
    pub fn new(id: &str, a: [[Scalar; 2]; 2]) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u1", PD::Input, SignalType::Continuous));
        ports.add(Port::new("u2", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y1", PD::Output, SignalType::Continuous));
        ports.add(Port::new("y2", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "MatrixMultiply".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            a,
        }
    }
}

impl Block for MatrixMultiply {
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
        let y1 = self.a[0][0] * u1 + self.a[0][1] * u2;
        let y2 = self.a[1][0] * u1 + self.a[1][1] * u2;
        if let Some(port) = self.ports.get_mut("y1") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(y1),
                self.current_time,
            ));
        }
        if let Some(port) = self.ports.get_mut("y2") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(y2),
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

    /// The 2x2 coefficient matrix is serialized as a `SignalValue::Matrix`, and
    /// the incoming data is rejected unless it is genuinely 2x2 and finite.
    fn configuration(&self) -> Vec<Parameter> {
        let flat: Vec<Scalar> = self.a.iter().flatten().copied().collect();
        vec![Parameter::new_config(
            "a",
            SignalValue::Matrix(2, 2, flat),
            "2x2 coefficient matrix",
        )]
    }
    fn apply_configuration(&mut self, params: &[Parameter]) -> usize {
        for p in params {
            if let ("a", SignalValue::Matrix(2, 2, data)) = (p.name.as_str(), &p.value) {
                if !all_finite(data) {
                    return 0;
                }
                self.a = [[data[0], data[1]], [data[2], data[3]]];
                return 1;
            }
        }
        0
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::EPSILON;

    fn feed_input(block: &mut dyn Block, port: &str, value: Scalar) {
        if let Some(p) = block.ports_mut().get_mut(port) {
            p.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(value),
                0.0,
            ));
        }
    }

    fn read_output(block: &dyn Block, port: &str) -> Scalar {
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
    fn test_adder() {
        let mut a = Adder::new("a1", 2.0, 3.0, 1.0);
        a.init().unwrap();
        feed_input(&mut a, "u1", 5.0);
        feed_input(&mut a, "u2", 2.0);
        a.output().unwrap();
        assert!((read_output(&a, "y") - (2.0 * 5.0 + 3.0 * 2.0 + 1.0)).abs() < EPSILON);
    }

    #[test]
    fn test_subtractor() {
        let mut s = Subtractor::new("s1");
        s.init().unwrap();
        feed_input(&mut s, "u1", 10.0);
        feed_input(&mut s, "u2", 3.0);
        s.output().unwrap();
        assert!((read_output(&s, "y") - 7.0).abs() < EPSILON);
    }

    #[test]
    fn test_multiplier() {
        let mut m = Multiplier::new("m1");
        m.init().unwrap();
        feed_input(&mut m, "u1", 4.0);
        feed_input(&mut m, "u2", 2.5);
        m.output().unwrap();
        assert!((read_output(&m, "y") - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_divider() {
        let mut d = Divider::new("d1");
        d.init().unwrap();
        feed_input(&mut d, "u1", 10.0);
        feed_input(&mut d, "u2", 4.0);
        d.output().unwrap();
        assert!((read_output(&d, "y") - 2.5).abs() < EPSILON);
    }

    #[test]
    fn test_divider_zero_guard() {
        let mut d = Divider::new("d2");
        d.init().unwrap();
        feed_input(&mut d, "u1", 5.0);
        feed_input(&mut d, "u2", 0.0);
        d.output().unwrap();
        // Should not panic; returns a finite value
        let y = read_output(&d, "y");
        assert!(y.is_finite());
    }

    #[test]
    fn test_gain() {
        let mut g = Gain::new("g1", 5.0);
        g.init().unwrap();
        feed_input(&mut g, "u", 3.0);
        g.output().unwrap();
        assert!((read_output(&g, "y") - 15.0).abs() < EPSILON);
    }

    #[test]
    fn test_trig_sin() {
        let mut t = TrigFunction::new("t1", TrigOp::Sin);
        t.init().unwrap();
        feed_input(&mut t, "u", std::f64::consts::FRAC_PI_2);
        t.output().unwrap();
        assert!((read_output(&t, "y") - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_trig_exp() {
        let mut t = TrigFunction::new("t2", TrigOp::Exp);
        t.init().unwrap();
        feed_input(&mut t, "u", 0.0);
        t.output().unwrap();
        assert!((read_output(&t, "y") - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_matrix_multiply() {
        let a = [[1.0, 2.0], [3.0, 4.0]];
        let mut m = MatrixMultiply::new("m1", a);
        m.init().unwrap();
        feed_input(&mut m, "u1", 5.0);
        feed_input(&mut m, "u2", 6.0);
        m.output().unwrap();
        // y1 = 1*5 + 2*6 = 17
        assert!((read_output(&m, "y1") - 17.0).abs() < EPSILON);
        // y2 = 3*5 + 4*6 = 39
        assert!((read_output(&m, "y2") - 39.0).abs() < EPSILON);
    }
}
