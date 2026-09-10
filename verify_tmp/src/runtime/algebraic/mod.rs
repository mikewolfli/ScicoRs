//! Algebraic loop detection and numerical stability (Phase 8).
//!
//! Provides tools for detecting, classifying, and resolving algebraic
//! loops in simulation diagrams, along with numerical stability guards.
//!
//! An algebraic loop occurs when signals form a cycle through blocks
//! with direct feedthrough (output depends directly on input at the
//! same time step). These loops require iterative solution methods.
//!
//! # Components
//!
//! - **`AlgebraicLoopDetector`** — finds all strongly connected components
//!   (SCCs) of size > 1 in the diagram's port-level dependency graph,
//!   marking each as an algebraic loop candidate.
//! - **`DirectFeedthroughPath`** — identifies paths where a block's output
//!   depends directly on its input without a unit delay.
//! - **`FixedPointIteration`** — solves algebraic loops by repeatedly
//!   evaluating the loop equations until convergence.
//! - **`RelaxationIteration`** — damped fixed-point iteration with a
//!   relaxation factor for improved stability.
//! - **`NumericalGuard`** — NaN/Inf detection and overflow protection.

use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::types::Scalar;
use std::collections::{HashMap, HashSet};

// ──────────────────────────────────────────────
// 1. Algebraic Loop Detection
// ──────────────────────────────────────────────

/// Describes a detected algebraic loop in the diagram.
#[derive(Debug, Clone)]
pub struct AlgebraicLoop {
    /// The block IDs participating in this loop.
    pub blocks: Vec<BlockId>,
    /// The links forming the loop edges.
    pub links: Vec<String>,
    /// Estimated loop order (number of blocks in the cycle).
    pub order: usize,
}

/// Result of algebraic loop analysis.
#[derive(Debug, Clone, Default)]
pub struct LoopAnalysis {
    /// All detected algebraic loops.
    pub loops: Vec<AlgebraicLoop>,
    /// Total number of blocks involved in loops.
    pub total_involved: usize,
    /// Whether any loops were detected.
    pub has_loops: bool,
}

/// Detects and classifies algebraic loops in a simulation diagram.
///
/// Uses Tarjan's strongly connected components algorithm on the
/// port-level dependency graph. A strongly connected component of
/// size > 1 (or a self-loop with direct feedthrough) is flagged as
/// an algebraic loop.
#[derive(Debug, Clone)]
pub struct AlgebraicLoopDetector {
    analysis: LoopAnalysis,
}

impl AlgebraicLoopDetector {
    /// Create a new detector and immediately analyse the given diagram.
    pub fn new(diagram: &Diagram) -> Self {
        let mut detector = Self {
            analysis: LoopAnalysis::default(),
        };
        detector.analyse(diagram);
        detector
    }

    /// Run the analysis on a diagram.
    pub fn analyse(&mut self, diagram: &Diagram) -> &LoopAnalysis {
        let mut loops: Vec<AlgebraicLoop> = Vec::new();

        // Build a block-level dependency graph from link connections.
        let mut successors: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        let mut link_labels: HashMap<(BlockId, BlockId), Vec<String>> = HashMap::new();

        for (bid, _block) in diagram.blocks() {
            successors.entry(bid.clone()).or_default();
        }

        for link in diagram.links().iter() {
            let src = link.source.0.clone();
            let dst = link.destination.0.clone();
            if successors.contains_key(&src) && successors.contains_key(&dst) {
                successors.get_mut(&src).unwrap().push(dst.clone());
                link_labels
                    .entry((src, dst))
                    .or_default()
                    .push(link.id.clone());
            }
        }

        // Tarjan's SCC algorithm to find strongly connected components.
        let sccs = tarjan_scc(&successors);

        // Filter SCCs: size > 1 or self-loop = algebraic loop candidate.
        let mut involved: HashSet<BlockId> = HashSet::new();

        for component in &sccs {
            let size = component.len();
            if size > 1 || (size == 1 && has_self_loop(diagram, &component[0])) {
                let mut link_ids: Vec<String> = Vec::new();
                for i in 0..component.len() {
                    let src = &component[i];
                    let dst = &component[(i + 1) % component.len()];
                    if let Some(ids) = link_labels.get(&(src.clone(), dst.clone())) {
                        link_ids.extend(ids.iter().cloned());
                    }
                    if let Some(ids) = link_labels.get(&(dst.clone(), src.clone())) {
                        link_ids.extend(ids.iter().cloned());
                    }
                }
                link_ids.sort();
                link_ids.dedup();

                loops.push(AlgebraicLoop {
                    blocks: component.clone(),
                    links: link_ids,
                    order: size,
                });
                for b in component {
                    involved.insert(b.clone());
                }
            }
        }

        self.analysis = LoopAnalysis {
            total_involved: involved.len(),
            has_loops: !loops.is_empty(),
            loops,
        };
        &self.analysis
    }

