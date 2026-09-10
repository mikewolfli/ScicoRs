//! Signal flow analysis for simulation diagrams.
//!
//! Analyzes the direction of signal flow through a diagram, identifying
//! source blocks (no input dependencies), sink blocks (no output consumers),
//! and computing propagation layers for ordered signal updates.

use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::types::PortDirection;
use std::collections::{HashMap, HashSet};

/// Result of signal flow analysis on a diagram.
#[derive(Debug, Clone)]
pub struct SignalFlowGraph {
    /// Source blocks: blocks that have only output ports (no input dependencies).
    pub sources: Vec<BlockId>,
    /// Sink blocks: blocks that have only input ports (no output propagation).
    pub sinks: Vec<BlockId>,
    /// Propagation order: blocks ordered by signal propagation layers.
    /// Each inner Vec contains blocks at the same layer that can be processed
    /// independently (potentially in parallel).
    pub propagation_order: Vec<Vec<BlockId>>,
}

/// Analyze the signal flow in a diagram.
///
/// Returns a `SignalFlowGraph` with identified sources, sinks, and
/// the propagation order organized into layers.
pub fn analyze_signal_flow(diagram: &Diagram) -> SignalFlowGraph {
    let mut sources = Vec::new();
    let mut sinks = Vec::new();

    // Count input/output ports per block
    for (id, block) in diagram.blocks() {
        let mut n_in = 0usize;
        let mut n_out = 0usize;
        for port in block.ports().iter() {
            match port.direction {
                PortDirection::Input => n_in += 1,
                PortDirection::Output => n_out += 1,
                PortDirection::InOut => {
                    n_in += 1;
                    n_out += 1;
                }
            }
        }
        if n_in == 0 && n_out > 0 {
            sources.push(id.clone());
        }
        if n_out == 0 && n_in > 0 {
            sinks.push(id.clone());
        }
    }

    let layers = compute_propagation_layers(diagram);

    SignalFlowGraph {
        sources,
        sinks,
        propagation_order: layers,
    }
}

/// Compute propagation layers for signal flow through the diagram.
///
/// Returns blocks grouped into layers where each layer's blocks have no
/// inter-dependencies and can be processed in parallel.
pub fn compute_propagation_layers(diagram: &Diagram) -> Vec<Vec<BlockId>> {
    let mut successors: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    let mut in_degree: HashMap<BlockId, usize> = HashMap::new();

    for (id, _block) in diagram.blocks() {
        successors.entry(id.clone()).or_default();
        in_degree.entry(id.clone()).or_insert(0);
    }

    for link in diagram.links().iter() {
        let src_id = &link.source.0;
        let dst_id = &link.destination.0;
        if successors.contains_key(src_id) && successors.contains_key(dst_id) {
            successors.get_mut(src_id).unwrap().push(dst_id.clone());
            *in_degree.entry(dst_id.clone()).or_insert(0) += 1;
        }
    }

    let mut layers: Vec<Vec<BlockId>> = Vec::new();
    let mut current_layer: Vec<BlockId> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut remaining_degree: HashMap<BlockId, usize> = in_degree;

    while !current_layer.is_empty() {
        layers.push(current_layer.clone());
        let mut next_layer = Vec::new();
        for node in &current_layer {
            if let Some(succs) = successors.get(node) {
                for succ in succs {
                    if let Some(deg) = remaining_degree.get_mut(succ) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            next_layer.push(succ.clone());
                        }
                    }
                }
            }
        }
        current_layer = next_layer;
    }

    layers
}

