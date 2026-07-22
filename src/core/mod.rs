//! Core Model Kernel
//!
//! The foundational layer of the simulation kernel. Defines the
//! basic building blocks for constructing simulation models:
//!
//! - **Block**: the fundamental functional simulation unit
//! - **Port**: input/output interfaces on blocks
//! - **Link**: directed signal connections between ports
//! - **Diagram**: a topology of interconnected blocks
//! - **Param**: static, configurable, and expression-bound parameters
//! - **Signal**: continuous, discrete, event, and bus signal types
//! - **Types**: shared numeric types, enums, and value representations
//! - **Tensor**: N-dimensional array type
//! - **IO**: declarative I/O specifications
//! - **State**: state variable declarations
//! - **Dependency**: inter-block dependency declarations
//! - **Component**: reusable component template system
//! - **DiagramSer**: JSON/TOML serialization
//! - **DiagramValidate**: diagram validation rules

pub mod block;
pub mod component;
pub mod dependency;
pub mod diagram;
pub mod diagram_ser;
pub mod diagram_validate;
pub mod error;
pub mod io;
pub mod link;
pub mod param;
pub mod port;
pub mod signal;
pub mod state;
pub mod tensor;
pub mod types;

pub use block::{Block, BlockError, SimpleBlock};
pub use component::{ComponentInstance, ComponentTemplate};
pub use dependency::{DependencyDecl, DependencySet};
pub use diagram::Diagram;
pub use diagram_ser::{diagram_to_json, diagram_to_toml, json_to_diagram, toml_to_diagram};
pub use diagram_validate::{ValidationResult, validate_diagram};
pub use error::{ErrorCode, SimError};
pub use io::{IODeclaration, InputDecl, OutputDecl};
pub use link::Link;
pub use param::{ExpressionParameter, Parameter, ParameterSet};
pub use port::Port;
pub use signal::{BusSignal, ContinuousSignal, DiscreteSignal, EventSignal, Signal};
pub use state::{ContinuousStateVar, DiscreteStateVar, StateDeclaration};
pub use tensor::{Tensor, TensorDims};
pub use types::{
    ComponentStatus, EPSILON, ExecutionPhase, Extent, Index, PortDirection, Rate, Scalar,
    SignalType, SignalValue, Time,
};
