//! Utilities and Helpers
//!
//! General-purpose utility functions used across the simulation kernel.

use crate::core::types::Scalar;

/// Format a duration (in seconds) as a human-readable string.
pub fn format_duration(seconds: f64) -> String {
    if seconds < 1e-9 {
        format!("{:.3} ps", seconds / 1e-12)
    } else if seconds < 1e-6 {
        format!("{:.3} ns", seconds / 1e-9)
    } else if seconds < 1e-3 {
        format!("{:.3} μs", seconds / 1e-6)
    } else if seconds < 1.0 {
        format!("{:.3} ms", seconds / 1e-3)
    } else if seconds < 60.0 {
        format!("{:.3} s", seconds)
    } else if seconds < 3600.0 {
        format!("{:.0} min {:.0} s", seconds / 60.0, seconds % 60.0)
    } else {
        format!("{:.1} h", seconds / 3600.0)
    }
}

/// Format a large number with SI prefix.
pub fn format_si(value: Scalar) -> String {
    let prefixes = [
        (1e18, "E"), (1e15, "P"), (1e12, "T"), (1e9, "G"),
        (1e6, "M"), (1e3, "k"), (1e0, ""),
        (1e-3, "m"), (1e-6, "μ"), (1e-9, "n"), (1e-12, "p"),
        (1e-15, "f"), (1e-18, "a"),
    ];
    let abs = value.abs();
    for (threshold, prefix) in &prefixes {
        if abs >= *threshold {
            return format!("{:.3} {prefix}", value / threshold);
        }
    }
    format!("{:.3e}", value)
}

/// Linear interpolation between two values.
pub fn lerp(a: Scalar, b: Scalar, t: Scalar) -> Scalar {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Clamp a value between min and max.
pub fn clamp(value: Scalar, min: Scalar, max: Scalar) -> Scalar {
    value.clamp(min, max)
}

/// Simple moving average filter.
#[derive(Debug, Clone)]
pub struct MovingAverage {
    buffer: Vec<Scalar>,
    index: usize,
    sum: Scalar,
    count: usize,
}

impl MovingAverage {
    pub fn new(window_size: usize) -> Self {
        Self {
            buffer: vec![0.0; window_size],
            index: 0,
            sum: 0.0,
            count: 0,
        }
    }

    pub fn push(&mut self, value: Scalar) -> Scalar {
        if self.count < self.buffer.len() {
            self.sum += value;
            self.buffer[self.index] = value;
            self.count += 1;
        } else {
            let old = self.buffer[self.index];
            self.sum = self.sum - old + value;
            self.buffer[self.index] = value;
        }
        self.index = (self.index + 1) % self.buffer.len();
        self.average()
    }

    pub fn average(&self) -> Scalar {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as Scalar
        }
    }

    pub fn reset(&mut self) {
        self.index = 0;
        self.sum = 0.0;
        self.count = 0;
    }
}

/// A simple logger for simulation events.
#[derive(Debug)]
pub struct SimulationLogger {
    pub level: LogLevel,
    entries: Vec<LogEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn should_log(&self, level: LogLevel) -> bool {
        *self as u8 <= level as u8
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub time: Scalar,
    pub message: String,
}

impl SimulationLogger {
    pub fn new(level: LogLevel) -> Self {
        Self { level, entries: Vec::new() }
    }

    pub fn log(&mut self, level: LogLevel, time: Scalar, message: &str) {
        if self.level.should_log(level) {
            self.entries.push(LogEntry { level, time, message: message.to_string() });
        }
    }

    pub fn info(&mut self, time: Scalar, message: &str) {
        self.log(LogLevel::Info, time, message);
    }

    pub fn warn(&mut self, time: Scalar, message: &str) {
        self.log(LogLevel::Warn, time, message);
    }

    pub fn error(&mut self, time: Scalar, message: &str) {
        self.log(LogLevel::Error, time, message);
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
