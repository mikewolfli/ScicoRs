//! Observation and sink blocks.
//!
//! Provides Block implementations for scopes (ring buffer), data
//! recorders, numeric displays, and chart buffers.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};

use crate::core::types::{ComponentStatus, PortDirection as PD, Scalar, SignalType, Time};

// ──────────────────────────────────────────────
// Scope
// ──────────────────────────────────────────────

/// Scope block — stores signal history in a ring buffer.
#[derive(Debug, Clone)]
pub struct Scope {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    /// Ring buffer of signal values.
    pub buffer: Vec<Scalar>,
    pub time_buffer: Vec<Time>,
    /// Maximum number of samples to store.
    pub capacity: usize,
    write_pos: usize,
    filled: bool,
}

impl Scope {
    pub fn new(id: &str, capacity: usize) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "Scope".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            buffer: Vec::with_capacity(capacity),
            time_buffer: Vec::with_capacity(capacity),
            capacity,
            write_pos: 0,
            filled: false,
        }
    }

    pub fn data(&self) -> Vec<(Time, Scalar)> {
        let n = if self.filled {
            self.capacity
        } else {
            self.write_pos
        };
        let mut result = Vec::with_capacity(n);
        if self.filled {
            for i in self.write_pos..self.capacity {
                result.push((self.time_buffer[i], self.buffer[i]));
            }
        }
        for i in 0..self.write_pos {
            result.push((self.time_buffer[i], self.buffer[i]));
        }
        result
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.time_buffer.clear();
        self.write_pos = 0;
        self.filled = false;
    }
}

impl Block for Scope {
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
        self.clear();
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
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
        // Ensure buffers are sized
        if self.buffer.len() < self.capacity {
            self.buffer.resize(self.capacity, 0.0);
            self.time_buffer.resize(self.capacity, 0.0);
        }
        self.buffer[self.write_pos] = u;
        self.time_buffer[self.write_pos] = self.current_time;
        self.write_pos += 1;
        if self.write_pos >= self.capacity {
            self.write_pos = 0;
            self.filled = true;
        }
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
// DataRecorder
// ──────────────────────────────────────────────

/// Records signal values with timestamps to an internal vector.
#[derive(Debug, Clone)]
pub struct DataRecorder {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    /// Recorded (time, value) pairs.
    pub records: Vec<(Time, Scalar)>,
    pub max_records: Option<usize>,
}

impl DataRecorder {
    pub fn new(id: &str, max_records: Option<usize>) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "DataRecorder".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            records: Vec::new(),
            max_records,
        }
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

impl Block for DataRecorder {
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
        self.records.clear();
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
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
        if let Some(max) = self.max_records
            && self.records.len() >= max
        {
            return Ok(());
        }
        self.records.push((self.current_time, u));
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
// NumericDisplay
// ──────────────────────────────────────────────

/// Converts signal value to a formatted string (for logging/display).
#[derive(Debug, Clone)]
pub struct NumericDisplay {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub prefix: String,
    pub last_value: Option<Scalar>,
    pub last_string: String,
}

impl NumericDisplay {
    pub fn new(id: &str, prefix: &str) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "NumericDisplay".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            prefix: prefix.to_string(),
            last_value: None,
            last_string: String::new(),
        }
    }

    pub fn last_display(&self) -> &str {
        &self.last_string
    }
}

