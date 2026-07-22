//! Visualization and Post-Processing
//!
//! Provides data recording, charting, and visualization capabilities
//! for simulation results. Includes oscilloscope-like signal viewers,
//! data recorders, and export utilities.

use crate::core::types::{SignalValue, Time};
use std::collections::HashMap;

/// A single data point recorded during simulation.
#[derive(Debug, Clone)]
pub struct DataPoint {
    pub time: Time,
    pub value: f64,
}

/// A trace is a named series of data points from a signal.
#[derive(Debug, Clone)]
pub struct Trace {
    pub name: String,
    pub unit: String,
    pub data: Vec<DataPoint>,
}

impl Trace {
    pub fn new(name: &str, unit: &str) -> Self {
        Self { name: name.to_string(), unit: unit.to_string(), data: Vec::new() }
    }

    pub fn record(&mut self, time: Time, value: f64) {
        self.data.push(DataPoint { time, value });
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn last_value(&self) -> Option<f64> {
        self.data.last().map(|dp| dp.value)
    }

    pub fn min(&self) -> Option<f64> {
        self.data.iter().map(|dp| dp.value).min_by(|a, b| a.partial_cmp(b).unwrap())
    }

    pub fn max(&self) -> Option<f64> {
        self.data.iter().map(|dp| dp.value).max_by(|a, b| a.partial_cmp(b).unwrap())
    }

    pub fn mean(&self) -> Option<f64> {
        if self.data.is_empty() {
            return None;
        }
        let sum: f64 = self.data.iter().map(|dp| dp.value).sum();
        Some(sum / self.data.len() as f64)
    }
}

/// Data recorder — collects traces during simulation.
#[derive(Debug, Default)]
pub struct DataRecorder {
    pub traces: HashMap<String, Trace>,
}

impl DataRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_trace(&mut self, name: &str, unit: &str) {
        self.traces.entry(name.to_string()).or_insert_with(|| Trace::new(name, unit));
    }

    pub fn record(&mut self, name: &str, time: Time, value: SignalValue) {
        if let Some(trace) = self.traces.get_mut(name) {
            let v = match value {
                SignalValue::Scalar(s) => s,
                SignalValue::Integer(i) => i as f64,
                SignalValue::Boolean(b) => if b { 1.0 } else { 0.0 },
                _ => return,
            };
            trace.record(time, v);
        }
    }

    pub fn get_trace(&self, name: &str) -> Option<&Trace> {
        self.traces.get(name)
    }

    pub fn clear(&mut self) {
        for trace in self.traces.values_mut() {
            trace.clear();
        }
    }

    /// Export trace data as CSV string.
    pub fn to_csv(&self) -> String {
        let mut csv = String::new();
        // Header
        csv.push_str("time");
        for name in self.traces.keys() {
            csv.push(',');
            csv.push_str(name);
        }
        csv.push('\n');

        // Find max length
        let max_len = self.traces.values().map(|t| t.data.len()).max().unwrap_or(0);
        for i in 0..max_len {
            if let Some(first) = self.traces.values().next()
                && i < first.data.len()
            {
                csv.push_str(&first.data[i].time.to_string());
            }
            for trace in self.traces.values() {
                csv.push(',');
                if i < trace.data.len() {
                    csv.push_str(&trace.data[i].value.to_string());
                }
            }
            csv.push('\n');
        }
        csv
    }
}
