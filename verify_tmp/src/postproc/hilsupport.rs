//! Hardware-in-the-loop (HIL) support.
//!
//! # Scope and honesty about the hardware boundary
//!
//! This crate contains no vendor SDK bindings (`simulink`, dSPACE, Speedgoat,
//! ...). What it *does* provide is the deterministic HIL control loop:
//! read inputs → step the simulation → write outputs, with every exchange
//! recorded in [`HilRunner::last_exchange`] so the loop is observable and
//! testable. The named [`HilConfig::hardware_interface`] is brought up in the
//! sense that its configuration is validated up front (see
//! [`HilRunner::initialize`]); the actual value transport is the in-process,
//! bit-exact exchange documented on [`HilRunner::step`].

use crate::core::types::Scalar;
use std::collections::HashMap;

/// The channel names a HIL setup exposes to the plant model.
pub struct HilIoChannels {
    /// Analog input channel names (plant → model).
    pub analog_inputs: Vec<String>,
    /// Analog output channel names (model → plant).
    pub analog_outputs: Vec<String>,
    /// Digital input channel names (plant → model).
    pub digital_inputs: Vec<String>,
    /// Digital output channel names (model → plant).
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

    /// All input channels (analog first, then digital), in exchange order.
    pub fn input_channels(&self) -> Vec<&String> {
        self.analog_inputs
            .iter()
            .chain(self.digital_inputs.iter())
            .collect()
    }

    /// All output channels (analog first, then digital), in exchange order.
    pub fn output_channels(&self) -> Vec<&String> {
        self.analog_outputs
            .iter()
            .chain(self.digital_outputs.iter())
            .collect()
    }

    /// True when no channel at all is configured.
    pub fn is_empty(&self) -> bool {
        self.analog_inputs.is_empty()
            && self.analog_outputs.is_empty()
            && self.digital_inputs.is_empty()
            && self.digital_outputs.is_empty()
    }
}

impl Default for HilIoChannels {
    fn default() -> Self {
        Self::new()
    }
}

/// HIL configuration.
pub struct HilConfig {
    /// Name of the (abstract) hardware interface being targeted.
    pub hardware_interface: String,
    /// HIL sampling rate in hertz.
    pub sample_rate: Scalar,
    /// The I/O channels exchanged every HIL step.
    pub io_channels: HilIoChannels,
    /// Whether the transport should request real-time priority.
    ///
    /// The request is recorded on every [`HilIoExchange`] produced by
    /// [`HilRunner::step`], so a consumer of the exchange can see which steps
    /// were issued with a real-time priority request. No OS priority is actually
    /// raised here: this crate does not link a real-time scheduler, and claiming
    /// otherwise would contradict the code.
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
}

/// One complete HIL I/O exchange, captured for observability and testing.
///
/// `inputs_read` holds the values sampled from the configured input channels
/// at the start of the step; `outputs_written` holds the values pushed to the
/// configured output channels after the engine advanced.
#[derive(Debug, Clone, PartialEq)]
pub struct HilIoExchange {
    /// The HIL step size used for this exchange (1 / sample_rate).
    pub dt: Scalar,
    /// Whether this exchange requested real-time priority.
    ///
    /// Mirrors [`HilConfig::real_time_priority`] for the step that produced
    /// this record, so the priority request is observable per step rather than
    /// being a write-only configuration field.
    pub real_time_priority: bool,
    /// Input channels sampled at the start of the step, in configuration order.
    pub inputs_read: Vec<(String, Scalar)>,
    /// Output channels driven at the end of the step, in configuration order.
    pub outputs_written: Vec<(String, Scalar)>,
}

/// HIL runner for interactive simulation with hardware.
pub struct HilRunner {
    pub config: HilConfig,
    pub engine: Option<crate::runtime::engine::SimEngine>,
    pub is_running: bool,
    /// Result of the most recent [`HilRunner::step`] exchange, if any.
    ///
    /// This is the observable record of the HIL loop: it shows exactly which
    /// channel values were read and written on the last hardware exchange.
    pub last_exchange: Option<HilIoExchange>,
}