/// Find implicit signal connections between blocks (shared signals, buses, etc.).
pub fn find_implicit_connections(diagram: &Diagram) -> Vec<(BlockId, BlockId)> {
    let mut connections = Vec::new();
    let block_ids: HashSet<BlockId> = diagram.blocks().map(|(id, _)| id.clone()).collect();

    let mut named_outputs: HashMap<String, Vec<BlockId>> = HashMap::new();
    for (id, block) in diagram.blocks() {
        for port in block.ports().iter() {
            if port.direction == PortDirection::Output {
                named_outputs
                    .entry(port.id.clone())
                    .or_default()
                    .push(id.clone());
            }
        }
    }

    for (id, block) in diagram.blocks() {
        for port in block.ports().iter() {
            if port.direction == PortDirection::Input
                && let Some(sources) = named_outputs.get(&port.id)
            {
                for src in sources {
                    if src != id
                        && block_ids.contains(src)
                        && !connections.contains(&(src.clone(), id.clone()))
                    {
                        connections.push((src.clone(), id.clone()));
                    }
                }
            }
        }
    }

    connections
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::diagram::Diagram;
    use crate::core::link::Link;
    use crate::core::types::SignalType;

    #[test]
    fn test_signal_flow_linear_chain() {
        let mut diagram = Diagram::new("test");
        let mut a = SimpleBlock::new("A", "Src");
        a.declare_output("out", SignalType::Continuous);
        let mut b = SimpleBlock::new("B", "Mid");
        b.declare_input("in", SignalType::Continuous);
        b.declare_output("out", SignalType::Continuous);
        let mut c = SimpleBlock::new("C", "Sink");
        c.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(a));
        diagram.add_block(Box::new(b));
        diagram.add_block(Box::new(c));
        diagram.add_link(Link::new("L1", "A", "out", "B", "in"));
        diagram.add_link(Link::new("L2", "B", "out", "C", "in"));

        let flow = analyze_signal_flow(&diagram);
        assert_eq!(flow.sources.len(), 1);
        assert!(flow.sources.contains(&"A".to_string()));
        assert_eq!(flow.sinks.len(), 1);
        assert!(flow.sinks.contains(&"C".to_string()));
        assert!(flow.propagation_order.len() >= 3);
    }

    #[test]
    fn test_propagation_layers() {
        let mut diagram = Diagram::new("test");
        let mut a = SimpleBlock::new("A", "Src");
        a.declare_output("out", SignalType::Continuous);
        let mut b = SimpleBlock::new("B", "Mid");
        b.declare_input("in", SignalType::Continuous);
        let mut c = SimpleBlock::new("C", "Mid");
        c.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(a));
        diagram.add_block(Box::new(b));
        diagram.add_block(Box::new(c));
        diagram.add_link(Link::new("L1", "A", "out", "B", "in"));
        diagram.add_link(Link::new("L2", "A", "out", "C", "in"));

        let layers = compute_propagation_layers(&diagram);
        assert_eq!(layers.len(), 2);
        assert!(layers[1].contains(&"B".to_string()));
        assert!(layers[1].contains(&"C".to_string()));
    }

    #[test]
    fn test_no_connections() {
        let mut diagram = Diagram::new("test");
        for i in 0..3 {
            diagram.add_block(Box::new(SimpleBlock::new(&format!("B{}", i), "Isolated")));
        }
        let flow = analyze_signal_flow(&diagram);
        assert!(flow.sources.is_empty());
        assert!(flow.sinks.is_empty());
        let layers = compute_propagation_layers(&diagram);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].len(), 3);
    }

    #[test]
    fn test_implicit_connections() {
        let mut diagram = Diagram::new("test");
        let mut a = SimpleBlock::new("A", "Clock");
        a.declare_output("clk", SignalType::Discrete);
        let mut b = SimpleBlock::new("B", "FF1");
        b.declare_input("clk", SignalType::Discrete);
        let mut c = SimpleBlock::new("C", "FF2");
        c.declare_input("clk", SignalType::Discrete);
        diagram.add_block(Box::new(a));
        diagram.add_block(Box::new(b));
        diagram.add_block(Box::new(c));

        let conns = find_implicit_connections(&diagram);
        assert!(!conns.is_empty());
    }
}