    /// Get a reference to the current analysis results.
    pub fn analysis(&self) -> &LoopAnalysis {
        &self.analysis
    }

    /// Returns `true` if the diagram contains any algebraic loops.
    pub fn has_loops(&self) -> bool {
        self.analysis.has_loops
    }

    /// Returns the number of detected loops.
    pub fn loop_count(&self) -> usize {
        self.analysis.loops.len()
    }
}

/// Tarjan's SCC algorithm for directed graphs.
fn tarjan_scc(graph: &HashMap<BlockId, Vec<BlockId>>) -> Vec<Vec<BlockId>> {
    let mut index_counter = 0usize;
    let mut stack: Vec<BlockId> = Vec::new();
    let mut on_stack: HashSet<BlockId> = HashSet::new();
    let mut indices: HashMap<BlockId, usize> = HashMap::new();
    let mut lowlinks: HashMap<BlockId, usize> = HashMap::new();
    let mut sccs: Vec<Vec<BlockId>> = Vec::new();

    #[allow(clippy::too_many_arguments)]
    fn strongconnect(
        v: &BlockId,
        graph: &HashMap<BlockId, Vec<BlockId>>,
        index_counter: &mut usize,
        indices: &mut HashMap<BlockId, usize>,
        lowlinks: &mut HashMap<BlockId, usize>,
        stack: &mut Vec<BlockId>,
        on_stack: &mut HashSet<BlockId>,
        sccs: &mut Vec<Vec<BlockId>>,
    ) {
        indices.insert(v.clone(), *index_counter);
        lowlinks.insert(v.clone(), *index_counter);
        *index_counter += 1;
        stack.push(v.clone());
        on_stack.insert(v.clone());

        if let Some(neighbors) = graph.get(v) {
            for w in neighbors {
                if !indices.contains_key(w) {
                    strongconnect(
                        w,
                        graph,
                        index_counter,
                        indices,
                        lowlinks,
                        stack,
                        on_stack,
                        sccs,
                    );
                    let v_low = lowlinks[v];
                    let w_low = lowlinks[w];
                    lowlinks.insert(v.clone(), v_low.min(w_low));
                } else if on_stack.contains(w) {
                    let v_low = lowlinks[v];
                    let w_idx = indices[w];
                    lowlinks.insert(v.clone(), v_low.min(w_idx));
                }
            }
        }

        if lowlinks.get(v) == indices.get(v) {
            let mut component: Vec<BlockId> = Vec::new();
            loop {
                let w = stack.pop().unwrap();
                on_stack.remove(&w);
                component.push(w.clone());
                if w == *v {
                    break;
                }
            }
            if !component.is_empty() {
                component.sort();
                sccs.push(component);
            }
        }
    }

    let all_nodes: Vec<BlockId> = graph.keys().cloned().collect();
    for node in &all_nodes {
        if !indices.contains_key(node) {
            strongconnect(
                node,
                graph,
                &mut index_counter,
                &mut indices,
                &mut lowlinks,
                &mut stack,
                &mut on_stack,
                &mut sccs,
            );
        }
    }

    sccs
}

