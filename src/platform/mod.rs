//! Cross-Platform and System Integration
//!
//! Provides platform abstraction for running the simulation kernel
//! on different operating systems, hardware architectures, and
//! cloud/distributed environments.

use std::time::{Duration, Instant};

/// System information about the current platform.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub cpu_cores: usize,
    pub memory_bytes: u64,
}

impl SystemInfo {
    /// Detect current system information.
    pub fn detect() -> Self {
        Self {
            os_name: std::env::consts::OS.to_string(),
            os_version: String::new(),
            cpu_cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            memory_bytes: 0,
        }
    }
}

/// A high-resolution simulation timer for real-time synchronization.
#[derive(Debug)]
pub struct SimulationTimer {
    start: Instant,
    accumulated: Duration,
    running: bool,
}

impl SimulationTimer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            accumulated: Duration::ZERO,
            running: false,
        }
    }

    pub fn start(&mut self) {
        self.start = Instant::now();
        self.running = true;
    }

    pub fn pause(&mut self) {
        if self.running {
            self.accumulated += self.start.elapsed();
            self.running = false;
        }
    }

    pub fn resume(&mut self) {
        if !self.running {
            self.start = Instant::now();
            self.running = true;
        }
    }

    pub fn elapsed_seconds(&self) -> f64 {
        let total = if self.running {
            self.accumulated + self.start.elapsed()
        } else {
            self.accumulated
        };
        total.as_secs_f64()
    }

    pub fn reset(&mut self) {
        self.accumulated = Duration::ZERO;
        self.start = Instant::now();
        self.running = false;
    }
}

impl Default for SimulationTimer {
    fn default() -> Self {
        Self::new()
    }
}