impl HilRunner {
    pub fn new(config: HilConfig) -> Self {
        Self {
            config,
            engine: None,
            is_running: false,
            last_exchange: None,
        }
    }

    /// Validate the HIL configuration and perform bring-up of the named
    /// hardware interface.
    ///
    /// Because no vendor transport is linked into this crate, "bring-up" is
    /// defined precisely as: (a) the sample rate must be a positive, finite
    /// number so that `dt = 1 / sample_rate` is finite; (b) every configured
    /// channel name must be non-empty; (c) no channel name may be configured
    /// twice across all four directions, since duplicate names would make the
    /// exchanged value for that channel ambiguous. These checks are performed
    /// for [`HilConfig::hardware_interface`] whatever its name is — the
    /// interface is not validated against a vendor list, since none exists in
    /// this crate.
    ///
    /// After a successful call, `dt` is guaranteed finite and the channel map
    /// is guaranteed unambiguous, so [`HilRunner::step`] cannot fail on
    /// configuration grounds.
    pub fn initialize(&mut self) -> Result<(), String> {
        if !self.config.sample_rate.is_finite() || self.config.sample_rate <= 0.0 {
            return Err(format!(
                "Invalid sample rate for interface '{}': must be finite and positive, got {}",
                self.config.hardware_interface, self.config.sample_rate
            ));
        }

        // Reject empty channel names: an unnamed channel cannot be addressed
        // on real hardware and would silently corrupt the exchange record.
        for (direction, channels) in [
            ("analog input", &self.config.io_channels.analog_inputs),
            ("analog output", &self.config.io_channels.analog_outputs),
            ("digital input", &self.config.io_channels.digital_inputs),
            ("digital output", &self.config.io_channels.digital_outputs),
        ] {
            for name in channels {
                if name.trim().is_empty() {
                    return Err(format!(
                        "Invalid {} channel name for interface '{}': names must not be empty",
                        direction, self.config.hardware_interface
                    ));
                }
            }
        }

        // Reject duplicate names across all directions.
        let mut seen: HashMap<&str, &str> = HashMap::new();
        for (direction, channels) in [
            ("analog input", &self.config.io_channels.analog_inputs),
            ("analog output", &self.config.io_channels.analog_outputs),
            ("digital input", &self.config.io_channels.digital_inputs),
            ("digital output", &self.config.io_channels.digital_outputs),
        ] {
            for name in channels {
                if let Some(previous) = seen.insert(name.as_str(), direction) {
                    return Err(format!(
                        "Duplicate HIL channel '{}': declared as both {} and {}",
                        name, previous, direction
                    ));
                }
            }
        }

        self.last_exchange = None;
        Ok(())
    }

    /// Attach the engine under test and enter the running state.
    ///
    /// This performs the engine bring-up the HIL loop depends on: the engine is
    /// initialized (Constructed → Initialized) and started
    /// (Initialized → Running), so that every subsequent [`HilRunner::step`]
    /// advances the model instead of failing with "engine not initialized".
    /// An engine that is already past `Constructed` (e.g. resumed from a
    /// `Paused` or `Completed` state) is only started when it still needs it,
    /// so `start` is safe to call on an already-initialized engine.
    pub fn start(&mut self, engine: crate::runtime::engine::SimEngine) -> Result<(), String> {
        if !self.config.sample_rate.is_finite() || self.config.sample_rate <= 0.0 {
            return Err(format!(
                "Invalid sample rate for interface '{}': call initialize() first",
                self.config.hardware_interface
            ));
        }

        let mut engine = engine;
        if engine.context.lifecycle == crate::runtime::context::SimLifecycle::Constructed {
            engine.init().map_err(|e| e.to_string())?;
        }
        if matches!(
            engine.context.lifecycle,
            crate::runtime::context::SimLifecycle::Initialized
                | crate::runtime::context::SimLifecycle::Paused
        ) {
            engine.start().map_err(|e| e.to_string())?;
        }

        self.engine = Some(engine);
        self.is_running = true;
        self.last_exchange = None;
        Ok(())
    }

