//! Topological sorting and cycle detection for simulation diagrams.
//!
//! Provides Kahn's algorithm for topological ordering of blocks in a diagram
//! based on their link connections, and DFS-based cycle detection for finding
//! all strongly connected components (algebraic loop candidates).

use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use std::collections::{HashMap, HashSet, VecDeque};

/// A directed graph representation used for topological operations.
#[derive(Debug, Clone)]
pub struct DiGraph {
    /// All nodes (block IDs).
    pub nodes: Vec<BlockId>,
    /// Adjacency list: node -> list of successor nodes.
    pub adjacency: HashMap<BlockId, Vec<BlockId>>,
}

impl DiGraph {
    /// Build a dependency graph from a diagram.
    ///
    /// Edge direction: source block (output port) → destination block (input port).
    /// This means if block A's output connects to block B's input, the edge is A → B.
    pub fn from_diagram(diagram: &Diagram) -> Self {
        let mut adjacency: HashMap<BlockId, Vec<BlockId>> = HashMap::new();

        // Initialize all blocks as nodes
        for (id, _block) in diagram.blocks() {
            adjacency.entry(id.clone()).or_default();
        }

        // Add edges from source to destination for each link
        for link in diagram.links().iter() {
            let src_id = &link.source.0;
            let dst_id = &link.destination.0;

            // Only add edge if both blocks exist in the diagram
            if adjacency.contains_key(src_id) && adjacency.contains_key(dst_id) {
                adjacency.get_mut(src_id).unwrap().push(dst_id.clone());
            }
        }

        let nodes: Vec<BlockId> = adjacency.keys().cloned().collect();
        Self { nodes, adjacency }
    }

    /// Perform topological sort using Kahn's algorithm.
    ///
    /// Returns `Ok(ordered_block_ids)` on success,
    /// or `Err(cycle_block_ids)` if a cycle is detected (listing blocks in the cycle).
    pub fn topological_sort(&self) -> Result<Vec<BlockId>, Vec<BlockId>> {
        // Compute in-degree for each node
        let mut in_degree: HashMap<&BlockId, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.entry(node).or_insert(0);
        }
        for successors in self.adjacency.values() {
            for succ in successors {
                *in_degree.entry(succ).or_insert(0) += 1;
            }
        }

        // Initialize queue with nodes of in-degree 0
        let mut queue: VecDeque<&BlockId> = VecDeque::new();
        for (node, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(node);
            }
        }

        let mut order: Vec<BlockId> = Vec::with_capacity(self.nodes.len());

        while let Some(node) = queue.pop_front() {
            order.push(node.clone());
            if let Some(successors) = self.adjacency.get(node) {
                for succ in successors {
                    if let Some(deg) = in_degree.get_mut(succ) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(succ);
                        }
                    }
                }
            }
        }

        if order.len() == self.nodes.len() {
            Ok(order)
        } else {
            // Find the cycle — nodes still with positive in-degree
            let cycle_nodes: Vec<BlockId> = self
                .nodes
                .iter()
                .filter(|n| in_degree.get(*n).copied().unwrap_or(0) > 0)
                .cloned()
                .collect();
            Err(cycle_nodes)
        }
    }
}

/// Result of cycle detection.
#[derive(Debug, Clone)]
pub struct CycleInfo {
    /// All detected cycles, each as a list of block IDs forming a cycle.
    pub cycles: Vec<Vec<BlockId>>,
    /// Total number of blocks participating in cycles.
    pub involved_blocks: usize,
}

/// Detect all cycles (strongly connected components of size > 1 or self-loops)
/// using depth-first search.
pub fn detect_cycles(diagram: &Diagram) -> CycleInfo {
    let graph = DiGraph::from_diagram(diagram);
    let mut visited: HashSet<BlockId> = HashSet::new();
    let mut in_stack: HashSet<BlockId> = HashSet::new();
    let mut stack: Vec<BlockId> = Vec::new();
    let mut cycles: Vec<Vec<BlockId>> = Vec::new();
    let mut involved: HashSet<BlockId> = HashSet::new();

    // Check self-loops first
    for (node, successors) in &graph.adjacency {
        if successors.contains(node) {
            cycles.push(vec![node.clone(), node.clone()]);
            involved.insert(node.clone());
        }
    }

    // DFS-based cycle detection for longer cycles
    for node in &graph.nodes {
        if !visited.contains(node) {
            dfs_cycle_detect(
                node,
                &graph,
                &mut visited,
                &mut in_stack,
                &mut stack,
                &mut cycles,
                &mut involved,
            );
        }
    }

    CycleInfo {
        cycles,
        involved_blocks: involved.len(),
    }
}

