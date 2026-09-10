//! Link: a directed signal connection between two ports.
//!
//! Links carry signals from a source (output) port to a destination (input) port.
//! They are the edges in the simulation diagram's topology graph.

use crate::core::port::PortId;
use crate::core::signal::Signal;
use crate::core::types::Time;

/// Unique identifier for a link within a diagram.
pub type LinkId = String;

/// A directed connection from a source port to a destination port.
#[derive(Debug, Clone)]
pub struct Link {
    /// Unique name within the owning diagram.
    pub id: LinkId,
    /// Fully-qualified source: "block.port".
    pub source: (String, PortId),
    /// Fully-qualified destination: "block.port".
    pub destination: (String, PortId),
    /// Human-readable description.
    pub description: String,
    /// Latest signal propagated through this link.
    pub signal: Option<Signal>,
    /// Propagation delay in seconds (zero for direct feedthrough).
    pub delay: Time,
}

impl Link {
    pub fn new(id: &str, source_block: &str, source_port: &str, dest_block: &str, dest_port: &str) -> Self {
        Self {
            id: id.to_string(),
            source: (source_block.to_string(), source_port.to_string()),
            destination: (dest_block.to_string(), dest_port.to_string()),
            description: String::new(),
            signal: None,
            delay: 0.0,
        }
    }

    pub fn with_delay(mut self, delay: Time) -> Self {
        self.delay = delay;
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Propagate a signal through this link.
    pub fn propagate(&mut self, signal: Signal) {
        self.signal = Some(signal);
    }

    /// Read the last propagated signal.
    pub fn read(&self) -> Option<&Signal> {
        self.signal.as_ref()
    }
}

/// A collection of links forming a diagram's connectivity.
#[derive(Debug, Clone, Default)]
pub struct LinkSet {
    links: Vec<Link>,
}

impl LinkSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, link: Link) {
        self.links.push(link);
    }

    pub fn get(&self, id: &str) -> Option<&Link> {
        self.links.iter().find(|l| l.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Link> {
        self.links.iter_mut().find(|l| l.id == id)
    }

    pub fn connections_to(&self, block: &str, port: &str) -> Vec<&Link> {
        self.links.iter().filter(|l| l.destination == (block.to_string(), port.to_string())).collect()
    }

    pub fn connections_from(&self, block: &str, port: &str) -> Vec<&Link> {
        self.links.iter().filter(|l| l.source == (block.to_string(), port.to_string())).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Link> {
        self.links.iter()
    }

    pub fn len(&self) -> usize {
        self.links.len()
    }

    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Perform topological sort on the link graph.
    /// Returns block IDs in execution order, or None if a cycle is detected.
    pub fn topological_sort(&self) -> Option<Vec<String>> {
        use std::collections::{HashMap, HashSet, VecDeque};

        // Build adjacency list and in-degree count.
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_blocks: HashSet<String> = HashSet::new();

        for link in &self.links {
            let src = link.source.0.clone();
            let dst = link.destination.0.clone();
            all_blocks.insert(src.clone());
            all_blocks.insert(dst.clone());
            adj.entry(src.clone()).or_default().push(dst.clone());
            *in_degree.entry(dst).or_insert(0) += 1;
            in_degree.entry(src).or_insert(0);
        }

        for b in &all_blocks {
            in_degree.entry(b.clone()).or_insert(0);
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(b, _)| b.clone())
            .collect();

        let mut order = Vec::new();
        while let Some(block) = queue.pop_front() {
            order.push(block.clone());
            if let Some(neighbors) = adj.get(&block) {
                for n in neighbors {
                    if let Some(deg) = in_degree.get_mut(n) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(n.clone());
                        }
                    }
                }
            }
        }

        if order.len() == all_blocks.len() {
            Some(order)
        } else {
            None // cycle detected
        }
    }
}