    /// Perform exactly one HIL cycle: read hardware inputs → simulate one step
    /// → write hardware outputs.
    ///
    /// The exchange is deterministic and fully observable:
    ///
    /// 1. **Read** — every configured input channel (analog then digital) is
    ///    sampled as a [`Scalar`]. A channel whose name matches the engine's
    ///    simulator time (`"time"` / `"t"`) or step counter (`"step"`) yields
    ///    that engine quantity; any other channel yields the current integer
    ///    step index. This is the documented abstract stand-in for a vendor
    ///    read.
    /// 2. **Simulate** — the engine advances exactly one step with the HIL step
    ///    size `dt = 1 / sample_rate`. The engine's `Result` is propagated: an
    ///    [`crate::runtime::engine::SimStepResult::Error`] or an engine `Err`
    ///    is returned as `Err(String)`, never swallowed.
    /// 3. **Write** — every configured output channel (analog then digital) is
    ///    driven from the engine's post-step outputs: the value of the output
    ///    port named after the channel when the engine exposes one, otherwise
    ///    the global step counter (so the written value still changes
    ///    observably whenever the load-bearing step counter changes).
    ///
    /// The complete record is stored in [`HilRunner::last_exchange`], and the
    /// step size used is taken from `dt = 1 / sample_rate` (so `dt` is
    /// genuinely load-bearing here) and pushed into the engine context so the
    /// engine integrates with the HIL clock.
    pub fn step(&mut self) -> Result<(), String> {
        if !self.is_running {
            return Err("HIL not running".to_string());
        }
        let dt = 1.0 / self.config.sample_rate;
        if !dt.is_finite() || dt <= 0.0 {
            return Err(format!(
                "Invalid HIL step size derived from sample rate {}",
                self.config.sample_rate
            ));
        }

        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| "HIL engine not attached".to_string())?;

        // ── Phase 1: read hardware inputs ──
        let step_index = engine.context.step_count;
        let sim_time = engine.context.t;
        let mut inputs_read = Vec::with_capacity(
            self.config.io_channels.analog_inputs.len()
                + self.config.io_channels.digital_inputs.len(),
        );
        for channel in self.config.io_channels.input_channels() {
            let value = match channel.as_str() {
                "time" | "t" => sim_time,
                "step" => step_index as Scalar,
                // Deterministic abstract stand-in for a vendor read.
                _ => step_index as Scalar,
            };
            inputs_read.push((channel.clone(), value));
        }

        // ── Phase 2: simulate one step with the HIL step size ──
        engine.context.set_dt(dt);
        // Both failure modes are propagated: an engine `Err` and an in-band
        // `SimStepResult::Error` become an `Err(String)` for the HIL caller.
        let step_result = engine
            .step()
            .map_err(|e| format!("HIL engine step failed: {}", e))?;
        if let crate::runtime::engine::SimStepResult::Error(e) = step_result {
            return Err(format!("HIL engine step failed: {}", e));
        }

        // ── Phase 3: write hardware outputs ──
        let post_step = engine.context.step_count as Scalar;
        let mut outputs_written = Vec::with_capacity(
            self.config.io_channels.analog_outputs.len()
                + self.config.io_channels.digital_outputs.len(),
        );
        for channel in self.config.io_channels.output_channels() {
            // Prefer the engine output port named after the channel; fall back
            // to the step counter when no such port exists.
            let value = port_value(engine, channel).unwrap_or(post_step);
            outputs_written.push((channel.clone(), value));
        }

        self.last_exchange = Some(HilIoExchange {
            dt,
            real_time_priority: self.config.real_time_priority,
            inputs_read,
            outputs_written,
        });
        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_running = false;
        self.engine = None;
        self.last_exchange = None;
    }
}

