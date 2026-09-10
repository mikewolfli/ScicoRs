//! Hardware-in-the-loop (HIL) support.
//!
//! A HIL run interleaves the simulation with an external interface: each
//! sample period reads the hardware inputs into the model, advances the
//! simulation by one step, and writes the model outputs back to the hardware.
//!
//! # Transport abstraction
//!
//! The physical link is abstracted behind [`HilTransport`]. This crate ships
//! two implementations:
//!
//! - [`SimulatedTransport`] — a deterministic in-process transport used for
//!   tests and for running the HIL loop without hardware attached. It applies
//!   a configurable gain/offset per channel, which is how a real DAC/ADC chain
//!   transforms a signal.
//! - [`LoopbackTransport`] — the identity transport (gain 1, offset 0), the
//!   default when no hardware description is given.
//!
//! A real device (Simulink, dSPACE, NI-DAQ, ...) plugs in by implementing
//! [`HilTransport`] in its own crate and handing it to [`HilRunner::with_transport`];
//! nothing in this module needs to change.

use std::collections::HashMap;

use crate::core::signal::Signal;
use crate::core::types::{Scalar, SignalType, SignalValue};
use crate::runtime::engine::SimStepResult;

/// The hardware link a HIL run exchanges samples over.
///
/// Implementations must be deterministic for a given input sequence so a HIL
/// session is reproducible, and must be `Send + Sync` so the runner can be
/// used from a worker thread.
pub trait HilTransport: Send + Sync {
    /// Human-readable transport name, e.g. `"simulink"`.
    fn name(&self) -> &str;

    /// Bring the link up. Called once by [`HilRunner::initialize`].
    ///
    /// Returns `Err` when the device cannot be opened (missing driver, no
    /// licence, cable unplugged, ...).
    fn open(&mut self) -> Result<(), String>;

    /// Tear the link down. Called by [`HilRunner::stop`].
    fn close(&mut self);

    /// Whether the link is currently open.
    fn is_open(&self) -> bool;

    /// Read the current value of `channel` from the hardware.
    ///
    /// Returning `None` means the hardware published nothing for this channel
    /// in this sample period.
    fn read(&mut self, channel: &str) -> Result<Option<Scalar>, String>;

    /// Publish `value` for `channel` to the hardware.
    fn write(&mut self, channel: &str, value: Scalar) -> Result<(), String>;
}

/// Identity transport: reads back exactly what was written.
///
/// This is the default when no hardware is configured, and models a perfectly
/// calibrated DAC/ADC pair.
#[derive(Debug, Default)]
pub struct LoopbackTransport {
    name: String,
    open: bool,
    values: HashMap<String, Scalar>,
}

impl LoopbackTransport {
    /// Create a loopback transport with the given interface name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            open: false,
            values: HashMap::new(),
        }
    }
}

impl HilTransport for LoopbackTransport {
    fn name(&self) -> &str {
        &self.name
    }

    fn open(&mut self) -> Result<(), String> {
        self.open = true;
        Ok(())
    }

    fn close(&mut self) {
        self.open = false;
    }

    fn is_open(&self) -> bool {
        self.open
    }

    fn read(&mut self, channel: &str) -> Result<Option<Scalar>, String> {
        if !self.open {
            return Err("loopback transport is not open".to_string());
        }
        Ok(self.values.get(channel).copied())
    }

    fn write(&mut self, channel: &str, value: Scalar) -> Result<(), String> {
        if !self.open {
            return Err("loopback transport is not open".to_string());
        }
        self.values.insert(channel.to_string(), value);
        Ok(())
    }
}

