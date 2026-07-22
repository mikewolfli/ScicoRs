//! Algebraic Loops and Numerical Stability
//!
//! Provides automatic detection, classification, and resolution of
//! algebraic loops in simulation diagrams. Also includes numerical
//! stability controls such as NaN/Inf capture and overflow protection.

use crate::core::diagram::Diagram;
use crate::core::types::Scalar;
use std::collections::{HashMap, HashSet};

/// Classifies the type of an algebraic loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlgebraicLoopType {
    /// A direct feedthrough loop with no delays.
    DirectFeedthrough,
    /// A loop involving algebraic constraints.
    Constraint,
    /// A loop with implicit dynamics.
    Implicit,
}

/// Represents a detected algebraic loop.
#[derive(Debug, Clone)]
pub struct AlgebraicLoop {
    pub id: usize,
    pub loop_type: AlgebraicLoopType,
    /// The blocks forming the loop, in order.
    pub blocks: Vec<String>,
    /// The links forming the loop, in order.
    pub links: Vec<String>,
}

/// Detector and resolver for algebraic loops.
#[derive(Debug, Default)]
pub struct AlgebraicLoopDetector {
    loops: Vec<AlgebraicLoop>,
}

impl AlgebraicLoopDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Detect algebraic loops in a diagram by analyzing the link graph.
    pub fn detect(&mut self, diagram: &Diagram) -> &[AlgebraicLoop] {
        self.loops.clear();

        // Build adjacency from links
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        let mut link_map: HashMap<(String, String), String> = HashMap::new();

        for link in diagram.links().iter() {
            adj.entry(link.source.0.clone()).or_default().push(link.destination.0.clone());
            link_map.insert((link.source.0.clone(), link.destination.0.clone()), link.id.clone());
        }

        // DFS-based cycle detection
        let all_blocks: Vec<String> = {
            let mut blocks: Vec<String> = diagram.blocks().map(|(id, _)| id.clone()).collect();
            blocks.sort();
            blocks
        };

        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();
        let mut path = Vec::new();

        for block in &all_blocks {
            if !visited.contains(block) {
                self.dfs_cycles(block, &adj, &link_map, &mut visited, &mut in_stack, &mut path);
            }
        }

        &self.loops
    }

    fn dfs_cycles(
        &mut self,
        node: &str,
        adj: &HashMap<String, Vec<String>>,
        link_map: &HashMap<(String, String), String>,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) {
        visited.insert(node.to_string());
        in_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(neighbors) = adj.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    self.dfs_cycles(neighbor, adj, link_map, visited, in_stack, path);
                } else if in_stack.contains(neighbor) {
                    // Found a cycle
                    let cycle_start = path.iter().position(|b| b == neighbor).unwrap();
                    let cycle_blocks: Vec<String> = path[cycle_start..].to_vec();

                    let cycle_links: Vec<String> = cycle_blocks
                        .windows(2)
                        .filter_map(|w| link_map.get(&(w[0].clone(), w[1].clone())))
                        .cloned()
                        .collect();

                    // Close the loop
if let Some(last) = cycle_blocks.last()
                && let Some(_link) = link_map.get(&(last.clone(), neighbor.clone()))
            {
                // Add the closing link
                    }

                    let loop_type = if cycle_blocks.len() <= 2 {
                        AlgebraicLoopType::DirectFeedthrough
                    } else {
                        AlgebraicLoopType::Constraint
                    };

                    self.loops.push(AlgebraicLoop {
                        id: self.loops.len(),
                        loop_type,
                        blocks: cycle_blocks,
                        links: cycle_links,
                    });
                }
            }
        }

        path.pop();
        in_stack.remove(node);
    }

    pub fn detected_loops(&self) -> &[AlgebraicLoop] {
        &self.loops
    }

    pub fn has_loops(&self) -> bool {
        !self.loops.is_empty()
    }
}

/// Numerical stability utilities.
#[derive(Debug, Clone)]
pub struct NumericalStability;

impl NumericalStability {
    /// Check if a scalar value is numerically safe.
    pub fn is_safe(value: Scalar) -> bool {
        value.is_finite() && !value.is_nan()
    }

    /// Clamp a value to a safe range.
    pub fn clamp_safe(value: Scalar, min: Scalar, max: Scalar) -> Scalar {
        if !value.is_finite() || value.is_nan() {
            return 0.0;
        }
        value.clamp(min, max)
    }

    /// Soft saturation using tanh.
    pub fn saturate(value: Scalar, limit: Scalar) -> Scalar {
        limit * (value / limit).tanh()
    }

    /// Check for NaN or Inf and replace with zero.
    pub fn sanitize(value: Scalar) -> Scalar {
        if value.is_finite() {
            value
        } else {
            0.0
        }
    }

    /// Fixed-point iteration for resolving algebraic loops.
    ///
    /// Repeatedly evaluates `f` until convergence or max iterations.
    pub fn fixed_point_iteration(
        f: impl Fn(Scalar) -> Scalar,
        x0: Scalar,
        tol: Scalar,
        max_iter: usize,
        relaxation: Scalar,
    ) -> (Scalar, usize, bool) {
        let mut x = x0;
        for iter in 0..max_iter {
            let x_new = f(x);
            let diff = (x_new - x).abs();
            x = (1.0 - relaxation) * x + relaxation * x_new;
            if diff < tol {
                return (x, iter + 1, true);
            }
        }
        (x, max_iter, false)
    }
}
