//! Core scheduler trait and supporting types.
//!
//! Defines the `Scheduler` interface that all scheduling strategies implement,
//! along with configuration and result types used throughout the scheduler module.

use super::signal_prop::SignalCache;
use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::runtime::event::EventQueue;

/// Result of a single scheduler step.
#[derive(Debug, Clone, PartialEq)]
pub enum ScheduleStepResult {
    /// Step completed normally, continue execution.
    StepCompleted,
    /// Simulation finished (end time reached or all blocks completed).
    Finished,
    /// Simulation paused by user or breakpoint.
    Paused,
    /// Breakpoint condition was met.
    BreakpointReached,
    /// An error occurred during the step.
    Error(SimError),
}

impl ScheduleStepResult {
    /// Returns `true` if execution should stop
    /// (finished, paused, breakpoint, or error).
    pub fn should_stop(&self) -> bool {
        matches!(
            self,
            Self::Finished | Self::Paused | Self::BreakpointReached | Self::Error(_)
        )
    }
}

/// The core trait that all scheduler implementations must provide.
pub trait Scheduler: Send + Sync {
    /// Human-readable name of this scheduling strategy.
    fn name(&self) -> &str;

    /// Initialize the scheduler from a diagram.
    ///
    /// Computes execution order, signal flow analysis, and sets up internal state.
    fn initialize(&mut self, diagram: &Diagram) -> Result<(), SimError>;

    /// Execute a single complete scheduling step.
    fn step(&mut self, ctx: &mut ScheduleContext) -> Result<ScheduleStepResult, SimError>;

    /// Re-schedule when the diagram changes.
    fn reschedule(&mut self, diagram: &Diagram) -> Result<(), SimError>;

    /// Get the current topological execution order.
    fn execution_order(&self) -> &[BlockId];

    /// Get a mutable reference to the scheduler's signal cache.
    ///
    /// Used by the engine to write block output values before propagation.
    fn signal_cache_mut(&mut self) -> &mut SignalCache;

    /// Advance the signal cache to the next time step.
    ///
    /// Moves current values to previous (for edge detection) and resets
    /// current values. Called by the engine at the end of each step.
    fn advance_cache(&mut self);
}

/// Execution context passed to the scheduler for each step.
pub struct ScheduleContext<'a> {
    /// The simulation diagram.
    pub diagram: &'a Diagram,
    /// Topological execution order.
    pub execution_order: &'a [BlockId],
    /// Current simulation time.
    pub current_time: crate::core::types::Time,
    /// Current step size.
    pub dt: crate::core::types::Scalar,
    /// Event queue for pending events.
    pub event_queue: &'a mut EventQueue,
    /// Signal cache for port values.
    pub signal_cache: &'a mut SignalCache,
}

/// Sequential scheduler — executes blocks one by one in topological order.
///
/// This is the default scheduler that orchestrates the 8-phase execution cycle.
/// It supports signal propagation, event handling, and optional ODE solver integration.
#[derive(Debug, Clone)]
pub struct SequentialScheduler {
    order: Vec<BlockId>,
    signal_cache: SignalCache,
    event_queue: EventQueue,
}

impl SequentialScheduler {
    /// Create a new sequential scheduler with default settings.
    pub fn new() -> Self {
        Self {
            order: Vec::new(),
            signal_cache: SignalCache::new(),
            event_queue: EventQueue::new(),
        }
    }

    /// Get a reference to the signal cache.
    pub fn signal_cache(&self) -> &SignalCache {
        &self.signal_cache
    }

    /// Get a mutable reference to the signal cache.
    pub fn signal_cache_mut(&mut self) -> &mut SignalCache {
        &mut self.signal_cache
    }

    /// Get a reference to the event queue.
    pub fn event_queue(&self) -> &EventQueue {
        &self.event_queue
    }

    /// Get a mutable reference to the event queue.
    pub fn event_queue_mut(&mut self) -> &mut EventQueue {
        &mut self.event_queue
    }
}

impl Default for SequentialScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for SequentialScheduler {
    fn name(&self) -> &str {
        "Sequential"
    }

    fn initialize(&mut self, diagram: &Diagram) -> Result<(), SimError> {
        let graph = super::topo::DiGraph::from_diagram(diagram);
        self.order = graph.topological_sort().map_err(|cycle| {
            SimError::runtime(format!(
                "cycle detected in diagram: {} blocks in cycle",
                cycle.len()
            ))
        })?;
        self.signal_cache = SignalCache::from_diagram(diagram);
        Ok(())
    }

    fn step(&mut self, ctx: &mut ScheduleContext) -> Result<ScheduleStepResult, SimError> {
        // Phase 1: Compute outputs
        super::hybrid::execute_output_phase(ctx.diagram, &self.order)?;

        // Phase 2: Propagate signals
        super::signal_prop::propagate_signals(ctx.diagram, &mut self.signal_cache)?;

        // Phase 3+: derivatives + integration handled externally by engine

        // Phase 5: Update discrete
        super::hybrid::execute_update_phase(ctx.diagram, &self.order)?;

        // Phase 6: Detect events
        super::hybrid::execute_event_detection(ctx.diagram, &self.order);

        // Phase 7: Handle events
        let _events = self.event_queue.drain_up_to(ctx.current_time);

        // Phase 8: Advance cache
        self.signal_cache.advance();

        Ok(ScheduleStepResult::StepCompleted)
    }

    fn reschedule(&mut self, diagram: &Diagram) -> Result<(), SimError> {
        self.initialize(diagram)
    }

    fn execution_order(&self) -> &[BlockId] {
        &self.order
    }

    fn signal_cache_mut(&mut self) -> &mut SignalCache {
        &mut self.signal_cache
    }

    fn advance_cache(&mut self) {
        self.signal_cache.advance();
    }
}
