//! Hybrid scheduler for mixed continuous/discrete/event/multi-rate systems.
//!
//! Provides the standard 8-phase execution cycle: ComputeOutputs → PropagateSignals →
//! ComputeDerivs → IntegrateStates → UpdateDiscrete → DetectEvents →
//! HandleEvents → AdvanceTime.

use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::types::Scalar;
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
pub fn execute_update_phase(diagram: &Diagram, order: &[BlockId]) -> Result<(), SimError> {
    for block_id in order {
        if diagram.get_block(block_id).is_none() {
            return Err(SimError::runtime(format!(
                "execute_update_phase: block '{}' not found",
                block_id
            )));
        }
    }
    Ok(())
}

/// Execute the DetectEvents phase (zero-crossing detection).
///
/// A crossing is reported for every zero-crossing signal whose value is within
/// `1e-12` of zero. The second tuple element is the index of the crossing
/// within the block's `zero_crossings()` result — the only stable identifier a
/// zero crossing carries.
pub fn execute_event_detection(diagram: &Diagram, order: &[BlockId]) -> Vec<(BlockId, Scalar)> {
    let mut events = Vec::new();
    for block_id in order {
        if let Some(block) = diagram.get_block(block_id) {
            let crossings = block.zero_crossings();
            for (i, &val) in crossings.iter().enumerate() {
                if val.abs() < 1e-12 {
                    events.push((block_id.clone(), i as Scalar));
                }
            }
        }
    }
    events
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
}