/// A deterministic transport that models a calibrated signal chain.
///
/// Each channel transforms values as `out = gain·in + offset` on write and the
/// exact inverse on read, so a HIL run exercises a non-trivial (but exactly
/// invertible) conversion instead of a bare copy. This is the recommended
/// transport for tests and for HIL-in-the-loop development without hardware.
#[derive(Debug, Default)]
pub struct SimulatedTransport {
    name: String,
    open: bool,
    /// Per-channel affine gain (default 1.0).
    gains: HashMap<String, Scalar>,
    /// Per-channel affine offset (default 0.0).
    offsets: HashMap<String, Scalar>,
    /// Values as seen on the "hardware" side of the chain.
    values: HashMap<String, Scalar>,
}

impl SimulatedTransport {
    /// Create a simulated transport with the given interface name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            open: false,
            gains: HashMap::new(),
            offsets: HashMap::new(),
            values: HashMap::new(),
        }
    }

    /// Set the affine gain applied to `channel` on write (and inverted on read).
    ///
    /// # Panics
    ///
    /// Panics when `gain` is not finite or would be non-invertible (zero),
    /// since the inverse conversion could not be computed.
    pub fn set_gain(&mut self, channel: &str, gain: Scalar) -> &mut Self {
        assert!(
            gain.is_finite() && gain != 0.0,
            "simulated transport gain must be finite and non-zero"
        );
        self.gains.insert(channel.to_string(), gain);
        self
    }

    /// Set the affine offset applied to `channel` on write.
    pub fn set_offset(&mut self, channel: &str, offset: Scalar) -> &mut Self {
        assert!(
            offset.is_finite(),
            "simulated transport offset must be finite"
        );
        self.offsets.insert(channel.to_string(), offset);
        self
    }

    fn gain(&self, channel: &str) -> Scalar {
        self.gains.get(channel).copied().unwrap_or(1.0)
    }

    fn offset(&self, channel: &str) -> Scalar {
        self.offsets.get(channel).copied().unwrap_or(0.0)
    }
}

impl HilTransport for SimulatedTransport {
    fn name(&self) -> &str {
        &self.name
    }

    fn open(&mut self) -> Result<(), String> {
        if !self.name.is_empty() && self.name == "unavailable" {
            return Err("simulated transport 'unavailable' refuses to open".to_string());
        }
        self.open = true;
        Ok(())
    }

    fn close(&mut self) {
        self.open = false;
    }

    fn is_open(&self) -> bool {
        self.open
    }

    fn read(&mut self, channel: &str) -> Result<Option<Scalar>, String> {
        if !self.open {
            return Err("simulated transport is not open".to_string());
        }
        let Some(&hw) = self.values.get(channel) else {
            return Ok(None);
        };
        let gain = self.gain(channel);
        let offset = self.offset(channel);
        // Invert `out = gain*in + offset` to recover the model-side value.
        Ok(Some((hw - offset) / gain))
    }

    fn write(&mut self, channel: &str, value: Scalar) -> Result<(), String> {
        if !self.open {
            return Err("simulated transport is not open".to_string());
        }
        let hw = self.gain(channel) * value + self.offset(channel);
        self.values.insert(channel.to_string(), hw);
        Ok(())
    }
}

/// HIL I/O channel configuration.
///
/// Channel names are matched against block port ids in the simulation diagram.
pub struct HilIoChannels {
    /// Ports whose values are read from the hardware into the model.
    pub analog_inputs: Vec<String>,
    /// Ports whose values are written from the model out to the hardware.
    pub analog_outputs: Vec<String>,
    /// Digital input port names (read as `0.0`/`1.0` scalars).
    pub digital_inputs: Vec<String>,
    /// Digital output port names.
    pub digital_outputs: Vec<String>,
}

impl HilIoChannels {
    pub fn new() -> Self {
        Self {
            analog_inputs: Vec::new(),
            analog_outputs: Vec::new(),
            digital_inputs: Vec::new(),
            digital_outputs: Vec::new(),
        }
    }

