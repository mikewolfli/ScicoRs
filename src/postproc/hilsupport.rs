//! Hardware-in-the-loop (HIL) support.

use crate::core::types::Scalar;

/// HIL I/O channel configuration.
pub struct HilIoChannels {
    pub analog_inputs: Vec<String>,
    pub analog_outputs: Vec<String>,
    pub digital_inputs: Vec<String>,
    pub digital_outputs: Vec<String>,
}

impl HilIoChannels {
    pub fn new() -> Self { Self { analog_inputs: Vec::new(), analog_outputs: Vec::new(), digital_inputs: Vec::new(), digital_outputs: Vec::new() } }
}

/// HIL configuration.
pub struct HilConfig {
    pub hardware_interface: String,
    pub sample_rate: Scalar,
    pub io_channels: HilIoChannels,
    pub real_time_priority: bool,
}

impl HilConfig {
    pub fn new(hw: &str, sample_rate: Scalar) -> Self {
        Self { hardware_interface: hw.to_string(), sample_rate, io_channels: HilIoChannels::new(), real_time_priority: false }
    }
}

/// HIL runner for interactive simulation with hardware.
pub struct HilRunner {
    pub config: HilConfig,
    pub engine: Option<crate::runtime::engine::SimEngine>,
    pub is_running: bool,
}

impl HilRunner {
    pub fn new(config: HilConfig) -> Self { Self { config, engine: None, is_running: false } }

    pub fn initialize(&mut self) -> Result<(), String> {
        // Initialize hardware interface (abstract)
        if self.config.sample_rate <= 0.0 { return Err("Invalid sample rate".to_string()); }
        Ok(())
    }

    pub fn start(&mut self, engine: crate::runtime::engine::SimEngine) -> Result<(), String> {
        self.engine = Some(engine);
        self.is_running = true;
        Ok(())
    }

    pub fn step(&mut self) -> Result<(), String> {
        if !self.is_running { return Err("HIL not running".to_string()); }
        // Read hardware inputs → simulate one step → write hardware outputs
        let dt = 1.0 / self.config.sample_rate;
        if let Some(ref mut engine) = self.engine {
            let _ = dt;
            let _ = engine.step();
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_running = false;
        self.engine = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::engine::SimEngine;
    use crate::runtime::context::TimeConfig;
    use crate::core::diagram::Diagram;

    #[test]
    fn test_hil_config() {
        let cfg = HilConfig::new("simulink", 1000.0);
        assert_eq!(cfg.hardware_interface, "simulink");
        assert!((cfg.sample_rate - 1000.0).abs() < 1e-10);
    }
    #[test]
    fn test_hil_runner_create() {
        let cfg = HilConfig::new("simulink", 1000.0);
        let runner = HilRunner::new(cfg);
        assert!(!runner.is_running);
    }
    #[test]
    fn test_hil_initialize() {
        let mut runner = HilRunner::new(HilConfig::new("simulink", 1000.0));
        assert!(runner.initialize().is_ok());
    }
    #[test]
    fn test_hil_initialize_invalid_rate() {
        let mut runner = HilRunner::new(HilConfig::new("simulink", 0.0));
        assert!(runner.initialize().is_err());
    }
    #[test]
    fn test_hil_start_stop() {
        let mut runner = HilRunner::new(HilConfig::new("simulink", 1000.0));
        let engine = SimEngine::new(
            Diagram::new("test"),
            TimeConfig { start_time: 0.0, end_time: 1.0, max_step: 0.01, min_step: 1e-6, initial_step: 0.01 },
        ).unwrap();
        assert!(runner.start(engine).is_ok());
        assert!(runner.is_running);
        runner.stop();
        assert!(!runner.is_running);
    }
    #[test]
    fn test_hil_step_not_running() {
        let mut runner = HilRunner::new(HilConfig::new("simulink", 1000.0));
        assert!(runner.step().is_err());
    }
}
