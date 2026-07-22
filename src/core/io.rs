//! I/O declaration types for simulation blocks.
//!
//! Provides declarative specifications for block input/output ports,
//! decoupled from the runtime Port struct. Used for component
//! definition, validation, and automatic port generation.

use crate::core::port::{Port, PortDirection, PortSet};
use crate::core::types::{Extent, SignalType, SignalValue};

/// Declaration of a single input port.
#[derive(Debug, Clone)]
pub struct InputDecl {
    /// Port name (must be unique within the block).
    pub name: String,
    /// Expected signal type.
    pub signal_type: SignalType,
    /// Dimensionality of the port data.
    pub extent: Extent,
    /// Human-readable description.
    pub description: String,
    /// Whether this input is required to be connected.
    pub required: bool,
    /// Default value if left unconnected.
    pub default: Option<SignalValue>,
}

impl InputDecl {
    pub fn new(name: &str, signal_type: SignalType) -> Self {
        Self {
            name: name.to_string(),
            signal_type,
            extent: Extent::scalar(),
            description: String::new(),
            required: true,
            default: None,
        }
    }

    pub fn with_extent(mut self, extent: Extent) -> Self {
        self.extent = extent;
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn with_default(mut self, value: SignalValue) -> Self {
        self.default = Some(value);
        self
    }
}

/// Declaration of a single output port.
#[derive(Debug, Clone)]
pub struct OutputDecl {
    /// Port name (must be unique within the block).
    pub name: String,
    /// Signal type produced by this output.
    pub signal_type: SignalType,
    /// Dimensionality of the output data.
    pub extent: Extent,
    /// Human-readable description.
    pub description: String,
}

impl OutputDecl {
    pub fn new(name: &str, signal_type: SignalType) -> Self {
        Self {
            name: name.to_string(),
            signal_type,
            extent: Extent::scalar(),
            description: String::new(),
        }
    }

    pub fn with_extent(mut self, extent: Extent) -> Self {
        self.extent = extent;
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
}

/// Complete I/O declaration for a block or component.
#[derive(Debug, Clone, Default)]
pub struct IODeclaration {
    /// Input port declarations.
    pub inputs: Vec<InputDecl>,
    /// Output port declarations.
    pub outputs: Vec<OutputDecl>,
}

impl IODeclaration {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_input(&mut self, input: InputDecl) {
        self.inputs.push(input);
    }

    pub fn add_output(&mut self, output: OutputDecl) {
        self.outputs.push(output);
    }

    /// Find an input declaration by name.
    pub fn find_input(&self, name: &str) -> Option<&InputDecl> {
        self.inputs.iter().find(|i| i.name == name)
    }

    /// Find an output declaration by name.
    pub fn find_output(&self, name: &str) -> Option<&OutputDecl> {
        self.outputs.iter().find(|o| o.name == name)
    }

    pub fn has_input(&self, name: &str) -> bool {
        self.inputs.iter().any(|i| i.name == name)
    }

    pub fn has_output(&self, name: &str) -> bool {
        self.outputs.iter().any(|o| o.name == name)
    }

    /// Convert this declaration into a concrete `PortSet`.
    pub fn to_port_set(&self) -> PortSet {
        let mut ports = PortSet::new();
        for decl in &self.inputs {
            let mut port = Port::new(&decl.name, PortDirection::Input, decl.signal_type);
            port.extent = decl.extent;
            port.description = decl.description.clone();
            ports.add(port);
        }
        for decl in &self.outputs {
            let mut port = Port::new(&decl.name, PortDirection::Output, decl.signal_type);
            port.extent = decl.extent;
            port.description = decl.description.clone();
            ports.add(port);
        }
        ports
    }

    /// Count of input ports.
    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    /// Count of output ports.
    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }

    /// Total port count.
    pub fn len(&self) -> usize {
        self.inputs.len() + self.outputs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty() && self.outputs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::SignalType;

    #[test]
    fn test_io_declaration_basic() {
        let mut io = IODeclaration::new();
        io.add_input(InputDecl::new("in1", SignalType::Continuous));
        io.add_output(OutputDecl::new("out1", SignalType::Continuous));
        assert_eq!(io.input_count(), 1);
        assert_eq!(io.output_count(), 1);
        assert!(io.has_input("in1"));
        assert!(io.has_output("out1"));
    }

    #[test]
    fn test_io_to_port_set() {
        let mut io = IODeclaration::new();
        io.add_input(InputDecl::new("u", SignalType::Continuous)
            .with_description("control input"));
        io.add_output(OutputDecl::new("y", SignalType::Continuous)
            .with_description("output signal"));
        let ports = io.to_port_set();
        assert_eq!(ports.len(), 2);
        assert!(ports.get("u").is_some());
        assert!(ports.get("y").is_some());
        assert!(ports.get("u").unwrap().is_input());
        assert!(ports.get("y").unwrap().is_output());
    }

    #[test]
    fn test_input_decl_optional() {
        let decl = InputDecl::new("opt", SignalType::Discrete)
            .optional()
            .with_default(SignalValue::Integer(0));
        assert!(!decl.required);
        assert!(decl.default.is_some());
    }
}
