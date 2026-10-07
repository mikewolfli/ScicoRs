// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Simulation execution engine.
//!
//! The `SimEngine` is the top-level orchestrator that drives a `Diagram`
//! through its full lifecycle: Constructed → Initialized → Running → Completed.
//! It manages time advancement, block execution ordering, state integration,
//! and supports multiple run modes (normal, real-time, single-step, breakpoint).

use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::state::StateDeclaration;
use crate::core::types::{ComponentStatus, ExecutionPhase, Scalar};
use crate::runtime::context::{SimContext, SimLifecycle, SimRunMode, TimeConfig};
use crate::runtime::scheduler::{Scheduler, SequentialScheduler};
use crate::runtime::solver::Euler;
use crate::runtime::solver::traits::OdeSolver;
use crate::runtime::state::SimStateManager;

/// Result of a single simulation step.
#[derive(Debug, Clone, PartialEq)]
pub enum SimStepResult {
    /// Step completed normally, more steps remain.
    StepCompleted,
    /// Simulation reached the end time or all blocks completed.
    Finished,
    /// Simulation was paused after this step.
    Paused,
    /// A breakpoint condition was triggered.
    BreakpointReached,
    /// An error occurred during step execution.
    Error(SimError),
}

/// Summary of a complete simulation run.
#[derive(Debug, Clone)]
pub struct SimSummary {
    /// Total number of steps executed.
    pub total_steps: u64,
    /// Final simulation time.
    pub final_time: Scalar,
    /// Whether the simulation completed normally.
    pub completed: bool,
    /// Any errors that occurred during the run.
    pub errors: Vec<SimError>,
    /// Final progress fraction.
    pub progress: f64,
}

/// The top-level simulation execution engine.
///
/// Owns a `Diagram`, a `SimContext`, and a `SimStateManager`. Drives blocks
/// through their execution phases, integrates continuous state, handles
/// discrete updates, and manages the simulation lifecycle.
pub struct SimEngine {
    /// Central simulation context (time, mode, lifecycle, shared data, logs).
    pub context: SimContext,
    /// Unified continuous + discrete state manager.
    pub state: SimStateManager,
    /// The diagram being simulated.
    diagram: Diagram,
    /// Cached topological execution order.
    execution_order: Vec<BlockId>,
    /// Numerical ODE solver used for continuous state integration.
    solver: Box<dyn OdeSolver>,
    /// Scheduler for block execution ordering, signal propagation, and event handling.
    scheduler: Box<dyn Scheduler>,
    /// Event channel shared with the scheduler: the engine schedules events here
    /// (e.g. from a breakpoint or an external trigger) and the scheduler drains
    /// it during the event phase.
    event_queue: crate::runtime::event::EventQueue,
    /// Total number of zero crossings detected across the run (mirrored from the
    /// scheduler so the count is observable from the engine).
    zero_crossings_detected: u64,
    /// Total number of events dispatched to blocks across the run.
    events_dispatched: u64,
}

impl std::fmt::Debug for SimEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimEngine")
            .field("lifecycle", &self.context.lifecycle)
            .field("time", &self.context.t)
            .field("step_count", &self.context.step_count)
            .field("block_count", &self.diagram.block_count())
            .field("state_vars", &self.state.total_len())
            .field("solver", &self.solver.name())
            .finish()
    }
}

impl SimEngine {
    /// Create a new simulation engine from a diagram and time configuration.
    ///
    /// Gathers state declarations from all blocks in the diagram and builds
    /// the unified state manager. Computes the topological execution order.
    /// The engine starts in the `Constructed` lifecycle state.
    pub fn new(mut diagram: Diagram, config: TimeConfig) -> Result<Self, SimError> {
        let ctx = SimContext::new(config);
        let execution_order = diagram
            .compute_execution_order()
            .ok_or_else(|| {
                SimError::new(
                    crate::core::error::ErrorCode::CycleDetected,
                    "cycle detected in diagram: cannot create engine",
                )
            })?
            .to_vec();

        // Gather state declarations from all blocks to build the state manager.
        let mut combined_decl = StateDeclaration::new();
        for block_id in &execution_order {
            if let Some(block) = diagram.get_block(block_id) {
                let decl = block.state_declaration();
                for var in &decl.continuous {
                    combined_decl.add_continuous(var.clone());
                }
                for var in &decl.discrete {
                    combined_decl.add_discrete(var.clone());
                }
            }
        }
        let state = SimStateManager::from_declaration(&combined_decl);

        // Initialize default sequential scheduler
        let mut scheduler: Box<dyn Scheduler> = Box::new(SequentialScheduler::new());
        scheduler.initialize(&diagram)?;
        scheduler.set_step_size(ctx.dt, &diagram);

        Ok(Self {
            context: ctx,
            state,
            diagram,
            execution_order,
            solver: Box::new(Euler::new()),
            scheduler,
            event_queue: crate::runtime::event::EventQueue::new(),
            zero_crossings_detected: 0,
            events_dispatched: 0,
        })
    }

    /// Total number of zero crossings detected so far in this run.
    ///
    /// Mirrors the scheduler's own count, so the value is observable without
    /// reaching into the scheduler.
    pub fn zero_crossings_detected(&self) -> u64 {
        self.zero_crossings_detected
    }

    /// Total number of events dispatched to blocks so far in this run.
    ///
    /// This is the end-to-end proof that the event chain is wired: a crossing
    /// detected in one step is enqueued, drained, and delivered to a block, and
    /// each delivery increments this counter.
    pub fn events_dispatched(&self) -> u64 {
        self.events_dispatched
    }

    /// The engine-side event queue, for scheduling external events.
    pub fn event_queue(&self) -> &crate::runtime::event::EventQueue {
        &self.event_queue
    }

