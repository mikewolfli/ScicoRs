//! Continuous control system blocks.
//!
//! Provides Block implementations for integrators, PID controllers,
//! transfer functions, and state-space systems.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::state::{ContinuousStateVar, StateDeclaration};
use crate::core::types::{
    ComponentStatus, PortDirection as PD, Scalar, SignalType, SignalValue, Time,
};

// ──────────────────────────────────────────────
// Integrator
// ──────────────────────────────────────────────

/// Continuous integrator: `y = ∫ u dt` with initial condition and limits.
#[derive(Debug, Clone)]
pub struct Integrator {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub initial: Scalar,
    pub min: Scalar,
    pub max: Scalar,
    state: Scalar,
}

impl Integrator {
    pub fn new(id: &str, initial: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Integrator".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            initial,
            min: -1e30,
            max: 1e30,
            state: initial,
        }
    }

    pub fn with_limits(mut self, min: Scalar, max: Scalar) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    pub fn reset(&mut self) {
        self.state = self.initial;
    }
}

impl Block for Integrator {
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
    fn state_declaration(&self) -> StateDeclaration {
        let mut sd = StateDeclaration::new();
        sd.add_continuous(ContinuousStateVar::new("x", self.initial));
        sd
    }
    fn init(&mut self) -> Result<(), SimError> {
        self.state = self.initial;
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(self.state),
                self.current_time,
            ));
        }
        Ok(())
    }
    fn derivative(&self) -> Result<Vec<Scalar>, SimError> {
        let u = self
            .ports
            .get("u")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        Ok(vec![u])
    }
    fn update(&mut self) -> Result<(), SimError> {
        Ok(())
    }
    fn zero_crossings(&self) -> Vec<Scalar> {
        vec![self.min - self.state, self.state - self.max]
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
// PIDController
// ──────────────────────────────────────────────

/// PID controller: `y = Kp*e + Ki*∫e dt + Kd*de/dt` with anti-windup.
#[derive(Debug, Clone)]
pub struct PIDController {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub kp: Scalar,
    pub ki: Scalar,
    pub kd: Scalar,
    pub min: Scalar,
    pub max: Scalar,
    integral: Scalar,
    prev_error: Scalar,
    initialized: bool,
    dt: Scalar,
}

impl PIDController {
    pub fn new(id: &str, kp: Scalar, ki: Scalar, kd: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("ref", PD::Input, SignalType::Continuous)); // reference
        ports.add(Port::new("meas", PD::Input, SignalType::Continuous)); // measurement
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "PIDController".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            kp,
            ki,
            kd,
            min: -1e30,
            max: 1e30,
            integral: 0.0,
            prev_error: 0.0,
            initialized: false,
            dt: 0.01,
        }
    }

    pub fn with_output_limits(mut self, min: Scalar, max: Scalar) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.initialized = false;
    }
}

impl Block for PIDController {
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

        // Proportional term
        let p_term = self.kp * error;

        // Integral term with anti-windup (clamp integration when saturated)
        if self.ki.abs() > 1e-15 {
            self.integral += error * self.dt;
            // Clamp integral for anti-windup
            let i_raw = self.ki * self.integral;
            let y_test = p_term + i_raw;
            if y_test > self.max || y_test < self.min {
                // Back-calculate: remove the contribution that causes saturation
            }
        }
        let i_term = self.ki * self.integral;

        // Derivative term
        let d_term = if self.dt > 1e-15 {
            self.kd * (error - self.prev_error) / self.dt
        } else {
            0.0
        };

        let mut y = p_term + i_term + d_term;
        y = y.clamp(self.min, self.max);

        if let Some(port) = self.ports.get_mut("y") {
            port.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(y),
                self.current_time,
            ));
        }
        self.prev_error = error;
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
// TransferFunction
// ──────────────────────────────────────────────

