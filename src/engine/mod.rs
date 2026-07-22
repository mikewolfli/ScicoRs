//! Scheduling and Execution Engine
//!
//! The engine orchestrates the simulation lifecycle: topological sorting,
//! block execution scheduling, signal propagation, and multi-rate task
//! coordination.

use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::types::{ExecutionPhase, Scalar, Time};

/// The simulation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationMode {
    /// Normal simulation mode (run to end time).
    Normal,
    /// Real-time simulation (wall-clock synchronized).
    RealTime,
    /// Single-step mode (advance one step at a time).
    SingleStep,
    /// Paused.
    Paused,
}

/// The overall simulation lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationState {
    /// Simulation has not been initialized.
    Uninitialized,
    /// Initialization complete, ready to run.
    Initialized,
    /// Simulation is running.
    Running,
    /// Simulation is paused.
    Paused,
    /// Simulation has completed.
    Completed,
    /// An error occurred.
    Error,
}

/// Configuration for the simulation engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Simulation start time.
    pub t_start: Time,
    /// Simulation end time.
    pub t_end: Time,
    /// Base step size.
    pub dt: Scalar,
    /// Simulation mode.
    pub mode: SimulationMode,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            t_start: 0.0,
            t_end: 10.0,
            dt: 1e-3,
            mode: SimulationMode::Normal,
        }
    }
}

/// The simulation engine — orchestrates execution of a diagram.
#[derive(Debug)]
pub struct SimulationEngine {
    /// The diagram being simulated.
    pub diagram: Diagram,
    /// Current engine configuration.
    pub config: EngineConfig,
    /// Current simulation state.
    pub state: SimulationState,
    /// Current simulation time.
    pub current_time: Time,
    /// Current step count.
    pub step_count: u64,
}

impl SimulationEngine {
    pub fn new(diagram: Diagram, config: EngineConfig) -> Self {
        let t_start = config.t_start;
        Self {
            diagram,
            config,
            state: SimulationState::Uninitialized,
            current_time: t_start,
            step_count: 0,
        }
    }

    /// Initialize the simulation.
    pub fn init(&mut self) -> Result<(), SimError> {
        // Compute execution order
        if self.diagram.compute_execution_order().is_none() {
            return Err(SimError::algebraic_cycle());
        }

        // Initialize all blocks
        self.diagram.init_all()?;
        self.state = SimulationState::Initialized;
        self.current_time = self.config.t_start;
        self.step_count = 0;
        Ok(())
    }

    /// Execute a single simulation step.
    pub fn step(&mut self) -> Result<SimulationState, SimError> {
        if self.state == SimulationState::Uninitialized {
            self.init()?;
        }

        if self.state == SimulationState::Completed
            || self.current_time >= self.config.t_end - 1e-12
        {
            self.state = SimulationState::Completed;
            return Ok(self.state);
        }

        self.state = SimulationState::Running;
        let dt = self.config.dt;

        // Get execution order
        let order = self
            .diagram
            .execution_order()
            .ok_or_else(SimError::no_execution_order)?
            .to_vec();

        // Phase 1: Compute outputs for all blocks
        for block_id in &order {
            if let Some(block) = self.diagram.get_block_mut(block_id) {
                block.set_time(self.current_time);
                block.execute_phase(ExecutionPhase::Output)?;
            }
        }

        // Propagate signals through links
        let link_clones: Vec<_> = self.diagram.links().iter().cloned().collect();
        for mut link_clone in link_clones {
            if let Some(src_block) = self.diagram.get_block(&link_clone.source.0)
                && let Some(port) = src_block.ports().get(&link_clone.source.1)
                && let Some(signal) = port.read()
            {
                link_clone.propagate(signal.clone());
            }
            // Write back propagated signal
            if let Some(l) = self.diagram.links_mut().get_mut(&link_clone.id)
                && let Some(sig) = link_clone.read()
            {
                l.propagate(sig.clone());
            }
        }

        // Phase 2: Compute derivatives
        for block_id in &order {
            if let Some(block) = self.diagram.get_block_mut(block_id) {
                block.execute_phase(ExecutionPhase::Deriv)?;
            }
        }

        // Phase 3: Update discrete states
        for block_id in &order {
            if let Some(block) = self.diagram.get_block_mut(block_id) {
                block.execute_phase(ExecutionPhase::Update)?;
            }
        }

        self.current_time += dt;
        self.step_count += 1;

        if self.current_time >= self.config.t_end - 1e-12 {
            // Terminate
            for block_id in &order {
                if let Some(block) = self.diagram.get_block_mut(block_id) {
                    block.execute_phase(ExecutionPhase::Terminate)?;
                }
            }
            self.state = SimulationState::Completed;
        }

        Ok(self.state)
    }

    /// Run the full simulation from start to end.
    pub fn run(&mut self) -> Result<SimulationState, SimError> {
        self.init()?;

        loop {
            let state = self.step()?;
            if state == SimulationState::Completed {
                break;
            }
            if state == SimulationState::Error {
                return Err(SimError::runtime("simulation encountered an error"));
            }
        }

        Ok(SimulationState::Completed)
    }

    /// Pause the simulation.
    pub fn pause(&mut self) {
        if self.state == SimulationState::Running {
            self.state = SimulationState::Paused;
        }
    }

    /// Resume the simulation.
    pub fn resume(&mut self) {
        if self.state == SimulationState::Paused {
            self.state = SimulationState::Running;
        }
    }

    /// Reset the simulation to its initial state.
    pub fn reset(&mut self) {
        self.diagram.reset_all();
        self.state = SimulationState::Uninitialized;
        self.current_time = self.config.t_start;
        self.step_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::link::Link;

    #[test]
    fn test_engine_run() {
        let mut diagram = Diagram::new("test");
        let src = SimpleBlock::new("src", "Source");
        let sink = SimpleBlock::new("sink", "Sink");
        diagram.add_block(Box::new(src));
        diagram.add_block(Box::new(sink));
        diagram.add_link(Link::new("l1", "src", "out", "sink", "in"));

        let config = EngineConfig {
            t_end: 1.0,
            dt: 0.1,
            ..Default::default()
        };

        let mut engine = SimulationEngine::new(diagram, config);
        let result = engine.run().unwrap();
        assert_eq!(result, SimulationState::Completed);
        assert_eq!(engine.step_count, 10);
    }
}
