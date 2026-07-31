//! Block: the fundamental simulation component.
//!
//! A block is a self-contained functional unit with ports, parameters,
//! internal state, and execution callbacks. The entire simulation is
//! built by connecting blocks into diagrams.

use crate::core::dependency::{DependencyDecl, DependencySet};
use crate::core::error::SimError;
use crate::core::io::{IODeclaration, InputDecl, OutputDecl};
use crate::core::param::ParameterSet;
use crate::core::port::{Port, PortSet};
use crate::core::state::{ContinuousStateVar, DiscreteStateVar, StateDeclaration};
use crate::core::types::{ComponentStatus, ExecutionPhase, Scalar, SignalType, SignalValue, Time};

/// Unique identifier for a block within a diagram.
pub type BlockId = String;

/// Alias for backward compatibility — all errors use unified `SimError`.
pub type BlockError = SimError;

/// The core trait that all simulation blocks must implement.
pub trait Block: Send + Sync {
    /// Return the unique identifier of this block.
    fn id(&self) -> &BlockId;

    /// Provide a human-readable type name for this block.
    fn block_type(&self) -> &str;

    /// Return a reference to the block's ports.
    fn ports(&self) -> &PortSet;

    /// Return a mutable reference to the block's ports.
    fn ports_mut(&mut self) -> &mut PortSet;

    /// Return a reference to the block's parameters.
    fn params(&self) -> &ParameterSet;

    /// Return a mutable reference to the block's parameters.
    fn params_mut(&mut self) -> &mut ParameterSet;

    /// Return the current status of this block.
    fn status(&self) -> ComponentStatus;

    /// Set the status of this block.
    fn set_status(&mut self, status: ComponentStatus);

    /// Advance the internal time of this block.
    fn set_time(&mut self, time: Time);

    /// Return the current internal time.
    fn time(&self) -> Time;

    /// Return the block's I/O declaration (optional, default empty).
    fn io_declaration(&self) -> IODeclaration {
        IODeclaration::new()
    }

    /// Return the block's state declaration (optional, default empty).
    fn state_declaration(&self) -> StateDeclaration {
        StateDeclaration::new()
    }

    /// Read the current continuous state values into a flat vector.
    ///
    /// The order and length must match `state_declaration().continuous`.
    /// The engine uses this to seed its unified state vector before
    /// integration. Default: no engine-managed continuous state (empty).
    fn read_state(&self) -> Vec<Scalar> {
        Vec::new()
    }

    /// Write continuous state values into the block's internal state.
    ///
    /// `state` has length `state_declaration().continuous_count()` and holds
    /// the solver's candidate/integrated state vector. The block must apply
    /// any internal limits. Default: no-op for blocks that keep their state
    /// entirely within the engine-managed vector.
    fn write_state(&mut self, state: &[Scalar]) -> Result<(), SimError> {
        let _ = state;
        Ok(())
    }

    /// Set the simulation step size used by the engine.
    ///
    /// Called once per step before `output()`. Blocks that perform their own
    /// discrete integration (e.g. PID controllers) use this to keep their
    /// internal `dt` in sync with the engine. Default: no-op.
    fn set_step(&mut self, _dt: Scalar) {}

    /// Whether `output()` mutates internal state beyond writing output ports.
    ///
    /// Multi-stage ODE solvers re-evaluate the network at intermediate times
    /// and states; blocks with output side effects (recorders, controllers
    /// that accumulate integrals inside `output()`) must not be re-run. Such
    /// blocks keep the step-start output during stage evaluations. Default:
    /// `false` (output is a pure function of state + inputs + time).
    fn has_output_side_effects(&self) -> bool {
        false
    }

    /// Return the block's dependency declarations (optional, default empty).
    fn dependencies(&self) -> DependencySet {
        DependencySet::new()
    }