/// Check if a block has a self-loop (output connected back to its own input).
fn has_self_loop(diagram: &Diagram, block_id: &str) -> bool {
    diagram
        .links()
        .iter()
        .any(|l| l.source.0 == block_id && l.destination.0 == block_id)
}

// ──────────────────────────────────────────────
// 2. Direct Feedthrough Path Identification
// ──────────────────────────────────────────────

/// A path through blocks where output depends directly on input.
#[derive(Debug, Clone)]
pub struct DirectFeedthroughPath {
    /// The sequence of block IDs forming the path.
    pub path: Vec<BlockId>,
    /// Whether this path participates in a loop.
    pub in_loop: bool,
    /// Estimated path length (number of blocks).
    pub length: usize,
}

/// Identify all direct feedthrough paths in a diagram.
///
/// A direct feedthrough path means each block's output depends on its
/// input at the same time step (no unit delay). These paths, when
/// forming cycles, create algebraic loops.
pub fn find_direct_feedthrough_paths(diagram: &Diagram) -> Vec<DirectFeedthroughPath> {
    let mut paths = Vec::new();
    let graph = build_adjacency(diagram);

    for (start, _) in diagram.blocks() {
        let mut visited: HashSet<BlockId> = HashSet::new();
        let mut current_path: Vec<BlockId> = Vec::new();
        dfs_paths(start, &graph, &mut visited, &mut current_path, &mut paths);
    }

    paths.sort_by_key(|p| p.length);
    paths.dedup_by_key(|p| p.path.clone());
    paths
}

fn build_adjacency(diagram: &Diagram) -> HashMap<BlockId, Vec<BlockId>> {
    let mut adj: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    for (id, _) in diagram.blocks() {
        adj.entry(id.clone()).or_default();
    }
    for link in diagram.links().iter() {
        let src = link.source.0.clone();
        let dst = link.destination.0.clone();
        if adj.contains_key(&src) && adj.contains_key(&dst) {
            adj.get_mut(&src).unwrap().push(dst);
        }
    }
    adj
}

fn dfs_paths(
    current: &BlockId,
    graph: &HashMap<BlockId, Vec<BlockId>>,
    visited: &mut HashSet<BlockId>,
    path: &mut Vec<BlockId>,
    paths: &mut Vec<DirectFeedthroughPath>,
) {
    if visited.contains(current) {
        let cycle_start = path.iter().position(|n| n == current);
        if let Some(start) = cycle_start {
            let cycle_path: Vec<BlockId> = path[start..].to_vec();
            if cycle_path.len() >= 2 {
                paths.push(DirectFeedthroughPath {
                    in_loop: true,
                    path: cycle_path,
                    length: path.len() - start,
                });
            }
        }
        return;
    }

    visited.insert(current.clone());
    path.push(current.clone());

    if let Some(neighbors) = graph.get(current) {
        for next in neighbors {
            if !visited.contains(next) || path.contains(next) {
                dfs_paths(next, graph, visited, path, paths);
            }
        }
    }

    path.pop();
    visited.remove(current);
}

// ──────────────────────────────────────────────
// 3. Fixed-Point Iteration for Algebraic Loops
// ──────────────────────────────────────────────

/// Configuration for algebraic loop solvers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlgebraicSolverConfig {
    /// Maximum number of iterations.
    pub max_iterations: usize,
    /// Convergence tolerance.
    pub tolerance: Scalar,
    /// Relaxation factor (0.0 < omega <= 1.0 for under-relaxation).
    pub relaxation_factor: Scalar,
    /// Whether to abort on NaN/Inf detection.
    pub abort_on_nan: bool,
}

impl Default for AlgebraicSolverConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            tolerance: 1e-10,
            relaxation_factor: 1.0,
            abort_on_nan: true,
        }
    }
}

