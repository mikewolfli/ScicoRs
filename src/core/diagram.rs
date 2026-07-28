//! Diagram: system topology and structure.
//!
//! A diagram is a directed graph of interconnected blocks.
//! It owns the blocks and links, and provides methods for
//! topological analysis, serialization, and validation.

use crate::core::block::{Block, BlockId};
use crate::core::error::SimError;
use crate::core::link::{Link, LinkSet};
use crate::core::types::ComponentStatus;
use std::collections::HashMap;

/// A diagram is a named collection of blocks and links forming a simulation model.
pub struct Diagram {
    /// Unique name for this diagram.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// The blocks in this diagram, keyed by ID.
    blocks: HashMap<BlockId, Box<dyn Block>>,
    /// The links connecting blocks.
    links: LinkSet,
    /// Execution order (computed by topological sort).
    execution_order: Option<Vec<BlockId>>,
}

impl std::fmt::Debug for Diagram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Diagram")
            .field("name", &self.name)
            .field("block_count", &self.blocks.len())
            .field("link_count", &self.links.len())
            .field("has_execution_order", &self.execution_order.is_some())
            .finish()
    }
}

impl Diagram {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            blocks: HashMap::new(),
            links: LinkSet::new(),
            execution_order: None,
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Add a block to the diagram.
    pub fn add_block(&mut self, block: Box<dyn Block>) {
        let id = block.id().clone();
        self.blocks.insert(id, block);
        self.execution_order = None; // invalidate cached order
    }

    /// Remove a block by ID.
    pub fn remove_block(&mut self, id: &str) -> Option<Box<dyn Block>> {
        let result = self.blocks.remove(id);
        if result.is_some() {
            self.execution_order = None;
        }
        result
    }

    /// Get a reference to a block by ID.
    pub fn get_block(&self, id: &str) -> Option<&dyn Block> {
        self.blocks.get(id).map(|b| b.as_ref())
    }

    /// Get a mutable reference to a block by ID.
    pub fn get_block_mut(&mut self, id: &str) -> Option<&mut Box<dyn Block>> {
        self.blocks.get_mut(id)
    }

    /// Add a link between two ports.
    pub fn add_link(&mut self, link: Link) {
        self.links.add(link);
        self.execution_order = None; // invalidate cached order
    }

    /// Get a reference to the link set.
    pub fn links(&self) -> &LinkSet {
        &self.links
    }

    /// Get a mutable reference to the link set.
    pub fn links_mut(&mut self) -> &mut LinkSet {
        &mut self.links
    }

    /// Number of blocks in the diagram.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Number of links in the diagram.
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Iterate over all blocks.
    pub fn blocks(&self) -> impl Iterator<Item = (&BlockId, &dyn Block)> {
        self.blocks.iter().map(|(id, b)| (id, b.as_ref()))
    }

    /// Iterate mutably over all blocks.
    pub fn blocks_mut(&mut self) -> impl Iterator<Item = (&BlockId, &mut Box<dyn Block>)> + '_ {
        self.blocks.iter_mut()
    }

    /// Compute topological execution order.
    /// Returns None if a cycle is detected.
    pub fn compute_execution_order(&mut self) -> Option<&[BlockId]> {
        // Get topological order from links (only blocks that participate in links).
        let linked_order = self.links.topological_sort()?;

        // Collect all block IDs from the diagram.
        let all_block_ids: std::collections::HashSet<BlockId> =
            self.blocks.keys().cloned().collect();
        let linked_set: std::collections::HashSet<BlockId> =
            linked_order.iter().cloned().collect();

        // Append any isolated blocks (no links) after the linked order.
        let mut order = linked_order;
        for bid in all_block_ids {
            if !linked_set.contains(&bid) {
                order.push(bid);
            }
        }

        self.execution_order = Some(order);
        Some(self.execution_order.as_ref().unwrap())
    }

    /// Get the cached execution order, if computed.
    pub fn execution_order(&self) -> Option<&[BlockId]> {
        self.execution_order.as_deref()
    }

    /// Initialize all blocks in the diagram.
    pub fn init_all(&mut self) -> Result<(), SimError> {
        for block in self.blocks.values_mut() {
            block.init()?;
            block.set_status(ComponentStatus::Ready);
        }
        Ok(())
    }

    /// Reset all blocks to inactive.
    pub fn reset_all(&mut self) {
        for block in self.blocks.values_mut() {
            block.set_status(ComponentStatus::Inactive);
        }
    }

    /// Check if all blocks have completed.
    pub fn all_completed(&self) -> bool {
        self.blocks
            .values()
            .all(|b| b.status() == ComponentStatus::Completed)
    }

    /// Deep-clone this diagram by cloning each block via the `Block` trait.
    pub fn clone_diagram(&self) -> Self {
        let mut new_blocks: HashMap<BlockId, Box<dyn Block>> = HashMap::new();
        for (id, block) in &self.blocks {
            new_blocks.insert(id.clone(), block.clone_block());
        }
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            blocks: new_blocks,
            links: self.links.clone(),
            execution_order: self.execution_order.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;

    #[test]
    fn test_diagram_add_block() {
        let mut diagram = Diagram::new("test");
        let block = SimpleBlock::new("b1", "Gain");
        diagram.add_block(Box::new(block));
        assert_eq!(diagram.block_count(), 1);
        assert!(diagram.get_block("b1").is_some());
    }

    #[test]
    fn test_diagram_topological_sort() {
        let mut diagram = Diagram::new("test_sort");
        let b1 = SimpleBlock::new("src", "Source");
        let b2 = SimpleBlock::new("gain", "Gain");
        let b3 = SimpleBlock::new("scope", "Scope");
        diagram.add_block(Box::new(b1));
        diagram.add_block(Box::new(b2));
        diagram.add_block(Box::new(b3));

        diagram.add_link(Link::new("l1", "src", "out", "gain", "in"));
        diagram.add_link(Link::new("l2", "gain", "out", "scope", "in"));

        let order = diagram.compute_execution_order();
        assert!(order.is_some());
        let order = order.unwrap();
        assert!(order.len() == 3);
    }
}