    /// Mutable access to the engine-side event queue, for scheduling events
    /// before a step (they are drained during the next event phase).
    pub fn event_queue_mut(&mut self) -> &mut crate::runtime::event::EventQueue {
        &mut self.event_queue
    }

    /// Replace the default Euler solver with a custom ODE solver.
    ///
    /// Use this to select RK4, RK45, BackwardEuler, or any other solver
    /// implementation that satisfies the `OdeSolver` trait.
    pub fn with_solver(mut self, solver: Box<dyn OdeSolver>) -> Self {
        self.solver = solver;
        self
    }

    /// Replace the default sequential scheduler with a custom scheduler.
    ///
    /// Use this to select hybrid, multi-rate, or other scheduling strategies.
    pub fn with_scheduler(mut self, scheduler: Box<dyn Scheduler>) -> Self {
        self.scheduler = scheduler;
        self
    }

    /// Get a reference to the current ODE solver.
    pub fn solver(&self) -> &dyn OdeSolver {
        self.solver.as_ref()
    }

    /// Get a reference to the current scheduler.
    pub fn scheduler(&self) -> &dyn Scheduler {
        self.scheduler.as_ref()
    }

    /// Dynamically re-schedule after diagram changes.
    ///
    /// Re-computes the topological execution order, re-initializes the
    /// scheduler's internal state (signal cache etc.), and updates the
    /// state manager to match any new blocks.
    pub fn reschedule(&mut self) -> Result<(), SimError> {
        // Recompute execution order
        self.execution_order = self
            .diagram
            .compute_execution_order()
            .ok_or_else(|| {
                SimError::new(
                    crate::core::error::ErrorCode::CycleDetected,
                    "cycle detected in diagram: cannot reschedule",
                )
            })?
            .to_vec();

        // Re-initialize scheduler
        self.scheduler.initialize(&self.diagram)?;
        // Re-size delay lines: the topology may have changed, so a link's
        // `delay` may now need a new history depth.
        self.scheduler.set_step_size(self.context.dt, &self.diagram);

        // Rebuild state manager
        let mut combined_decl = StateDeclaration::new();
        for block_id in &self.execution_order {
            if let Some(block) = self.diagram.get_block(block_id) {
                let decl = block.state_declaration();
                for var in &decl.continuous {
                    combined_decl.add_continuous(var.clone());
                }
                for var in &decl.discrete {
                    combined_decl.add_discrete(var.clone());
                }
            }
        }
        self.state = SimStateManager::from_declaration(&combined_decl);

        Ok(())
    }

    /// Initialize all blocks in the diagram.
    ///
    /// Transitions lifecycle from `Constructed` → `Initialized`.
    /// Calls `init()` on every block and collects any errors.
    pub fn init(&mut self) -> Result<(), SimError> {
        if self.context.lifecycle != SimLifecycle::Constructed {
            return Err(SimError::runtime(format!(
                "cannot init from lifecycle {:?}, expected Constructed",
                self.context.lifecycle
            )));
        }

        for block_id in self.execution_order.clone() {
            if let Some(block) = self.diagram.get_block_mut(&block_id) {
                block.set_time(self.context.t);
                block.execute_phase(ExecutionPhase::Init).map_err(|e| {
                    SimError::runtime(format!("block '{}' init failed: {}", block_id, e))
                })?;
            }
        }

        // Reset state to initial values from declarations.
        self.state.reset();
        self.context.set_lifecycle(SimLifecycle::Initialized);
        self.context.info(format!(
            "engine initialized with {} blocks",
            self.diagram.block_count()
        ));
        Ok(())
    }

    /// Start the simulation. Transitions from `Initialized` → `Running`.
    pub fn start(&mut self) -> Result<(), SimError> {
        if self.context.lifecycle != SimLifecycle::Initialized
            && self.context.lifecycle != SimLifecycle::Paused
        {
            return Err(SimError::runtime(format!(
                "cannot start from lifecycle {:?}, expected Initialized or Paused",
                self.context.lifecycle
            )));
        }
        self.context.set_lifecycle(SimLifecycle::Running);
        self.context.info("simulation started");
        Ok(())
    }

    /// Seed the engine's unified continuous-state vector from the blocks'
    /// internal state, so the ODE solver starts from the current block values.
    fn collect_state_from_blocks(&mut self) -> Result<(), SimError> {
        let mut offset = 0;
        for block_id in &self.execution_order {
            if let Some(block) = self.diagram.get_block(block_id) {
                let n_cont = block.state_declaration().continuous_count();
                if n_cont > 0 {
                    let vals = block.read_state();
                    for (j, v) in vals.iter().take(n_cont).enumerate() {
                        if offset + j < self.state.continuous.len() {
                            self.state.continuous.set_index(offset + j, *v);
                        }
                    }
                    offset += n_cont;
                }
            }
        }
        Ok(())
    }

    /// Write the engine's integrated continuous-state vector back into the
    /// blocks' internal state so their next `output()` reflects new values.
    fn push_state_to_blocks(&mut self) -> Result<(), SimError> {
        let block_ids: Vec<String> = self.execution_order.clone();
        let mut offset = 0;
        for block_id in &block_ids {
            if let Some(block) = self.diagram.get_block_mut(block_id) {
                let n_cont = block.state_declaration().continuous_count();
                if n_cont > 0 {
                    let vals: Vec<Scalar> = (0..n_cont)
                        .map(|j| self.state.continuous.get_index(offset + j))
                        .collect();
                    block.write_state(&vals)?;
                    offset += n_cont;
                }
            }
        }
        Ok(())
    }

