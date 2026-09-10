//! Core scheduler trait and supporting types.
//!
//! Defines the `Scheduler` interface that all scheduling strategies implement,
//! along with configuration and result types used throughout the scheduler module.

use super::signal_prop::SignalCache;
use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::types::{ExecutionPhase, SignalValue};
use crate::runtime::event::{Event, EventQueue, EventType};
use std::collections::HashMap;

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

    /// Get a reference to the signal cache.
    fn signal_cache(&self) -> &SignalCache;

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
///
/// Zero-crossings detected during the DetectEvents phase are enqueued into the
/// scheduler's own [`EventQueue`] and then dispatched (see [`SequentialScheduler::dispatch_event`])
/// during the HandleEvents phase of the same step.
#[derive(Debug, Clone)]
pub struct SequentialScheduler {
    order: Vec<BlockId>,
    signal_cache: SignalCache,
    event_queue: EventQueue,
    /// Monotonic counter used to build unique event identifiers.
    event_sequence: u64,
    /// Blocks that were dispatched events, keyed by block id, with the number
    /// of events delivered to them. Observable proof that HandleEvents ran.
    dispatched_events: HashMap<BlockId, u64>,
}

impl SequentialScheduler {
    /// Create a new sequential scheduler with default settings.
    pub fn new() -> Self {
        Self {
            order: Vec::new(),
            signal_cache: SignalCache::new(),
            event_queue: EventQueue::new(),
            event_sequence: 0,
            dispatched_events: HashMap::new(),
        }
    }

    /// Get a reference to the event queue.
    pub fn event_queue(&self) -> &EventQueue {
        &self.event_queue
    }

    /// Get a mutable reference to the event queue.
    pub fn event_queue_mut(&mut self) -> &mut EventQueue {
        &mut self.event_queue
    }

    /// Number of events dispatched to `block_id` since the scheduler was created.
    ///
    /// Returns `0` for a block that has never received an event.
    pub fn dispatched_event_count(&self, block_id: &str) -> u64 {
        self.dispatched_events.get(block_id).copied().unwrap_or(0)
    }

    /// Total number of events dispatched across all blocks.
    pub fn total_dispatched_events(&self) -> u64 {
        self.dispatched_events.values().sum()
    }

    /// Enqueue the zero-crossings detected by [`super::hybrid::execute_event_detection`].
    ///
    /// Each crossing becomes a [`EventType::ZeroCrossing`] event scheduled at
    /// `current_time`. Returns the number of events successfully enqueued; a
    /// crossing whose event is refused (queue at capacity) is skipped instead of
    /// aborting the whole step.
    fn enqueue_detected_events(
        &mut self,
        crossings: &[(BlockId, crate::core::types::Scalar)],
        current_time: crate::core::types::Time,
    ) -> usize {
        let mut enqueued = 0;
        for (block_id, crossing_index) in crossings {
            self.event_sequence += 1;
            let id = format!("zc:{block_id}:{crossing_index}:{}", self.event_sequence);
            let event = Event::new(
                &id,
                current_time,
                EventType::ZeroCrossing,
                SignalValue::Scalar(*crossing_index),
            );
            if self.event_queue.push(event).is_ok() {
                enqueued += 1;
            }
        }
        enqueued
    }

    /// Dispatch the events drained from the queue to the block they refer to.
    ///
    /// Blocks are woken through their `ExecutionPhase::Event` hook, and the
    /// delivery is recorded in [`Self::dispatched_event_count`]. An event whose
    /// block is missing from the diagram is reported as an error rather than
    /// silently dropped.
    fn dispatch_events(
        &mut self,
        mut events: Vec<Event>,
        diagram: &Diagram,
    ) -> Result<usize, SimError> {
        // Pop from the end so the earliest event is dispatched first.
        events.reverse();

        let mut dispatched = 0;
        for event in events {
            let block_id = event.id.split(':').nth(1).unwrap_or_default().to_string();
            self.dispatch_event(&block_id, event.event_type, diagram)?;
            dispatched += 1;
        }
        Ok(dispatched)
    }