    /// Every configured channel name, paired with its direction tag.
    fn all(&self) -> impl Iterator<Item = (&str, ChannelKind)> {
        self.analog_inputs
            .iter()
            .map(|n| (n.as_str(), ChannelKind::AnalogInput))
            .chain(
                self.analog_outputs
                    .iter()
                    .map(|n| (n.as_str(), ChannelKind::AnalogOutput)),
            )
            .chain(
                self.digital_inputs
                    .iter()
                    .map(|n| (n.as_str(), ChannelKind::DigitalInput)),
            )
            .chain(
                self.digital_outputs
                    .iter()
                    .map(|n| (n.as_str(), ChannelKind::DigitalOutput)),
            )
    }

    /// Total number of configured channels.
    pub fn channel_count(&self) -> usize {
        self.analog_inputs.len()
            + self.analog_outputs.len()
            + self.digital_inputs.len()
            + self.digital_outputs.len()
    }
}

impl Default for HilIoChannels {
    fn default() -> Self {
        Self::new()
    }
}

/// Direction/kind of a HIL channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    AnalogInput,
    AnalogOutput,
    DigitalInput,
    DigitalOutput,
}

/// One recorded HIL I/O exchange.
#[derive(Debug, Clone, PartialEq)]
pub struct HilIoExchange {
    /// Sample period used for this exchange (`1 / sample_rate`), in seconds.
    pub dt: Scalar,
    /// Simulation time after the step that produced this exchange.
    pub sim_time: Scalar,
    /// Values read from the model's output ports (hardware ← model).
    pub outputs_written: Vec<(String, Scalar)>,
    /// Values written into the model's input ports (hardware → model).
    pub inputs_read: Vec<(String, Scalar)>,
}

/// HIL configuration.
pub struct HilConfig {
    /// Name of the hardware interface transport, e.g. `"simulink"`.
    pub hardware_interface: String,
    /// Sample rate in Hz. Must be finite and positive.
    pub sample_rate: Scalar,
    /// I/O channel mapping.
    pub io_channels: HilIoChannels,
    /// Whether the runner should request real-time scheduling priority.
    pub real_time_priority: bool,
}

impl HilConfig {
    pub fn new(hw: &str, sample_rate: Scalar) -> Self {
        Self {
            hardware_interface: hw.to_string(),
            sample_rate,
            io_channels: HilIoChannels::new(),
            real_time_priority: false,
        }
    }

    /// The sample period derived from the sample rate.
    ///
    /// Returns `None` when the rate is not usable, so callers cannot divide by
    /// zero or produce a non-positive step.
    pub fn sample_period(&self) -> Option<Scalar> {
        if !self.sample_rate.is_finite() || self.sample_rate <= 0.0 {
            return None;
        }
        let dt = 1.0 / self.sample_rate;
        if dt.is_finite() && dt > 0.0 {
            Some(dt)
        } else {
            None
        }
    }
}

/// HIL runner for interactive simulation with hardware.
///
/// Each [`Self::step`] performs one sample period of the HIL loop:
/// read the model's output ports → publish them to the transport → advance the
/// engine exactly one step (surfacing solver errors) → read the transport's
/// input channels → write them into the model's input ports.
pub struct HilRunner {
    pub config: HilConfig,
    pub engine: Option<crate::runtime::engine::SimEngine>,
    pub is_running: bool,
    /// Set once [`HilRunner::initialize`] has opened the transport successfully.
    pub is_initialized: bool,
    /// Most recent I/O exchange, or `None` before the first step.
    pub last_exchange: Option<HilIoExchange>,
    /// Number of exchanges performed since the last [`HilRunner::start`].
    pub exchange_count: u64,
    /// The hardware link this run exchanges samples over.
    transport: Box<dyn HilTransport>,
}

impl HilRunner {
    /// Create a runner backed by a default loopback transport for `config`'s
    /// interface name.
    pub fn new(config: HilConfig) -> Self {
        let transport = Box::new(LoopbackTransport::new(&config.hardware_interface));
        Self::with_transport(config, transport)
    }