    /// Project a flat solver state vector into the blocks' internal state.
    ///
    /// Called for every internal stage evaluation of a multi-stage ODE solver
    /// so `derivative()` is evaluated at the candidate state `x` rather than
    /// the stale step-start state.
    fn project_state(
        diagram: &mut Diagram,
        execution_order: &[BlockId],
        x: &[Scalar],
    ) -> Result<(), SimError> {
        let mut offset = 0;
        for block_id in execution_order {
            if let Some(block) = diagram.get_block_mut(block_id) {
                let n_cont = block.state_declaration().continuous_count();
                if n_cont > 0 {
                    let vals: Vec<Scalar> = (0..n_cont)
                        .map(|j| {
                            if offset + j < x.len() {
                                x[offset + j]
                            } else {
                                0.0
                            }
                        })
                        .collect();
                    block.write_state(&vals)?;
                    offset += n_cont;
                }
            }
        }
        Ok(())
    }

    /// Execute a single time step.
    ///
    /// The step performs the following phases in order:
    /// 1. Set context time on all blocks
    /// 2. Call `output()` on all blocks (topological order)
    /// 3. Collect derivatives from all blocks
    /// 4. Integrate continuous state (Euler): x += dt * dx
    /// 5. Call `update()` on all blocks (discrete updates)
    /// 6. Signal propagation & event handling via scheduler
    /// 7. Advance simulation time
    /// 8. Check stop conditions
    pub fn step(&mut self) -> Result<SimStepResult, SimError> {
        // Validate state
        if self.context.lifecycle == SimLifecycle::Constructed {
            return Err(SimError::runtime(
                "engine not initialized; call init() first",
            ));
        }
        if self.context.lifecycle == SimLifecycle::Completed {
            return Ok(SimStepResult::Finished);
        }
        if self.context.lifecycle == SimLifecycle::Paused {
            return Ok(SimStepResult::Paused);
        }

        // Check for finished before stepping
        if self.context.is_finished() {
            self.finish();
            return Ok(SimStepResult::Finished);
        }

        // ── Phase 1: Set time and step size on all blocks ──
        let step_dt = self.context.dt;
        for block_id in &self.execution_order {
            if let Some(block) = self.diagram.get_block_mut(block_id) {
                block.set_time(self.context.t);
                block.set_step(step_dt);
            }
        }

        // ── Phase 2: Output computation ──
        for block_id in &self.execution_order {
            if let Some(block) = self.diagram.get_block_mut(block_id) {
                block.execute_phase(ExecutionPhase::Output).map_err(|e| {
                    self.context.set_error(e.clone());
                    SimError::runtime(format!("block '{}' output failed: {}", block_id, e))
                })?;
            }
        }

        // ── Phase 3: Signal propagation via scheduler ──
        // Extract block outputs to the scheduler's signal cache, propagate
        // through links, and write the propagated values back into the
        // destination blocks' input ports so downstream blocks actually
        // observe their inputs on the next phase.
        crate::runtime::scheduler::signal_prop::extract_outputs(
            &self.diagram,
            self.scheduler.signal_cache_mut(),
        )?;
        crate::runtime::scheduler::signal_prop::propagate_signals(
            &self.diagram,
            self.scheduler.signal_cache_mut(),
        )?;
        crate::runtime::scheduler::signal_prop::update_inputs(
            &mut self.diagram,
            self.scheduler.signal_cache(),
            self.context.t,
        )?;

        // ── Phase 4: Integrate continuous state using ODE solver ──
        // The solver trait provides a unified interface that all solver
        // implementations satisfy (Euler, RK4, RK45, BackwardEuler, etc.).
        if !self.state.continuous.is_empty() {
            let t = self.context.t;
            let dt = self.context.dt;

            // Seed the engine's unified state vector from the blocks' own
            // internal state (source of truth at the start of the step).
            self.collect_state_from_blocks()?;

            // Build RHS: project the solver's candidate state vector into the
            // blocks, advance their internal clock to the stage time, and
            // re-evaluate the (pure) network outputs so time-varying sources
            // and feedthrough blocks are correct at each solver stage. Blocks
            // with output side effects (e.g. PID integral accumulation) are
            // not re-run; they keep the step-start output.
            {
                let execution_order = self.execution_order.clone();
                let diagram = &mut self.diagram;
                let cache = self.scheduler.signal_cache_mut();

                let mut rhs = |x: &[f64],
                               stage_t: f64,
                               dx_out: &mut [f64]|
                 -> Result<(), SimError> {
                    // 1. Project the candidate state into the blocks so
                    //    derivative() is evaluated at the correct state.
                    Self::project_state(diagram, &execution_order, x)?;

                    // 2. Advance block clocks to the stage time.
                    for block_id in &execution_order {
                        if let Some(block) = diagram.get_block_mut(block_id) {
                            block.set_time(stage_t);
                        }
                    }

                    // 3. Re-evaluate outputs at the stage state/time.
                    for block_id in &execution_order {
                        if let Some(block) = diagram.get_block_mut(block_id) {
                            if !block.has_output_side_effects() {
                                block.execute_phase(ExecutionPhase::Output)?;
                            }
                        }
                    }

                    // 4. Re-propagate the stage outputs to input ports.
                    crate::runtime::scheduler::signal_prop::extract_outputs(diagram, cache)?;
                    crate::runtime::scheduler::signal_prop::propagate_signals(diagram, cache)?;
                    crate::runtime::scheduler::signal_prop::update_inputs(diagram, cache, stage_t)?;

                    // 5. Collect derivatives from each continuous block.
                    let mut offset = 0;
                    for block_id in &execution_order {
                        if let Some(block) = diagram.get_block(block_id) {
                            let n_cont = block.state_declaration().continuous_count();
                            if n_cont > 0 {
                                let block_dx = block.derivative()?;
                                for (j, &val) in block_dx.iter().enumerate() {
                                    if offset + j < dx_out.len() {
                                        dx_out[offset + j] = val;
                                    }
                                }
                                offset += n_cont;
                            }
                        }
                    }
                    Ok(())
                };

                let state_slice = self.state.continuous.values_mut();
                let step_result = self.solver.step(&mut rhs, state_slice, t, dt)?;
                if !step_result.is_ok() {
                    return Err(SimError::runtime(format!(
                        "solver '{}' rejected step at t={} (engine uses fixed dt={}); \
                         use a fixed-step solver (Euler/RK4/Heun/Midpoint) or reduce dt",
                        self.solver.name(),
                        t,
                        dt
                    )));
                }
            }

            // Write the integrated state back into the blocks' internal state
            // so their next `output()` reflects the new continuous values.
            self.push_state_to_blocks()?;
        }

        // ── Phase 5: Discrete update ──
        for block_id in &self.execution_order {
            if let Some(block) = self.diagram.get_block_mut(block_id) {
                block.execute_phase(ExecutionPhase::Update).map_err(|e| {
                    self.context.set_error(e.clone());
                    SimError::runtime(format!("block '{}' update failed: {}", block_id, e))
                })?;
            }
        }

        // ── Phase 6: Events — detect zero crossings, enqueue, dispatch ──
        //
        // Delegated to the scheduler, which owns the crossing history, the event
        // queue, and dispatch bookkeeping. This is what makes a crossing actually
        // reach a block: the engine previously computed `zero_crossings()` and
        // discarded it, leaving the whole event subsystem unreachable.
        {
            let dispatched = self.scheduler.run_event_phase(
                &self.diagram,
                &self.execution_order,
                self.context.t,
                &mut self.event_queue,
            )?;
            self.events_dispatched += dispatched as u64;

            // Mirror the scheduler's crossing count so the engine stays
            // observable without the caller having to reach into the scheduler.
            let detected = self.scheduler.crossing_count();
            self.zero_crossings_detected = detected;
        }

        // ── Phase 7: Advance time ──
        self.context.advance_time();

        // Advance scheduler's signal cache for next step
        self.scheduler.advance_cache();

        // ── Phase 8: Check stop conditions ──
        if self.context.is_finished() || self.diagram.all_completed() {
            self.finish();
            return Ok(SimStepResult::Finished);
        }

        // Check breakpoint
        if let SimRunMode::Breakpoint { ref condition } = self.context.mode
            && condition(&self.context)
        {
            self.context.mode = SimRunMode::Paused;
            self.context.set_lifecycle(SimLifecycle::Paused);
            return Ok(SimStepResult::BreakpointReached);
        }

        // Single-step mode: auto-pause after each step
        if self.context.mode.is_single_step() {
            self.context.set_lifecycle(SimLifecycle::Paused);
            return Ok(SimStepResult::StepCompleted);
        }

        Ok(SimStepResult::StepCompleted)
    }