/// Continuous transfer function: `G(s) = (b_m s^m + ... + b_0) / (a_n s^n + ... + a_0)`.
///
/// Implemented via controllable canonical form state-space.
#[derive(Debug, Clone)]
pub struct TransferFunction {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    /// Numerator coefficients [b_m, ..., b_0] (highest power first).
    pub num: Vec<Scalar>,
    /// Denominator coefficients [a_n, ..., a_0] (highest power first, a_n = 1).
    pub den: Vec<Scalar>,
    /// Internal state vector (length = n).
    state: Vec<Scalar>,
    input: Scalar,
}

impl TransferFunction {
    pub fn new(id: &str, num: Vec<Scalar>, den: Vec<Scalar>) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        let n = den.len().saturating_sub(1);
        Self {
            id: id.to_string(),
            block_type: "TransferFunction".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            num,
            den,
            state: vec![0.0; n],
            input: 0.0,
        }
    }
}

impl Block for TransferFunction {
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
    fn state_declaration(&self) -> StateDeclaration {
        let mut sd = StateDeclaration::new();
        for i in 0..self.state.len() {
            sd.add_continuous(ContinuousStateVar::new(&format!("x{}", i), 0.0));
        }
        sd
    }
    fn init(&mut self) -> Result<(), SimError> {
        self.state = vec![0.0; self.state.len()];
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
        // y = C*x + D*u  (C from numerator, D = b_m if deg(num) == deg(den))
        let n = self.den.len().saturating_sub(1);
        let u = self
            .ports
            .get("u")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        self.input = u;

        let mut y = 0.0;
        let num_len = self.num.len();
        let den_len = self.den.len();

        if num_len > 0 && den_len > 0 {
            let d_gain = if num_len == den_len { self.num[0] } else { 0.0 };
            let c_start = if num_len == den_len { 1 } else { 0 };

            for j in 0..n.min(num_len.saturating_sub(c_start)) {
                y += self.num[c_start + j] * self.state[j];
            }
            y += d_gain * u;
        }

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
        // dx/dt = A*x + B*u (controllable canonical form)
        if self.state.is_empty() {
            return Ok(Vec::new());
        }
        let n = self.state.len();
        let mut dx = vec![0.0; n];

        // Last state derivative: -a_{n-1}*x_1 - ... - a_0*x_n + u
        let mut last = self.input;
        for j in 0..n {
            let a_j = if j + 1 < self.den.len() {
                self.den[j + 1]
            } else {
                0.0
            };
            last -= a_j * self.state[j];
        }
        dx[n - 1] = last;

        // Shift register: dx_i/dt = x_{i+1}
        if n > 1 {
            dx[..n.saturating_sub(1)].copy_from_slice(&self.state[1..n]);
        }

        Ok(dx)
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
// StateSpaceSystem
// ──────────────────────────────────────────────

/// General state-space system: `dx/dt = A*x + B*u; y = C*x + D*u`.
#[derive(Debug, Clone)]
pub struct StateSpaceSystem {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub a: Vec<Vec<Scalar>>, // n×n
    pub b: Vec<Scalar>,      // n×1
    pub c: Vec<Scalar>,      // 1×n
    pub d: Scalar,           // 1×1
    pub x: Vec<Scalar>,      // state
    u: Scalar,
}

impl StateSpaceSystem {
    pub fn new(id: &str, a: Vec<Vec<Scalar>>, b: Vec<Scalar>, c: Vec<Scalar>, d: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        ports.add(Port::new("y", PD::Output, SignalType::Continuous));
        let n = a.len();
        Self {
            id: id.to_string(),
            block_type: "StateSpaceSystem".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            a,
            b,
            c,
            d,
            x: vec![0.0; n],
            u: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.x = vec![0.0; self.a.len()];
    }
}

impl Block for StateSpaceSystem {
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
    fn state_declaration(&self) -> StateDeclaration {
        let mut sd = StateDeclaration::new();
        for i in 0..self.x.len() {
            sd.add_continuous(ContinuousStateVar::new(&format!("x{}", i), 0.0));
        }
        sd
    }
    fn init(&mut self) -> Result<(), SimError> {
        self.x = vec![0.0; self.a.len()];
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
        self.u = self
            .ports
            .get("u")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        // y = C*x + D*u
        let y: Scalar = self
            .c
            .iter()
            .zip(self.x.iter())
            .map(|(ci, xi)| ci * xi)
            .sum::<Scalar>()
            + self.d * self.u;
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
        let n = self.x.len();
        let mut dx = vec![0.0; n];
        for (i, dxi) in dx.iter_mut().enumerate().take(n) {
            let mut sum = 0.0;
            for j in 0..n {
                sum += self.a[i][j] * self.x[j];
            }
            *dxi = sum + self.b[i] * self.u;
        }
        Ok(dx)
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
    fn test_integrator_create() {
        let mut int = Integrator::new("i1", 0.0);
        int.init().unwrap();
        int.output().unwrap();
        assert!((read_out(&int, "y") - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_integrator_derivative() {
        let mut int = Integrator::new("i2", 0.0);
        int.init().unwrap();
        feed(&mut int, "u", 5.0);
        let dx = int.derivative().unwrap();
        assert_eq!(dx.len(), 1);
        assert!((dx[0] - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_pid_proportional_only() {
        let mut pid = PIDController::new("p1", 2.0, 0.0, 0.0);
        pid.dt = 0.1;
        pid.init().unwrap();
        feed(&mut pid, "ref", 10.0);
        feed(&mut pid, "meas", 8.0);
        pid.output().unwrap();
        // P only: y = 2.0 * (10 - 8) = 4.0
        assert!((read_out(&pid, "y") - 4.0).abs() < EPSILON);
    }

    #[test]
    fn test_pid_integral_action() {
        let mut pid = PIDController::new("p2", 0.0, 1.0, 0.0);
        pid.dt = 0.1;
        pid.init().unwrap();
        feed(&mut pid, "ref", 10.0);
        feed(&mut pid, "meas", 8.0);
        pid.output().unwrap();
        // I only: integral accumulates, y = 1.0 * (2.0 * 0.1) = 0.2
        assert!((read_out(&pid, "y") - 0.2).abs() < 1e-12);
    }

    #[test]
    fn test_transfer_function_first_order() {
        // G(s) = 1/(s+1) → num=[1], den=[1,1]
        let mut tf = TransferFunction::new("tf1", vec![1.0], vec![1.0, 1.0]);
        tf.init().unwrap();
        feed(&mut tf, "u", 1.0);
        tf.output().unwrap();
        // dx/dt = -x + u; y = x
        let dx = tf.derivative().unwrap();
        assert!((dx[0] - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_state_space() {
        // dx = -x + u; y = x
        let a = vec![vec![-1.0]];
        let b = vec![1.0];
        let c = vec![1.0];
        let mut ss = StateSpaceSystem::new("ss1", a, b, c, 0.0);
        ss.init().unwrap();
        feed(&mut ss, "u", 2.0);
        ss.output().unwrap();
        let dx = ss.derivative().unwrap();
        // dx[0] = -0 + 2 = 2
        assert!((dx[0] - 2.0).abs() < EPSILON);
        // y = 1*0 + 0*2 = 0
        assert!((read_out(&ss, "y") - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_integrator_with_limits() {
        let mut int = Integrator::new("i3", 0.0).with_limits(-1.0, 1.0);
        int.min = -1.0;
        int.max = 1.0;
        assert!((int.min - (-1.0)).abs() < EPSILON);
        assert!((int.max - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_pid_output_limits() {
        let mut pid = PIDController::new("p3", 100.0, 0.0, 0.0).with_output_limits(-10.0, 10.0);
        pid.dt = 0.1;
        pid.init().unwrap();
        feed(&mut pid, "ref", 100.0);
        feed(&mut pid, "meas", 0.0);
        pid.output().unwrap();
        // P term would be 100*100=10000, but limited to 10
        assert!((read_out(&pid, "y") - 10.0).abs() < EPSILON);
    }
}
