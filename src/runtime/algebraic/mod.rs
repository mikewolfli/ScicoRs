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

/// Tarjan's SCC algorithm for directed graphs, implemented iteratively.
///
/// The classic formulation is recursive, but an explicit work stack is used
/// here: a diagram can legitimately contain a long chain of blocks (imported or
/// generated models reach thousands of nodes), and a recursive `strongconnect`
/// would overflow the thread stack around that size. The iterative form is
/// algorithmically identical — same index/lowlink bookkeeping, same SCC emission
/// order — so results are unchanged for every input the recursive version
/// handled.
///
/// Each frame records the node and how many of its neighbours have already been
/// visited, which is what replaces the recursion's program counter.
fn tarjan_scc(graph: &HashMap<BlockId, Vec<BlockId>>) -> Vec<Vec<BlockId>> {
    let mut index_counter = 0usize;
    let mut stack: Vec<BlockId> = Vec::new();
    let mut on_stack: HashSet<BlockId> = HashSet::new();
    let mut indices: HashMap<BlockId, usize> = HashMap::new();
    let mut lowlinks: HashMap<BlockId, usize> = HashMap::new();
    let mut sccs: Vec<Vec<BlockId>> = Vec::new();

    // A node's neighbours, defaulting to empty for a leaf.
    let neighbors_of =
        |v: &BlockId| -> &[BlockId] { graph.get(v).map(|n| n.as_slice()).unwrap_or(&[]) };

    /// One suspended call to the recursive formulation.
    struct Frame {
        node: BlockId,
        child_index: usize,
    }

    let all_nodes: Vec<BlockId> = graph.keys().cloned().collect();
    for root in &all_nodes {
        if indices.contains_key(root) {
            continue;
        }

        // `work` mirrors the call stack; the last frame is the active call.
        let mut work: Vec<Frame> = vec![Frame {
            node: root.clone(),
            child_index: 0,
        }];
        indices.insert(root.clone(), index_counter);
        lowlinks.insert(root.clone(), index_counter);
        index_counter += 1;
        stack.push(root.clone());
        on_stack.insert(root.clone());

        while let Some(frame) = work.last_mut() {
            let v = frame.node.clone();
            let neighbors = neighbors_of(&v);

            if frame.child_index < neighbors.len() {
                let w = neighbors[frame.child_index].clone();
                frame.child_index += 1;

                if !indices.contains_key(&w) {
                    // "Recurse" into `w`.
                    indices.insert(w.clone(), index_counter);
                    lowlinks.insert(w.clone(), index_counter);
                    index_counter += 1;
                    stack.push(w.clone());
                    on_stack.insert(w.clone());
                    work.push(Frame {
                        node: w,
                        child_index: 0,
                    });
                } else if on_stack.contains(&w) {
                    let v_low = lowlinks[&v];
                    let w_idx = indices[&w];
                    lowlinks.insert(v.clone(), v_low.min(w_idx));
                }
                continue;
            }

            // All neighbours visited: this call is returning.
            if lowlinks.get(&v) == indices.get(&v) {
                let mut component: Vec<BlockId> = Vec::new();
                // Pop until this node, which closes its SCC.
                while let Some(w) = stack.pop() {
                    on_stack.remove(&w);
                    let done = w == v;
                    component.push(w);
                    if done {
                        break;
                    }
                }
                if !component.is_empty() {
                    component.sort();
                    sccs.push(component);
                }
            }

            work.pop();
            // Propagate this node's lowlink into its parent, exactly as the
            // `lowlinks[parent] = min(lowlinks[parent], lowlinks[child])`
            // line after the recursive call does.
            if let Some(parent) = work.last() {
                let p = parent.node.clone();
                let p_low = lowlinks[&p];
                let v_low = lowlinks[&v];
                lowlinks.insert(p, p_low.min(v_low));
            }
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

    /// The iterative Tarjan must agree with a straightforward reference
    /// implementation on the same graph, so the rewrite did not change results.
    #[test]
    fn test_tarjan_scc_matches_reference_on_varied_graphs() {
        // Reference: repeated forward reachability closure, which is O(V*E) but
        // obviously correct and independent of the algorithm under test.
        fn reference_sccs(graph: &HashMap<BlockId, Vec<BlockId>>) -> Vec<Vec<BlockId>> {
            let nodes: Vec<BlockId> = graph.keys().cloned().collect();
            let reach = |start: &BlockId| -> HashSet<BlockId> {
                let mut seen = HashSet::new();
                let mut stack = vec![start.clone()];
                while let Some(n) = stack.pop() {
                    if !seen.insert(n.clone()) {
                        continue;
                    }
                    if let Some(ns) = graph.get(&n) {
                        stack.extend(ns.iter().cloned());
                    }
                }
                seen
            };
            let mut out: Vec<Vec<BlockId>> = Vec::new();
            let mut assigned: HashSet<BlockId> = HashSet::new();
            for n in &nodes {
                if assigned.contains(n) {
                    continue;
                }
                let from_n = reach(n);
                // The component of `n` is every node mutually reachable with it.
                let mut comp: Vec<BlockId> = from_n
                    .iter()
                    .filter(|m| reach(m).contains(n))
                    .cloned()
                    .collect();
                for c in &comp {
                    assigned.insert(c.clone());
                }
                comp.sort();
                if !comp.is_empty() {
                    out.push(comp);
                }
            }
            out.sort();
            out
        }

        // Build a graph from an edge list; `BlockId` is a String.
        let build = |edges: &[(&str, &str)]| -> HashMap<BlockId, Vec<BlockId>> {
            let mut g: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
            for (a, b) in edges {
                g.entry(a.to_string()).or_default().push(b.to_string());
                g.entry(b.to_string()).or_default();
            }
            g
        };

        let cases: Vec<Vec<(&str, &str)>> = vec![
            vec![],                                                           // empty
            vec![("a", "b")],                         // single edge, no cycle
            vec![("a", "b"), ("b", "a")],             // 2-cycle
            vec![("a", "b"), ("b", "c"), ("c", "a")], // 3-cycle
            vec![("a", "a")],                         // self loop
            vec![("a", "b"), ("b", "c")],             // chain
            vec![("a", "b"), ("c", "d")],             // two components
            vec![("a", "b"), ("b", "a"), ("c", "d"), ("d", "c"), ("b", "c")], // joined cycles
            vec![("a", "b"), ("b", "c"), ("c", "b"), ("c", "d")], // cycle in the middle
        ];

        for (i, edges) in cases.iter().enumerate() {
            let g = build(edges);
            let mut got = tarjan_scc(&g);
            got.sort();
            let want = reference_sccs(&g);
            assert_eq!(got, want, "SCC mismatch for case {i}: edges={edges:?}");
        }
    }

    /// A long chain used to risk a stack overflow because the traversal was
    /// recursive. It must now complete for a deeply nested diagram.
    #[test]
    fn test_tarjan_handles_a_deep_chain_without_overflow() {
        // 50_000 nodes in a single path; a recursive implementation would blow
        // the default 8 MiB thread stack well before this.
        let depth = 50_000usize;
        let mut graph: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        for i in 0..depth {
            let from = format!("n{}", i);
            let to = format!("n{}", i + 1);
            graph.entry(from).or_default().push(to.clone());
            graph.entry(to).or_default();
        }

        let sccs = tarjan_scc(&graph);
        // A path has no cycles, so every node is its own singleton component.
        assert_eq!(
            sccs.len(),
            depth + 1,
            "a chain of {depth} edges must yield {} singleton SCCs",
            depth + 1
        );
        assert!(sccs.iter().all(|c| c.len() == 1));
    }

    /// The same depth but closed into one giant cycle: exactly one SCC.
    #[test]
    fn test_tarjan_handles_a_deep_cycle() {
        let depth = 20_000usize;
        let mut graph: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        for i in 0..depth {
            let from = format!("n{}", i);
            let to = format!("n{}", (i + 1) % depth);
            graph.entry(from).or_default().push(to);
        }
        let sccs = tarjan_scc(&graph);
        assert_eq!(sccs.len(), 1, "a single cycle is one SCC");
        assert_eq!(sccs[0].len(), depth);
    }
}