    /// Run the simulation to completion (or until an error or pause).
    ///
    /// Calls `init()`, `start()`, then repeatedly `step()` until finished,
    /// paused, or an error occurs.
    pub fn run(&mut self) -> SimSummary {
        let mut errors = Vec::new();
        let mut completed = false;

        // Auto-init if needed
        if self.context.lifecycle == SimLifecycle::Constructed
            && let Err(e) = self.init()
        {
            errors.push(e);
            return self.summary(0, false, errors);
        }

        // Auto-start if initialized
        if (self.context.lifecycle == SimLifecycle::Initialized
            || self.context.lifecycle == SimLifecycle::Paused)
            && let Err(e) = self.start()
        {
            errors.push(e);
            return self.summary(0, false, errors);
        }

        // Main loop
        loop {
            match self.step() {
                Ok(SimStepResult::Finished) => {
                    completed = true;
                    break;
                }
                Ok(SimStepResult::Paused) | Ok(SimStepResult::BreakpointReached) => {
                    break;
                }
                Ok(SimStepResult::StepCompleted) => {
                    // Continue stepping
                }
                Ok(SimStepResult::Error(e)) => {
                    errors.push(e);
                    break;
                }
                Err(e) => {
                    errors.push(e);
                    break;
                }
            }
        }

        self.summary(self.context.step_count, completed, errors)
    }

    /// Pause the simulation.
    pub fn pause(&mut self) {
        if self.context.lifecycle == SimLifecycle::Running {
            self.context.set_lifecycle(SimLifecycle::Paused);
            self.context.info("simulation paused");
        }
    }

    /// Resume the simulation from pause.
    pub fn resume(&mut self) -> Result<(), SimError> {
        if self.context.lifecycle != SimLifecycle::Paused {
            return Err(SimError::runtime(format!(
                "cannot resume from lifecycle {:?}",
                self.context.lifecycle
            )));
        }
        self.context.set_lifecycle(SimLifecycle::Running);
        self.context.info("simulation resumed");
        Ok(())
    }

    /// Stop the simulation and set lifecycle to Completed.
    pub fn stop(&mut self) {
        self.finish();
    }

    /// Full reset: restore engine to Constructed state.
    pub fn reset(&mut self) {
        self.diagram.reset_all();
        self.state.reset();
        self.context = SimContext::new(self.context.config);
        self.context.info("engine reset");
    }

    // ── Accessors ──

    /// Get a reference to the diagram.
    pub fn diagram(&self) -> &Diagram {
        &self.diagram
    }

    /// Get a mutable reference to the diagram.
    pub fn diagram_mut(&mut self) -> &mut Diagram {
        &mut self.diagram
    }

    /// Get the execution order.
    pub fn execution_order(&self) -> &[BlockId] {
        &self.execution_order
    }