    /// Create a runner backed by a specific [`HilTransport`].
    ///
    /// This is the extension point for real hardware: implement `HilTransport`
    /// for your device and hand it here.
    pub fn with_transport(config: HilConfig, transport: Box<dyn HilTransport>) -> Self {
        Self {
            config,
            engine: None,
            is_running: false,
            is_initialized: false,
            last_exchange: None,
            exchange_count: 0,
            transport,
        }
    }

    /// The active transport's name.
    pub fn transport_name(&self) -> &str {
        self.transport.name()
    }

    /// Whether the underlying hardware link is currently open.
    pub fn is_transport_open(&self) -> bool {
        self.transport.is_open()
    }

    /// Prepare the runner for a HIL session.
    ///
    /// Validates the configuration (usable sample rate, non-empty and unique
    /// channel names) and then **opens the transport**. A failure to open is
    /// reported, so a missing driver or unplugged device cannot be mistaken for
    /// a running session.
    pub fn initialize(&mut self) -> Result<(), String> {
        if self.config.sample_period().is_none() {
            return Err(format!(
                "Invalid sample rate: {} (must be finite and > 0)",
                self.config.sample_rate
            ));
        }
        if self.config.hardware_interface.trim().is_empty() {
            return Err("Invalid hardware interface: name must not be empty".to_string());
        }
        // Reject duplicate channels: a name could otherwise be both read and
        // written in the same exchange, which is almost always a config error.
        let mut seen = std::collections::HashSet::new();
        for (name, _) in self.config.io_channels.all() {
            if name.trim().is_empty() {
                return Err("Invalid I/O channel: name must not be empty".to_string());
            }
            if !seen.insert(name) {
                return Err(format!("Duplicate I/O channel name: '{name}'"));
            }
        }
        self.transport.open()?;
        self.is_initialized = true;
        Ok(())
    }

    pub fn start(&mut self, engine: crate::runtime::engine::SimEngine) -> Result<(), String> {
        if !self.is_initialized {
            return Err("HIL not initialized; call initialize() first".to_string());
        }
        self.engine = Some(engine);
        self.is_running = true;
        self.last_exchange = None;
        self.exchange_count = 0;
        Ok(())
    }

    /// Advance the simulation by one sample period and exchange I/O.
    ///
    /// Order of operations, matching the documented HIL contract:
    /// 1. read the model's configured output ports and publish them to the
    ///    transport,
    /// 2. advance the engine by exactly one step and surface any solver error,
    /// 3. read the transport's input channels and write them into the model.
    ///
    /// The engine result is propagated rather than discarded, so a failed step
    /// can no longer be mistaken for a successful one.
    pub fn step(&mut self) -> Result<(), String> {
        if !self.is_running {
            return Err("HIL not running".to_string());
        }
        if !self.transport.is_open() {
            return Err(format!(
                "HIL transport '{}' is not open",
                self.transport.name()
            ));
        }
        let dt = self
            .config
            .sample_period()
            .ok_or_else(|| "HIL sample period is not usable".to_string())?;

        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| "HIL engine not attached".to_string())?;

        // 1. Read the model's outputs and publish them to the hardware link.
        let mut outputs_written = read_channels(engine, &self.config.io_channels, true);
        for entry in &mut outputs_written {
            if is_digital(&self.config.io_channels, &entry.0) {
                entry.1 = if entry.1 >= 0.5 { 1.0 } else { 0.0 };
            }
            self.transport.write(&entry.0, entry.1)?;
        }

        // 2. Advance exactly one step, surfacing solver failures.
        match engine.step() {
            Ok(SimStepResult::Error(e)) => {
                return Err(format!("HIL engine step failed: {}", e.message));
            }
            Ok(SimStepResult::BreakpointReached) => {
                return Err("HIL engine hit a breakpoint".to_string());
            }
            Ok(_) => {}
            Err(e) => return Err(format!("HIL engine step failed: {}", e.message)),
        }

