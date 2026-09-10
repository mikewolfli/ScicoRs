//! Hybrid scheduler for mixed continuous/discrete/event/multi-rate systems.
//!
//! Provides the standard 8-phase execution cycle: ComputeOutputs → PropagateSignals →
//! ComputeDerivs → IntegrateStates → UpdateDiscrete → DetectEvents →
//! HandleEvents → AdvanceTime.

use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::types::{ExecutionPhase, Scalar};
use std::collections::HashMap;

/// Classification of a block's execution type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTaskType {
    /// Continuous system block (requires ODE solver integration).
    Continuous,
    /// Discrete system block (fixed-step update).
    Discrete,
    /// Event-driven block (responds to event triggers).
    EventDriven,
    /// Multi-rate block (operates at a different rate than base step).
    MultiRate,
}

/// The standard 8 phases of a simulation time step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchedulePhase {
    ComputeOutputs,
    PropagateSignals,
    ComputeDerivs,
    IntegrateStates,
    UpdateDiscrete,
    DetectEvents,
    HandleEvents,
    AdvanceTime,
}

impl SchedulePhase {
    /// All phases in execution order.
    pub fn all() -> [SchedulePhase; 8] {
        [
            SchedulePhase::ComputeOutputs,
            SchedulePhase::PropagateSignals,
            SchedulePhase::ComputeDerivs,
            SchedulePhase::IntegrateStates,
            SchedulePhase::UpdateDiscrete,
            SchedulePhase::DetectEvents,
            SchedulePhase::HandleEvents,
            SchedulePhase::AdvanceTime,
        ]
    }

    /// True for phases that only make sense when the diagram contains blocks
    /// that actually need them.
    ///
    /// - `IntegrateStates` is only required when at least one block declares
    ///   continuous state (otherwise there is nothing for an ODE solver to
    ///   advance); `ComputeDerivs` follows it, since a derivative is only
    ///   meaningful when there is continuous state to advance.
    /// - `DetectEvents` / `HandleEvents` are only required when at least one
    ///   block participates in event handling (an event-typed port).
    pub fn is_relevant_for(self, task_types: &[BlockTaskType]) -> bool {
        let has_continuous = task_types.contains(&BlockTaskType::Continuous);
        let has_events = task_types
            .iter()
            .any(|t| *t == BlockTaskType::EventDriven || *t == BlockTaskType::MultiRate);

        match self {
            SchedulePhase::ComputeDerivs | SchedulePhase::IntegrateStates => has_continuous,
            SchedulePhase::DetectEvents | SchedulePhase::HandleEvents => has_events,
            _ => true,
        }
    }
}

/// Configuration for the scheduler.
#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    pub discrete_step: Option<Scalar>,
    pub event_queue_capacity: usize,
    pub enable_signal_propagation: bool,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            discrete_step: None,
            event_queue_capacity: 1024,
            enable_signal_propagation: true,
        }
    }
}

/// Classify all blocks in a diagram by their execution type.
pub fn classify_blocks(diagram: &Diagram) -> HashMap<BlockId, BlockTaskType> {
    let mut classifications: HashMap<BlockId, BlockTaskType> = HashMap::new();

    for (bid, block) in diagram.blocks() {
        let decl = block.state_declaration();

        if decl.continuous_count() > 0 {
            classifications.insert(bid.clone(), BlockTaskType::Continuous);
        } else {
            let has_event = block
                .ports()
                .iter()
                .any(|p| p.signal_type == crate::core::types::SignalType::Event);

            if has_event {
                classifications.insert(bid.clone(), BlockTaskType::EventDriven);
            } else {
                classifications.insert(bid.clone(), BlockTaskType::Discrete);
            }
        }
    }

    classifications
}

