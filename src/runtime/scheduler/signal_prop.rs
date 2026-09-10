//! Port signal propagation, caching, and synchronization.
//!
//! Provides the `SignalCache` for storing port signal values and the
//! propagation engine that transfers values from source output ports
//! through links to destination input ports.

use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::types::{PortDirection, Scalar, SignalValue, Time};
use std::collections::HashMap;

/// Upper bound on the number of steps a single delay line may hold.
///
/// A model with `delay = 1 s` at `dt = 1e-6` asks for 1e6 steps; the buffer is
/// allocated lazily so this bounds eventual size, not an up-front reservation.
/// Beyond this the delay is clamped and the effective lag is shorter than
/// requested, which is preferable to an unbounded allocation.
const MAX_DELAY_STEPS: usize = 1_000_000;

/// A bounded history for one delayed link.
///
/// `capacity` is the number of steps of delay. The line holds `capacity + 1`
/// samples so that the sample published at step `t` is the one pushed at step
/// `t - capacity`: the current sample must be pushed *before* the delayed one is
/// read, but must not yet be published.
#[derive(Debug, Clone)]
struct DelayLine {
    /// Samples, oldest first.
    history: std::collections::VecDeque<SignalValue>,
    /// Number of steps of delay (`>= 1`).
    steps: usize,
}

impl DelayLine {
    fn new(steps: usize) -> Self {
        let steps = steps.max(1);
        Self {
            // Deliberately NOT `with_capacity(steps + 1)`: a large `delay/dt`
            // (e.g. a 1 s delay at dt = 1e-6) would eagerly reserve tens of MiB
            // per delayed link before a single sample is stored. The deque grows
            // to `steps + 1` entries as the line fills, so capacity tracks actual
            // use instead of the theoretical maximum.
            history: std::collections::VecDeque::new(),
            steps,
        }
    }

    /// Push a new sample, keeping `steps + 1` samples so the line can expose the
    /// value from exactly `steps` calls ago.
    ///
    /// The extra entry matters: the newest sample is pushed *before* the delayed
    /// one is read, so a buffer of exactly `steps` would expose the sample from
    /// `steps - 1` ago and the effective delay would be one step short.
    fn push(&mut self, value: SignalValue) {
        self.history.push_back(value);
        while self.history.len() > self.steps + 1 {
            self.history.pop_front();
        }
    }

    /// The value delayed by exactly `steps` samples, if the line has filled.
    ///
    /// Returns `None` until enough history exists: a signal that has not arrived
    /// yet is absent, not zero.
    fn delayed(&self) -> Option<&SignalValue> {
        if self.history.len() < self.steps + 1 {
            return None;
        }
        // Full: the newest sample is at the back, so the one from `steps` calls
        // ago is at the front.
        self.history.front()
    }
}

/// A cache of signal values for all ports in a diagram.
///
/// Stores both current and previous signal values for edge detection
/// and signal change tracking.
#[derive(Debug, Clone)]
pub struct SignalCache {
    /// Current signal values: (block_id, port_id) -> SignalValue
    current: HashMap<(BlockId, String), SignalValue>,
    /// Previous time-step signal values (for edge detection)
    previous: HashMap<(BlockId, String), SignalValue>,
    /// Per-link delay lines, keyed by source `(block, port)`.
    ///
    /// Only links with `delay > 0` appear here. A bounded ring per link keeps
    /// memory proportional to `delay / dt`, not to the run length.
    delays: HashMap<(BlockId, String), DelayLine>,
    /// Current step size used to convert each link's `delay` into a number of
    /// steps (`steps = ceil(delay / dt)`, at least 1).
    pub dt: Time,
}

impl SignalCache {
    /// Create a new empty signal cache.
    pub fn new() -> Self {
        Self {
            current: HashMap::new(),
            previous: HashMap::new(),
            delays: HashMap::new(),
            dt: 0.0,
        }
    }

    /// Initialize the cache with all ports from a diagram.
    /// All ports start with SignalValue::None.
    pub fn from_diagram(diagram: &Diagram) -> Self {
        let mut cache = Self::new();
        for (id, block) in diagram.blocks() {
            for port in block.ports().iter() {
                let key = (id.clone(), port.id.clone());
                cache.current.insert(key.clone(), SignalValue::None);
                cache.previous.insert(key, SignalValue::None);
            }
        }
        cache
    }

