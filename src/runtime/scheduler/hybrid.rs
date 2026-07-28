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

/// Build a full 8-phase schedule for a single time step.
pub fn build_schedule(_diagram: &Diagram, _config: &ScheduleConfig) -> Vec<SchedulePhase> {
    SchedulePhase::all().to_vec()
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