/// Build the execution schedule for a single time step.
///
/// The schedule is derived from both inputs, so every [`ScheduleConfig`] knob
/// has an observable effect:
///
/// - `enable_signal_propagation`: when `false`, the `PropagateSignals` phase is
///   omitted, because the diagram is wired so that inputs are written directly
///   (e.g. bus/hierarchical diagrams that resolve links themselves).
/// - `discrete_step`: when `Some(step)`, the diagram is treated as a hybrid
///   multi-rate system, so `UpdateDiscrete` runs every step while
///   `ComputeDerivs`/`IntegrateStates` are only kept when the diagram contains
///   continuous blocks (the caller is expected to advance the continuous part
///   with the configured `step`).
/// - `event_queue_capacity == 0`: event handling is disabled, so
///   `DetectEvents`/`HandleEvents` are omitted (no queue exists to receive or
///   dispatch the detected zero-crossings).
///
/// Blocks that do not need a phase are skipped: a purely discrete diagram has
/// no `IntegrateStates` phase and a diagram without event ports has no event
/// phases.
pub fn build_schedule(diagram: &Diagram, config: &ScheduleConfig) -> Vec<SchedulePhase> {
    let task_types: Vec<BlockTaskType> = classify_blocks(diagram).into_values().collect();

    SchedulePhase::all()
        .into_iter()
        .filter(|phase| match phase {
            SchedulePhase::PropagateSignals => config.enable_signal_propagation,
            SchedulePhase::DetectEvents | SchedulePhase::HandleEvents => {
                config.event_queue_capacity > 0
            }
            _ => true,
        })
        // `discrete_step` marks a hybrid multi-rate diagram: the engine drives
        // the discrete part every step and the continuous part with the
        // configured step, so continuous-only phases are kept only when the
        // diagram actually owns continuous state, and the event phases are kept
        // only when some block reacts to events.
        .filter(|phase| match config.discrete_step {
            Some(_) => phase.is_relevant_for(&task_types),
            None => true,
        })
        .collect()
}

/// Validate that all blocks in the execution order exist before output computation.
///
/// This function performs a read-only validation check: it confirms each block ID
/// in `order` exists in the `diagram`. The actual `output()` mutation is performed
/// by the engine (which holds `&mut Diagram`). After the engine calls `output()` on
/// each block, it writes the results into the signal cache via `extract_outputs()`.
///
/// This design separates validation (done here with `&Diagram`) from mutation
/// (done by the engine with `&mut Diagram`), avoiding borrow conflicts.
pub fn execute_output_phase(diagram: &Diagram, order: &[BlockId]) -> Result<(), SimError> {
    for block_id in order {
        if diagram.get_block(block_id).is_none() {
            return Err(SimError::runtime(format!(
                "execute_output_phase: block '{}' not found",
                block_id
            )));
        }
    }
    Ok(())
}

/// Execute the ComputeDerivs phase for all continuous blocks.
pub fn execute_deriv_phase(diagram: &Diagram, order: &[BlockId]) -> Result<Vec<Scalar>, SimError> {
    let mut all_derivs = Vec::new();
    for block_id in order {
        if let Some(block) = diagram.get_block(block_id)
            && block.state_declaration().continuous_count() > 0
        {
            let derivs = block.derivative()?;
            all_derivs.extend(derivs);
        }
    }
    Ok(all_derivs)
}

/// Validate that all blocks exist before discrete update phase.
///
/// This function performs a read-only validation check. The actual `update()`
/// mutation is performed by the engine (which holds `&mut Diagram`). After the
/// engine calls `update()` on each block, it advances the signal cache for the
/// next time step.
///
/// This design separates validation (done here with `&Diagram`) from mutation
/// (done by the engine with `&mut Diagram`), avoiding borrow conflicts.
/// Validate that every block in `order` still exists, without mutating anything.
///
/// Use this from a context that only has `&Diagram`. The phase that actually
/// advances discrete state is [`execute_update_phase_mut`], which needs mutable
/// access; `SimEngine` owns that access and calls it.
pub fn validate_update_phase(diagram: &Diagram, order: &[BlockId]) -> Result<(), SimError> {
    for block_id in order {
        if diagram.get_block(block_id).is_none() {
            return Err(SimError::runtime(format!(
                "validate_update_phase: block '{}' not found",
                block_id
            )));
        }
    }
    Ok(())
}