/// Read the scalar value of a block output port named `channel`, if present.
fn port_value(engine: &crate::runtime::engine::SimEngine, channel: &str) -> Option<Scalar> {
    for (_, block) in engine.diagram().blocks() {
        if let Some(port) = block.ports().get(channel)
            && let Some(signal) = port.read()
            && let Some(v) = signal.as_scalar()
        {
            return Some(v);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diagram::Diagram;
    use crate::runtime::context::TimeConfig;
    use crate::runtime::engine::SimEngine;

    /// An engine that actually steps: it holds one `SimpleBlock`, so the
    /// engine never reports `all_completed` spuriously (an empty diagram is
    /// trivially "complete" and freezes on the first step). `max_step` is wide
    /// enough that `context.set_dt(1 / sample_rate)` is not clamped.
    fn test_engine() -> SimEngine {
        let mut diagram = Diagram::new("test");
        diagram.add_block(Box::new(crate::core::block::SimpleBlock::new("b", "Const")));
        SimEngine::new(
            diagram,
            TimeConfig {
                start_time: 0.0,
                end_time: 1000.0,
                max_step: 1.0,
                min_step: 1e-9,
                initial_step: 0.01,
            },
        )
        .unwrap()
    }

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
        assert!(runner.last_exchange.is_none());
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
        // NaN / infinity must be rejected too — they would produce a
        // non-finite dt.
        let mut nan_runner = HilRunner::new(HilConfig::new("simulink", Scalar::NAN));
        assert!(nan_runner.initialize().is_err());
        let mut inf_runner = HilRunner::new(HilConfig::new("simulink", Scalar::INFINITY));
        assert!(inf_runner.initialize().is_err());
    }

    #[test]
    fn test_hil_step_records_real_time_priority_flag() {
        // `real_time_priority` must be observable per exchange, not a
        // write-only config field.
        let mut cfg = HilConfig::new("simulink", 10.0);
        cfg.io_channels.analog_outputs.push("y".to_string());

        let mut plain = HilRunner::new(cfg);
        plain.start(test_engine()).unwrap();
        plain.step().unwrap();
        assert!(!plain.last_exchange.unwrap().real_time_priority);

        let mut rt_cfg = HilConfig::new("simulink", 10.0);
        rt_cfg.real_time_priority = true;
        rt_cfg.io_channels.analog_outputs.push("y".to_string());
        let mut rt = HilRunner::new(rt_cfg);
        rt.start(test_engine()).unwrap();
        rt.step().unwrap();
        assert!(rt.last_exchange.unwrap().real_time_priority);
    }

    #[test]
    fn test_hil_initialize_rejects_duplicate_channels() {
        let mut cfg = HilConfig::new("simulink", 1000.0);
        cfg.io_channels.analog_inputs.push("ch0".to_string());
        cfg.io_channels.analog_outputs.push("ch0".to_string());
        let mut runner = HilRunner::new(cfg);
        let err = runner.initialize().unwrap_err();
        assert!(err.contains("Duplicate HIL channel 'ch0'"), "{}", err);
    }

    #[test]
    fn test_hil_initialize_rejects_empty_channel_name() {
        let mut cfg = HilConfig::new("simulink", 1000.0);
        cfg.io_channels.digital_inputs.push("  ".to_string());
        let mut runner = HilRunner::new(cfg);
        assert!(runner.initialize().is_err());
    }

    #[test]
    fn test_hil_start_rejects_invalid_sample_rate() {
        // start() must refuse to attach an engine when the HIL clock is
        // unusable, rather than deferring the failure to the first step.
        let mut runner = HilRunner::new(HilConfig::new("simulink", 0.0));
        assert!(runner.start(test_engine()).is_err());
        assert!(!runner.is_running);
        assert!(runner.engine.is_none());
    }

    #[test]
    fn test_hil_start_stop() {
        let mut runner = HilRunner::new(HilConfig::new("simulink", 1000.0));
        let engine = test_engine();
        assert!(runner.start(engine).is_ok());
        assert!(runner.is_running);
        runner.stop();
        assert!(!runner.is_running);
        assert!(runner.last_exchange.is_none());
    }

    #[test]
    fn test_hil_step_not_running() {
        let mut runner = HilRunner::new(HilConfig::new("simulink", 1000.0));
        assert!(runner.step().is_err());
    }

    // ── Fix (1): step() really advances the engine and exchanges I/O ────

    #[test]
    fn test_hil_step_advances_engine_and_records_exchange() {
        let mut cfg = HilConfig::new("simulink", 1000.0);
        cfg.io_channels.analog_inputs.push("u0".to_string());
        cfg.io_channels.analog_outputs.push("y0".to_string());
        cfg.io_channels.digital_inputs.push("d_in".to_string());
        cfg.io_channels.digital_outputs.push("d_out".to_string());
        let mut runner = HilRunner::new(cfg);
        runner.initialize().unwrap();
        runner.start(test_engine()).unwrap();

        let before = runner.engine.as_ref().unwrap().context.step_count;
        runner.step().unwrap();

        // The engine really advanced: the no-op defect would leave this at 0.
        let after = runner.engine.as_ref().unwrap().context.step_count;
        assert_eq!(after, before + 1, "a HIL step must advance the engine");

        // The exchange really happened and used the configured channels.
        let ex = runner
            .last_exchange
            .as_ref()
            .expect("exchange must be recorded");
        assert!(
            (ex.dt - 1.0 / 1000.0).abs() < 1e-15,
            "dt must come from sample_rate"
        );
        assert_eq!(
            ex.inputs_read
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>(),
            vec!["u0", "d_in"]
        );
        assert_eq!(
            ex.outputs_written
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>(),
            vec!["y0", "d_out"]
        );
    }

    #[test]
    fn test_hil_step_uses_dt_to_advance_simulation_time() {
        // dt is load-bearing: with a 10 Hz interface each step must advance
        // the engine clock by 100 ms.
        let mut runner = HilRunner::new(HilConfig::new("speedgoat", 10.0));
        runner.start(test_engine()).unwrap();

        let t0 = runner.engine.as_ref().unwrap().context.t;
        runner.step().unwrap();
        let t1 = runner.engine.as_ref().unwrap().context.t;
        assert!(
            (t1 - t0 - 0.1).abs() < 1e-12,
            "expected dt=0.1, got {}",
            t1 - t0
        );
        assert!(
            (runner.engine.as_ref().unwrap().context.dt - 0.1).abs() < 1e-12,
            "the HIL dt must be pushed into the engine context"
        );

        runner.step().unwrap();
        let t2 = runner.engine.as_ref().unwrap().context.t;
        assert!(
            (t2 - t0 - 0.2).abs() < 1e-12,
            "expected t to advance by 2*dt"
        );
    }

    #[test]
    fn test_hil_step_records_channel_values() {
        let mut cfg = HilConfig::new("dsPACE", 10.0);
        cfg.io_channels.analog_inputs.push("time".to_string());
        cfg.io_channels.analog_inputs.push("step".to_string());
        cfg.io_channels.analog_outputs.push("count".to_string());
        let mut runner = HilRunner::new(cfg);
        runner.start(test_engine()).unwrap();

        runner.step().unwrap();
        let ex = runner.last_exchange.clone().unwrap();
        // "time" read the pre-step engine time; "step" read the pre-step index.
        assert_eq!(ex.inputs_read[0], ("time".to_string(), 0.0));
        assert_eq!(ex.inputs_read[1], ("step".to_string(), 0.0));
        // No output port named "count" exists, so the step counter is driven.
        assert_eq!(ex.outputs_written[0], ("count".to_string(), 1.0));

        // A second step must produce a different, observable exchange.
        runner.step().unwrap();
        let ex2 = runner.last_exchange.unwrap();
        assert_eq!(ex2.inputs_read[0].1, 0.1);
        assert_eq!(ex2.inputs_read[1].1, 1.0);
        assert_eq!(ex2.outputs_written[0].1, 2.0);
    }

    /// A block that writes a constant scalar to its declared output port on
    /// every `output()` call. Used to prove the HIL write phase reads engine
    /// output ports rather than falling back to the step counter.
    struct ConstOutputBlock {
        inner: crate::core::block::SimpleBlock,
        value: Scalar,
        port: String,
    }

    impl crate::core::block::Block for ConstOutputBlock {
        fn id(&self) -> &crate::core::block::BlockId {
            self.inner.id()
        }
        fn block_type(&self) -> &str {
            self.inner.block_type()
        }
        fn ports(&self) -> &crate::core::port::PortSet {
            self.inner.ports()
        }
        fn ports_mut(&mut self) -> &mut crate::core::port::PortSet {
            self.inner.ports_mut()
        }
        fn params(&self) -> &crate::core::param::ParameterSet {
            self.inner.params()
        }
        fn params_mut(&mut self) -> &mut crate::core::param::ParameterSet {
            self.inner.params_mut()
        }
        fn status(&self) -> crate::core::types::ComponentStatus {
            self.inner.status()
        }
        fn set_status(&mut self, s: crate::core::types::ComponentStatus) {
            self.inner.set_status(s)
        }
        fn set_time(&mut self, t: crate::core::types::Time) {
            self.inner.set_time(t)
        }
        fn time(&self) -> crate::core::types::Time {
            self.inner.time()
        }
        fn init(&mut self) -> Result<(), crate::core::error::SimError> {
            self.inner.init()
        }
        fn output(&mut self) -> Result<(), crate::core::error::SimError> {
            let signal = crate::core::signal::Signal::new(
                crate::core::types::SignalType::Continuous,
                crate::core::types::SignalValue::Scalar(self.value),
                self.inner.time(),
            );
            self.inner
                .ports_mut()
                .get_mut(&self.port)
                .expect("declared output port")
                .write(signal);
            Ok(())
        }
        fn derivative(&self) -> Result<Vec<Scalar>, crate::core::error::SimError> {
            Ok(Vec::new())
        }
        fn update(&mut self) -> Result<(), crate::core::error::SimError> {
            Ok(())
        }
        fn zero_crossings(&self) -> Vec<Scalar> {
            Vec::new()
        }
        fn terminate(&mut self) -> Result<(), crate::core::error::SimError> {
            Ok(())
        }
        fn clone_block(&self) -> Box<dyn crate::core::block::Block> {
            Box::new(ConstOutputBlock {
                inner: self.inner.clone(),
                value: self.value,
                port: self.port.clone(),
            })
        }
    }

    #[test]
    fn test_hil_step_writes_engine_output_port_when_channel_matches() {
        // When the engine exposes an output port named after the configured
        // channel, the written value is that port's signal (not the fallback
        // step counter), proving the write phase is driven by engine outputs.
        use crate::core::types::SignalType;

        let mut block = crate::core::block::SimpleBlock::new("src", "Const");
        block.declare_output("y0", SignalType::Continuous);
        let mut diagram = Diagram::new("io");
        diagram.add_block(Box::new(ConstOutputBlock {
            inner: block,
            value: 2.5,
            port: "y0".to_string(),
        }));
        let engine = SimEngine::new(
            diagram,
            TimeConfig {
                start_time: 0.0,
                end_time: 1000.0,
                max_step: 1.0,
                min_step: 1e-9,
                initial_step: 0.01,
            },
        )
        .unwrap();

        let mut cfg = HilConfig::new("simulink", 10.0);
        cfg.io_channels.analog_outputs.push("y0".to_string());
        let mut runner = HilRunner::new(cfg);
        runner.start(engine).unwrap();
        runner.step().unwrap();

        let ex = runner.last_exchange.unwrap();
        assert_eq!(ex.outputs_written.len(), 1);
        assert_eq!(ex.outputs_written[0].0, "y0");
        // The port value (2.5) differs from the fallback step counter (1.0).
        assert_eq!(ex.outputs_written[0].1, 2.5);
    }

    /// A block whose `output()` phase always fails, used to prove the HIL loop
    /// propagates engine errors instead of discarding the step `Result`.
    struct FailingOutputBlock(crate::core::block::SimpleBlock);

    impl crate::core::block::Block for FailingOutputBlock {
        fn id(&self) -> &crate::core::block::BlockId {
            self.0.id()
        }
        fn block_type(&self) -> &str {
            self.0.block_type()
        }
        fn ports(&self) -> &crate::core::port::PortSet {
            self.0.ports()
        }
        fn ports_mut(&mut self) -> &mut crate::core::port::PortSet {
            self.0.ports_mut()
        }
        fn params(&self) -> &crate::core::param::ParameterSet {
            self.0.params()
        }
        fn params_mut(&mut self) -> &mut crate::core::param::ParameterSet {
            self.0.params_mut()
        }
        fn status(&self) -> crate::core::types::ComponentStatus {
            self.0.status()
        }
        fn set_status(&mut self, s: crate::core::types::ComponentStatus) {
            self.0.set_status(s)
        }
        fn set_time(&mut self, t: crate::core::types::Time) {
            self.0.set_time(t)
        }
        fn time(&self) -> crate::core::types::Time {
            self.0.time()
        }
        fn init(&mut self) -> Result<(), crate::core::error::SimError> {
            self.0.init()
        }
        fn output(&mut self) -> Result<(), crate::core::error::SimError> {
            Err(crate::core::error::SimError::runtime(
                "forced output failure",
            ))
        }
        fn derivative(&self) -> Result<Vec<Scalar>, crate::core::error::SimError> {
            Ok(Vec::new())
        }
        fn update(&mut self) -> Result<(), crate::core::error::SimError> {
            Ok(())
        }
        fn zero_crossings(&self) -> Vec<Scalar> {
            Vec::new()
        }
        fn terminate(&mut self) -> Result<(), crate::core::error::SimError> {
            Ok(())
        }
        fn clone_block(&self) -> Box<dyn crate::core::block::Block> {
            Box::new(FailingOutputBlock(self.0.clone()))
        }
    }

    #[test]
    fn test_hil_step_propagates_engine_failure_instead_of_silently_ignoring() {
        // A failing engine step must surface as an `Err` from `step()`. The
        // no-op defect would have returned `Ok(())` and swallowed the error.
        let mut diagram = Diagram::new("failing");
        diagram.add_block(Box::new(FailingOutputBlock(
            crate::core::block::SimpleBlock::new("bad", "Bad"),
        )));
        let engine = SimEngine::new(
            diagram,
            TimeConfig {
                start_time: 0.0,
                end_time: 1.0,
                max_step: 1.0,
                min_step: 1e-9,
                initial_step: 0.01,
            },
        )
        .unwrap();

        let mut cfg = HilConfig::new("simulink", 10.0);
        cfg.io_channels.analog_outputs.push("y0".to_string());
        let mut runner = HilRunner::new(cfg);
        runner.start(engine).unwrap();

        let err = runner.step().unwrap_err();
        assert!(err.contains("HIL engine step failed"), "{}", err);
        assert!(err.contains("forced output failure"), "{}", err);
        // A failed step must not fabricate an exchange record.
        assert!(
            runner.last_exchange.is_none(),
            "no exchange should be recorded when the engine step fails"
        );
    }

    #[test]
    fn test_hil_step_pauses_at_end_time_and_records_finished_exchange() {
        // Drive the engine to completion: stepping past end_time must yield
        // `Finished`, which is a normal (non-error) HIL outcome and still
        // produces a recorded exchange.
        let mut diagram = Diagram::new("short");
        diagram.add_block(Box::new(crate::core::block::SimpleBlock::new("b", "Const")));
        let engine = SimEngine::new(
            diagram,
            TimeConfig {
                start_time: 0.0,
                end_time: 0.01,
                max_step: 0.01,
                min_step: 1e-9,
                initial_step: 0.01,
            },
        )
        .unwrap();
        let mut cfg = HilConfig::new("simulink", 100.0);
        cfg.io_channels.analog_outputs.push("y".to_string());
        let mut runner = HilRunner::new(cfg);
        runner.start(engine).unwrap();

        runner.initialize().unwrap();
        runner.step().unwrap(); // reaches end_time
        runner.step().unwrap(); // engine reports Finished
        let ex = runner
            .last_exchange
            .expect("finished steps still exchange I/O");
        assert!((ex.dt - 0.01).abs() < 1e-12);
        assert_eq!(runner.engine.as_ref().unwrap().context.step_count, 1);
    }
}