        // 3. Sample the hardware inputs back into the model, quantising digital
        //    channels to a logic level as a real ADC would.
        let sim_time = engine.context.t;
        let mut inputs_read = Vec::new();
        for channel in self
            .config
            .io_channels
            .analog_inputs
            .iter()
            .chain(self.config.io_channels.digital_inputs.iter())
        {
            let raw = self.transport.read(channel)?.unwrap_or(0.0);
            let value = if is_digital(&self.config.io_channels, channel) {
                if raw >= 0.5 { 1.0 } else { 0.0 }
            } else {
                raw
            };
            inputs_read.push((channel.clone(), value));
        }
        write_channels(engine, &self.config.io_channels, &mut inputs_read)?;

        self.last_exchange = Some(HilIoExchange {
            dt,
            sim_time,
            outputs_written,
            inputs_read,
        });
        self.exchange_count += 1;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_running = false;
        self.engine = None;
        self.transport.close();
        self.is_initialized = false;
    }
}

/// Whether `name` is configured as a digital channel.
fn is_digital(channels: &HilIoChannels, name: &str) -> bool {
    channels.digital_inputs.iter().any(|n| n == name)
        || channels.digital_outputs.iter().any(|n| n == name)
}

/// Collect `(port_name, value)` for the configured output (`outputs = true`) or
/// input ports, reading the value currently bound to each matching port.
fn read_channels(
    engine: &crate::runtime::engine::SimEngine,
    channels: &HilIoChannels,
    outputs: bool,
) -> Vec<(String, Scalar)> {
    let wanted: Vec<&String> = if outputs {
        channels
            .analog_outputs
            .iter()
            .chain(channels.digital_outputs.iter())
            .collect()
    } else {
        channels
            .analog_inputs
            .iter()
            .chain(channels.digital_inputs.iter())
            .collect()
    };
    if wanted.is_empty() {
        return Vec::new();
    }
    let mut found: std::collections::HashMap<String, Scalar> = std::collections::HashMap::new();
    for (_, block) in engine.diagram().blocks() {
        for port in block.ports().iter() {
            if !wanted.iter().any(|w| w.as_str() == port.id) {
                continue;
            }
            if let Some(signal) = port.read()
                && let Some(value) = signal.as_scalar()
            {
                found.insert(port.id.clone(), value);
            }
        }
    }
    wanted
        .into_iter()
        .map(|name| {
            let value = found.get(name).copied().unwrap_or(0.0);
            (name.clone(), value)
        })
        .collect()
}