    /// Initialize the cache, sizing a delay line for every link that needs one.
    ///
    /// `dt` is the nominal step size; a link's `delay` becomes
    /// `max(1, ceil(delay / dt))` steps of history. Links with a non-positive or
    /// non-finite delay get no line and propagate immediately, which is the
    /// documented meaning of a zero delay.
    pub fn from_diagram_with_dt(diagram: &Diagram, dt: Time) -> Self {
        let mut cache = Self::from_diagram(diagram);
        cache.dt = dt;
        cache.configure_delays(diagram);
        cache
    }

    /// (Re)create the delay lines for every delayed link in `diagram`.
    ///
    /// Called on construction and whenever the topology or step size changes.
    /// Existing history is preserved for links that are still delayed, so a
    /// `dt` change does not silently discard in-flight signals.
    pub fn configure_delays(&mut self, diagram: &Diagram) {
        let mut wanted: HashMap<(BlockId, String), usize> = HashMap::new();
        if self.dt.is_finite() && self.dt > 0.0 {
            for link in diagram.links().iter() {
                if !link.delay.is_finite() || link.delay <= 0.0 {
                    continue;
                }
                // `ceil` on a raw ratio is unsafe: `3.0 * 0.1 / 0.1` evaluates to
                // 3.0000000000000004, so a request for exactly 3 steps would
                // silently become 4. Snap to the nearest integer when the ratio
                // is within a relative epsilon of it, then take the ceiling.
                let ratio = link.delay / self.dt;
                let nearest = ratio.round();
                let ratio = if (ratio - nearest).abs() <= 1e-9 * nearest.abs().max(1.0) {
                    nearest
                } else {
                    ratio
                };
                let steps = ratio.ceil().max(1.0);
                // Bound the history so a pathological `delay/dt` cannot grow
                // without limit. Note the buffer is allocated lazily, so this cap
                // bounds *eventual* size rather than an up-front reservation.
                let steps = if steps > MAX_DELAY_STEPS as f64 {
                    MAX_DELAY_STEPS
                } else {
                    steps as usize
                };
                wanted
                    .entry((link.source.0.clone(), link.source.1.clone()))
                    .and_modify(|s| *s = (*s).max(steps))
                    .or_insert(steps);
            }
        }

        // Drop lines for links that are no longer delayed.
        self.delays.retain(|k, _| wanted.contains_key(k));
        // Create or resize the remaining ones, keeping any existing samples.
        for (key, steps) in wanted {
            match self.delays.get_mut(&key) {
                Some(line) if line.steps == steps => {}
                Some(line) => {
                    line.steps = steps;
                    while line.history.len() > steps + 1 {
                        line.history.pop_front();
                    }
                }
                None => {
                    self.delays.insert(key, DelayLine::new(steps));
                }
            }
        }
    }

    /// Number of delayed links currently tracked (observable for tests).
    pub fn delayed_link_count(&self) -> usize {
        self.delays.len()
    }

    /// Push a source value into its delay line, if that link is delayed.
    ///
    /// Returns the value that should be published *now* on the far side of the
    /// link, or `None` when the link is not delayed (the caller should then use
    /// the live value).
    fn delay_sample(&mut self, key: &(BlockId, String), value: SignalValue) -> Option<SignalValue> {
        let line = self.delays.get_mut(key)?;
        line.push(value);
        // `capacity` steps of history means the newest sample is published
        // `capacity` steps late, so read the oldest entry.
        line.delayed().cloned()
    }

    /// Get the current signal value for a port.
    pub fn get(&self, block_id: &str, port_id: &str) -> Option<&SignalValue> {
        self.current
            .get(&(block_id.to_string(), port_id.to_string()))
    }

    /// Set the current signal value for a port.
    pub fn set(&mut self, block_id: &str, port_id: &str, value: SignalValue) {
        self.current
            .insert((block_id.to_string(), port_id.to_string()), value);
    }

    /// Get the previous time-step signal value for a port.
    pub fn get_previous(&self, block_id: &str, port_id: &str) -> Option<&SignalValue> {
        self.previous
            .get(&(block_id.to_string(), port_id.to_string()))
    }

    /// Advance the cache: move current values to previous and clear current.
    /// Called at the end of each simulation step.
    ///
    /// After this call every port that existed before is present in `current`
    /// with [`SignalValue::None`], so a port whose producer disappeared reads as
    /// "no signal" rather than retaining a two-step-old value.
    pub fn advance(&mut self) {
        std::mem::swap(&mut self.current, &mut self.previous);
        // `current` now holds the old `previous` map. It must be *reset*, not
        // merely back-filled: `entry().or_insert()` would leave the stale
        // values in place for every key that was already present.
        let ports: Vec<(String, String)> = self.current.keys().cloned().collect();
        self.current.clear();
        for key in ports {
            self.current.insert(key, SignalValue::None);
        }
    }

