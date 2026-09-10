//! Port: the input/output interface for simulation blocks.
//!
//! Ports are the connection points through which blocks communicate
//! via signals. Each port has a direction, an associated signal type,
//! and an extent that describes the dimensionality of the data.

use crate::core::signal::Signal;
pub use crate::core::types::{Extent, PortDirection, SignalType};

/// Unique identifier for a port within a block.
pub type PortId = String;

/// A port defines a single connection point on a block.
#[derive(Debug, Clone)]
pub struct Port {
    /// Unique name within the owning block.
    pub id: PortId,
    /// Direction of data flow.
    pub direction: PortDirection,
    /// Type of signal this port carries.
    pub signal_type: SignalType,
    /// Dimensionality of the port data.
    pub extent: Extent,
    /// Human-readable description.
    pub description: String,
    /// The current signal value held at this port.
    pub signal: Option<Signal>,
}

impl Port {
    pub fn new(id: &str, direction: PortDirection, signal_type: SignalType) -> Self {
        Self {
            id: id.to_string(),
            direction,
            signal_type,
            extent: Extent::scalar(),
            description: String::new(),
            signal: None,
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

    /// Bind a signal to this port.
    pub fn write(&mut self, signal: Signal) {
        self.signal = Some(signal);
    }

    /// Read the current signal, if any.
    pub fn read(&self) -> Option<&Signal> {
        self.signal.as_ref()
    }

    /// Clear the current signal.
    pub fn clear(&mut self) {
        self.signal = None;
    }

    /// Returns true if this is an input port.
    pub fn is_input(&self) -> bool {
        self.direction == PortDirection::Input
    }

    /// Returns true if this is an output port.
    pub fn is_output(&self) -> bool {
        self.direction == PortDirection::Output
    }
}

/// A collection of ports for a block.
#[derive(Debug, Clone, Default)]
pub struct PortSet {
    ports: Vec<Port>,
}

impl PortSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, port: Port) {
        self.ports.push(port);
    }

    pub fn get(&self, id: &str) -> Option<&Port> {
        self.ports.iter().find(|p| p.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Port> {
        self.ports.iter_mut().find(|p| p.id == id)
    }

    pub fn inputs(&self) -> impl Iterator<Item = &Port> {
        self.ports.iter().filter(|p| p.is_input())
    }

    pub fn outputs(&self) -> impl Iterator<Item = &Port> {
        self.ports.iter().filter(|p| p.is_output())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Port> {
        self.ports.iter()
    }

    pub fn len(&self) -> usize {
        self.ports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ports.is_empty()
    }
}
