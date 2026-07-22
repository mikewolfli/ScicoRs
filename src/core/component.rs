//! Reusable component template system.
//!
//! A `ComponentTemplate` is a pre-configured Diagram that can be
//! instantiated as a Block, enabling hierarchical model composition.
//! `ComponentInstance` wraps a template and presents it as a standard Block.

use crate::core::block::{Block, BlockId};
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::io::IODeclaration;
use crate::core::param::{Parameter, ParameterSet};
use crate::core::port::PortSet;
use crate::core::types::{ComponentStatus, ExecutionPhase, Scalar, SignalValue, Time};
use std::collections::HashMap;

/// A reusable component template built from an internal diagram.
#[derive(Debug)]
pub struct ComponentTemplate {
    /// Template name.
    pub name: String,
    /// I/O declaration for external ports.
    pub io: IODeclaration,
    /// The internal diagram (blocks + links).
    pub internal_diagram: Diagram,
    /// Mapping from external parameter name to "block.param" path.
    pub parameter_mappings: HashMap<String, String>,
    /// Mapping from external port name to (internal_block_id, internal_port_name).
    pub port_mappings: HashMap<String, (String, String)>,
}

impl ComponentTemplate {
    pub fn new(name: &str, io: IODeclaration) -> Self {
        Self {
            name: name.to_string(),
            io,
            internal_diagram: Diagram::new(&format!("{}_internal", name)),
            parameter_mappings: HashMap::new(),
            port_mappings: HashMap::new(),
        }
    }

    pub fn map_parameter(&mut self, external_name: &str, internal_path: &str) {
        self.parameter_mappings
            .insert(external_name.to_string(), internal_path.to_string());
    }