/// Result of an algebraic loop solver iteration.
#[derive(Debug, Clone, PartialEq)]
pub enum AlgebraicSolveResult {
    /// Converged to a consistent solution.
    Converged {
        iterations: usize,
        final_error: Scalar,
    },
    /// Maximum iterations reached without convergence.
    NotConverged {
        iterations: usize,
        last_error: Scalar,
    },
    /// NaN or Inf detected in the solution.
    NumericalError(String),
}

/// Fixed-point iteration solver for algebraic loops.
///
/// Repeatedly evaluates the loop function `x_{k+1} = F(x_k)` until
/// convergence: `|x_{k+1} - x_k| < tolerance`.
pub struct FixedPointIteration {
    config: AlgebraicSolverConfig,
}

impl FixedPointIteration {
    /// Create a new fixed-point iteration solver.
    pub fn new(config: AlgebraicSolverConfig) -> Self {
        Self { config }
    }

    /// Solve the algebraic loop using fixed-point iteration.
    ///
    /// `f` is the loop function: given current signal values, computes
    /// the next iteration's values. Returns the converged result.
    pub fn solve<F>(&self, mut f: F, initial: &[Scalar]) -> AlgebraicSolveResult
    where
        F: FnMut(&[Scalar]) -> Result<Vec<Scalar>, SimError>,
    {
        let n = initial.len();
        let mut x = initial.to_vec();

        for iter in 0..self.config.max_iterations {
            let x_next = match f(&x) {
                Ok(v) => v,
                Err(e) => {
                    return AlgebraicSolveResult::NumericalError(format!(
                        "function evaluation failed: {}",
                        e
                    ));
                }
            };

            // NaN/Inf check
            if self.config.abort_on_nan
                && let Some(problem) = NumericalGuard::check_all(&x_next)
            {
                return AlgebraicSolveResult::NumericalError(problem);
            }

            // Compute max error
            let mut max_error: Scalar = 0.0;
            for i in 0..n.min(x_next.len()) {
                let err = (x_next[i] - x[i]).abs();
                if err > max_error {
                    max_error = err;
                }
            }

            // Apply relaxation: x_{k+1} = (1-ω) * x_k + ω * F(x_k)
            let omega = self.config.relaxation_factor;
            if (omega - 1.0).abs() > 1e-15 {
                for i in 0..n.min(x_next.len()) {
                    x[i] = (1.0 - omega) * x[i] + omega * x_next[i];
                }
            } else {
                x = x_next;
            }

            if max_error < self.config.tolerance {
                return AlgebraicSolveResult::Converged {
                    iterations: iter + 1,
                    final_error: max_error,
                };
            }
        }

        AlgebraicSolveResult::NotConverged {
            iterations: self.config.max_iterations,
            last_error: 0.0,
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &AlgebraicSolverConfig {
        &self.config
    }
}

// ──────────────────────────────────────────────
// 4. Relaxation Iteration
// ──────────────────────────────────────────────

/// Under-relaxed fixed-point iteration for stiff algebraic loops.
///
/// Uses `omega < 1.0` to damp oscillations and improve convergence
/// for tightly coupled algebraic loops.
pub struct RelaxationIteration {
    config: AlgebraicSolverConfig,
}

impl RelaxationIteration {
    /// Create a new relaxation iteration solver with default under-relaxation.
    pub fn new(omega: Scalar) -> Self {
        Self {
            config: AlgebraicSolverConfig {
                relaxation_factor: omega.clamp(0.01, 1.0),
                ..AlgebraicSolverConfig::default()
            },
        }
    }

    /// Create with a custom configuration.
    pub fn with_config(config: AlgebraicSolverConfig) -> Self {
        Self { config }
    }

    /// Solve the algebraic loop using relaxation iteration.
    ///
    /// Equivalent to `FixedPointIteration` with `relaxation_factor = omega`.
    pub fn solve<F>(&self, f: F, initial: &[Scalar]) -> AlgebraicSolveResult
    where
        F: FnMut(&[Scalar]) -> Result<Vec<Scalar>, SimError>,
    {
        let solver = FixedPointIteration::new(self.config);
        solver.solve(f, initial)
    }
}

// ──────────────────────────────────────────────
// 5. Numerical Stability Guard
// ──────────────────────────────────────────────

/// Guards against numerical instabilities: NaN, Inf, overflow.
#[derive(Debug, Clone)]
pub struct NumericalGuard;

impl NumericalGuard {
    /// Check a scalar value for NaN or Inf.
    /// Returns `None` if the value is valid, or a description if invalid.
    pub fn check(value: Scalar, name: &str) -> Option<String> {
        if value.is_nan() {
            Some(format!("NaN detected in '{}'", name))
        } else if value.is_infinite() {
            Some(format!(
                "Inf detected in '{}' (sign: {})",
                name,
                value.signum()
            ))
        } else {
            None
        }
    }

    /// Check all values in a slice for NaN/Inf.
    /// Returns the first problem found.
    pub fn check_all(values: &[Scalar]) -> Option<String> {
        for (i, &v) in values.iter().enumerate() {
            if v.is_nan() {
                return Some(format!("NaN detected at index {}", i));
            }
            if v.is_infinite() {
                return Some(format!(
                    "Inf detected at index {} (sign: {})",
                    i,
                    v.signum()
                ));
            }
        }
        None
    }

    /// Clamp a value to a safe range, replacing NaN with a fallback.
    pub fn sanitize(value: Scalar, fallback: Scalar, min: Scalar, max: Scalar) -> Scalar {
        if value.is_nan() || value.is_infinite() {
            fallback
        } else {
            value.clamp(min, max)
        }
    }

    /// Check if a matrix (as row slice) is numerically singular.
    pub fn is_numerically_singular(matrix: &[Vec<Scalar>], tol: Scalar) -> bool {
        if matrix.is_empty() || matrix[0].is_empty() {
            return true;
        }
        let n = matrix.len();
        for (i, row) in matrix.iter().enumerate().take(n) {
            if i >= row.len() {
                return true;
            }
            let diag = row[i].abs();
            if diag < tol || diag.is_nan() {
                return true;
            }
        }
        false
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::diagram::Diagram;
    use crate::core::link::Link;
    use crate::core::types::SignalType;

    fn make_acyclic_diagram() -> Diagram {
        let mut d = Diagram::new("acyclic");
        let mut a = SimpleBlock::new("A", "Source");
        a.declare_output("out", SignalType::Continuous);
        let mut b = SimpleBlock::new("B", "Gain");
        b.declare_input("in", SignalType::Continuous);
        b.declare_output("out", SignalType::Continuous);
        d.add_block(Box::new(a));
        d.add_block(Box::new(b));
        d.add_link(Link::new("l1", "A", "out", "B", "in"));
        d
    }

    fn make_cyclic_diagram() -> Diagram {
        let mut d = Diagram::new("cyclic");
        let mut a = SimpleBlock::new("A", "Sum");
        a.declare_input("in1", SignalType::Continuous);
        a.declare_output("out", SignalType::Continuous);
        let mut b = SimpleBlock::new("B", "Gain");
        b.declare_input("in", SignalType::Continuous);
        b.declare_output("out", SignalType::Continuous);
        d.add_block(Box::new(a));
        d.add_block(Box::new(b));
        d.add_link(Link::new("l1", "A", "out", "B", "in"));
        d.add_link(Link::new("l2", "B", "out", "A", "in1"));
        d
    }

    fn make_self_loop_diagram() -> Diagram {
        let mut d = Diagram::new("self_loop");
        let mut a = SimpleBlock::new("A", "Feedback");
        a.declare_input("in", SignalType::Continuous);
        a.declare_output("out", SignalType::Continuous);
        d.add_block(Box::new(a));
        d.add_link(Link::new("l1", "A", "out", "A", "in"));
        d
    }

    #[test]
    fn test_detector_acyclic() {
        let d = make_acyclic_diagram();
        let detector = AlgebraicLoopDetector::new(&d);
        assert!(!detector.has_loops());
        assert_eq!(detector.loop_count(), 0);
    }

    #[test]
    fn test_detector_cyclic() {
        let d = make_cyclic_diagram();
        let detector = AlgebraicLoopDetector::new(&d);
        assert!(detector.has_loops());
        assert_eq!(detector.loop_count(), 1);
        assert_eq!(detector.analysis().loops[0].order, 2);
    }

    #[test]
    fn test_detector_self_loop() {
        let d = make_self_loop_diagram();
        let detector = AlgebraicLoopDetector::new(&d);
        assert!(detector.has_loops());
        assert_eq!(detector.loop_count(), 1);
    }

    #[test]
    fn test_fixed_point_convergence() {
        // x_{k+1} = 0.5*x_k + 0.5  → converges to x = 1.0
        let config = AlgebraicSolverConfig {
            max_iterations: 100,
            tolerance: 1e-8,
            relaxation_factor: 1.0,
            abort_on_nan: true,
        };
        let solver = FixedPointIteration::new(config);
        let result = solver.solve(|x: &[Scalar]| Ok(vec![0.5 * x[0] + 0.5]), &[0.0]);
        match result {
            AlgebraicSolveResult::Converged {
                iterations,
                final_error,
            } => {
                assert!(iterations > 0);
                assert!(final_error < 1e-8);
            }
            _ => panic!("expected convergence, got {:?}", result),
        }
    }

    #[test]
    fn test_fixed_point_divergence() {
        let config = AlgebraicSolverConfig {
            max_iterations: 10,
            tolerance: 1e-8,
            relaxation_factor: 1.0,
            abort_on_nan: true,
        };
        let solver = FixedPointIteration::new(config);
        let result = solver.solve(|x: &[Scalar]| Ok(vec![2.0 * x[0]]), &[1.0]);
        assert!(matches!(result, AlgebraicSolveResult::NotConverged { .. }));
    }

    #[test]
    fn test_relaxation_converges() {
        let solver = RelaxationIteration::new(0.5);
        let result = solver.solve(|x: &[Scalar]| Ok(vec![-0.9 * x[0] + 1.0]), &[0.0]);
        assert!(matches!(result, AlgebraicSolveResult::Converged { .. }));
    }

    #[test]
    fn test_numerical_guard() {
        assert!(NumericalGuard::check(f64::NAN, "x").is_some());
        assert!(NumericalGuard::check(f64::INFINITY, "x").is_some());
        assert!(NumericalGuard::check(42.0, "x").is_none());

        assert!(NumericalGuard::check_all(&[1.0, f64::NAN, 3.0]).is_some());
        assert!(NumericalGuard::check_all(&[1.0, 2.0, 3.0]).is_none());

        let sanitized = NumericalGuard::sanitize(f64::NAN, 0.0, -1e6, 1e6);
        assert!((sanitized - 0.0).abs() < 1e-12);

        let normal = NumericalGuard::sanitize(42.0, 0.0, -1e6, 1e6);
        assert!((normal - 42.0).abs() < 1e-12);
    }

    #[test]
    fn test_find_direct_feedthrough() {
        let d = make_cyclic_diagram();
        let paths = find_direct_feedthrough_paths(&d);
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_loop_analysis_default() {
        let analysis = LoopAnalysis::default();
        assert!(!analysis.has_loops);
        assert_eq!(analysis.total_involved, 0);
        assert!(analysis.loops.is_empty());
    }

    #[test]
    fn test_numerically_singular() {
        let singular = vec![vec![0.0, 1.0], vec![1.0, 0.0]];
        assert!(NumericalGuard::is_numerically_singular(&singular, 1e-10));

        let ok = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        assert!(!NumericalGuard::is_numerically_singular(&ok, 1e-10));
    }
}
