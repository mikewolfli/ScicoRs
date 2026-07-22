//! Simulation context — centralized time, mode, lifecycle, and shared data.
//!
//! The `SimContext` is the central nervous system of a running simulation.
//! It owns the current time, step size, run mode, lifecycle state, a shared
//! data map for inter-block communication, a log buffer, and error tracking.
//! Every block and engine operation has access to the context.

use crate::core::error::SimError;
use crate::core::types::{SignalValue, Time, EPSILON};
use std::fmt;
use std::sync::Arc;

/// Simulation execution mode.
#[derive(Clone)]
pub enum SimRunMode {
    /// Normal continuous execution at fixed/variable step.
    Normal,
    /// Wall-clock synchronized execution at the given time scale factor.
    /// e.g. 2.0 = run at 2x realtime, 0.5 = run at half speed.
    RealTime { time_scale: f64 },
    /// Execute one step, then pause automatically.
    SingleStep,
    /// Paused — no time advancement.
    Paused,
    /// Running until a breakpoint condition is met.
    Breakpoint {
        condition: Arc<dyn Fn(&SimContext) -> bool + Send + Sync>,
    },
}
impl PartialEq for SimRunMode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Normal, Self::Normal) => true,
            (Self::RealTime { time_scale: a }, Self::RealTime { time_scale: b }) => (a - b).abs() < EPSILON,
            (Self::SingleStep, Self::SingleStep) => true,
            (Self::Paused, Self::Paused) => true,
            _ => false,
        }
    }
}
impl fmt::Debug for SimRunMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => write!(f, "Normal"),
            Self::RealTime { time_scale } => f.debug_struct("RealTime")
                .field("time_scale", time_scale).finish(),
            Self::SingleStep => write!(f, "SingleStep"),
            Self::Paused => write!(f, "Paused"),
            Self::Breakpoint { .. } => write!(f, "Breakpoint(<condition>)"),
        }
    }
}

impl SimRunMode {
    /// Returns `true` if time should advance in this mode.
    pub fn advances_time(&self) -> bool {
        matches!(self, Self::Normal | Self::RealTime { .. } | Self::SingleStep | Self::Breakpoint { .. })
    }

    /// Returns `true` if this mode automatically stops after one step.
    pub fn is_single_step(&self) -> bool {
        matches!(self, Self::SingleStep)
    }
}

/// Simulation lifecycle finite state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum SimLifecycle {
    /// Diagram constructed, not yet initialized.
    Constructed,
    /// All blocks initialized, ready to run.
    Initialized,
    /// Simulation is running (advancing time).
    Running,
    /// Simulation is paused.
    Paused,
    /// Simulation has completed (reached end_time or all blocks completed).
    Completed,
    /// Simulation encountered an unrecoverable error.
    Error(String),
}

impl SimLifecycle {
    /// Returns `true` if the simulation is in an active state (can be stepped).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }

    /// Returns `true` if the simulation has terminated.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Error(_))
    }
}

/// A single log entry in the simulation log buffer.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Simulation time when this entry was logged.
    pub time: Time,
    /// Step count when this entry was logged.
    pub step: u64,
    /// Log level.
    pub level: LogLevel,
    /// The log message.
    pub message: String,
}

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

/// Time configuration for a simulation run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeConfig {
    /// Simulation start time in seconds.
    pub start_time: Time,
    /// Simulation end time in seconds.
    pub end_time: Time,
    /// Maximum allowed step size in seconds.
    pub max_step: Time,
    /// Minimum allowed step size in seconds.
    pub min_step: Time,
    /// Initial step size in seconds.
    pub initial_step: Time,
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            start_time: 0.0,
            end_time: 10.0,
            max_step: 1.0,
            min_step: 1e-12,
            initial_step: 0.01,
        }
    }
}

impl TimeConfig {
    /// Create a new TimeConfig with explicit parameters.
    pub fn new(start_time: Time, end_time: Time, initial_step: Time) -> Self {
        Self {
            start_time,
            end_time,
            max_step: initial_step.max(1.0),
            min_step: 1e-12,
            initial_step,
        }
    }