/// Recursive DFS helper for cycle detection.
fn dfs_cycle_detect(
    node: &BlockId,
    graph: &DiGraph,
    visited: &mut HashSet<BlockId>,
    in_stack: &mut HashSet<BlockId>,
    stack: &mut Vec<BlockId>,
    cycles: &mut Vec<Vec<BlockId>>,
    involved: &mut HashSet<BlockId>,
) {
    visited.insert(node.clone());
    in_stack.insert(node.clone());
    stack.push(node.clone());

    if let Some(successors) = graph.adjacency.get(node) {
        for succ in successors {
            if !visited.contains(succ) {
                dfs_cycle_detect(succ, graph, visited, in_stack, stack, cycles, involved);
            } else if in_stack.contains(succ) {
                // Found a cycle: extract from stack
                if let Some(cycle_start) = stack.iter().position(|n| n == succ) {
                    let cycle: Vec<BlockId> = stack[cycle_start..].to_vec();
                    // Avoid duplicate cycles
                    if !cycles.contains(&cycle) && cycle.len() > 1 {
                        cycles.push(cycle.clone());
                        for b in &cycle {
                            involved.insert(b.clone());
                        }
                    }
                }
            }
        }
    }

    stack.pop();
    in_stack.remove(node);
}

/// Perform topological sort on a diagram.
///
/// Convenience function that builds the graph and sorts in one call.
pub fn topological_sort(diagram: &Diagram) -> Result<Vec<BlockId>, Vec<BlockId>> {
    let graph = DiGraph::from_diagram(diagram);
    graph.topological_sort()
}

/// Check whether a diagram has any cycles.
pub fn has_cycles(diagram: &Diagram) -> bool {
    detect_cycles(diagram).involved_blocks > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::diagram::Diagram;
    use crate::core::link::Link;

    fn create_diagram_with_links(link_pairs: &[(&str, &str)]) -> Diagram {
        let mut diagram = Diagram::new("test");
        // Collect unique block IDs from link pairs
        let mut ids: Vec<&str> = Vec::new();
        for (src, dst) in link_pairs {
            if !ids.contains(src) {
                ids.push(src);
            }
            if !ids.contains(dst) {
                ids.push(dst);
            }
        }
        for id in &ids {
            let block = SimpleBlock::new(id, "TestBlock");
            diagram.add_block(Box::new(block));
        }
        for (i, (src, dst)) in link_pairs.iter().enumerate() {
            let link = Link::new(&format!("L{}", i), src, "out", dst, "in");
            diagram.add_link(link);
        }
        diagram
    }

    #[test]
    fn test_linear_chain() {
        let diagram = create_diagram_with_links(&[("B0", "B1"), ("B1", "B2")]);
        let order = topological_sort(&diagram).unwrap();
        assert_eq!(order.len(), 3);
        let p0 = order.iter().position(|x| x == "B0").unwrap();
        let p1 = order.iter().position(|x| x == "B1").unwrap();
        let p2 = order.iter().position(|x| x == "B2").unwrap();
        assert!(p0 < p1, "B0 must come before B1");
        assert!(p1 < p2, "B1 must come before B2");
    }

    #[test]
    fn test_branching_dag() {
        let diagram =
            create_diagram_with_links(&[("B0", "B1"), ("B0", "B2"), ("B1", "B3"), ("B2", "B3")]);
        let order = topological_sort(&diagram).unwrap();
        assert_eq!(order.len(), 4);
        let p0 = order.iter().position(|x| x == "B0").unwrap();
        let p1 = order.iter().position(|x| x == "B1").unwrap();
        let p2 = order.iter().position(|x| x == "B2").unwrap();
        let p3 = order.iter().position(|x| x == "B3").unwrap();
        assert!(p0 < p1 && p0 < p2);
        assert!(p1 < p3 && p2 < p3);
    }

    #[test]
    fn test_simple_cycle_detection() {
        let diagram = create_diagram_with_links(&[("B0", "B1"), ("B1", "B0")]);
        let result = topological_sort(&diagram);
        assert!(result.is_err(), "Expected cycle detection error");
        let cycle_blocks = result.unwrap_err();
        assert_eq!(cycle_blocks.len(), 2);
    }

    #[test]
    fn test_self_loop_detection() {
        let mut diagram = Diagram::new("test");
        let block = SimpleBlock::new("B0", "SelfLoop");
        diagram.add_block(Box::new(block));
        let link = Link::new("L0", "B0", "out", "B0", "in");
        diagram.add_link(link);

        let cycle_info = detect_cycles(&diagram);
        assert!(cycle_info.involved_blocks > 0);
        let result = topological_sort(&diagram);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_diagram() {
        let diagram = Diagram::new("empty");
        let order = topological_sort(&diagram).unwrap();
        assert!(order.is_empty());
    }

    #[test]
    fn test_single_node() {
        let mut diagram = Diagram::new("single");
        diagram.add_block(Box::new(SimpleBlock::new("B0", "Solo")));
        let order = topological_sort(&diagram).unwrap();
        assert_eq!(order, vec!["B0".to_string()]);
    }

    #[test]
    fn test_no_edges() {
        let mut diagram = Diagram::new("no_edges");
        for i in 0..3 {
            diagram.add_block(Box::new(SimpleBlock::new(&format!("B{}", i), "Isolated")));
        }
        let order = topological_sort(&diagram).unwrap();
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn test_detect_no_cycles() {
        let diagram = create_diagram_with_links(&[("B0", "B1")]);
        let cycle_info = detect_cycles(&diagram);
        assert_eq!(cycle_info.cycles.len(), 0);
        assert_eq!(cycle_info.involved_blocks, 0);
        assert!(!has_cycles(&diagram));
    }
}
