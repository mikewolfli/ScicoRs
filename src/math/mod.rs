//! Math, Signal, and Control Foundation Library
//!
//! Provides signal sources, mathematical operations, logic blocks,
//! continuous-time control blocks (integrator, transfer function, PID),
//! and discrete-time blocks.

use crate::core::block::{Block, BlockError};
use crate::core::error::SimError;
use crate::core::param::{Parameter, ParameterSet};
use crate::core::port::{Port, PortDirection, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, Scalar, SignalType, SignalValue, Time,
};
use std::f64::consts;

// ---------------------------------------------------------------------------
// Signal Sources
// ---------------------------------------------------------------------------

/// A constant signal source.
#[derive(Debug)]
pub struct ConstantSource {
    id: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    value: Scalar,
}

impl ConstantSource {
    pub fn new(id: &str, value: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("out", PortDirection::Output, SignalType::Continuous));

        let mut params = ParameterSet::new();
        params.add(Parameter::new_static("value", SignalValue::Scalar(value), "constant output value"));

        Self {
            id: id.to_string(),
            ports,
            params,
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            value,
        }
    }
}

impl Block for ConstantSource {
    fn id(&self) -> &String { &self.id }
    fn block_type(&self) -> &str { "ConstantSource" }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), BlockError> {
        if let Some(out) = self.ports.get_mut("out") {
            out.write(Signal::new(SignalType::Continuous, SignalValue::Scalar(self.value), self.current_time));
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> { Ok(Vec::new()) }
    fn update(&mut self) -> Result<(), BlockError> { Ok(()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }
}

/// A sine wave generator.
#[derive(Debug)]
pub struct SineSource {
    id: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    amplitude: Scalar,
    frequency: Scalar,
    phase: Scalar,
    offset: Scalar,
}

impl SineSource {
    pub fn new(id: &str, amplitude: Scalar, frequency: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("out", PortDirection::Output, SignalType::Continuous));

        let mut params = ParameterSet::new();
        params.add(Parameter::new_static("amplitude", SignalValue::Scalar(amplitude), "sine amplitude"));
        params.add(Parameter::new_static("frequency", SignalValue::Scalar(frequency), "sine frequency (Hz)"));
        params.add(Parameter::new_config("phase", SignalValue::Scalar(0.0), "initial phase (rad)"));
        params.add(Parameter::new_config("offset", SignalValue::Scalar(0.0), "DC offset"));

        Self {
            id: id.to_string(),
            ports,
            params,
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            amplitude,
            frequency,
            phase: 0.0,
            offset: 0.0,
        }
    }
}

impl Block for SineSource {
    fn id(&self) -> &String { &self.id }
    fn block_type(&self) -> &str { "SineSource" }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), BlockError> {
        self.phase = self.params.get_scalar("phase").unwrap_or(0.0);
        self.offset = self.params.get_scalar("offset").unwrap_or(0.0);
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), BlockError> {
        let omega = 2.0 * consts::PI * self.frequency;
        let value = self.offset + self.amplitude * (omega * self.current_time + self.phase).sin();
        if let Some(out) = self.ports.get_mut("out") {
            out.write(Signal::new(SignalType::Continuous, SignalValue::Scalar(value), self.current_time));
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> { Ok(Vec::new()) }
    fn update(&mut self) -> Result<(), BlockError> { Ok(()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }
}

/// A step signal generator.
#[derive(Debug)]
pub struct StepSource {
    id: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    step_time: Time,
    before: Scalar,
    after: Scalar,
}

impl StepSource {
    pub fn new(id: &str, step_time: Time, before: Scalar, after: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("out", PortDirection::Output, SignalType::Continuous));

        Self {
            id: id.to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            step_time,
            before,
            after,
        }
    }
}

impl Block for StepSource {
    fn id(&self) -> &String { &self.id }
    fn block_type(&self) -> &str { "StepSource" }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), BlockError> {
        let value = if self.current_time >= self.step_time { self.after } else { self.before };
        if let Some(out) = self.ports.get_mut("out") {
            out.write(Signal::new(SignalType::Continuous, SignalValue::Scalar(value), self.current_time));
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> { Ok(Vec::new()) }
    fn update(&mut self) -> Result<(), BlockError> { Ok(()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Math Operations
// ---------------------------------------------------------------------------

/// A configurable math operation block.
#[derive(Debug)]
pub struct MathOp {
    id: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    op: MathOpType,
}

/// Supported math operations.
#[derive(Debug, Clone, Copy)]
pub enum MathOpType {
    Add,
    Subtract,
    Multiply,
    Divide,
    Sin,
    Cos,
    Tan,
    Exp,
    Log,
    Sqrt,
    Abs,
    Negate,
    Pow(Scalar),
}

impl MathOp {
    pub fn new(id: &str, op: MathOpType) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("in1", PortDirection::Input, SignalType::Continuous));
        ports.add(Port::new("out", PortDirection::Output, SignalType::Continuous));
        if matches!(op, MathOpType::Add | MathOpType::Subtract | MathOpType::Multiply | MathOpType::Divide) {
            ports.add(Port::new("in2", PortDirection::Input, SignalType::Continuous));
        }

        Self {
            id: id.to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            op,
        }
    }
}

impl Block for MathOp {
    fn id(&self) -> &String { &self.id }
    fn block_type(&self) -> &str { "MathOp" }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), BlockError> {
        let in1 = self.ports.get("in1")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .ok_or_else(|| SimError::missing_port("in1"))?;

        let result = match self.op {
            MathOpType::Add => {
                let in2 = self.ports.get("in2")
                    .and_then(|p| p.read())
                    .and_then(|s| s.as_scalar())
                    .ok_or_else(|| SimError::missing_port("in2"))?;
                in1 + in2
            }
            MathOpType::Subtract => {
                let in2 = self.ports.get("in2")
                    .and_then(|p| p.read())
                    .and_then(|s| s.as_scalar())
                    .ok_or_else(|| SimError::missing_port("in2"))?;
                in1 - in2
            }
            MathOpType::Multiply => {
                let in2 = self.ports.get("in2")
                    .and_then(|p| p.read())
                    .and_then(|s| s.as_scalar())
                    .ok_or_else(|| SimError::missing_port("in2"))?;
                in1 * in2
            }
            MathOpType::Divide => {
                let in2 = self.ports.get("in2")
                    .and_then(|p| p.read())
                    .and_then(|s| s.as_scalar())
                    .ok_or_else(|| SimError::missing_port("in2"))?;
                if in2 == 0.0 {
                    return Err(SimError::numerical("division by zero"));
                }
                in1 / in2
            }
            MathOpType::Sin => in1.sin(),
            MathOpType::Cos => in1.cos(),
            MathOpType::Tan => in1.tan(),
            MathOpType::Exp => in1.exp(),
            MathOpType::Log => {
                if in1 <= 0.0 {
                    return Err(SimError::numerical("log of non-positive value"));
                }
                in1.ln()
            }
            MathOpType::Sqrt => {
                if in1 < 0.0 {
                    return Err(SimError::numerical("sqrt of negative value"));
                }
                in1.sqrt()
            }
            MathOpType::Abs => in1.abs(),
            MathOpType::Negate => -in1,
            MathOpType::Pow(exp) => in1.powf(exp),
        };

        if let Some(out) = self.ports.get_mut("out") {
            out.write(Signal::new(SignalType::Continuous, SignalValue::Scalar(result), self.current_time));
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> { Ok(Vec::new()) }
    fn update(&mut self) -> Result<(), BlockError> { Ok(()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PID Controller
// ---------------------------------------------------------------------------

/// A continuous-time PID controller block.
#[derive(Debug)]
pub struct PidController {
    id: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    kp: Scalar,
    ki: Scalar,
    kd: Scalar,
    integral: Scalar,
    prev_error: Scalar,
    prev_time: Time,
    output_lim_min: Scalar,
    output_lim_max: Scalar,
}

impl PidController {
    pub fn new(id: &str, kp: Scalar, ki: Scalar, kd: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("ref", PortDirection::Input, SignalType::Continuous));
        ports.add(Port::new("meas", PortDirection::Input, SignalType::Continuous));
        ports.add(Port::new("out", PortDirection::Output, SignalType::Continuous));

        let mut params = ParameterSet::new();
        params.add(Parameter::new_config("kp", SignalValue::Scalar(kp), "proportional gain"));
        params.add(Parameter::new_config("ki", SignalValue::Scalar(ki), "integral gain"));
        params.add(Parameter::new_config("kd", SignalValue::Scalar(kd), "derivative gain"));
        params.add(Parameter::new_config("min", SignalValue::Scalar(-1e6), "output lower limit"));
        params.add(Parameter::new_config("max", SignalValue::Scalar(1e6), "output upper limit"));

        Self {
            id: id.to_string(),
            ports,
            params,
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            kp, ki, kd,
            integral: 0.0,
            prev_error: 0.0,
            prev_time: 0.0,
            output_lim_min: -1e6,
            output_lim_max: 1e6,
        }
    }
}

impl Block for PidController {
    fn id(&self) -> &String { &self.id }
    fn block_type(&self) -> &str { "PIDController" }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, s: ComponentStatus) { self.status = s; }
    fn set_time(&mut self, t: Time) { self.current_time = t; }
    fn time(&self) -> Time { self.current_time }

    fn init(&mut self) -> Result<(), BlockError> {
        self.kp = self.params.get_scalar("kp").unwrap_or(1.0);
        self.ki = self.params.get_scalar("ki").unwrap_or(0.0);
        self.kd = self.params.get_scalar("kd").unwrap_or(0.0);
        self.output_lim_min = self.params.get_scalar("min").unwrap_or(-1e6);
        self.output_lim_max = self.params.get_scalar("max").unwrap_or(1e6);
        self.integral = 0.0;
        self.prev_error = 0.0;
        self.prev_time = self.current_time;
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), BlockError> {
        let reference = self.ports.get("ref")
            .and_then(|p| p.read()).and_then(|s| s.as_scalar())
            .ok_or_else(|| SimError::missing_port("ref"))?;
        let measurement = self.ports.get("meas")
            .and_then(|p| p.read()).and_then(|s| s.as_scalar())
            .ok_or_else(|| SimError::missing_port("meas"))?;

        let error = reference - measurement;
        let dt = self.current_time - self.prev_time;

        // Proportional term
        let p_term = self.kp * error;

        // Integral term (trapezoidal integration)
        if dt > 0.0 && self.ki != 0.0 {
            self.integral += 0.5 * self.ki * (error + self.prev_error) * dt;
            // Anti-windup clamping
            self.integral = self.integral.clamp(self.output_lim_min, self.output_lim_max);
        }
        let i_term = self.integral;

        // Derivative term
        let d_term = if dt > 0.0 && self.kd != 0.0 {
            self.kd * (error - self.prev_error) / dt
        } else {
            0.0
        };

        let output = (p_term + i_term + d_term).clamp(self.output_lim_min, self.output_lim_max);

        if let Some(out) = self.ports.get_mut("out") {
            out.write(Signal::new(SignalType::Continuous, SignalValue::Scalar(output), self.current_time));
        }

        self.prev_error = error;
        self.prev_time = self.current_time;
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> {
        Ok(vec![self.ki * self.prev_error]) // d(integral)/dt = ki * error
    }

    fn update(&mut self) -> Result<(), BlockError> { Ok(()) }
    fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
    fn terminate(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }
}