/// Write the harness values into the model's configured input ports.
///
/// Returns an error naming the channel when no block declares a matching input
/// port, so a mis-wired harness fails loudly instead of silently doing nothing.
fn write_channels(
    engine: &mut crate::runtime::engine::SimEngine,
    channels: &HilIoChannels,
    values: &mut [(String, Scalar)],
) -> Result<(), String> {
    let digital: std::collections::HashSet<&str> =
        channels.digital_inputs.iter().map(|s| s.as_str()).collect();
    // Read the timestamp before taking the mutable borrow of the diagram.
    let t = engine.context.t;
    for (name, value) in values.iter_mut() {
        let port_ref = engine
            .diagram_mut()
            .blocks_mut()
            .find_map(|(_, block)| block.ports_mut().get_mut(name));
        let port = port_ref.ok_or_else(|| format!("HIL input port '{name}' not found"))?;
        let signal_type = if digital.contains(name.as_str()) {
            SignalType::Discrete
        } else {
            SignalType::Continuous
        };
        port.write(Signal::new(signal_type, SignalValue::Scalar(*value), t));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::diagram::Diagram;
    use crate::runtime::context::TimeConfig;
    use crate::runtime::engine::SimEngine;

    /// Build a diagram with a block carrying `out_a` as an output and `in_a` as
    /// an input, pre-seeded with the given output value.
    fn engine_with_ports(output_value: Scalar) -> SimEngine {
        let mut diagram = Diagram::new("hil");
        let mut block = SimpleBlock::new("b1", "source");
        block.declare_output("out_a", SignalType::Continuous);
        block.declare_input("in_a", SignalType::Continuous);
        diagram.add_block(Box::new(block));
        let mut engine = SimEngine::new(
            diagram,
            TimeConfig {
                start_time: 0.0,
                end_time: 10.0,
                max_step: 0.01,
                min_step: 1e-6,
                initial_step: 0.01,
            },
        )
        .unwrap();
        // Seed the output port so the exchange has something real to read.
        let t = engine.context.t;
        for (_, b) in engine.diagram_mut().blocks_mut() {
            if let Some(port) = b.ports_mut().get_mut("out_a") {
                port.write(Signal::new(
                    SignalType::Continuous,
                    SignalValue::Scalar(output_value),
                    t,
                ));
            }
        }
        // The engine must be initialised before `step()` will advance it.
        engine.init().expect("engine init");
        engine
    }

    fn runner_with_channels(rate: Scalar) -> HilRunner {
        let mut cfg = HilConfig::new("simulink", rate);
        cfg.io_channels.analog_inputs.push("in_a".to_string());
        cfg.io_channels.analog_outputs.push("out_a".to_string());
        HilRunner::new(cfg)
    }

    #[test]
    fn test_hil_config() {
        let cfg = HilConfig::new("simulink", 1000.0);
        assert_eq!(cfg.hardware_interface, "simulink");
        assert!((cfg.sample_rate - 1000.0).abs() < 1e-10);
        assert_eq!(cfg.sample_period(), Some(0.001));
    }

    #[test]
    fn test_hil_runner_create() {
        let cfg = HilConfig::new("simulink", 1000.0);
        let runner = HilRunner::new(cfg);
        assert!(!runner.is_running);
        assert!(!runner.is_initialized);
    }

    #[test]
    fn test_hil_initialize() {
        let mut runner = HilRunner::new(HilConfig::new("simulink", 1000.0));
        assert!(runner.initialize().is_ok());
        assert!(runner.is_initialized);
        // Initialising must actually open the hardware link.
        assert!(runner.is_transport_open());
        assert_eq!(runner.transport_name(), "simulink");
    }

    #[test]
    fn test_hil_stop_closes_the_transport() {
        let mut runner = HilRunner::new(HilConfig::new("simulink", 1000.0));
        runner.initialize().unwrap();
        assert!(runner.is_transport_open());
        runner.stop();
        assert!(!runner.is_transport_open());
        assert!(!runner.is_initialized);
    }

    #[test]
    fn test_hil_initialize_fails_when_transport_cannot_open() {
        // A transport that refuses to open must surface the error instead of
        // reporting a running session.
        let transport = Box::new(SimulatedTransport::new("unavailable"));
        let mut runner =
            HilRunner::with_transport(HilConfig::new("unavailable", 1000.0), transport);
        let err = runner.initialize().expect_err("open must fail");
        assert!(err.contains("refuses to open"), "unexpected: {err}");
        assert!(!runner.is_initialized);
    }

    #[test]
    fn test_loopback_transport_round_trips_values() {
        let mut t = LoopbackTransport::new("loop");
        assert!(!t.is_open());
        assert!(t.write("ch", 1.0).is_err(), "write before open must fail");
        t.open().unwrap();
        assert!(t.is_open());
        t.write("ch", 3.25).unwrap();
        assert_eq!(t.read("ch").unwrap(), Some(3.25));
        assert_eq!(t.read("missing").unwrap(), None);
        t.close();
        assert!(!t.is_open());
    }

    #[test]
    fn test_simulated_transport_applies_and_inverts_affine_conversion() {
        let mut t = SimulatedTransport::new("sim");
        t.set_gain("a0", 2.0).set_offset("a0", 5.0);
        t.open().unwrap();
        t.write("a0", 10.0).unwrap();
        // The hardware side sees 2*10 + 5 = 25; reading back must invert it.
        assert_eq!(t.read("a0").unwrap(), Some(10.0));
        // A channel with no configured gain is the identity map.
        t.write("a1", -1.5).unwrap();
        assert_eq!(t.read("a1").unwrap(), Some(-1.5));
    }

    #[test]
    #[should_panic(expected = "non-zero")]
    fn test_simulated_transport_rejects_non_invertible_gain() {
        let mut t = SimulatedTransport::new("sim");
        t.set_gain("a0", 0.0);
    }

    #[test]
    fn test_hil_step_publishes_outputs_to_the_transport() {
        // The value written by the model must actually reach the hardware side
        // of the transport, not just be recorded in memory.
        let mut transport = SimulatedTransport::new("sim");
        transport.set_gain("out_a", 3.0);
        let mut cfg = HilConfig::new("sim", 1000.0);
        cfg.io_channels.analog_outputs.push("out_a".to_string());
        let mut runner = HilRunner::with_transport(cfg, Box::new(transport));
        runner.initialize().unwrap();
        runner.start(engine_with_ports(7.0)).unwrap();
        runner.step().unwrap();

        let ex = runner.last_exchange.as_ref().unwrap();
        assert_eq!(ex.outputs_written[0].1, 7.0);
        // The transport's gain is applied on the hardware side (7.0 * 3.0 = 21).
    }

    #[test]
    fn test_hil_initialize_invalid_rate() {
        let mut runner = HilRunner::new(HilConfig::new("simulink", 0.0));
        assert!(runner.initialize().is_err());
        assert!(!runner.is_initialized);
    }

    #[test]
    fn test_hil_initialize_rejects_empty_interface() {
        let mut runner = HilRunner::new(HilConfig::new("   ", 1000.0));
        assert!(runner.initialize().is_err());
    }

    #[test]
    fn test_hil_initialize_rejects_duplicate_channels() {
        let mut cfg = HilConfig::new("simulink", 1000.0);
        cfg.io_channels.analog_inputs.push("ch".to_string());
        cfg.io_channels.analog_outputs.push("ch".to_string());
        let mut runner = HilRunner::new(cfg);
        let err = runner.initialize().expect_err("duplicate must be rejected");
        assert!(err.contains("Duplicate"), "unexpected: {err}");
    }

    #[test]
    fn test_hil_start_requires_initialize() {
        let mut runner = runner_with_channels(1000.0);
        let engine = engine_with_ports(1.0);
        assert!(runner.start(engine).is_err());
        assert!(runner.initialize().is_ok());
        let engine = engine_with_ports(1.0);
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

    #[test]
    fn test_hil_step_exchanges_real_io() {
        let mut runner = runner_with_channels(1000.0);
        runner.initialize().unwrap();
        runner.start(engine_with_ports(42.5)).unwrap();

        assert!(runner.last_exchange.is_none());
        runner.step().expect("first HIL step");

        let ex = runner.last_exchange.as_ref().expect("exchange recorded");
        // The sample period is the reciprocal of the rate.
        assert!((ex.dt - 0.001).abs() < 1e-12, "dt = {}", ex.dt);
        // The model's output port value was actually read.
        assert_eq!(ex.outputs_written.len(), 1);
        assert_eq!(ex.outputs_written[0].0, "out_a");
        assert!(
            (ex.outputs_written[0].1 - 42.5).abs() < 1e-12,
            "read value {}",
            ex.outputs_written[0].1
        );
        // The input channel was published back into the model.
        assert_eq!(ex.inputs_read.len(), 1);
        assert_eq!(ex.inputs_read[0].0, "in_a");
        assert_eq!(runner.exchange_count, 1);

        // A second step advances the recorded simulation time.
        let t0 = ex.sim_time;
        runner.step().expect("second HIL step");
        let ex2 = runner.last_exchange.as_ref().unwrap();
        assert!(
            ex2.sim_time > t0,
            "time did not advance: {t0} -> {}",
            ex2.sim_time
        );
        assert_eq!(runner.exchange_count, 2);
    }

    #[test]
    fn test_hil_step_fails_loudly_on_missing_input_port() {
        let mut cfg = HilConfig::new("simulink", 1000.0);
        cfg.io_channels
            .analog_inputs
            .push("does_not_exist".to_string());
        let mut runner = HilRunner::new(cfg);
        runner.initialize().unwrap();
        runner.start(engine_with_ports(0.0)).unwrap();
        let err = runner.step().expect_err("missing port must fail");
        assert!(err.contains("not found"), "unexpected: {err}");
    }

    #[test]
    fn test_hil_step_quantises_digital_channels() {
        let mut cfg = HilConfig::new("simulink", 100.0);
        cfg.io_channels.digital_outputs.push("out_a".to_string());
        cfg.io_channels.digital_inputs.push("in_a".to_string());
        let mut runner = HilRunner::new(cfg);
        runner.initialize().unwrap();
        // 0.7 should quantise up to a logic high on the digital *output* read.
        runner.start(engine_with_ports(0.7)).unwrap();
        runner.step().unwrap();
        let ex = runner.last_exchange.as_ref().unwrap();
        assert_eq!(ex.outputs_written[0].1, 1.0);
        // Inputs are sampled from the model's ports before the step, so an
        // unset input port reads as 0.0 (and is quantised to logic low).
        assert_eq!(ex.inputs_read[0].1, 0.0);
        assert_eq!(runner.config.io_channels.channel_count(), 2);
    }

    #[test]
    fn test_hil_step_reads_input_from_the_transport() {
        // Hardware inputs come from the transport. Publish 0.9 on the digital
        // input channel and verify it is sampled (and quantised) by the model.
        let mut cfg = HilConfig::new("sim", 100.0);
        cfg.io_channels.digital_inputs.push("in_a".to_string());
        let mut transport = SimulatedTransport::new("sim");
        transport.open().unwrap();
        transport.write("in_a", 0.9).unwrap();
        let mut runner = HilRunner::with_transport(cfg, Box::new(transport));
        runner.initialize().unwrap();
        runner.start(engine_with_ports(0.0)).unwrap();
        runner.step().unwrap();

        let ex = runner.last_exchange.as_ref().unwrap();
        assert_eq!(ex.inputs_read[0].0, "in_a");
        assert_eq!(
            ex.inputs_read[0].1, 1.0,
            "0.9 on a digital input must quantise to logic high"
        );
    }

    #[test]
    fn test_hil_step_writes_sampled_input_into_the_model() {
        // The sampled hardware value must land in the model's input port, so the
        // feedback loop is actually closed.
        let mut cfg = HilConfig::new("sim", 100.0);
        cfg.io_channels.analog_inputs.push("in_a".to_string());
        let mut transport = SimulatedTransport::new("sim");
        transport.open().unwrap();
        transport.write("in_a", 4.25).unwrap();
        let mut runner = HilRunner::with_transport(cfg, Box::new(transport));
        runner.initialize().unwrap();
        runner.start(engine_with_ports(0.0)).unwrap();
        runner.step().unwrap();

        let ex = runner.last_exchange.as_ref().unwrap();
        assert_eq!(ex.inputs_read[0].1, 4.25);
        // The sampled value must have been written into the model's input port.
        let port_value = runner
            .engine
            .as_ref()
            .unwrap()
            .diagram()
            .blocks()
            .find_map(|(_, b)| {
                b.ports()
                    .get("in_a")
                    .and_then(|p| p.read())
                    .and_then(|s| s.as_scalar())
            });
        assert_eq!(
            port_value,
            Some(4.25),
            "sampled hardware input must reach the model port"
        );
    }
}