/// Execute the Update phase **mutably**, actually running each block's update.
///
/// The immutable variant above exists only to validate the block set; this is
/// the one that advances discrete state.
pub fn execute_update_phase_mut(diagram: &mut Diagram, order: &[BlockId]) -> Result<(), SimError> {
    for block_id in order {
        let Some(block) = diagram.get_block_mut(block_id) else {
            return Err(SimError::runtime(format!(
                "execute_update_phase_mut: block '{}' not found",
                block_id
            )));
        };
        block.execute_phase(ExecutionPhase::Update)?;
    }
    Ok(())
}

/// Detect zero crossings by **sign change**, not by exact magnitude.
///
/// The previous test was `val.abs() < 1e-12`: it required the block's crossing
/// signal to land within `1e-12` of zero at the sampled instant, which a finite
/// step size makes essentially measure-zero. A signal stepping from `-1e-6` to
/// `+1e-6` produced `-1e-6` and `+1e-6` and was **never** reported.
///
/// A crossing is now detected when a signal changes sign relative to the
/// previous sample, or when it is exactly zero (which is a genuine crossing that
/// carries no sign information). `previous` holds the last observed value per
/// `(block, crossing index)`; pass the same map back on the next step.
///
/// The second tuple element is the index of the crossing within the block's
/// `zero_crossings()` result — the only stable identifier a crossing carries.
pub fn execute_event_detection_with_state(
    diagram: &Diagram,
    order: &[BlockId],
    previous: &mut HashMap<(BlockId, usize), Scalar>,
) -> Vec<(BlockId, Scalar)> {
    let mut events = Vec::new();
    // Blocks currently present, so stale history can be pruned below.
    let mut seen: std::collections::HashSet<(BlockId, usize)> = std::collections::HashSet::new();
    for block_id in order {
        let Some(block) = diagram.get_block(block_id) else {
            continue;
        };
        let crossings = block.zero_crossings();
        for (i, &val) in crossings.iter().enumerate() {
            let key = (block_id.clone(), i);
            seen.insert(key.clone());

            if !val.is_finite() {
                // A non-finite sample carries no sign, and leaving the previous
                // value in place would compare the *next* finite sample against a
                // value from before the gap, reporting a crossing that never
                // happened. Forget the history so the signal restarts.
                previous.remove(&key);
                continue;
            }

            let crossed = match previous.get(&key) {
                // A crossing is a *sign change* between two consecutive samples.
                // An exact zero is only a crossing when it accompanies a sign
                // change (or when it is the first sample after a gap): a signal
                // that merely touches zero and returns has not crossed, and a
                // signal parked at zero must not re-fire every step.
                Some(&prev) => (prev < 0.0 && val >= 0.0) || (prev > 0.0 && val <= 0.0),
                // First finite observation: only an exact zero counts, since
                // there is no prior sign to compare against.
                None => val == 0.0,
            };
            previous.insert(key, val);
            if crossed {
                events.push((block_id.clone(), i as Scalar));
            }
        }
    }

    // Drop history for crossing signals that no longer exist, so a later signal
    // that reuses the same index is not compared against an unrelated value.
    previous.retain(|k, _| seen.contains(k));
    events
}

