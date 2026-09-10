//! Core scheduler trait and supporting types.
//!
//! Defines the `Scheduler` interface that all scheduling strategies implement,
//! along with configuration and result types used throughout the scheduler module.

use super::signal_prop::SignalCache;
use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::types::{ExecutionPhase, SignalValue, Time};
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

    /// Inform the scheduler of the current step size.
    ///
    /// Delay lines are sized in *steps*, so a scheduler must know `dt` to model
    /// a link's `delay` in seconds. Called by the engine before each step; the
    /// default implementation does nothing, which leaves every link as a direct
    /// feedthrough (the documented meaning of `delay == 0`).
    fn set_step_size(&mut self, dt: Time, diagram: &Diagram) {
        let _ = (dt, diagram);
    }

    /// Run the event phases: detect zero crossings, enqueue them, and dispatch
    /// every event that is due.
    ///
    /// The engine owns phases 1–5 (output, propagation, integration, discrete
    /// update) because it needs to interleave the ODE solver and its own state
    /// vector, but the event chain belongs to the scheduler: only the scheduler
    /// holds the crossing history, the event queue, and the dispatch bookkeeping.
    /// Calling this after the engine's phases is what makes a crossing actually
    /// reach a block instead of being observed and discarded.
    ///
    /// Returns the number of events dispatched. The default implementation does
    /// nothing so a minimal scheduler stays valid; `SequentialScheduler`
    /// implements the full chain.
    fn run_event_phase(
        &mut self,
        diagram: &Diagram,
        order: &[BlockId],
        current_time: Time,
        engine_queue: &mut EventQueue,
    ) -> Result<usize, SimError> {
        let _ = (diagram, order, current_time, engine_queue);
        Ok(0)
    }

    /// Total number of zero crossings detected since this scheduler was created.
    ///
    /// Observable proof that the DetectEvents phase ran; the default is `0` for
    /// a scheduler that does not implement event detection.
    fn crossing_count(&self) -> u64 {
        0
    }
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
    /// Last observed value of each block's zero-crossing signal, keyed by
    /// `(block, crossing index)`.
    ///
    /// Held here (not on the engine) because sign-change detection is the
    /// scheduler's concern: the engine runs the numerical phases, the scheduler
    /// owns event detection and dispatch.
    crossing_history: HashMap<(BlockId, usize), crate::core::types::Scalar>,
    /// Total zero crossings detected, monotonic for the scheduler's lifetime.
    crossings_detected: u64,
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
            crossing_history: HashMap::new(),
            crossings_detected: 0,
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
            // The id is for diagnostics only; routing uses `target`, so a
            // `BlockId` containing any character (including ':') is safe.
            let id = format!("zc:{}:{}", self.event_sequence, crossing_index);
            let event = Event::new(
                &id,
                current_time,
                EventType::ZeroCrossing,
                SignalValue::Scalar(*crossing_index),
            )
            .with_target(block_id);
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
            // Route by the structured target, not by parsing the id: an event
            // with no target is a broadcast/free-standing event and is recorded
            // without being delivered to a block.
            let Some(block_id) = event.target.clone() else {
                continue;
            };
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

    /// Size the cache's delay lines for this diagram and step size, so a link's
    /// `delay` is actually modelled rather than silently ignored.
    fn set_step_size(&mut self, dt: Time, diagram: &Diagram) {
        self.signal_cache.dt = dt;
        self.signal_cache.configure_delays(diagram);
    }

    fn crossing_count(&self) -> u64 {
        self.crossings_detected
    }

    /// Detect zero crossings, enqueue them as events, and dispatch everything due.
    ///
    /// This is the single implementation of the event chain, shared by
    /// [`Scheduler::step`] and the engine. Keeping one implementation is what
    /// prevents the two paths from drifting apart: previously the engine had its
    /// own inline Phase 6 that computed crossings and discarded them, while this
    /// scheduler had a complete chain that nothing on the engine path invoked.
    fn run_event_phase(
        &mut self,
        diagram: &Diagram,
        order: &[BlockId],
        current_time: Time,
        engine_queue: &mut EventQueue,
    ) -> Result<usize, SimError> {
        // Detect crossings by sign change against the previous sample. This
        // mutably borrows `self.crossing_history`, so the detection runs before
        // any other borrow of `self` below.
        let crossings = super::hybrid::execute_event_detection_with_state(
            diagram,
            order,
            &mut self.crossing_history,
        );
        self.crossings_detected += crossings.len() as u64;

        // Enqueue the crossings, then dispatch every event that is due: the
        // scheduler's own queue plus whatever the engine scheduled.
        self.enqueue_detected_events(&crossings, current_time);

        let mut events = self.event_queue.drain_up_to(current_time);
        events.extend(engine_queue.drain_up_to(current_time));
        if events.is_empty() {
            return Ok(0);
        }
        self.dispatch_events(events, diagram)
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

        // Phase 5 (discrete update) is also run by the engine, which owns mutable
        // access to the diagram. It is deliberately *not* repeated here: the
        // scheduler's `diagram` is immutable, so it could only validate the block
        // set, and running the phase twice would double-advance discrete state.
        super::hybrid::validate_update_phase(ctx.diagram, &self.order)?;

        // Phases 6-7: Detect zero crossings, enqueue, and dispatch due events.
        // Shared with the engine's path so the two cannot diverge.
        let order = self.order.clone();
        self.run_event_phase(ctx.diagram, &order, ctx.current_time, ctx.event_queue)?;

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

    /// Test block whose crossing signal alternates sign on every step, so each
    /// step is a genuine crossing.
    #[derive(Debug, Clone)]
    struct AlternatingCrossing {
        inner: SimpleBlock,
        dispatched: Arc<Mutex<Vec<EventType>>>,
        sign: Arc<Mutex<f64>>,
    }

    impl AlternatingCrossing {
        fn new(id: &str) -> Self {
            let mut inner = SimpleBlock::new(id, "AlternatingCrossing");
            inner.declare_output("y", SignalType::Continuous);
            Self {
                inner,
                dispatched: Arc::new(Mutex::new(Vec::new())),
                sign: Arc::new(Mutex::new(-1.0)),
            }
        }

        fn dispatched_handle(&self) -> Arc<Mutex<Vec<EventType>>> {
            Arc::clone(&self.dispatched)
        }
    }

    impl crate::core::block::Block for AlternatingCrossing {
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
        fn set_status(&mut self, s: crate::core::types::ComponentStatus) {
            self.inner.set_status(s);
        }
        fn set_time(&mut self, t: Time) {
            self.inner.set_time(t);
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
        /// Flip the sign on every update, so the next detection sees a change.
        fn update(&mut self) -> Result<(), SimError> {
            let mut s = self.sign.lock().unwrap();
            *s = -*s;
            Ok(())
        }
        fn zero_crossings(&self) -> Vec<Scalar> {
            vec![*self.sign.lock().unwrap()]
        }
        fn terminate(&mut self) -> Result<(), SimError> {
            Ok(())
        }
        fn clone_block(&self) -> Box<dyn crate::core::block::Block> {
            Box::new(self.clone())
        }
        fn execute_phase(&mut self, phase: ExecutionPhase) -> Result<(), SimError> {
            match phase {
                ExecutionPhase::Update => self.update(),
                ExecutionPhase::Event => {
                    self.dispatched
                        .lock()
                        .unwrap()
                        .push(EventType::ZeroCrossing);
                    Ok(())
                }
                _ => Ok(()),
            }
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

        // The step is no longer a no-op: a second step with the same crossing
        // signal does **not** re-fire, because a signal sitting at zero carries
        // no new sign change. This is the fix for the repeated-firing bug (a
        // signal parked at zero used to raise an event on every single step).
        let order = scheduler.execution_order().to_vec();
        {
            let mut ctx = ctx(&diagram, &order, &mut engine_queue, &mut cache, 1.0);
            scheduler.step(&mut ctx).expect("step should succeed");
        }
        assert_eq!(
            scheduler.dispatched_event_count("X1"),
            1,
            "a crossing signal unchanged at zero must not re-fire"
        );
        assert_eq!(dispatched.lock().unwrap().len(), 1);
    }

    /// A crossing that alternates sign across steps must fire on each crossing,
    /// proving the repeated-firing fix did not suppress genuine events.
    #[test]
    fn test_step_refires_when_the_signal_actually_crosses_again() {
        // A block whose crossing signal alternates sign via its step counter.
        let block = AlternatingCrossing::new("A");
        let handle = block.dispatched_handle();
        let mut diagram = Diagram::new("alternating");
        diagram.add_block(Box::new(block));

        let mut scheduler = SequentialScheduler::new();
        scheduler.initialize(&diagram).expect("initialize");
        let order = scheduler.execution_order().to_vec();
        let mut engine_queue = EventQueue::new();
        let mut cache = SignalCache::new();

        // Drive several steps. The `Update` phase is the engine's responsibility
        // (it needs mutable diagram access), so this test performs it explicitly
        // to emulate what the engine does between scheduler steps.
        for step in 0..4 {
            crate::runtime::scheduler::hybrid::execute_update_phase_mut(&mut diagram, &order)
                .expect("update phase");
            let mut ctx = ctx(
                &diagram,
                &order,
                &mut engine_queue,
                &mut cache,
                step as Time,
            );
            scheduler.step(&mut ctx).expect("step should succeed");
        }

        assert!(
            scheduler.crossing_count() >= 2,
            "successive sign changes must each be detected, got {}",
            scheduler.crossing_count()
        );
        assert_eq!(
            handle.lock().unwrap().len(),
            scheduler.dispatched_event_count("A") as usize,
            "every detected crossing must have been dispatched"
        );
        assert!(
            scheduler.dispatched_event_count("A") >= 2,
            "each genuine crossing must produce a dispatch"
        );
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
            .push(
                Event::new(
                    "zc:99:0",
                    5.0,
                    EventType::ZeroCrossing,
                    SignalValue::Scalar(0.0),
                )
                .with_target("X1"),
            )
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
            .push(
                Event::new(
                    "zc:1:0",
                    0.0,
                    EventType::ZeroCrossing,
                    SignalValue::Scalar(0.0),
                )
                .with_target("X1"),
            )
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

        let events = vec![
            Event::new(
                "zc:1:0",
                0.0,
                EventType::ZeroCrossing,
                SignalValue::Scalar(0.0),
            )
            .with_target("ghost"),
        ];

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
            .push(
                Event::new(
                    "zc:1:0",
                    0.0,
                    EventType::ZeroCrossing,
                    SignalValue::Scalar(0.0),
                )
                .with_target("ghost"),
            )
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

    /// A block id containing the old id-delimiter (`:`) must still be routed
    /// correctly. The previous implementation recovered the target by
    /// `event.id.split(':').nth(1)`, so a namespaced id like `"plant:gain"` was
    /// truncated to `"plant"` and the dispatch failed with "block not found".
    #[test]
    fn test_event_routing_handles_a_block_id_containing_a_colon() {
        let block = CrossingRecorder::new("plant:gain", vec![0.0]);
        let handle = block.dispatched_handle();
        let mut diagram = Diagram::new("namespaced");
        diagram.add_block(Box::new(block));

        let mut scheduler = SequentialScheduler::new();
        let order = vec!["plant:gain".to_string()];
        scheduler.initialize(&diagram).unwrap();

        let mut engine_queue = EventQueue::new();
        let mut cache = SignalCache::new();
        let mut schedule_ctx = ctx(&diagram, &order, &mut engine_queue, &mut cache, 0.5);

        let result = scheduler.step(&mut schedule_ctx);
        assert!(
            result.is_ok(),
            "a colon in the block id must not break routing: {result:?}"
        );

        let dispatched = handle.lock().unwrap();
        assert!(
            !dispatched.is_empty(),
            "the crossing event must reach the block whose id contains ':'"
        );
    }

    /// An event with no target is not routed to any block, and must not be
    /// mistaken for one addressed to the empty-named block.
    #[test]
    fn test_event_without_a_target_is_not_dispatched_to_a_block() {
        let (diagram, handle) = crossing_diagram(vec![0.0]);
        let mut scheduler = SequentialScheduler::new();
        scheduler.initialize(&diagram).unwrap();

        let events = vec![Event::new(
            "free-standing",
            0.0,
            EventType::ZeroCrossing,
            SignalValue::Scalar(0.0),
        )];
        let dispatched = scheduler.dispatch_events(events, &diagram).unwrap();
        assert_eq!(dispatched, 0, "an untargeted event routes nowhere");
        assert!(
            handle.lock().unwrap().is_empty(),
            "no block may receive an untargeted event"
        );
    }
}