    /// Create a TimeConfig that only specifies end_time (uses defaults for rest).
    pub fn until(end_time: Time) -> Self {
        Self {
            end_time,
            ..Default::default()
        }
    }

    /// Validate the configuration; returns errors if invalid.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.end_time <= self.start_time {
            errors.push(format!(
                "end_time ({}) must be greater than start_time ({})",
                self.end_time, self.start_time
            ));
        }
        if self.max_step <= 0.0 {
            errors.push("max_step must be positive".to_string());
        }
        if self.min_step <= 0.0 {
            errors.push("min_step must be positive".to_string());
        }
        if self.min_step > self.max_step {
            errors.push("min_step must not exceed max_step".to_string());
        }
        if self.initial_step <= 0.0 {
            errors.push("initial_step must be positive".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Total simulation duration.
    pub fn duration(&self) -> Time {
        self.end_time - self.start_time
    }
}

/// Centralized simulation context holding time, mode, lifecycle, and shared data.
#[derive(Debug, Clone)]
pub struct SimContext {
    // ── Time management ──
    /// Current simulation time.
    pub t: Time,
    /// Current step size.
    pub dt: Time,
    /// Time configuration.
    pub config: TimeConfig,
    /// Number of steps executed.
    pub step_count: u64,

    // ── Mode & lifecycle ──
    /// Current execution mode.
    pub mode: SimRunMode,
    /// Current lifecycle state.
    pub lifecycle: SimLifecycle,

    // ── Shared data ──
    shared: std::collections::HashMap<String, SignalValue>,

    // ── Logging ──
    log_buffer: Vec<LogEntry>,

    // ── Error tracking ──
    last_error: Option<SimError>,
}

impl SimContext {
    /// Create a new simulation context with the given time configuration.
    pub fn new(config: TimeConfig) -> Self {
        Self {
            t: config.start_time,
            dt: config.initial_step,
            config,
            step_count: 0,
            mode: SimRunMode::Normal,
            lifecycle: SimLifecycle::Constructed,
            shared: std::collections::HashMap::new(),
            log_buffer: Vec::new(),
            last_error: None,
        }
    }

    /// Create a context with default config and a specific end time.
    pub fn with_end_time(end_time: Time) -> Self {
        Self::new(TimeConfig::until(end_time))
    }

    /// Advance the simulation time by `dt` and increment the step counter.
    /// Returns `true` if the simulation is now finished.
    pub fn advance_time(&mut self) {
        self.t += self.dt;
        self.step_count += 1;
    }

    /// Set the current step size, clamped to [min_step, max_step].
    pub fn set_dt(&mut self, dt: Time) {
        self.dt = dt.clamp(self.config.min_step, self.config.max_step);
    }

    /// Returns `true` if the simulation has reached or passed the end time.
    pub fn is_finished(&self) -> bool {
        self.t >= self.config.end_time - EPSILON
    }

    /// Returns the simulation progress as a fraction [0.0, 1.0].
    pub fn progress(&self) -> f64 {
        let total = self.config.duration();
        if total <= EPSILON {
            return 1.0;
        }
        ((self.t - self.config.start_time) / total).clamp(0.0, 1.0)
    }

    /// Remaining simulation time.
    pub fn remaining_time(&self) -> Time {
        (self.config.end_time - self.t).max(0.0)
    }

    // ── Shared data ──

    /// Set a shared data value.
    pub fn set_shared(&mut self, key: impl Into<String>, value: SignalValue) {
        self.shared.insert(key.into(), value);
    }

    /// Get a shared data value by key.
    pub fn get_shared(&self, key: &str) -> Option<&SignalValue> {
        self.shared.get(key)
    }

    /// Check if a shared key exists.
    pub fn has_shared(&self, key: &str) -> bool {
        self.shared.contains_key(key)
    }

    /// Remove a shared data entry.
    pub fn remove_shared(&mut self, key: &str) -> Option<SignalValue> {
        self.shared.remove(key)
    }

    /// Clear all shared data.
    pub fn clear_shared(&mut self) {
        self.shared.clear();
    }

    // ── Logging ──

    /// Append a log entry.
    pub fn log(&mut self, level: LogLevel, message: impl Into<String>) {
        self.log_buffer.push(LogEntry {
            time: self.t,
            step: self.step_count,
            level,
            message: message.into(),
        });
    }

    /// Convenience: log at INFO level.
    pub fn info(&mut self, msg: impl Into<String>) {
        self.log(LogLevel::Info, msg);
    }

    /// Convenience: log at WARNING level.
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.log(LogLevel::Warning, msg);
    }

    /// Convenience: log at ERROR level.
    pub fn error_log(&mut self, msg: impl Into<String>) {
        self.log(LogLevel::Error, msg);
    }

    /// Access the log buffer.
    pub fn logs(&self) -> &[LogEntry] {
        &self.log_buffer
    }

    /// Clear all log entries.
    pub fn clear_log(&mut self) {
        self.log_buffer.clear();
    }

    /// Get log entries filtered by level.
    pub fn logs_by_level(&self, level: LogLevel) -> Vec<&LogEntry> {
        self.log_buffer.iter().filter(|e| e.level == level).collect()
    }

    // ── Error tracking ──

    /// Record the last error.
    pub fn set_error(&mut self, err: SimError) {
        self.last_error = Some(err.clone());
        self.error_log(err.to_string());
        self.lifecycle = SimLifecycle::Error(err.to_string());
    }

    /// Get a reference to the last error.
    pub fn last_error(&self) -> Option<&SimError> {
        self.last_error.as_ref()
    }

    /// Clear the last error.
    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    /// Check if the context is in an error state.
    pub fn has_error(&self) -> bool {
        self.last_error.is_some()
    }

    // ── Mode helpers ──

    /// Set the run mode.
    pub fn set_mode(&mut self, mode: SimRunMode) {
        self.mode = mode;
    }

    /// Returns `true` if the current mode advances time.
    pub fn time_advances(&self) -> bool {
        self.mode.advances_time() && self.lifecycle == SimLifecycle::Running
    }

    /// Set the lifecycle state.
    pub fn set_lifecycle(&mut self, state: SimLifecycle) {
        self.lifecycle = state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_config_default() {
        let cfg = TimeConfig::default();
        assert_eq!(cfg.start_time, 0.0);
        assert_eq!(cfg.end_time, 10.0);
        assert_eq!(cfg.initial_step, 0.01);
    }

    #[test]
    fn test_time_config_validation() {
        assert!(TimeConfig::default().validate().is_ok());

        let bad = TimeConfig {
            end_time: 0.0,
            start_time: 1.0,
            ..Default::default()
        };
        assert!(bad.validate().is_err());

        let bad2 = TimeConfig {
            min_step: 2.0,
            max_step: 1.0,
            ..Default::default()
        };
        assert!(bad2.validate().is_err());
    }

    #[test]
    fn test_time_config_until() {
        let cfg = TimeConfig::until(5.0);
        assert_eq!(cfg.end_time, 5.0);
        assert_eq!(cfg.start_time, 0.0);
    }

    #[test]
    fn test_sim_context_creation() {
        let ctx = SimContext::new(TimeConfig::until(1.0));
        assert_eq!(ctx.t, 0.0);
        assert_eq!(ctx.dt, 0.01);
        assert_eq!(ctx.step_count, 0);
        assert_eq!(ctx.lifecycle, SimLifecycle::Constructed);
    }

    #[test]
    fn test_time_advancement() {
        let mut ctx = SimContext::new(TimeConfig::until(1.0));
        ctx.dt = 0.1;
        ctx.advance_time();
        assert!((ctx.t - 0.1).abs() < EPSILON);
        assert_eq!(ctx.step_count, 1);
    }

    #[test]
    fn test_progress() {
        let mut ctx = SimContext::new(TimeConfig::new(0.0, 10.0, 0.1));
        assert!((ctx.progress() - 0.0).abs() < EPSILON);
        ctx.t = 5.0;
        assert!((ctx.progress() - 0.5).abs() < EPSILON);
        ctx.t = 10.0;
        assert!((ctx.progress() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_finished_detection() {
        let mut ctx = SimContext::new(TimeConfig::until(1.0));
        assert!(!ctx.is_finished());
        ctx.t = 1.0;
        assert!(ctx.is_finished());
    }

    #[test]
    fn test_remaining_time() {
        let mut ctx = SimContext::new(TimeConfig::until(10.0));
        assert!((ctx.remaining_time() - 10.0).abs() < EPSILON);
        ctx.t = 3.0;
        assert!((ctx.remaining_time() - 7.0).abs() < EPSILON);
    }

    #[test]
    fn test_shared_data() {
        let mut ctx = SimContext::new(TimeConfig::default());
        ctx.set_shared("gain", SignalValue::Scalar(2.0));
        assert!(ctx.has_shared("gain"));
        assert_eq!(
            ctx.get_shared("gain"),
            Some(&SignalValue::Scalar(2.0))
        );
        ctx.remove_shared("gain");
        assert!(!ctx.has_shared("gain"));
    }

    #[test]
    fn test_logging() {
        let mut ctx = SimContext::new(TimeConfig::default());
        ctx.info("simulation started");
        ctx.warn("temperature high");
        ctx.error_log("critical failure");
        assert_eq!(ctx.logs().len(), 3);
        assert_eq!(ctx.logs_by_level(LogLevel::Info).len(), 1);
        assert_eq!(ctx.logs_by_level(LogLevel::Warning).len(), 1);
        assert_eq!(ctx.logs_by_level(LogLevel::Error).len(), 1);
        ctx.clear_log();
        assert_eq!(ctx.logs().len(), 0);
    }

    #[test]
    fn test_set_dt_clamping() {
        let mut ctx = SimContext::new(TimeConfig::default());
        ctx.set_dt(100.0); // above max_step
        assert!((ctx.dt - ctx.config.max_step).abs() < EPSILON);
        ctx.set_dt(-1.0); // below min_step
        assert!((ctx.dt - ctx.config.min_step).abs() < EPSILON);
        ctx.set_dt(0.5);
        assert!((ctx.dt - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_run_mode_behavior() {
        assert!(SimRunMode::Normal.advances_time());
        assert!(SimRunMode::RealTime { time_scale: 1.0 }.advances_time());
        assert!(SimRunMode::SingleStep.advances_time());
        assert!(SimRunMode::SingleStep.is_single_step());
        assert!(!SimRunMode::Paused.advances_time());
    }

    #[test]
    fn test_lifecycle_transitions() {
        let mut ctx = SimContext::new(TimeConfig::default());
        assert_eq!(ctx.lifecycle, SimLifecycle::Constructed);
        assert!(!ctx.lifecycle.is_active());
        assert!(!ctx.lifecycle.is_terminal());

        ctx.set_lifecycle(SimLifecycle::Running);
        assert!(ctx.lifecycle.is_active());

        ctx.set_lifecycle(SimLifecycle::Completed);
        assert!(ctx.lifecycle.is_terminal());

        ctx.set_lifecycle(SimLifecycle::Error("kaboom".to_string()));
        assert!(ctx.lifecycle.is_terminal());
    }

    #[test]
    fn test_error_tracking() {
        let mut ctx = SimContext::new(TimeConfig::default());
        assert!(!ctx.has_error());
        let err = SimError::runtime("test error");
        ctx.set_error(err.clone());
        assert!(ctx.has_error());
        assert_eq!(ctx.last_error().unwrap().message, "test error");
        assert!(matches!(ctx.lifecycle, SimLifecycle::Error(_)));
        ctx.clear_error();
        assert!(!ctx.has_error());
    }

    #[test]
    fn test_with_end_time_constructor() {
        let ctx = SimContext::with_end_time(100.0);
        assert_eq!(ctx.config.end_time, 100.0);
        assert_eq!(ctx.config.start_time, 0.0);
    }

    #[test]
    fn test_time_config_duration() {
        let cfg = TimeConfig::new(1.0, 11.0, 0.1);
        assert!((cfg.duration() - 10.0).abs() < EPSILON);
    }
}