    /// Validate the block's configuration against its declarations.
    /// Returns Ok(()) if valid, or Err with a list of issues.
    fn validate_configuration(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let io = self.io_declaration();
        for input in &io.inputs {
            if input.required && self.ports().get(&input.name).is_none() {
                errors.push(format!("missing required input port: {}", input.name));
            }
        }
        for output in &io.outputs {
            if self.ports().get(&output.name).is_none() {
                errors.push(format!("missing declared output port: {}", output.name));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Initialize the block. Called once at simulation start.
    fn init(&mut self) -> Result<(), SimError>;

    /// Compute output signals from input signals and internal state.
    fn output(&mut self) -> Result<(), SimError>;

    /// Compute the time derivative of the continuous state.
    /// Returns a vector of derivatives for each continuous state variable.
    fn derivative(&self) -> Result<Vec<Scalar>, SimError>;

    /// Update the discrete state (for discrete-time blocks).
    fn update(&mut self) -> Result<(), SimError>;

    /// Detect zero-crossing events. Returns crossing signals.
    fn zero_crossings(&self) -> Vec<Scalar>;

    /// Terminate the block. Called once at simulation end.
    fn terminate(&mut self) -> Result<(), SimError>;

    /// Execute a phase of the block's lifecycle.
    fn execute_phase(&mut self, phase: ExecutionPhase) -> Result<(), SimError> {
        match phase {
            ExecutionPhase::Init => self.init(),
            ExecutionPhase::Output => self.output(),
            ExecutionPhase::Deriv => {
                self.derivative()?;
                Ok(())
            }
            ExecutionPhase::Update => self.update(),
            ExecutionPhase::Event => Ok(()),
            ExecutionPhase::Terminate => self.terminate(),
        }
    }

    /// Clone this block into a boxed trait object.
    /// Required for hierarchical component instantiation.
    fn clone_block(&self) -> Box<dyn Block>;
}

/// A simple concrete block implementation for testing and stateless operations.
#[derive(Debug, Clone)]
pub struct SimpleBlock {
    id: BlockId,
    block_type: String,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
    io_decl: IODeclaration,
    state_decl: StateDeclaration,
    dep_set: DependencySet,
}

impl SimpleBlock {
    pub fn new(id: &str, block_type: &str) -> Self {
        Self {
            id: id.to_string(),
            block_type: block_type.to_string(),
            ports: PortSet::new(),
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            current_time: 0.0,
            io_decl: IODeclaration::new(),
            state_decl: StateDeclaration::new(),
            dep_set: DependencySet::new(),
        }
    }

    /// Replace the I/O declaration.
    pub fn with_io(mut self, decl: IODeclaration) -> Self {
        self.io_decl = decl;
        self
    }

    /// Replace the state declaration.
    pub fn with_state(mut self, decl: StateDeclaration) -> Self {
        self.state_decl = decl;
        self
    }

    /// Replace the dependency set.
    pub fn with_dependencies(mut self, deps: DependencySet) -> Self {
        self.dep_set = deps;
        self
    }

    pub fn add_port(&mut self, port: Port) {
        self.ports.add(port);
    }

    /// Declare an input port via I/O declaration and add the port.
    pub fn declare_input(&mut self, name: &str, signal_type: SignalType) {
        self.io_decl.add_input(InputDecl::new(name, signal_type));
        self.ports.add(Port::new(
            name,
            crate::core::types::PortDirection::Input,
            signal_type,
        ));
    }

    /// Declare an output port via I/O declaration and add the port.
    pub fn declare_output(&mut self, name: &str, signal_type: SignalType) {
        self.io_decl.add_output(OutputDecl::new(name, signal_type));
        self.ports.add(Port::new(
            name,
            crate::core::types::PortDirection::Output,
            signal_type,
        ));
    }

    /// Add continuous state variable.
    pub fn add_continuous_state(&mut self, name: &str, initial: Scalar) {
        self.state_decl
            .add_continuous(ContinuousStateVar::new(name, initial));
    }

    /// Add discrete state variable.
    pub fn add_discrete_state(&mut self, name: &str, initial: SignalValue) {
        self.state_decl
            .add_discrete(DiscreteStateVar::new(name, initial));
    }

    /// Add a dependency declaration.
    pub fn add_dependency(&mut self, dep: DependencyDecl) {
        self.dep_set.add(dep);
    }
}

impl Block for SimpleBlock {
    fn id(&self) -> &BlockId {
        &self.id
    }

    fn block_type(&self) -> &str {
        &self.block_type
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

    fn set_status(&mut self, status: ComponentStatus) {
        self.status = status;
    }

    fn set_time(&mut self, time: Time) {
        self.current_time = time;
    }

    fn time(&self) -> Time {
        self.current_time
    }

    fn io_declaration(&self) -> IODeclaration {
        self.io_decl.clone()
    }

    fn state_declaration(&self) -> StateDeclaration {
        self.state_decl.clone()
    }

    fn dependencies(&self) -> DependencySet {
        self.dep_set.clone()
    }

    fn init(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), BlockError> {
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> {
        Ok(Vec::new())
    }

    fn update(&mut self) -> Result<(), BlockError> {
        Ok(())
    }

    fn zero_crossings(&self) -> Vec<Scalar> {
        Vec::new()
    }

    fn terminate(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }

    fn clone_block(&self) -> Box<dyn Block> {
        Box::new(SimpleBlock {
            id: self.id.clone(),
            block_type: self.block_type.clone(),
            ports: self.ports.clone(),
            params: self.params.clone(),
            status: self.status,
            current_time: self.current_time,
            io_decl: self.io_decl.clone(),
            state_decl: self.state_decl.clone(),
            dep_set: self.dep_set.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::state::{ContinuousStateVar, DiscreteStateVar};
    use crate::core::types::SignalType;

    #[test]
    fn test_simple_block_builder_with_io() {
        let mut io = IODeclaration::new();
        io.add_input(InputDecl::new("u", SignalType::Continuous));
        io.add_output(OutputDecl::new("y", SignalType::Continuous));
        let block = SimpleBlock::new("b1", "Test").with_io(io);
        assert!(block.io_declaration().has_input("u"));
        assert!(block.io_declaration().has_output("y"));
    }

    #[test]
    fn test_simple_block_builder_with_state() {
        let mut sd = StateDeclaration::new();
        sd.add_continuous(ContinuousStateVar::new("x", 0.0));
        sd.add_discrete(DiscreteStateVar::new("z", SignalValue::Integer(0)));
        let block = SimpleBlock::new("b2", "TestState").with_state(sd);
        assert_eq!(block.state_declaration().continuous_count(), 1);
        assert_eq!(block.state_declaration().discrete_count(), 1);
    }

    #[test]
    fn test_simple_block_builder_with_deps() {
        let mut ds = DependencySet::new();
        ds.add(DependencyDecl::new("provider", "out", "in"));
        let block = SimpleBlock::new("b3", "TestDep").with_dependencies(ds);
        assert_eq!(block.dependencies().len(), 1);
    }

    #[test]
    fn test_simple_block_lifecycle() {
        let mut block = SimpleBlock::new("life", "Lifecycle");
        assert_eq!(block.status(), ComponentStatus::Inactive);

        block.init().unwrap();
        assert_eq!(block.status(), ComponentStatus::Ready);

        block.output().unwrap();
        block.update().unwrap();
        block.terminate().unwrap();
        assert_eq!(block.status(), ComponentStatus::Completed);
    }
}
