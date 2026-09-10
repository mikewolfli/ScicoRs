//! Port signal propagation, caching, and synchronization.
//!
//! Provides the `SignalCache` for storing port signal values and the
//! propagation engine that transfers values from source output ports
//! through links to destination input ports.

use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::types::{PortDirection, SignalValue};
use std::collections::HashMap;

/// A cache of signal values for all ports in a diagram.
///
/// Stores both current and previous signal values for edge detection
/// and signal change tracking.
#[derive(Debug, Clone)]
pub struct SignalCache {
    /// Current signal values: (block_id, port_id) -> SignalValue
    current: HashMap<(BlockId, String), SignalValue>,
    /// Previous time-step signal values (for edge detection)
    previous: HashMap<(BlockId, String), SignalValue>,
}

impl SignalCache {
    /// Create a new empty signal cache.
    pub fn new() -> Self {
        Self {
            current: HashMap::new(),
            previous: HashMap::new(),
        }
    }

    /// Initialize the cache with all ports from a diagram.
    /// All ports start with SignalValue::None.
    pub fn from_diagram(diagram: &Diagram) -> Self {
        let mut cache = Self::new();
        for (id, block) in diagram.blocks() {
            for port in block.ports().iter() {
                let key = (id.clone(), port.id.clone());
                cache.current.insert(key.clone(), SignalValue::None);
                cache.previous.insert(key, SignalValue::None);
            }
        }
        cache
    }

    /// Get the current signal value for a port.
    pub fn get(&self, block_id: &str, port_id: &str) -> Option<&SignalValue> {
        self.current
            .get(&(block_id.to_string(), port_id.to_string()))
    }

    /// Set the current signal value for a port.
    pub fn set(&mut self, block_id: &str, port_id: &str, value: SignalValue) {
        self.current
            .insert((block_id.to_string(), port_id.to_string()), value);
    }

    /// Get the previous time-step signal value for a port.
    pub fn get_previous(&self, block_id: &str, port_id: &str) -> Option<&SignalValue> {
        self.previous
            .get(&(block_id.to_string(), port_id.to_string()))
    }

    /// Advance the cache: move current values to previous.
    /// Called at the end of each simulation step.
    pub fn advance(&mut self) {
        std::mem::swap(&mut self.current, &mut self.previous);
        // Reset current to None for all ports
        for key in self.previous.keys() {
            self.current.entry(key.clone()).or_insert(SignalValue::None);
        }
    }

    /// Check if a signal value changed from the previous step.
    pub fn has_changed(&self, block_id: &str, port_id: &str) -> bool {
        let key = (block_id.to_string(), port_id.to_string());
        match (self.current.get(&key), self.previous.get(&key)) {
            (Some(curr), Some(prev)) => curr != prev,
            _ => false,
        }
    }

    /// Number of cached port values.
    pub fn len(&self) -> usize {
        self.current.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }
}

impl Default for SignalCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Propagate signals through links: copy source output values to destination inputs.
///
/// For each link in the diagram, reads the source block's output port value from
/// the cache and writes it to the destination block's input port in the cache.
pub fn propagate_signals(diagram: &Diagram, cache: &mut SignalCache) -> Result<(), SimError> {
    for link in diagram.links().iter() {
        let src_key = (link.source.0.clone(), link.source.1.clone());
        let dst_key = (link.destination.0.clone(), link.destination.1.clone());

        let value = cache
            .current
            .get(&src_key)
            .cloned()
            .unwrap_or(SignalValue::None);

        cache.current.insert(dst_key, value);
    }
    Ok(())
}

/// Extract all output port values from blocks into the signal cache.
pub fn extract_outputs(diagram: &Diagram, cache: &mut SignalCache) -> Result<(), SimError> {
    for (id, block) in diagram.blocks() {
        for port in block.ports().iter() {
            if port.direction == PortDirection::Output {
                let value = port
                    .signal
                    .as_ref()
                    .map(|s| s.value.clone())
                    .unwrap_or(SignalValue::None);
                cache.set(id, &port.id, value);
            }
        }
    }
    Ok(())
}

/// Write cached signal values back to block input ports.
///
/// For each block in the diagram, iterates over its input ports and writes
/// the corresponding cached signal value (previously propagated from source
/// output ports via `propagate_signals()`) into the port's signal field.
/// Requires `&mut Diagram` because port writes mutate block state.
pub fn update_inputs(diagram: &mut Diagram, cache: &SignalCache) -> Result<(), SimError> {
    // Collect all block IDs first to avoid borrow conflicts
    let block_ids: Vec<String> = diagram.blocks().map(|(id, _)| id.clone()).collect();
    for id in &block_ids {
        if let Some(block) = diagram.get_block_mut(id) {
            let port_ids: Vec<String> = block.ports().inputs().map(|p| p.id.clone()).collect();
            for port_id in &port_ids {
                if let Some(value) = cache.get(id, port_id)
                    && !matches!(value, SignalValue::None)
                    && let Some(port) = block.ports_mut().get_mut(port_id)
                {
                    port.write(crate::core::signal::Signal::new(
                        port.signal_type,
                        value.clone(),
                        0.0, // time will be set by engine
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::diagram::Diagram;
    use crate::core::link::Link;
    use crate::core::types::SignalType;

    #[test]
    fn test_signal_cache_create() {
        let cache = SignalCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_signal_cache_from_diagram() {
        let mut diagram = Diagram::new("test");
        let mut block = SimpleBlock::new("B1", "Test");
        block.declare_input("in", SignalType::Continuous);
        block.declare_output("out", SignalType::Continuous);
        diagram.add_block(Box::new(block));

        let cache = SignalCache::from_diagram(&diagram);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("B1", "in"), Some(&SignalValue::None));
        assert_eq!(cache.get("B1", "out"), Some(&SignalValue::None));
    }

    #[test]
    fn test_signal_cache_set_get() {
        let mut cache = SignalCache::new();
        cache.set("B1", "out", SignalValue::Scalar(42.0));
        assert_eq!(cache.get("B1", "out"), Some(&SignalValue::Scalar(42.0)));
        assert_eq!(cache.get("B1", "in"), None);
    }

    #[test]
    fn test_signal_propagation() {
        let mut diagram = Diagram::new("test");
        let mut src = SimpleBlock::new("Src", "Source");
        src.declare_output("out", SignalType::Continuous);
        let mut dst = SimpleBlock::new("Dst", "Sink");
        dst.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(src));
        diagram.add_block(Box::new(dst));
        diagram.add_link(Link::new("L1", "Src", "out", "Dst", "in"));

        let mut cache = SignalCache::from_diagram(&diagram);
        cache.set("Src", "out", SignalValue::Scalar(42.0_f64));
        propagate_signals(&diagram, &mut cache).unwrap();
        assert_eq!(cache.get("Dst", "in"), Some(&SignalValue::Scalar(42.0_f64)));
    }

    #[test]
    fn test_signal_cache_advance() {
        let mut cache = SignalCache::new();
        cache.set("B1", "out", SignalValue::Scalar(1.0));
        cache.advance();
        assert_eq!(
            cache.get_previous("B1", "out"),
            Some(&SignalValue::Scalar(1.0))
        );
        assert_eq!(cache.get("B1", "out"), Some(&SignalValue::None));
    }

    #[test]
    fn test_signal_changed_detection() {
        let mut cache = SignalCache::new();
        cache.set("B1", "out", SignalValue::Scalar(1.0));
        cache.advance();
        cache.set("B1", "out", SignalValue::Scalar(2.0));
        assert!(cache.has_changed("B1", "out"));
    }
}