/// Stateless convenience wrapper around [`execute_event_detection_with_state`].
///
/// Without a sign history only exact zeros can be detected, so this is suitable
/// for a one-shot scan but **not** for detecting crossings across steps. Callers
/// that sample a running simulation must keep the `previous` map.
pub fn execute_event_detection(diagram: &Diagram, order: &[BlockId]) -> Vec<(BlockId, Scalar)> {
    let mut previous = HashMap::new();
    execute_event_detection_with_state(diagram, order, &mut previous)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;

    #[test]
    fn test_phase_enum_all_phases() {
        let phases = SchedulePhase::all();
        assert_eq!(phases.len(), 8);
    }

    #[test]
    fn test_classify_default_discrete() {
        let mut diagram = Diagram::new("test");
        diagram.add_block(Box::new(SimpleBlock::new("B1", "Gain")));
        let classes = classify_blocks(&diagram);
        assert_eq!(classes.get("B1"), Some(&BlockTaskType::Discrete));
    }

    #[test]
    fn test_schedule_phase_count() {
        let diagram = Diagram::new("test");
        let config = ScheduleConfig::default();
        let phases = build_schedule(&diagram, &config);
        assert_eq!(phases.len(), 8);
        assert_eq!(phases, SchedulePhase::all().to_vec());
    }

    #[test]
    fn test_schedule_config_disables_signal_propagation() {
        let diagram = Diagram::new("test");
        let config = ScheduleConfig {
            enable_signal_propagation: false,
            ..ScheduleConfig::default()
        };
        let phases = build_schedule(&diagram, &config);

        assert!(
            !phases.contains(&SchedulePhase::PropagateSignals),
            "PropagateSignals must be omitted when signal propagation is disabled"
        );
        assert_eq!(phases.len(), 7);
        // Everything else is untouched.
        assert_eq!(
            phases,
            SchedulePhase::all()
                .into_iter()
                .filter(|p| *p != SchedulePhase::PropagateSignals)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_schedule_config_zero_event_capacity_disables_event_phases() {
        let diagram = Diagram::new("test");
        let config = ScheduleConfig {
            event_queue_capacity: 0,
            ..ScheduleConfig::default()
        };
        let phases = build_schedule(&diagram, &config);

        assert!(!phases.contains(&SchedulePhase::DetectEvents));
        assert!(!phases.contains(&SchedulePhase::HandleEvents));
        assert_eq!(phases.len(), 6);
    }

    #[test]
    fn test_schedule_config_discrete_step_prunes_unneeded_continuous_phases() {
        // A purely discrete diagram: no continuous state, no event ports.
        let mut diagram = Diagram::new("discrete_only");
        diagram.add_block(Box::new(SimpleBlock::new("B1", "Gain")));

        let config = ScheduleConfig {
            discrete_step: Some(0.01),
            ..ScheduleConfig::default()
        };
        let phases = build_schedule(&diagram, &config);

        // The hybrid multi-rate schedule runs discrete updates every step but
        // must not schedule phases the diagram cannot use.
        assert!(phases.contains(&SchedulePhase::UpdateDiscrete));
        assert!(phases.contains(&SchedulePhase::ComputeOutputs));
        assert!(phases.contains(&SchedulePhase::PropagateSignals));
        assert!(!phases.contains(&SchedulePhase::IntegrateStates));
        assert!(!phases.contains(&SchedulePhase::ComputeDerivs));
        assert!(!phases.contains(&SchedulePhase::DetectEvents));
        assert!(!phases.contains(&SchedulePhase::HandleEvents));
        assert_eq!(phases.len(), 4);
    }

    #[test]
    fn test_schedule_config_discrete_step_keeps_continuous_phases() {
        let mut diagram = Diagram::new("hybrid");
        let mut continuous = SimpleBlock::new("C1", "Integrator");
        continuous.declare_input("u", crate::core::types::SignalType::Continuous);
        continuous.declare_output("y", crate::core::types::SignalType::Continuous);
        continuous.add_continuous_state("x", 0.0);
        diagram.add_block(Box::new(continuous));

        let config = ScheduleConfig {
            discrete_step: Some(0.01),
            ..ScheduleConfig::default()
        };
        let phases = build_schedule(&diagram, &config);

        // Continuous state exists, so the derivative + integration phases stay;
        // no event ports exist, so the event phases are dropped.
        assert!(phases.contains(&SchedulePhase::ComputeDerivs));
        assert!(phases.contains(&SchedulePhase::IntegrateStates));
        assert!(!phases.contains(&SchedulePhase::DetectEvents));
        assert!(!phases.contains(&SchedulePhase::HandleEvents));
        assert_eq!(phases.len(), 6);
    }

    #[test]
    fn test_schedule_config_discrete_step_keeps_event_phases_with_event_ports() {
        let mut diagram = Diagram::new("event_driven");
        let mut event_block = SimpleBlock::new("E1", "EventSource");
        event_block.declare_output("evt", crate::core::types::SignalType::Event);
        diagram.add_block(Box::new(event_block));

        let config = ScheduleConfig {
            discrete_step: Some(0.01),
            ..ScheduleConfig::default()
        };
        let phases = build_schedule(&diagram, &config);

        assert!(phases.contains(&SchedulePhase::DetectEvents));
        assert!(phases.contains(&SchedulePhase::HandleEvents));
        assert!(!phases.contains(&SchedulePhase::IntegrateStates));
        assert!(!phases.contains(&SchedulePhase::ComputeDerivs));
        assert_eq!(phases.len(), 6);
    }

    #[test]
    fn test_execute_event_detection_reports_crossing_index() {
        let mut diagram = Diagram::new("crossing");
        diagram.add_block(Box::new(CrossingBlock::new("X1", vec![1.0, 0.0, 2.0])));

        let events = execute_event_detection(&diagram, &["X1".to_string()]);
        assert_eq!(events, vec![("X1".to_string(), 1.0)]);
    }

    /// The old detector required `|val| < 1e-12`, so a signal stepping from
    /// `-1e-6` to `+1e-6` — a genuine crossing — was never reported. Detection
    /// must work from the *sign change* between samples.
    #[test]
    fn test_event_detection_finds_a_sign_change_between_samples() {
        let mut diagram = Diagram::new("sign");
        // A single crossing signal, which we will drive through zero.
        diagram.add_block(Box::new(CrossingBlock::new("B", vec![-1e-6])));
        let order = vec!["B".to_string()];
        let mut history = HashMap::new();

        // First sample: negative, and this is the first observation, so no
        // crossing yet (there is no previous sign to compare against).
        let first = execute_event_detection_with_state(&diagram, &order, &mut history);
        assert!(
            first.is_empty(),
            "the first observation cannot establish a crossing, got {first:?}"
        );

        // Second sample: positive. The signal crossed zero in between, even
        // though it was never within 1e-12 of zero at a sampled instant.
        diagram.remove_block("B");
        diagram.add_block(Box::new(CrossingBlock::new("B", vec![1e-6])));
        let second = execute_event_detection_with_state(&diagram, &order, &mut history);
        assert_eq!(
            second,
            vec![("B".to_string(), 0.0)],
            "a sign change must be reported as a crossing"
        );
    }

    /// Staying on one side of zero must not produce repeated crossings.
    #[test]
    fn test_event_detection_does_not_refire_without_a_sign_change() {
        let order = vec!["B".to_string()];
        let mut history = HashMap::new();

        for value in [1.0, 2.0, 3.0, 0.5] {
            let mut diagram = Diagram::new("same_sign");
            diagram.add_block(Box::new(CrossingBlock::new("B", vec![value])));
            let events = execute_event_detection_with_state(&diagram, &order, &mut history);
            assert!(
                events.is_empty(),
                "value {value} keeps the same sign, so it is not a crossing"
            );
        }
    }

    /// A signal that is exactly zero is a crossing even with no sign history.
    #[test]
    fn test_event_detection_reports_an_exact_zero() {
        let mut diagram = Diagram::new("zero");
        diagram.add_block(Box::new(CrossingBlock::new("Z", vec![0.0])));
        let mut history = HashMap::new();
        let events = execute_event_detection_with_state(&diagram, &["Z".to_string()], &mut history);
        assert_eq!(events, vec![("Z".to_string(), 0.0)]);
    }

    /// A non-finite crossing signal must be skipped, not reported as a crossing
    /// and not allowed to poison the history.
    #[test]
    fn test_event_detection_skips_non_finite_values() {
        let mut diagram = Diagram::new("nan");
        diagram.add_block(Box::new(CrossingBlock::new(
            "N",
            vec![Scalar::NAN, Scalar::INFINITY],
        )));
        let mut history = HashMap::new();
        let events = execute_event_detection_with_state(&diagram, &["N".to_string()], &mut history);
        assert!(events.is_empty(), "non-finite values are not crossings");
        assert!(
            history.is_empty(),
            "non-finite values must not be recorded as a prior sign"
        );
    }

    /// Multiple crossing signals on one block are reported with their own index.
    #[test]
    fn test_event_detection_reports_each_crossing_index_independently() {
        let order = vec!["M".to_string()];
        let mut history = HashMap::new();

        // Seed: index 0 negative, index 1 positive.
        let mut diagram = Diagram::new("multi");
        diagram.add_block(Box::new(CrossingBlock::new("M", vec![-1.0, 1.0])));
        assert!(execute_event_detection_with_state(&diagram, &order, &mut history).is_empty());

        // Only index 1 flips sign.
        diagram.remove_block("M");
        diagram.add_block(Box::new(CrossingBlock::new("M", vec![-0.5, -1.0])));
        let events = execute_event_detection_with_state(&diagram, &order, &mut history);
        assert_eq!(
            events,
            vec![("M".to_string(), 1.0)],
            "only the crossing signal whose sign flipped may fire"
        );
    }

    /// Test block that reports a fixed set of zero-crossing signals.
    #[derive(Debug, Clone)]
    struct CrossingBlock {
        inner: SimpleBlock,
        crossings: Vec<Scalar>,
    }

    impl CrossingBlock {
        fn new(id: &str, crossings: Vec<Scalar>) -> Self {
            Self {
                inner: SimpleBlock::new(id, "Crossing"),
                crossings,
            }
        }
    }

    impl crate::core::block::Block for CrossingBlock {
        fn id(&self) -> &BlockId {
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
        fn set_status(&mut self, status: crate::core::types::ComponentStatus) {
            self.inner.set_status(status);
        }
        fn set_time(&mut self, time: crate::core::types::Time) {
            self.inner.set_time(time);
        }
        fn time(&self) -> crate::core::types::Time {
            self.inner.time()
        }
        fn init(&mut self) -> Result<(), SimError> {
            Ok(())
        }
        fn output(&mut self) -> Result<(), SimError> {
            Ok(())
        }
        fn derivative(&self) -> Result<Vec<Scalar>, SimError> {
            Ok(Vec::new())
        }
        fn update(&mut self) -> Result<(), SimError> {
            Ok(())
        }
        fn zero_crossings(&self) -> Vec<Scalar> {
            self.crossings.clone()
        }
        fn terminate(&mut self) -> Result<(), SimError> {
            Ok(())
        }
        fn clone_block(&self) -> Box<dyn crate::core::block::Block> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn test_execute_output_phase() {
        let mut diagram = Diagram::new("test");
        diagram.add_block(Box::new(SimpleBlock::new("B1", "Test")));
        // Should not error
        assert!(execute_output_phase(&diagram, &["B1".to_string()]).is_ok());
    }

    #[test]
    fn test_deriv_phase_empty() {
        let mut diagram = Diagram::new("test");
        diagram.add_block(Box::new(SimpleBlock::new("B1", "Test")));
        let derivs = execute_deriv_phase(&diagram, &["B1".to_string()]).unwrap();
        assert!(derivs.is_empty());
    }

    /// A signal that reaches exactly zero and returns to the same side has
    /// **not** crossed. The previous `sign_change || val == 0.0` test reported
    /// the zero sample as a crossing even when the signal came from and returned
    /// to the same sign.
    ///
    /// Note: `-1 → 0` *is* a legitimate crossing (the signal moved from negative
    /// to non-negative), so the test starts from zero to isolate the case where
    /// the touch carries no sign change at all.
    #[test]
    fn test_zero_touch_without_sign_change_is_not_a_crossing() {
        let order = vec!["T".to_string()];
        let mut history = HashMap::new();

        // Seed at exactly zero (first observation: one crossing, then history).
        let mut diagram = Diagram::new("touch");
        diagram.add_block(Box::new(CrossingBlock::new("T", vec![0.0])));
        let first = execute_event_detection_with_state(&diagram, &order, &mut history);
        assert_eq!(first.len(), 1, "the first zero observation is a crossing");

        // Return to zero repeatedly: the sign is unchanged, so this must not
        // re-fire even though the value is exactly zero.
        for _ in 0..5 {
            let events = execute_event_detection_with_state(&diagram, &order, &mut history);
            assert!(
                events.is_empty(),
                "a repeated zero with no sign change is not a new crossing"
            );
        }
    }

    /// A signal parked at exactly zero must not fire a crossing on every step.
    #[test]
    fn test_signal_stuck_at_zero_does_not_refire() {
        let order = vec!["Z".to_string()];
        let mut history = HashMap::new();
        let mut total = 0;

        for _ in 0..10 {
            let mut diagram = Diagram::new("stuck");
            diagram.add_block(Box::new(CrossingBlock::new("Z", vec![0.0])));
            total += execute_event_detection_with_state(&diagram, &order, &mut history).len();
        }

        assert_eq!(
            total, 1,
            "a signal at zero may report the initial crossing once, not every step"
        );
    }

    /// A non-finite sample must reset the history, so the next finite sample is
    /// not compared against a value from before the gap.
    #[test]
    fn test_non_finite_sample_resets_history_across_the_gap() {
        let order = vec!["G".to_string()];
        let mut history = HashMap::new();

        // Seed with a negative value.
        let mut diagram = Diagram::new("gap");
        diagram.add_block(Box::new(CrossingBlock::new("G", vec![-1.0])));
        assert!(execute_event_detection_with_state(&diagram, &order, &mut history).is_empty());

        // A NaN sample must clear the history for this crossing signal.
        diagram.remove_block("G");
        diagram.add_block(Box::new(CrossingBlock::new("G", vec![Scalar::NAN])));
        assert!(execute_event_detection_with_state(&diagram, &order, &mut history).is_empty());
        assert!(
            history.is_empty(),
            "a non-finite sample must forget the prior sign, not carry it across"
        );

        // The next positive sample must therefore be treated as a fresh start,
        // not as a sign change from the pre-gap negative value.
        diagram.remove_block("G");
        diagram.add_block(Box::new(CrossingBlock::new("G", vec![1.0])));
        let events = execute_event_detection_with_state(&diagram, &order, &mut history);
        assert!(
            events.is_empty(),
            "the crossing must not be attributed across a NaN gap, got {events:?}"
        );
    }

    /// History for a crossing signal that disappears must be dropped, so a later
    /// signal reusing the same index is not compared against an unrelated value.
    #[test]
    fn test_stale_crossing_history_is_pruned() {
        let order = vec!["S".to_string()];
        let mut history = HashMap::new();

        // Step 1: two signals, index 1 is positive.
        let mut diagram = Diagram::new("stale");
        diagram.add_block(Box::new(CrossingBlock::new("S", vec![-5.0, 7.0])));
        assert!(execute_event_detection_with_state(&diagram, &order, &mut history).is_empty());
        assert_eq!(history.len(), 2, "both indices must be tracked");

        // Step 2: back to one signal. Index 1 no longer exists.
        diagram.remove_block("S");
        diagram.add_block(Box::new(CrossingBlock::new("S", vec![-5.0])));
        assert!(execute_event_detection_with_state(&diagram, &order, &mut history).is_empty());
        assert_eq!(
            history.len(),
            1,
            "history for the vanished crossing signal must be pruned"
        );
    }
}