    /// Check if a signal value changed from the previous step.
    pub fn has_changed(&self, block_id: &str, port_id: &str) -> bool {
        let key = (block_id.to_string(), port_id.to_string());
        match (self.current.get(&key), self.previous.get(&key)) {
            (Some(curr), Some(prev)) => curr != prev,
            _ => false,
        }
    }

    /// Number of cached port values.
    pub fn len(&self) -> usize {
        self.current.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }
}

impl Default for SignalCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Propagate signals through links: copy source output values to destination inputs.
///
/// For each link in the diagram, reads the source block's output port value from
/// the cache and writes it to the destination block's input port in the cache.
///
/// A link with `delay > 0` is routed through its delay line instead of being
/// copied directly, so a transport delay is actually modelled. Until the line
/// has filled, the destination receives `None` (the signal has not arrived yet),
/// which is the physically correct behaviour for a pure delay.
pub fn propagate_signals(diagram: &Diagram, cache: &mut SignalCache) -> Result<(), SimError> {
    for link in diagram.links().iter() {
        let src_key = (link.source.0.clone(), link.source.1.clone());
        let dst_key = (link.destination.0.clone(), link.destination.1.clone());

        let value = cache
            .current
            .get(&src_key)
            .cloned()
            .unwrap_or(SignalValue::None);

        // A delayed link publishes a sample from `delay` seconds ago; the live
        // value is only used when no delay line exists for this link.
        let published = match cache.delay_sample(&src_key, value.clone()) {
            Some(delayed) => delayed,
            None if cache.delays.contains_key(&src_key) => {
                // The line exists but is still filling: nothing has arrived yet.
                SignalValue::None
            }
            None => value,
        };

        cache.current.insert(dst_key, published);
    }
    Ok(())
}

/// Extract all output port values from blocks into the signal cache.
pub fn extract_outputs(diagram: &Diagram, cache: &mut SignalCache) -> Result<(), SimError> {
    for (id, block) in diagram.blocks() {
        for port in block.ports().iter() {
            if port.direction == PortDirection::Output {
                let value = port
                    .signal
                    .as_ref()
                    .map(|s| s.value.clone())
                    .unwrap_or(SignalValue::None);
                cache.set(id, &port.id, value);
            }
        }
    }
    Ok(())
}

