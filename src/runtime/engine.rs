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

        Ok(Self {
            context: ctx,
            state,
            diagram,
            execution_order,
            solver: Box::new(Euler::new()),
        })
    }

    /// Replace the default Euler solver with a custom ODE solver.
    ///
    /// Use this to select RK4, RK45, BackwardEuler, or any other solver
    /// implementation that satisfies the `OdeSolver` trait.
    pub fn with_solver(mut self, solver: Box<dyn OdeSolver>) -> Self {
        self.solver = solver;
        self
    }

    /// Get a reference to the current ODE solver.
    pub fn solver(&self) -> &dyn OdeSolver {
        self.solver.as_ref()
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

    /// Execute a single time step.
    ///
    /// The step performs the following phases in order:
    /// 1. Set context time on all blocks
    /// 2. Call `output()` on all blocks (topological order)
    /// 3. Collect derivatives from all blocks
    /// 4. Integrate continuous state (Euler): x += dt * dx
    /// 5. Call `update()` on all blocks (discrete updates)
    /// 6. Advance simulation time
    /// 7. Check stop conditions
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

        // ── Phase 1: Set time on all blocks ──
        for block_id in &self.execution_order {
            if let Some(block) = self.diagram.get_block_mut(block_id) {
                block.set_time(self.context.t);
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

        // ── Phase 4: Integrate continuous state using ODE solver ──
        // Use the configured ODE solver instead of hardcoded Euler integration.
        // The solver trait provides a unified interface that all solver
        // implementations satisfy (Euler, RK4, RK45, BackwardEuler, etc.).
        if !self.state.continuous.is_empty() {
            let t = self.context.t;
            let dt = self.context.dt;

            // Build RHS: collect derivatives from each block into a flat vector.
            // This wraps the multi-block derivative computation into a single ODE function.
            {
                let execution_order = self.execution_order.clone();

                let mut rhs = |_x: &[f64], _t: f64, dx_out: &mut [f64]| -> Result<(), SimError> {
                    // Collect derivatives from each continuous block
                    let mut offset = 0;
                    for block_id in &execution_order {
                        if let Some(block) = self.diagram.get_block(block_id) {
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
                self.solver.step(&mut rhs, state_slice, t, dt)?;
            }
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

        // ── Phase 6: Zero-crossing detection (informational, no action yet) ──
        for block_id in &self.execution_order {
            if let Some(block) = self.diagram.get_block(block_id) {
                let _crossings = block.zero_crossings();
                // Future: handle zero-crossing events for variable-step solvers
            }
        }

        // ── Phase 7: Advance time ──
        self.context.advance_time();

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
    use crate::core::block::SimpleBlock;
    use crate::core::link::Link;
    use crate::core::types::SignalType;

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
}