impl Block for NumericDisplay {
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
        self.last_value = None;
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
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
        self.last_value = Some(u);
        self.last_string = format!("{}: {:.6} @ t={:.6}", self.prefix, u, self.current_time);
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
// ChartBuffer
// ──────────────────────────────────────────────

/// Accumulates (time, value) pairs for external charting/plotting.
#[derive(Debug, Clone)]
pub struct ChartBuffer {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    pub data: Vec<(Time, Scalar)>,
    pub max_points: usize,
}

impl ChartBuffer {
    pub fn new(id: &str, max_points: usize) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("u", PD::Input, SignalType::Continuous));
        Self {
            id: id.to_string(),
            block_type: "ChartBuffer".to_string(),
            ports,
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            data: Vec::with_capacity(max_points),
            max_points,
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Return a copy of x-values (time) and y-values (signal).
    pub fn xy(&self) -> (Vec<Time>, Vec<Scalar>) {
        let (mut xs, mut ys) = (
            Vec::with_capacity(self.data.len()),
            Vec::with_capacity(self.data.len()),
        );
        for &(t, v) in &self.data {
            xs.push(t);
            ys.push(v);
        }
        (xs, ys)
    }
}

impl Block for ChartBuffer {
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
        self.data.clear();
        self.status = ComponentStatus::Ready;
        Ok(())
    }
    fn output(&mut self) -> Result<(), SimError> {
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
        if self.data.len() < self.max_points {
            self.data.push((self.current_time, u));
        }
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
    use crate::core::Time;
    use crate::core::signal::Signal;
    use crate::core::types::{EPSILON, Scalar, SignalType, SignalValue};

    fn feed(block: &mut dyn Block, port: &str, v: Scalar) {
        if let Some(p) = block.ports_mut().get_mut(port) {
            p.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(v),
                0.0,
            ));
        }
    }

    #[test]
    fn test_scope_record() {
        let mut scope = Scope::new("sc1", 5);
        scope.init().unwrap();
        for i in 0..3 {
            feed(&mut scope, "u", i as Scalar);
            scope.set_time(i as Time);
            scope.update().unwrap();
        }
        let data = scope.data();
        assert_eq!(data.len(), 3);
        assert!((data[0].1 - 0.0).abs() < EPSILON);
        assert!((data[2].1 - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_scope_ring_buffer() {
        let mut scope = Scope::new("sc2", 3);
        scope.init().unwrap();
        for i in 0..6 {
            feed(&mut scope, "u", i as Scalar);
            scope.set_time(i as Time);
            scope.update().unwrap();
        }
        // After 6 writes to capacity 3, should have the last 3
        let data = scope.data();
        assert_eq!(data.len(), 3);
        assert!((data[0].1 - 3.0).abs() < EPSILON);
        assert!((data[2].1 - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_data_recorder() {
        let mut rec = DataRecorder::new("rec1", Some(5));
        rec.init().unwrap();
        for i in 0..10 {
            feed(&mut rec, "u", i as Scalar);
            rec.set_time(i as Time);
            rec.update().unwrap();
        }
        assert_eq!(rec.records.len(), 5);
        rec.clear();
        assert!(rec.records.is_empty());
    }

    #[test]
    fn test_numeric_display() {
        let mut disp = NumericDisplay::new("dsp1", "sensor1");
        disp.init().unwrap();
        feed(&mut disp, "u", std::f64::consts::PI);
        disp.update().unwrap();
        assert!((disp.last_value.unwrap() - std::f64::consts::PI).abs() < 1e-12);
        assert!(disp.last_display().contains("sensor1"));
    }

    #[test]
    fn test_chart_buffer() {
        let mut chart = ChartBuffer::new("ch1", 100);
        chart.init().unwrap();
        for i in 0..50 {
            feed(&mut chart, "u", (i as Scalar).sin());
            chart.set_time(i as Time);
            chart.update().unwrap();
        }
        assert_eq!(chart.data.len(), 50);
        let (xs, ys) = chart.xy();
        assert_eq!(xs.len(), 50);
        assert_eq!(ys.len(), 50);
        chart.clear();
        assert!(chart.data.is_empty());
    }

    #[test]
    fn test_scope_clear() {
        let mut scope = Scope::new("sc3", 10);
        scope.init().unwrap();
        feed(&mut scope, "u", 42.0);
        scope.update().unwrap();
        assert!(!scope.data().is_empty());
        scope.clear();
        assert!(scope.data().is_empty());
    }
}