    // ── Private helpers ──

    fn finish(&mut self) {
        // Call terminate on all blocks
        for block_id in self.execution_order.clone() {
            if let Some(block) = self.diagram.get_block_mut(&block_id) {
                let _ = block.execute_phase(ExecutionPhase::Terminate);
                block.set_status(ComponentStatus::Completed);
            }
        }
        self.context.set_lifecycle(SimLifecycle::Completed);
        self.context.info(format!(
            "simulation completed: {} steps, t={}",
            self.context.step_count, self.context.t
        ));
    }

    fn summary(&self, total_steps: u64, completed: bool, errors: Vec<SimError>) -> SimSummary {
        SimSummary {
            total_steps,
            final_time: self.context.t,
            completed,
            errors,
            progress: self.context.progress(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::{Block, SimpleBlock};
    use crate::core::link::Link;
    use crate::core::types::{SignalType, SignalValue};

    /// Helper: create a simple source→sink diagram.
    fn create_test_diagram() -> Diagram {
        let mut d = Diagram::new("test_sim");
        let mut src = SimpleBlock::new("src", "Source");
        src.declare_output("out", SignalType::Continuous);
        let mut sink = SimpleBlock::new("sink", "Sink");
        sink.declare_input("in", SignalType::Continuous);
        d.add_block(Box::new(src));
        d.add_block(Box::new(sink));
        d.add_link(Link::new("l1", "src", "out", "sink", "in"));
        d
    }

    /// Zero crossings must be genuinely detected from the engine path and must be
    /// observable. Before this was fixed, Phase 6 computed `zero_crossings()` and
    /// discarded the result, so the entire event subsystem was unreachable.
    #[test]
    fn test_engine_detects_and_exposes_zero_crossings() {
        use crate::core::block::{Block, BlockId};
        use crate::core::error::SimError;
        use crate::core::param::ParameterSet;
        use crate::core::port::PortSet;
        use crate::core::state::StateDeclaration;
        use crate::core::types::ComponentStatus;
        use crate::core::types::{PortDirection, Scalar, SignalValue, Time};

        /// A block whose single crossing signal follows `sin` of the sim time,
        /// so it really does cross zero as the simulation advances.
        #[derive(Debug, Clone)]
        struct Oscillator {
            id: BlockId,
            ports: PortSet,
            params: ParameterSet,
            status: ComponentStatus,
            t: Time,
        }

        impl Oscillator {
            fn new(id: &str) -> Self {
                let mut ports = PortSet::new();
                ports.add(crate::core::port::Port::new(
                    "out",
                    PortDirection::Output,
                    SignalType::Continuous,
                ));
                Self {
                    id: id.to_string(),
                    ports,
                    params: ParameterSet::new(),
                    status: ComponentStatus::Inactive,
                    t: 0.0,
                }
            }
        }

        impl Block for Oscillator {
            fn id(&self) -> &BlockId {
                &self.id
            }
            fn block_type(&self) -> &str {
                "Oscillator"
            }
            fn ports(&self) -> &PortSet {
                &self.ports
            }
            fn ports_mut(&mut self) -> &mut PortSet {
                &mut self.ports
            }
            fn params(&self) -> &ParameterSet {
                &self.params
            }
            fn params_mut(&mut self) -> &mut ParameterSet {
                &mut self.params
            }
            fn status(&self) -> ComponentStatus {
                self.status
            }
            fn set_status(&mut self, s: ComponentStatus) {
                self.status = s;
            }
            fn set_time(&mut self, t: Time) {
                self.t = t;
            }
            fn time(&self) -> Time {
                self.t
            }
            fn state_declaration(&self) -> StateDeclaration {
                StateDeclaration::new()
            }
            fn init(&mut self) -> Result<(), SimError> {
                self.status = ComponentStatus::Ready;
                Ok(())
            }
            fn output(&mut self) -> Result<(), SimError> {
                // A sine wave: crosses zero twice per period.
                let v = (std::f64::consts::TAU * self.t).sin();
                if let Some(p) = self.ports.get_mut("out") {
                    p.write(crate::core::signal::Signal::new(
                        SignalType::Continuous,
                        SignalValue::Scalar(v),
                        self.t,
                    ));
                }
                Ok(())
            }
            fn derivative(&self) -> Result<Vec<Scalar>, SimError> {
                Ok(Vec::new())
            }
            fn update(&mut self) -> Result<(), SimError> {
                Ok(())
            }
            /// The crossing signal is the output value itself.
            fn zero_crossings(&self) -> Vec<Scalar> {
                vec![(std::f64::consts::TAU * self.t).sin()]
            }
            fn terminate(&mut self) -> Result<(), SimError> {
                Ok(())
            }
            fn clone_block(&self) -> Box<dyn Block> {
                Box::new(self.clone())
            }
            fn execute_phase(
                &mut self,
                phase: crate::core::types::ExecutionPhase,
            ) -> Result<(), SimError> {
                match phase {
                    crate::core::types::ExecutionPhase::Init => self.init(),
                    crate::core::types::ExecutionPhase::Output => self.output(),
                    _ => Ok(()),
                }
            }
        }

        let mut d = Diagram::new("osc");
        d.add_block(Box::new(Oscillator::new("osc")));
        let mut engine = SimEngine::new(d, TimeConfig::new(0.0, 2.0, 0.01)).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();

        let mut steps = 0;
        while steps < 300 {
            match engine.step().unwrap() {
                SimStepResult::Finished => break,
                _ => steps += 1,
            }
        }

        assert!(
            engine.zero_crossings_detected() > 0,
            "a sine wave over 2 s must produce zero crossings, got {}",
            engine.zero_crossings_detected()
        );
        // The chain must be closed end to end: each detected crossing is
        // enqueued and then *dispatched to a block*. Before the event phase was
        // wired, the count could be non-zero while nothing ever reached a block.
        assert!(
            engine.events_dispatched() > 0,
            "detected crossings must actually be delivered to blocks, got {} dispatches",
            engine.events_dispatched()
        );
    }

    /// A diagram with no crossing signals must not produce crossings or events.
    #[test]
    fn test_engine_without_crossings_produces_no_events() {
        let d = create_test_diagram();
        let mut engine = SimEngine::new(d, TimeConfig::until(0.1)).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();
        for _ in 0..10 {
            if engine.step().is_err() {
                break;
            }
        }
        // `SimpleBlock::zero_crossings` returns nothing, so there is nothing to
        // detect, enqueue, or dispatch.
        assert_eq!(engine.zero_crossings_detected(), 0);
        assert_eq!(engine.events_dispatched(), 0);
    }

    /// An event scheduled on the engine's queue must be delivered to its target
    /// block during the next event phase, proving the engine queue is wired to
    /// the scheduler.
    #[test]
    fn test_engine_scheduled_event_reaches_its_target_block() {
        use crate::runtime::event::{Event, EventType};
        use std::sync::{Arc, Mutex};

        /// Records every `ExecutionPhase::Event` it receives.
        #[derive(Debug, Clone)]
        struct EventSink {
            inner: SimpleBlock,
            received: Arc<Mutex<u64>>,
        }

        impl Block for EventSink {
            fn id(&self) -> &crate::core::block::BlockId {
                self.inner.id()
            }
            fn block_type(&self) -> &str {
                "EventSink"
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
            fn set_time(&mut self, t: crate::core::types::Time) {
                self.inner.set_time(t);
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
                Vec::new()
            }
            fn terminate(&mut self) -> Result<(), SimError> {
                Ok(())
            }
            fn clone_block(&self) -> Box<dyn Block> {
                Box::new(self.clone())
            }
            fn execute_phase(
                &mut self,
                phase: crate::core::types::ExecutionPhase,
            ) -> Result<(), SimError> {
                if matches!(phase, crate::core::types::ExecutionPhase::Event) {
                    *self.received.lock().unwrap() += 1;
                }
                Ok(())
            }
        }

        let received = Arc::new(Mutex::new(0u64));
        let sink = {
            let mut inner = SimpleBlock::new("target", "EventSink");
            inner.declare_input("u", SignalType::Continuous);
            EventSink {
                inner,
                received: Arc::clone(&received),
            }
        };
        let mut d = Diagram::new("scheduled");
        d.add_block(Box::new(sink));

        let mut engine = SimEngine::new(d, TimeConfig::until(0.05)).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();

        // Schedule an event addressed to the block, due at the current time.
        engine
            .event_queue_mut()
            .push(
                Event::new("ext", 0.0, EventType::External, SignalValue::Scalar(0.0))
                    .with_target("target"),
            )
            .expect("the queue must accept the event");

        let _ = engine.step();

        assert_eq!(
            *received.lock().unwrap(),
            1,
            "the scheduled event must be delivered to its target block exactly once"
        );
        assert!(
            engine.events_dispatched() >= 1,
            "the dispatch must be counted on the engine"
        );
    }

    #[test]
    fn test_engine_creation() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);
        let engine = SimEngine::new(d, config).unwrap();
        assert_eq!(engine.context.lifecycle, SimLifecycle::Constructed);
        assert_eq!(engine.context.config.end_time, 1.0);
        assert_eq!(engine.execution_order().len(), 2);
    }

    #[test]
    fn test_engine_init_and_start() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);
        let mut engine = SimEngine::new(d, config).unwrap();

        engine.init().unwrap();
        assert_eq!(engine.context.lifecycle, SimLifecycle::Initialized);

        engine.start().unwrap();
        assert_eq!(engine.context.lifecycle, SimLifecycle::Running);
    }