    pub fn map_port(&mut self, external_name: &str, internal_block: &str, internal_port: &str) {
        self.port_mappings.insert(
            external_name.to_string(),
            (internal_block.to_string(), internal_port.to_string()),
        );
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        for (ext_port, (int_block, int_port)) in &self.port_mappings {
            let block = self.internal_diagram.get_block(int_block);
            if block.is_none() {
                errors.push(format!(
                    "port mapping '{}' refers to missing internal block '{}'",
                    ext_port, int_block
                ));
                continue;
            }
            if block.unwrap().ports().get(int_port).is_none() {
                errors.push(format!(
                    "port mapping '{}' refers to missing internal port '{}.{}'",
                    ext_port, int_block, int_port
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Export the I/O declaration (used for introspection and automatic wiring).
    pub fn export_io(&self) -> IODeclaration {
        self.io.clone()
    }

    /// Build a ComponentInstance from this template.
    pub fn instantiate(
        &self,
        id: &str,
        param_overrides: &HashMap<String, SignalValue>,
    ) -> Result<ComponentInstance, SimError> {
        let external_ports = self.io.to_port_set();

        // Copy the internal diagram so each instance has its own blocks.
        let mut instance_diagram = self.internal_diagram.clone_diagram();

        // Apply parameter overrides through the parameter_mappings.
        let mut params = ParameterSet::new();
        for ext_param in self.parameter_mappings.keys() {
            let value = param_overrides
                .get(ext_param)
                .cloned()
                .unwrap_or(SignalValue::None);
            params.add(Parameter::new_config(
                ext_param,
                value,
                &format!("mapped parameter: {}", ext_param),
            ));
        }

        // Apply overrides to internal blocks' parameters.
        for (ext_name, internal_path) in &self.parameter_mappings {
            if let Some(override_value) = param_overrides.get(ext_name) {
                // internal_path format: "block_id.param_name"
                if let Some((block_id, param_name)) = internal_path.split_once('.')
                    && let Some(block) = instance_diagram.get_block_mut(block_id)
                {
                    block.params_mut().set(param_name, override_value.clone());
                }
            }
        }

        Ok(ComponentInstance {
            id: id.to_string(),
            block_type: self.name.clone(),
            io: self.io.clone(),
            port_mappings: self.port_mappings.clone(),
            instance_diagram,
            external_ports,
            params,
            status: ComponentStatus::Inactive,
            current_time: 0.0,
        })
    }
}

/// A Block wrapping a ComponentTemplate instance.
#[derive(Debug)]
pub struct ComponentInstance {
    id: BlockId,
    block_type: String,
    io: IODeclaration,
    port_mappings: HashMap<String, (String, String)>,
    instance_diagram: Diagram,
    external_ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}

impl Block for ComponentInstance {
    fn id(&self) -> &BlockId {
        &self.id
    }
    fn block_type(&self) -> &str {
        &self.block_type
    }
    fn ports(&self) -> &PortSet {
        &self.external_ports
    }
    fn ports_mut(&mut self) -> &mut PortSet {
        &mut self.external_ports
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
        self.current_time = t;
    }
    fn time(&self) -> Time {
        self.current_time
    }

    fn io_declaration(&self) -> IODeclaration {
        self.io.clone()
    }

    fn init(&mut self) -> Result<(), SimError> {
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), SimError> {
        // Route external input signals to internal blocks.
        for (ext_port, (int_block, int_port)) in &self.port_mappings {
            if let Some(ext_signal) = self.external_ports.get(ext_port).and_then(|p| p.read())
                && let Some(block) = self.instance_diagram.get_block_mut(int_block)
                && let Some(port) = block.ports_mut().get_mut(int_port)
            {
                port.write(ext_signal.clone());
            }
        }

        let order = self
            .instance_diagram
            .execution_order()
            .ok_or_else(SimError::no_execution_order)?
            .to_vec();

        for block_id in &order {
            if let Some(block) = self.instance_diagram.get_block_mut(block_id) {
                block.set_time(self.current_time);
                block.execute_phase(ExecutionPhase::Output)?;
            }
        }

        // Route internal outputs to external ports.
        for (ext_port, (int_block, int_port)) in &self.port_mappings {
            if let Some(block) = self.instance_diagram.get_block(int_block)
                && let Some(port) = block.ports().get(int_port)
                && let Some(signal) = port.read()
                && let Some(ext) = self.external_ports.get_mut(ext_port)
            {
                ext.write(signal.clone());
            }
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, SimError> {
        Ok(Vec::new())
    }

    fn update(&mut self) -> Result<(), SimError> {
        let order = self
            .instance_diagram
            .execution_order()
            .ok_or_else(SimError::no_execution_order)?
            .to_vec();
        for block_id in &order {
            if let Some(block) = self.instance_diagram.get_block_mut(block_id) {
                block.execute_phase(ExecutionPhase::Update)?;
            }
        }
        Ok(())
    }

    fn zero_crossings(&self) -> Vec<Scalar> {
        Vec::new()
    }

    fn terminate(&mut self) -> Result<(), SimError> {
        let order = self
            .instance_diagram
            .execution_order()
            .ok_or_else(SimError::no_execution_order)?
            .to_vec();
        for block_id in &order {
            if let Some(block) = self.instance_diagram.get_block_mut(block_id) {
                block.execute_phase(ExecutionPhase::Terminate)?;
            }
        }
        self.status = ComponentStatus::Completed;
        Ok(())
    }

    fn clone_block(&self) -> Box<dyn Block> {
        Box::new(Self {
            id: self.id.clone(),
            block_type: self.block_type.clone(),
            io: self.io.clone(),
            port_mappings: self.port_mappings.clone(),
            instance_diagram: self.instance_diagram.clone_diagram(),
            external_ports: self.external_ports.clone(),
            params: self.params.clone(),
            status: self.status,
            current_time: self.current_time,
        })
    }

    fn validate_configuration(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        for input in &self.io.inputs {
            if !self.port_mappings.contains_key(&input.name) {
                errors.push(format!(
                    "input port '{}' has no internal mapping",
                    input.name
                ));
            }
        }
        for output in &self.io.outputs {
            if !self.port_mappings.contains_key(&output.name) {
                errors.push(format!(
                    "output port '{}' has no internal mapping",
                    output.name
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::io::{InputDecl, OutputDecl};
    use crate::core::link::Link;
    use crate::core::types::SignalType;

    #[test]
    fn test_component_template_validation() {
        let io = IODeclaration::new();
        let template = ComponentTemplate::new("test_comp", io);
        assert!(template.validate().is_ok());
    }

    #[test]
    fn test_component_instantiation() {
        let mut io = IODeclaration::new();
        io.add_input(InputDecl::new("ext_in", SignalType::Continuous));
        io.add_output(OutputDecl::new("ext_out", SignalType::Continuous));

        let mut template = ComponentTemplate::new("amplifier", io);
        let mut src = SimpleBlock::new("src", "Source");
        src.declare_output("out", SignalType::Continuous);
        let mut sink = SimpleBlock::new("sink", "Sink");
        sink.declare_input("in", SignalType::Continuous);

        template.internal_diagram.add_block(Box::new(src));
        template.internal_diagram.add_block(Box::new(sink));
        template
            .internal_diagram
            .add_link(Link::new("l1", "src", "out", "sink", "in"));
        template.internal_diagram.compute_execution_order();

        template.map_port("ext_in", "sink", "in");
        template.map_port("ext_out", "src", "out");

        let params = HashMap::new();
        let instance = template.instantiate("amp1", &params).unwrap();
        assert_eq!(instance.id(), "amp1");
        assert!(instance.ports().get("ext_in").is_some());
        assert!(instance.ports().get("ext_out").is_some());
    }

    #[test]
    fn test_component_export_io() {
        let mut io = IODeclaration::new();
        io.add_input(InputDecl::new("in1", SignalType::Continuous));
        io.add_output(OutputDecl::new("out1", SignalType::Continuous));
        let template = ComponentTemplate::new("export_test", io);
        let exported = template.export_io();
        assert!(exported.has_input("in1"));
        assert!(exported.has_output("out1"));
        assert_eq!(exported.input_count(), 1);
        assert_eq!(exported.output_count(), 1);
    }

    #[test]
    fn test_component_instance_has_internal_blocks() {
        let mut io = IODeclaration::new();
        io.add_input(InputDecl::new("ext_in", SignalType::Continuous));
        io.add_output(OutputDecl::new("ext_out", SignalType::Continuous));

        let mut template = ComponentTemplate::new("nested", io);
        let mut src = SimpleBlock::new("src", "Source");
        src.declare_output("out", SignalType::Continuous);
        let mut sink = SimpleBlock::new("sink", "Sink");
        sink.declare_input("in", SignalType::Continuous);

        template.internal_diagram.add_block(Box::new(src));
        template.internal_diagram.add_block(Box::new(sink));
        template.internal_diagram.add_link(Link::new("l1", "src", "out", "sink", "in"));
        template.internal_diagram.compute_execution_order();

        template.map_port("ext_in", "sink", "in");
        template.map_port("ext_out", "src", "out");

        let params = HashMap::new();
        let mut instance = template.instantiate("n1", &params).unwrap();

        // Verify the instance can execute (would fail if internal diagram empty)
        instance.init().unwrap();
        instance.output().unwrap();
        instance.update().unwrap();
        instance.terminate().unwrap();
        assert_eq!(instance.status(), ComponentStatus::Completed);
    }
}