/// Write cached signal values back to block input ports.
///
/// For each block in the diagram, iterates over its input ports and writes
/// the corresponding cached signal value (previously propagated from source
/// output ports via `propagate_signals()`) into the port's signal field.
///
/// An input port with **no** cached value is cleared instead of being left with
/// its previous contents. Without this, disconnecting a link (or removing its
/// producer block) would leave the stale value on the port forever, making
/// "not connected" indistinguishable from "hold the last value".
///
/// Requires `&mut Diagram` because port writes mutate block state.
/// `time` stamps every propagated input signal with the current simulation time
/// so blocks that inspect `Signal::time` do not read a constant zero.
pub fn update_inputs(
    diagram: &mut Diagram,
    cache: &SignalCache,
    time: Scalar,
) -> Result<(), SimError> {
    // Collect all block IDs first to avoid borrow conflicts
    let block_ids: Vec<String> = diagram.blocks().map(|(id, _)| id.clone()).collect();
    for id in &block_ids {
        if let Some(block) = diagram.get_block_mut(id) {
            let port_ids: Vec<String> = block.ports().inputs().map(|p| p.id.clone()).collect();
            for port_id in &port_ids {
                let value = cache.get(id, port_id).cloned();
                if let Some(port) = block.ports_mut().get_mut(port_id) {
                    match value {
                        Some(v) if !matches!(v, SignalValue::None) => {
                            port.write(crate::core::signal::Signal::new(port.signal_type, v, time));
                        }
                        // No producer (or a None value): make the absence
                        // observable instead of silently holding stale data.
                        _ => port.clear(),
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::diagram::Diagram;
    use crate::core::link::Link;
    use crate::core::types::SignalType;

    #[test]
    fn test_signal_cache_create() {
        let cache = SignalCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_signal_cache_from_diagram() {
        let mut diagram = Diagram::new("test");
        let mut block = SimpleBlock::new("B1", "Test");
        block.declare_input("in", SignalType::Continuous);
        block.declare_output("out", SignalType::Continuous);
        diagram.add_block(Box::new(block));

        let cache = SignalCache::from_diagram(&diagram);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("B1", "in"), Some(&SignalValue::None));
        assert_eq!(cache.get("B1", "out"), Some(&SignalValue::None));
    }

    #[test]
    fn test_signal_cache_set_get() {
        let mut cache = SignalCache::new();
        cache.set("B1", "out", SignalValue::Scalar(42.0));
        assert_eq!(cache.get("B1", "out"), Some(&SignalValue::Scalar(42.0)));
        assert_eq!(cache.get("B1", "in"), None);
    }

    #[test]
    fn test_signal_propagation() {
        let mut diagram = Diagram::new("test");
        let mut src = SimpleBlock::new("Src", "Source");
        src.declare_output("out", SignalType::Continuous);
        let mut dst = SimpleBlock::new("Dst", "Sink");
        dst.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(src));
        diagram.add_block(Box::new(dst));
        diagram.add_link(Link::new("L1", "Src", "out", "Dst", "in"));

        let mut cache = SignalCache::from_diagram(&diagram);
        cache.set("Src", "out", SignalValue::Scalar(42.0_f64));
        propagate_signals(&diagram, &mut cache).unwrap();
        assert_eq!(cache.get("Dst", "in"), Some(&SignalValue::Scalar(42.0_f64)));
    }

    /// A zero-delay link is a direct feedthrough: the destination sees the
    /// source value in the same step.
    #[test]
    fn test_zero_delay_link_is_a_direct_feedthrough() {
        let mut diagram = Diagram::new("d0");
        let mut src = SimpleBlock::new("Src", "Source");
        src.declare_output("out", SignalType::Continuous);
        let mut dst = SimpleBlock::new("Dst", "Sink");
        dst.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(src));
        diagram.add_block(Box::new(dst));
        diagram.add_link(Link::new("L1", "Src", "out", "Dst", "in"));

        let mut cache = SignalCache::from_diagram_with_dt(&diagram, 0.1);
        assert_eq!(
            cache.delayed_link_count(),
            0,
            "a zero-delay link needs no delay line"
        );
        cache.set("Src", "out", SignalValue::Scalar(7.0));
        propagate_signals(&diagram, &mut cache).unwrap();
        assert_eq!(cache.get("Dst", "in"), Some(&SignalValue::Scalar(7.0)));
    }

    /// A link with a transport delay must actually delay its samples.
    ///
    /// `delay = 3 * dt` means the destination sees the value the source held
    /// three steps earlier, and sees `None` until the line has filled.
    #[test]
    fn test_delayed_link_shifts_samples_by_the_delay() {
        let dt = 0.1;
        let mut diagram = Diagram::new("d3");
        let mut src = SimpleBlock::new("Src", "Source");
        src.declare_output("out", SignalType::Continuous);
        let mut dst = SimpleBlock::new("Dst", "Sink");
        dst.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(src));
        diagram.add_block(Box::new(dst));
        diagram.add_link(Link::new("L1", "Src", "out", "Dst", "in").with_delay(3.0 * dt));

        let mut cache = SignalCache::from_diagram_with_dt(&diagram, dt);
        assert_eq!(
            cache.delayed_link_count(),
            1,
            "a delayed link must get a delay line"
        );

        // Drive a ramp: source value = step index.
        let mut seen = Vec::new();
        for step in 0..7 {
            cache.set("Src", "out", SignalValue::Scalar(step as f64));
            propagate_signals(&diagram, &mut cache).unwrap();
            seen.push(cache.get("Dst", "in").cloned());
        }

        // The first three steps are still filling the line, so nothing arrives.
        let none = SignalValue::None;
        assert_eq!(
            seen[0],
            Some(none.clone()),
            "step 0 has no delayed value yet"
        );
        assert_eq!(
            seen[1],
            Some(none.clone()),
            "step 1 has no delayed value yet"
        );
        assert_eq!(
            seen[2],
            Some(none.clone()),
            "step 2 has no delayed value yet"
        );

        // From step 3 the destination lags the source by exactly 3 steps.
        for step in 3..7 {
            let expected = (step - 3) as f64;
            assert_eq!(
                seen[step],
                Some(SignalValue::Scalar(expected)),
                "step {step} must observe the source value from {expected}"
            );
        }
    }

    /// The delay line's memory is bounded by `delay / dt`, not by the run length.
    #[test]
    fn test_delay_line_memory_is_bounded() {
        let dt = 0.01;
        let mut diagram = Diagram::new("bounded");
        let mut src = SimpleBlock::new("Src", "Source");
        src.declare_output("out", SignalType::Continuous);
        let mut dst = SimpleBlock::new("Dst", "Sink");
        dst.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(src));
        diagram.add_block(Box::new(dst));
        // 5 steps of delay.
        diagram.add_link(Link::new("L1", "Src", "out", "Dst", "in").with_delay(5.0 * dt));

        let mut cache = SignalCache::from_diagram_with_dt(&diagram, dt);
        for step in 0..1000 {
            cache.set("Src", "out", SignalValue::Scalar(step as f64));
            propagate_signals(&diagram, &mut cache).unwrap();
        }
        // After 1000 steps the destination must lag by exactly 5: the source
        // held 994 five steps before the final step (999 - 5 = 994).
        assert_eq!(
            cache.get("Dst", "in"),
            Some(&SignalValue::Scalar(994.0)),
            "a 5-step delay must still lag by exactly 5 after 1000 steps"
        );
    }

    /// A delay shorter than one step must still delay by one step rather than
    /// silently becoming a feedthrough.
    #[test]
    fn test_sub_step_delay_rounds_up_to_one_step() {
        let dt = 0.1;
        let mut diagram = Diagram::new("sub");
        let mut src = SimpleBlock::new("Src", "Source");
        src.declare_output("out", SignalType::Continuous);
        let mut dst = SimpleBlock::new("Dst", "Sink");
        dst.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(src));
        diagram.add_block(Box::new(dst));
        diagram.add_link(Link::new("L1", "Src", "out", "Dst", "in").with_delay(dt / 10.0));

        let mut cache = SignalCache::from_diagram_with_dt(&diagram, dt);
        cache.set("Src", "out", SignalValue::Scalar(1.0));
        propagate_signals(&diagram, &mut cache).unwrap();
        assert_eq!(
            cache.get("Dst", "in"),
            Some(&SignalValue::None),
            "a sub-step delay must not act as a feedthrough on the first step"
        );
        cache.set("Src", "out", SignalValue::Scalar(2.0));
        propagate_signals(&diagram, &mut cache).unwrap();
        assert_eq!(
            cache.get("Dst", "in"),
            Some(&SignalValue::Scalar(1.0)),
            "the sample from one step earlier must arrive"
        );
    }

    /// A delay expressed as an exact multiple of `dt` must produce exactly that
    /// many steps.
    ///
    /// `ceil(delay / dt)` is unsafe in binary floating point: `3.0 * 0.1 / 0.1`
    /// evaluates to `3.0000000000000004`, so a request for 3 steps silently
    /// became 4. This walks several multiples that are known to be inexact.
    #[test]
    fn test_delay_that_is_an_exact_multiple_of_dt_gets_exact_steps() {
        for (dt, multiple) in [
            (0.1, 3.0),
            (0.1, 5.0),
            (0.1, 7.0),
            (0.01, 5.0),
            (0.2, 3.0),
            (0.05, 6.0),
            (0.3, 2.0),
        ] {
            let mut diagram = Diagram::new("exact");
            let mut src = SimpleBlock::new("Src", "Source");
            src.declare_output("out", SignalType::Continuous);
            let mut dst = SimpleBlock::new("Dst", "Sink");
            dst.declare_input("in", SignalType::Continuous);
            diagram.add_block(Box::new(src));
            diagram.add_block(Box::new(dst));
            diagram.add_link(Link::new("L1", "Src", "out", "Dst", "in").with_delay(multiple * dt));

            let mut cache = SignalCache::from_diagram_with_dt(&diagram, dt);
            let steps = multiple as usize;

            // The first `steps` samples must not have arrived yet...
            for step in 0..steps {
                cache.set("Src", "out", SignalValue::Scalar(step as f64));
                propagate_signals(&diagram, &mut cache).unwrap();
                assert_eq!(
                    cache.get("Dst", "in"),
                    Some(&SignalValue::None),
                    "dt={dt}, delay={multiple}*dt: step {step} of {steps} must still be in flight"
                );
            }
            // ...and the sample from step 0 must arrive exactly at step `steps`.
            cache.set("Src", "out", SignalValue::Scalar(steps as f64));
            propagate_signals(&diagram, &mut cache).unwrap();
            assert_eq!(
                cache.get("Dst", "in"),
                Some(&SignalValue::Scalar(0.0)),
                "dt={dt}, delay={multiple}*dt: the lag must be exactly {steps} steps"
            );
        }
    }

    /// A non-finite or negative delay is treated as no delay rather than
    /// producing an unbounded or nonsensical history.
    #[test]
    fn test_invalid_delay_is_treated_as_a_feedthrough() {
        for bad in [0.0, -1.0, Scalar::NAN, Scalar::INFINITY] {
            let mut diagram = Diagram::new("bad");
            let mut src = SimpleBlock::new("Src", "Source");
            src.declare_output("out", SignalType::Continuous);
            let mut dst = SimpleBlock::new("Dst", "Sink");
            dst.declare_input("in", SignalType::Continuous);
            diagram.add_block(Box::new(src));
            diagram.add_block(Box::new(dst));
            diagram.add_link(Link::new("L1", "Src", "out", "Dst", "in").with_delay(bad));

            let mut cache = SignalCache::from_diagram_with_dt(&diagram, 0.1);
            assert_eq!(
                cache.delayed_link_count(),
                0,
                "delay {bad} must not create a delay line"
            );
            cache.set("Src", "out", SignalValue::Scalar(9.0));
            propagate_signals(&diagram, &mut cache).unwrap();
            assert_eq!(
                cache.get("Dst", "in"),
                Some(&SignalValue::Scalar(9.0)),
                "delay {bad} must behave as a direct feedthrough"
            );
        }
    }

    #[test]
    fn test_signal_cache_advance() {
        let mut cache = SignalCache::new();
        cache.set("B1", "out", SignalValue::Scalar(1.0));
        cache.advance();
        // The old value must be visible as the *previous* value so edge
        // detection can compare against it.
        assert_eq!(
            cache.get_previous("B1", "out"),
            Some(&SignalValue::Scalar(1.0))
        );
        // ...and the current value must be cleared, not left stale. Absent and
        // explicit-None must both read as "no current value".
        assert!(
            cache.get("B1", "out").is_none()
                || matches!(cache.get("B1", "out"), Some(SignalValue::None)),
            "the current value must not survive an advance, got {:?}",
            cache.get("B1", "out")
        );
    }

    /// Regression: `advance()` used `entry().or_insert(None)`, which left the
    /// swapped-in stale values in place for every key already present, so a
    /// disconnected port kept serving a two-step-old signal.
    #[test]
    fn test_signal_cache_advance_does_not_retain_stale_values() {
        let mut cache = SignalCache::new();
        cache.set("B1", "out", SignalValue::Scalar(7.0));
        cache.advance();
        cache.advance();
        // Two advances later the original 7.0 must be gone from `current`
        // entirely: it may live on in `previous`, but never as "current".
        match cache.get("B1", "out") {
            None | Some(SignalValue::None) => {}
            other => panic!("stale value survived two advances: {other:?}"),
        }
    }

    /// Regression: an input port whose producer disappeared kept its last value
    /// forever, so "disconnected" was indistinguishable from "hold last value".
    #[test]
    fn test_update_inputs_clears_port_when_no_producer_exists() {
        use crate::core::block::SimpleBlock;
        use crate::core::types::SignalType;

        let mut diagram = Diagram::new("stale");
        let mut dst = SimpleBlock::new("Dst", "Sink");
        dst.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(dst));

        let mut cache = SignalCache::from_diagram(&diagram);

        // First: a real producer feeds the port.
        cache.set("Dst", "in", SignalValue::Scalar(5.0));
        update_inputs(&mut diagram, &cache, 1.0).unwrap();
        let port = diagram.get_block("Dst").unwrap().ports().get("in").unwrap();
        assert_eq!(
            port.read().and_then(|s| s.as_scalar()),
            Some(5.0),
            "a present signal must be written through"
        );
        assert_eq!(
            port.read().map(|s| s.time),
            Some(1.0),
            "the propagated signal must carry the current simulation time"
        );

        // Now the producer vanishes: the cached value is cleared.
        cache.set("Dst", "in", SignalValue::None);
        update_inputs(&mut diagram, &cache, 2.0).unwrap();
        let port = diagram.get_block("Dst").unwrap().ports().get("in").unwrap();
        assert!(
            port.read().is_none(),
            "a port with no producer must be cleared, not left stale; got {:?}",
            port.read()
        );
    }

    #[test]
    fn test_signal_changed_detection() {
        let mut cache = SignalCache::new();
        cache.set("B1", "out", SignalValue::Scalar(1.0));
        cache.advance();
        cache.set("B1", "out", SignalValue::Scalar(2.0));
        assert!(cache.has_changed("B1", "out"));
    }
}