    #[test]
    fn test_engine_single_step() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);
        let mut engine = SimEngine::new(d, config).unwrap();

        engine.init().unwrap();
        engine.start().unwrap();

        let result = engine.step().unwrap();
        assert_eq!(result, SimStepResult::StepCompleted);
        assert_eq!(engine.context.step_count, 1);
        assert!((engine.context.t - engine.context.dt).abs() < 1e-12);
    }

    #[test]
    fn test_engine_run_to_completion() {
        let d = create_test_diagram();
        // Use a large step to finish quickly
        let mut config = TimeConfig::until(1.0);
        config.initial_step = 1.0;
        config.max_step = 1.0;
        let mut engine = SimEngine::new(d, config).unwrap();

        let summary = engine.run();
        assert!(summary.completed);
        assert_eq!(engine.context.lifecycle, SimLifecycle::Completed);
        assert!((summary.final_time - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_engine_pause_resume() {
        let d = create_test_diagram();
        let config = TimeConfig::until(10.0);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();

        engine.step().unwrap();
        engine.pause();
        assert_eq!(engine.context.lifecycle, SimLifecycle::Paused);

        engine.resume().unwrap();
        assert_eq!(engine.context.lifecycle, SimLifecycle::Running);
    }

    #[test]
    fn test_engine_reset() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();
        engine.step().unwrap();

        engine.reset();
        assert_eq!(engine.context.lifecycle, SimLifecycle::Constructed);
        assert_eq!(engine.context.step_count, 0);
        assert!((engine.context.t - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_engine_lifecycle_validation() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);
        let mut engine = SimEngine::new(d, config).unwrap();

        // Cannot start without init
        assert!(engine.start().is_err());

        // Cannot step without init
        assert!(engine.step().is_err());
    }

    #[test]
    fn test_engine_single_step_mode() {
        let d = create_test_diagram();
        let config = TimeConfig::until(10.0);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.context.set_mode(SimRunMode::SingleStep);
        engine.init().unwrap();
        engine.start().unwrap();

        let result = engine.step().unwrap();
        assert_eq!(result, SimStepResult::StepCompleted);
        // In single-step mode, lifecycle should be Paused after step
        assert_eq!(engine.context.lifecycle, SimLifecycle::Paused);
    }

    #[test]
    fn test_engine_terminate_on_finished() {
        let d = create_test_diagram();
        let mut config = TimeConfig::until(0.01);
        config.initial_step = 0.1; // step will overshoot end_time
        config.max_step = 0.1;
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();

        let summary = engine.run();
        assert!(summary.completed);
        assert_eq!(engine.context.lifecycle, SimLifecycle::Completed);
    }

    #[test]
    fn test_engine_state_manager_created() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);
        let engine = SimEngine::new(d, config).unwrap();
        assert_eq!(engine.state.total_len(), 0); // SimpleBlocks have no state
    }

    #[test]
    fn test_engine_shared_data() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine
            .context
            .set_shared("test_key", crate::core::types::SignalValue::Scalar(42.0));
        assert!(engine.context.has_shared("test_key"));
    }

    #[test]
    fn test_engine_logging() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.context.info("engine created");
        assert_eq!(engine.context.logs().len(), 1);
    }

    #[test]
    fn test_engine_step_multiple_times() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();

        for _ in 0..5 {
            let result = engine.step().unwrap();
            if result == SimStepResult::Finished {
                break;
            }
        }
        assert_eq!(engine.context.step_count, 5);
        assert!((engine.context.t - 5.0 * engine.context.dt).abs() < 1e-12);
    }

    #[test]
    fn test_engine_cannot_step_after_completion() {
        let d = create_test_diagram();
        let mut config = TimeConfig::until(0.1);
        config.initial_step = 1.0;
        config.max_step = 1.0;
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();

        // First step should finish
        let r1 = engine.step().unwrap();
        assert_eq!(r1, SimStepResult::Finished);

        // Subsequent step should return Finished
        let r2 = engine.step().unwrap();
        assert_eq!(r2, SimStepResult::Finished);
    }

    #[test]
    fn test_engine_stop() {
        let d = create_test_diagram();
        let config = TimeConfig::until(10.0);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();
        engine.step().unwrap();

        engine.stop();
        assert_eq!(engine.context.lifecycle, SimLifecycle::Completed);
    }

    #[test]
    fn test_engine_creation_with_cycle_detection() {
        let mut d = Diagram::new("cyclic");
        let mut a = SimpleBlock::new("a", "A");
        a.declare_output("out", SignalType::Continuous);
        a.declare_input("in", SignalType::Continuous);
        let mut b = SimpleBlock::new("b", "B");
        b.declare_output("out", SignalType::Continuous);
        b.declare_input("in", SignalType::Continuous);
        d.add_block(Box::new(a));
        d.add_block(Box::new(b));
        d.add_link(Link::new("l1", "a", "out", "b", "in"));
        d.add_link(Link::new("l2", "b", "out", "a", "in"));

        let result = SimEngine::new(d, TimeConfig::until(1.0));
        assert!(result.is_err());
    }

    #[test]
    fn test_engine_solver_switch_accuracy() {
        // Create a diagram with a block that has continuous state
        let mut d = Diagram::new("solver_test");
        let mut integrator = SimpleBlock::new("int", "Integrator");
        integrator.declare_input("in", SignalType::Continuous);
        integrator.declare_output("out", SignalType::Continuous);
        integrator.add_continuous_state("x", 1.0);
        d.add_block(Box::new(integrator));
        d.compute_execution_order();

        let config = TimeConfig::until(1.0);

        // Run with Euler (1st order)
        let mut engine_euler = SimEngine::new(d.clone_diagram(), config).unwrap();
        engine_euler.init().unwrap();
        engine_euler.start().unwrap();
        let summary_euler = engine_euler.run();

        // Run with RK4 (4th order) — should be more accurate
        let mut engine_rk4 = SimEngine::new(d.clone_diagram(), config)
            .unwrap()
            .with_solver(Box::new(crate::runtime::solver::RK4::new()));
        engine_rk4.init().unwrap();
        engine_rk4.start().unwrap();
        let summary_rk4 = engine_rk4.run();

        // Both should complete successfully
        assert!(summary_euler.completed);
        assert!(summary_rk4.completed);

        // Both should advance time to ~1.0
        assert!((summary_euler.final_time - 1.0).abs() < 1e-6);
        assert!((summary_rk4.final_time - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_engine_solver_names() {
        let d = create_test_diagram();
        let config = TimeConfig::until(1.0);

        let engine = SimEngine::new(d, config).unwrap();
        assert_eq!(engine.solver().name(), "Euler");

        let engine = engine.with_solver(Box::new(crate::runtime::solver::RK4::new()));
        assert_eq!(engine.solver().name(), "RK4");
    }

    #[test]
    fn test_engine_solver_stats() {
        let mut d = Diagram::new("stats_test");
        let mut block = SimpleBlock::new("b", "TestBlock");
        block.declare_input("in", SignalType::Continuous);
        block.add_continuous_state("x", 1.0);
        d.add_block(Box::new(block));
        d.compute_execution_order();

        let config = TimeConfig::until(0.1);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();
        let _ = engine.run();

        // Solver should have recorded some stats
        let solver = engine.solver();
        assert!(solver.stats().steps_accepted > 0);
        assert!(solver.stats().function_evals > 0);
    }

    // ────────────────────────────────────────────────────────────
    // Real-behavior integration tests (verify the dataflow actually
    // moves signals between blocks and integrates continuous state).
    // ────────────────────────────────────────────────────────────

    /// A `ConstantSource -> Gain -> (read gain output)` chain must propagate
    /// the constant through the link so the Gain outputs `k * value`.
    #[test]
    fn test_engine_signal_flow_reaches_blocks() {
        use crate::blocks::math::Gain;
        use crate::blocks::sources::ConstantSource;

        let mut d = Diagram::new("flow");
        d.add_block(Box::new(ConstantSource::scalar("src", 3.0)));
        d.add_block(Box::new(Gain::new("gain", 5.0)));
        d.add_link(Link::new("l1", "src", "out", "gain", "u"));
        d.compute_execution_order();

        let config = TimeConfig::until(1.0);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();
        // Run enough steps for the value to propagate through the link.
        for _ in 0..3 {
            engine.step().unwrap();
        }

        // The Gain's output port must now hold 5 * 3 = 15 (written by output()).
        let gain_out = engine
            .diagram()
            .get_block("gain")
            .and_then(|b| b.ports().get("y"))
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar());
        assert_eq!(gain_out, Some(15.0), "gain output should be 5 * constant");
    }

    /// A `ConstantSource(2) -> Integrator(0)` diagram must integrate the
    /// constant so the engine's continuous state reaches ~2.0 at t = 1.0.
    #[test]
    fn test_engine_continuous_state_integrates() {
        use crate::blocks::continuous::Integrator;
        use crate::blocks::sources::ConstantSource;

        let mut d = Diagram::new("integrate");
        d.add_block(Box::new(ConstantSource::scalar("src", 2.0)));
        d.add_block(Box::new(Integrator::new("int", 0.0)));
        d.add_link(Link::new("l1", "src", "out", "int", "u"));
        d.compute_execution_order();

        let config = TimeConfig::until(1.0);
        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();
        let summary = engine.run();

        assert!(summary.completed);
        assert!((summary.final_time - 1.0).abs() < 1e-6);
        let x = engine.state.continuous.values()[0];
        assert!(
            (x - 2.0).abs() < 1e-9,
            "integrator of constant 2.0 over 1.0 s should reach ~2.0, got {}",
            x
        );
    }

    /// RK4 must be more accurate than Euler for a time-varying input
    /// (integrating sin(t) over [0,1] = 1 - cos(1) ≈ 0.45969769).
    #[test]
    fn test_engine_rk4_more_accurate_than_euler() {
        use crate::blocks::continuous::Integrator;
        use crate::blocks::sources::SineSource;

        let build = || {
            let mut d = Diagram::new("rk4_acc");
            // ω = 1 ⇒ freq = 1/(2π); integrate sin(t).
            d.add_block(Box::new(SineSource::new(
                "src",
                1.0,
                1.0 / (2.0 * std::f64::consts::PI),
                0.0,
                0.0,
            )));
            d.add_block(Box::new(Integrator::new("int", 0.0)));
            d.add_link(Link::new("l1", "src", "out", "int", "u"));
            d.compute_execution_order();
            d
        };

        let mut config = TimeConfig::until(1.0);
        config.initial_step = 0.01;
        config.max_step = 0.01;
        config.min_step = 0.01;

        let mut euler = SimEngine::new(build(), config).unwrap();
        let mut rk4 = SimEngine::new(build(), config)
            .unwrap()
            .with_solver(Box::new(crate::runtime::solver::RK4::new()));
        euler.run();
        rk4.run();

        let exact = 1.0 - 1.0_f64.cos();
        let x_euler = euler.state.continuous.values()[0];
        let x_rk4 = rk4.state.continuous.values()[0];
        let err_euler = (x_euler - exact).abs();
        let err_rk4 = (x_rk4 - exact).abs();
        assert!(
            err_rk4 < err_euler,
            "RK4 error {} should be < Euler error {}",
            err_rk4,
            err_euler
        );
        assert!(
            err_rk4 < 1e-6,
            "RK4 should be highly accurate, got {}",
            x_rk4
        );
    }

    /// The PID controller's step size must be synced from the engine, so the
    /// integral term uses the actual simulation dt, not the hardcoded 0.01.
    #[test]
    fn test_engine_pid_uses_engine_step() {
        use crate::blocks::continuous::PIDController;
        use crate::blocks::sources::ConstantSource;

        let mut d = Diagram::new("pid");
        d.add_block(Box::new(ConstantSource::scalar("ref", 10.0)));
        d.add_block(Box::new(ConstantSource::scalar("meas", 0.0)));
        d.add_block(Box::new(PIDController::new("pid", 1.0, 2.0, 0.0)));
        d.add_link(Link::new("l1", "ref", "out", "pid", "ref"));
        d.add_link(Link::new("l2", "meas", "out", "pid", "meas"));
        d.compute_execution_order();

        let mut config = TimeConfig::until(1.0);
        config.initial_step = 0.25;
        config.max_step = 0.25;
        config.min_step = 0.25;

        let mut engine = SimEngine::new(d, config).unwrap();
        engine.init().unwrap();
        engine.start().unwrap();
        let _ = engine.run();

        // With ki=2.0, dt=0.25 and error=10 from step 2 onward:
        // integral grows by 2.5 per step → y = kp*e + ki*∫e ≈ 25 after 4 steps
        // (a 5th step would reach 30, but the run stops at t=1.0).
        let y = engine
            .diagram()
            .get_block("pid")
            .and_then(|b| b.ports().get("y"))
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar())
            .unwrap_or(0.0);
        assert!(
            (y - 25.0).abs() < 0.5,
            "PID output should reflect engine dt (≈25), got {}",
            y
        );
    }
}