    /// Deliver a single event to its owning block.
    ///
    /// The dispatcher only needs read access to the diagram (phase 6 already
    /// ran), so it can reuse the `&Diagram` borrowed by `ScheduleContext` even
    /// though `self` is mutably borrowed to record the delivery.
    fn dispatch_event(
        &mut self,
        block_id: &str,
        event_type: EventType,
        diagram: &Diagram,
    ) -> Result<(), SimError> {
        // Only blocks scheduled for this step are dispatchable; this mirrors the
        // validation in `step` and keeps the borrow checker happy by cloning the
        // block into a local dispatcher.
        let mut block = diagram
            .get_block(block_id)
            .ok_or_else(|| {
                SimError::runtime(format!(
                    "dispatch_event: block '{block_id}' not found in diagram"
                ))
            })?
            .clone_block();

        block.execute_phase(ExecutionPhase::Event).map_err(|e| {
            SimError::runtime(format!(
                "dispatch_event: block '{block_id}' failed handling {event_type:?}: {e}"
            ))
        })?;

        *self
            .dispatched_events
            .entry(block_id.to_string())
            .or_insert(0) += 1;
        Ok(())
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
        // Reject diagrams whose block set changed without a reschedule; every
        // phase below indexes blocks by id and must not silently skip work.
        for block_id in &self.order {
            if ctx.diagram.get_block(block_id).is_none() {
                return Err(SimError::runtime(format!(
                    "SequentialScheduler: block '{block_id}' is missing from the diagram; \
                     call reschedule() after modifying the diagram"
                )));
            }
        }

        // Phase 1: Compute outputs
        super::hybrid::execute_output_phase(ctx.diagram, &self.order)?;

        // Phase 2: Propagate signals
        super::signal_prop::propagate_signals(ctx.diagram, &mut self.signal_cache)?;

        // Phase 3+: derivatives + integration handled externally by engine

        // Phase 5: Update discrete
        super::hybrid::execute_update_phase(ctx.diagram, &self.order)?;

        // Phase 6: Detect events
        let crossings = super::hybrid::execute_event_detection(ctx.diagram, &self.order);

        // Phase 7: Handle events — enqueue the crossings detected above and
        // dispatch every event that is due at the current time.
        self.enqueue_detected_events(&crossings, ctx.current_time);

        // The engine's queue is the shared event channel: merge anything it has
        // scheduled with the scheduler's own crossings for this step.
        let due = self.event_queue.drain_up_to(ctx.current_time);
        let mut events = ctx.event_queue.drain_up_to(ctx.current_time);
        events.extend(due);
        self.dispatch_events(events, ctx.diagram)?;

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

    fn signal_cache(&self) -> &SignalCache {
        &self.signal_cache
    }

    fn signal_cache_mut(&mut self) -> &mut SignalCache {
        &mut self.signal_cache
    }

    fn advance_cache(&mut self) {
        self.signal_cache.advance();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::{Block, SimpleBlock};
    use crate::core::param::ParameterSet;
    use crate::core::port::PortSet;
    use crate::core::types::{ComponentStatus, Scalar, SignalType, SignalValue, Time};
    use std::sync::{Arc, Mutex};

    /// Test block that reports a fixed set of zero-crossing signals and records
    /// every event dispatch it receives.
    struct CrossingRecorder {
        inner: SimpleBlock,
        crossings: Vec<Scalar>,
        dispatched: Arc<Mutex<Vec<EventType>>>,
    }

    impl CrossingRecorder {
        fn new(id: &str, crossings: Vec<Scalar>) -> Self {
            let mut inner = SimpleBlock::new(id, "CrossingRecorder");
            inner.declare_output("y", SignalType::Continuous);
            Self {
                inner,
                crossings,
                dispatched: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn dispatched_handle(&self) -> Arc<Mutex<Vec<EventType>>> {
            Arc::clone(&self.dispatched)
        }
    }

    impl Block for CrossingRecorder {
        fn id(&self) -> &BlockId {
            self.inner.id()
        }
        fn block_type(&self) -> &str {
            self.inner.block_type()
        }
        fn ports(&self) -> &PortSet {
            self.inner.ports()
        }
        fn ports_mut(&mut self) -> &mut PortSet {
            self.inner.ports_mut()
        }
        fn params(&self) -> &ParameterSet {
            self.inner.params()
        }
        fn params_mut(&mut self) -> &mut ParameterSet {
            self.inner.params_mut()
        }
        fn status(&self) -> ComponentStatus {
            self.inner.status()
        }
        fn set_status(&mut self, status: ComponentStatus) {
            self.inner.set_status(status);
        }
        fn set_time(&mut self, time: Time) {
            self.inner.set_time(time);
        }
        fn time(&self) -> Time {
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
        // The Event execution phase is the dispatch hook used by the scheduler.
        fn execute_phase(&mut self, phase: ExecutionPhase) -> Result<(), SimError> {
            if phase == ExecutionPhase::Event {
                self.dispatched
                    .lock()
                    .unwrap()
                    .push(EventType::ZeroCrossing);
            }
            Ok(())
        }
        fn clone_block(&self) -> Box<dyn Block> {
            Box::new(Self {
                inner: self.inner.clone(),
                crossings: self.crossings.clone(),
                dispatched: Arc::clone(&self.dispatched),
            })
        }
    }

    /// Build a diagram containing a single crossing recorder and return the
    /// handle that observes its event dispatches.
    fn crossing_diagram(crossings: Vec<Scalar>) -> (Diagram, Arc<Mutex<Vec<EventType>>>) {
        let block = CrossingRecorder::new("X1", crossings);
        let handle = block.dispatched_handle();
        let mut diagram = Diagram::new("crossing");
        diagram.add_block(Box::new(block));
        (diagram, handle)
    }

    fn ctx<'a>(
        diagram: &'a Diagram,
        order: &'a [BlockId],
        event_queue: &'a mut EventQueue,
        signal_cache: &'a mut SignalCache,
        current_time: Time,
    ) -> ScheduleContext<'a> {
        ScheduleContext {
            diagram,
            execution_order: order,
            current_time,
            dt: 0.1,
            event_queue,
            signal_cache,
        }
    }

    #[test]
    fn test_step_enqueues_and_dispatches_detected_crossing() {
        let (diagram, dispatched) = crossing_diagram(vec![1.0, 0.0, 2.0]);

        let mut scheduler = SequentialScheduler::new();
        scheduler
            .initialize(&diagram)
            .expect("initialize should succeed");
        assert_eq!(scheduler.execution_order().len(), 1);

        let order = scheduler.execution_order().to_vec();
        let mut engine_queue = EventQueue::new();
        let mut cache = SignalCache::new();

        // The crossing is detected at t = 0.5 and is due immediately.
        {
            let mut ctx = ctx(&diagram, &order, &mut engine_queue, &mut cache, 0.5);
            let result = scheduler.step(&mut ctx).expect("step should succeed");
            assert_eq!(result, ScheduleStepResult::StepCompleted);
        }

        // 1. The detected crossing was enqueued and then drained/dispatched, so
        //    nothing is left pending in either queue.
        assert_eq!(scheduler.event_queue().len(), 0);
        assert!(scheduler.event_queue().is_empty());
        assert_eq!(engine_queue.len(), 0);

        // 2. The queue really did hold the event: `processed` proves a pop
        //    happened (drain), and the dispatch bookkeeping proves delivery.
        assert_eq!(scheduler.event_queue().processed, 1);
        assert_eq!(scheduler.dispatched_event_count("X1"), 1);
        assert_eq!(scheduler.total_dispatched_events(), 1);

        // 3. The block's own Event hook observed the dispatch.
        assert_eq!(dispatched.lock().unwrap().len(), 1);

        // The step is no longer a no-op: a second step with no new events
        // dispatches nothing further.
        let order = scheduler.execution_order().to_vec();
        {
            let mut ctx = ctx(&diagram, &order, &mut engine_queue, &mut cache, 1.0);
            scheduler.step(&mut ctx).expect("step should succeed");
        }
        assert_eq!(scheduler.dispatched_event_count("X1"), 2);
        assert_eq!(dispatched.lock().unwrap().len(), 2);
    }

    #[test]
    fn test_step_leaves_future_events_queued() {
        let (diagram, _dispatched) = crossing_diagram(Vec::new());

        let mut scheduler = SequentialScheduler::new();
        scheduler.initialize(&diagram).expect("initialize");
        let order = scheduler.execution_order().to_vec();

        // A crossing scheduled for t = 5.0 must not be handled at t = 0.0.
        scheduler
            .event_queue_mut()
            .push(Event::new(
                "zc:X1:0:99",
                5.0,
                EventType::ZeroCrossing,
                SignalValue::Scalar(0.0),
            ))
            .expect("push");

        let mut engine_queue = EventQueue::new();
        let mut cache = SignalCache::new();
        {
            let mut ctx = ctx(&diagram, &order, &mut engine_queue, &mut cache, 0.0);
            scheduler.step(&mut ctx).expect("step");
        }

        assert_eq!(
            scheduler.event_queue().len(),
            1,
            "future event stays queued"
        );
        assert_eq!(scheduler.dispatched_event_count("X1"), 0);
    }

    #[test]
    fn test_step_dispatches_engine_queue_events() {
        let (diagram, dispatched) = crossing_diagram(Vec::new());

        let mut scheduler = SequentialScheduler::new();
        scheduler.initialize(&diagram).expect("initialize");
        let order = scheduler.execution_order().to_vec();

        // The engine hands the scheduler a due event through the context queue.
        let mut engine_queue = EventQueue::new();
        engine_queue
            .push(Event::new(
                "zc:X1:0:1",
                0.0,
                EventType::ZeroCrossing,
                SignalValue::Scalar(0.0),
            ))
            .expect("push");
        let mut cache = SignalCache::new();

        {
            let mut ctx = ctx(&diagram, &order, &mut engine_queue, &mut cache, 0.0);
            scheduler.step(&mut ctx).expect("step");
        }

        assert_eq!(engine_queue.len(), 0, "engine queue must be drained");
        assert_eq!(scheduler.total_dispatched_events(), 1);
        assert_eq!(dispatched.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_dispatch_event_errors_when_block_missing() {
        let diagram = Diagram::new("empty");
        let mut scheduler = SequentialScheduler::new();

        let events = vec![Event::new(
            "zc:ghost:0:1",
            0.0,
            EventType::ZeroCrossing,
            SignalValue::Scalar(0.0),
        )];

        let err = scheduler
            .dispatch_events(events, &diagram)
            .expect_err("dispatch must fail when the event block is missing");
        assert!(format!("{err}").contains("ghost"));
        assert_eq!(scheduler.total_dispatched_events(), 0);
    }

    #[test]
    fn test_step_errors_when_event_references_missing_block() {
        let diagram = Diagram::new("empty");
        let mut scheduler = SequentialScheduler::new();
        let order: Vec<BlockId> = Vec::new();
        let mut engine_queue = EventQueue::new();
        engine_queue
            .push(Event::new(
                "zc:ghost:0:1",
                0.0,
                EventType::ZeroCrossing,
                SignalValue::Scalar(0.0),
            ))
            .expect("push");
        let mut cache = SignalCache::new();

        let err = {
            let mut ctx = ctx(&diagram, &order, &mut engine_queue, &mut cache, 0.0);
            scheduler.step(&mut ctx)
        }
        .expect_err("step must fail when an event references a missing block");
        assert!(format!("{err}").contains("ghost"));
    }

    #[test]
    fn test_step_after_reschedule_with_new_block() {
        let mut diagram = Diagram::new("empty");
        let mut scheduler = SequentialScheduler::new();
        scheduler.initialize(&diagram).expect("initialize");

        // The diagram learns about a block after the scheduler was initialized;
        // rescheduling must pick it up and the step must then include it.
        diagram.add_block(Box::new(SimpleBlock::new("B1", "Test")));
        scheduler.reschedule(&diagram).expect("reschedule");
        assert_eq!(scheduler.execution_order(), &["B1".to_string()]);

        let order = scheduler.execution_order().to_vec();
        let mut engine_queue = EventQueue::new();
        let mut cache = SignalCache::new();
        {
            let mut ctx = ctx(&diagram, &order, &mut engine_queue, &mut cache, 0.0);
            scheduler.step(&mut ctx).expect("step after reschedule");
        }
    }

    #[test]
    fn test_new_scheduler_starts_with_empty_event_state() {
        let scheduler = SequentialScheduler::new();
        assert!(scheduler.event_queue().is_empty());
        assert_eq!(scheduler.total_dispatched_events(), 0);
        assert_eq!(scheduler.dispatched_event_count("anything"), 0);
    }
}
