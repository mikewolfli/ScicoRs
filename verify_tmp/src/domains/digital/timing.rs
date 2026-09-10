//! Digital timing analysis utilities.
//!
//! Provides setup/hold timing analysis, critical path delay computation,
//! clock jitter modeling, and slack reporting for digital circuits.

use crate::core::types::Scalar;
use std::collections::HashMap;

// ──────────────────────────────────────────────
// 1. Gate Connection
// ──────────────────────────────────────────────

/// A connection between two gates/nodes in a digital netlist.
#[derive(Debug, Clone)]
pub struct GateConnection {
    /// Source gate/node index.
    pub source: usize,
    /// Destination gate/node index.
    pub destination: usize,
    /// Gate type name (for delay lookup).
    pub gate_type: String,
}

// ──────────────────────────────────────────────
// 2. Timing Analyzer
// ──────────────────────────────────────────────

/// Analyzes timing in a digital logic network.
///
/// Computes critical path delays, checks setup/hold timing constraints,
/// and provides slack reports for timing closure.
#[derive(Debug, Clone)]
pub struct TimingAnalyzer {
    /// Gate type → intrinsic propagation delay (s).
    pub gate_delays: HashMap<String, Scalar>,
    /// Wire delays between node pairs (source, dest) → delay (s).
    pub wire_delays: HashMap<(usize, usize), Scalar>,
    /// Clock period (s).
    pub clock_period: Scalar,
    /// Setup time requirement (s).
    pub setup_time: Scalar,
    /// Hold time requirement (s).
    pub hold_time: Scalar,
    /// Clock jitter (s), peak-to-peak.
    pub clock_jitter: Scalar,
}

impl TimingAnalyzer {
    /// Create a new timing analyzer with default delay values.
    pub fn new(clock_period: Scalar) -> Self {
        let mut gate_delays = HashMap::new();
        gate_delays.insert("INV".to_string(), 10e-12);   // 10 ps
        gate_delays.insert("NAND2".to_string(), 15e-12); // 15 ps
        gate_delays.insert("NOR2".to_string(), 15e-12);  // 15 ps
        gate_delays.insert("AND2".to_string(), 20e-12);  // 20 ps
        gate_delays.insert("OR2".to_string(), 20e-12);   // 20 ps
        gate_delays.insert("XOR2".to_string(), 30e-12);  // 30 ps
        gate_delays.insert("DFF".to_string(), 50e-12);   // 50 ps (clk→q)
        gate_delays.insert("BUF".to_string(), 10e-12);   // 10 ps
        gate_delays.insert("MUX2".to_string(), 25e-12);  // 25 ps
        gate_delays.insert("ADD".to_string(), 100e-12);  // 100 ps (approx per bit)

        Self {
            gate_delays,
            wire_delays: HashMap::new(),
            clock_period,
            setup_time: 20e-12,   // 20 ps
            hold_time: 5e-12,     // 5 ps
            clock_jitter: 10e-12, // 10 ps peak-to-peak
        }
    }

    /// Add a custom gate delay.
    pub fn add_gate_delay(&mut self, gate_type: &str, delay: Scalar) {
        self.gate_delays.insert(gate_type.to_string(), delay);
    }

    /// Add a wire delay between two nodes.
    pub fn add_wire_delay(&mut self, source: usize, dest: usize, delay: Scalar) {
        self.wire_delays.insert((source, dest), delay);
    }

    /// Get the delay for a gate type.
    pub fn gate_delay(&self, gate_type: &str) -> Scalar {
        self.gate_delays.get(gate_type).copied().unwrap_or(10e-12)
    }

    /// Get the wire delay between two nodes.
    pub fn wire_delay(&self, source: usize, dest: usize) -> Scalar {
        self.wire_delays.get(&(source, dest)).copied().unwrap_or(5e-12)
    }

    /// Compute the critical path delay through a netlist.
    ///
    /// Uses topological order to compute the longest path delay from
    /// inputs/outputs specified in the netlist.
    ///
    /// # Arguments
    /// * `netlist` - List of gate connections forming the logic network
    /// * `primary_inputs` - Set of node indices that are primary inputs
    /// * `primary_outputs` - Set of node indices that are primary outputs
    pub fn critical_path_delay(
        &self,
        netlist: &[GateConnection],
        primary_inputs: &[usize],
        primary_outputs: &[usize],
    ) -> Scalar {
        // Build adjacency list and compute node delays
        let mut node_delays: HashMap<usize, Scalar> = HashMap::new();
        let mut successors: HashMap<usize, Vec<(usize, Scalar)>> = HashMap::new();
        let mut predecessors: HashMap<usize, Vec<usize>> = HashMap::new();

        // Initialize primary inputs with zero delay
        for &pi in primary_inputs {
            node_delays.entry(pi).or_insert(0.0);
        }

        // Build graph
        for conn in netlist {
            let gate_d = self.gate_delay(&conn.gate_type);
            let wire_d = self.wire_delay(conn.source, conn.destination);

            successors
                .entry(conn.source)
                .or_default()
                .push((conn.destination, gate_d + wire_d));
            predecessors
                .entry(conn.destination)
                .or_default()
                .push(conn.source);

            // Initialize destination delay if not set
            node_delays.entry(conn.destination).or_insert(0.0);
            node_delays.entry(conn.source).or_insert(0.0);
        }

        // Topological sort: Kahn's algorithm
        let mut in_degree: HashMap<usize, usize> = HashMap::new();
        for &node in node_delays.keys() {
            in_degree.insert(node, predecessors.get(&node).map(|p| p.len()).unwrap_or(0));
        }

        let mut queue: Vec<usize> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(&n, _)| n)
            .collect();

        let mut topo_order = Vec::new();
        while let Some(node) = queue.pop() {
            topo_order.push(node);
            if let Some(succs) = successors.get(&node) {
                for &(succ, _) in succs {
                    if let Some(deg) = in_degree.get_mut(&succ) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push(succ);
                        }
                    }
                }
            }
        }

        // Compute arrival times (forward pass)
        let mut arrival: HashMap<usize, Scalar> = HashMap::new();
        for &pi in primary_inputs {
            arrival.insert(pi, 0.0);
        }

        for &node in &topo_order {
            let current_arrival = *arrival.get(&node).unwrap_or(&0.0);
            if let Some(succs) = successors.get(&node) {
                for &(succ, delay) in succs {
                    let new_arrival = current_arrival + delay;
                    let entry = arrival.entry(succ).or_insert(0.0);
                    *entry = (*entry).max(new_arrival);
                }
            }
        }

        // Find maximum arrival time at primary outputs
        let mut max_delay = 0.0_f64;
        for &po in primary_outputs {
            if let Some(&arr) = arrival.get(&po) {
                max_delay = max_delay.max(arr);
            }
        }

        // If no primary outputs specified, take max over all nodes
        if primary_outputs.is_empty() {
            max_delay = arrival.values().copied().fold(0.0, Scalar::max);
        }

        max_delay
    }

    /// Check if setup time is met.
    ///
    /// Setup constraint: T_clk - T_critical_path - T_jitter ≥ T_setup
    pub fn check_setup(&self, path_delay: Scalar) -> bool {
        let available = self.clock_period - path_delay - self.clock_jitter;
        available >= self.setup_time
    }

    /// Check if hold time is met.
    ///
    /// Hold constraint: T_shortest_path - T_jitter ≥ T_hold
    pub fn check_hold(&self, path_delay: Scalar) -> bool {
        path_delay - self.clock_jitter >= self.hold_time
    }

    /// Compute setup slack: positive = timing met, negative = violation.
    pub fn setup_slack(&self, path_delay: Scalar) -> Scalar {
        self.clock_period - path_delay - self.clock_jitter - self.setup_time
    }

    /// Compute hold slack: positive = timing met, negative = violation.
    pub fn hold_slack(&self, path_delay: Scalar) -> Scalar {
        path_delay - self.clock_jitter - self.hold_time
    }

    /// Generate a slack report for multiple timing paths.
    ///
    /// Returns a list of `(path_index, setup_slack, hold_slack)` tuples.
    pub fn slack_report(&self, path_delays: &[Scalar]) -> Vec<(usize, Scalar, Scalar)> {
        path_delays
            .iter()
            .enumerate()
            .map(|(i, &d)| (i, self.setup_slack(d), self.hold_slack(d)))
            .collect()
    }

    /// Report the maximum frequency achievable given the critical path delay.
    pub fn max_frequency(&self, path_delay: Scalar) -> Scalar {
        let min_period = path_delay + self.setup_time + self.clock_jitter;
        if min_period > 0.0 {
            1.0 / min_period
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_analyzer_creation() {
        let ta = TimingAnalyzer::new(1e-9); // 1 ns clock
        assert!((ta.clock_period - 1e-9).abs() < 1e-15);
        assert!(ta.gate_delays.contains_key("INV"));
        assert!(ta.gate_delays.contains_key("NAND2"));
    }

    #[test]
    fn test_critical_path_simple() {
        let ta = TimingAnalyzer::new(1e-9);
        // Simple path: input(0) → INV(1) → output(2)
        let netlist = vec![
            GateConnection { source: 0, destination: 1, gate_type: "INV".to_string() },
            GateConnection { source: 1, destination: 2, gate_type: "BUF".to_string() },
        ];
        let delay = ta.critical_path_delay(&netlist, &[0], &[2]);
        // INV delay (10ps) + wire(5ps) + BUF delay (10ps) + wire(5ps) = 30ps
        assert!((20e-12..=40e-12).contains(&delay));
    }

    #[test]
    fn test_critical_path_parallel() {
        let ta = TimingAnalyzer::new(1e-9);
        // Two parallel paths from input 0:
        // Path A: 0 → INV(1) → BUF(2)
        // Path B: 0 → NAND2(3) → BUF(4)
        let netlist = vec![
            GateConnection { source: 0, destination: 1, gate_type: "INV".to_string() },
            GateConnection { source: 1, destination: 2, gate_type: "BUF".to_string() },
            GateConnection { source: 0, destination: 3, gate_type: "NAND2".to_string() },
            GateConnection { source: 3, destination: 4, gate_type: "BUF".to_string() },
        ];
        let delay_a = ta.critical_path_delay(&netlist, &[0], &[2]);
        let delay_b = ta.critical_path_delay(&netlist, &[0], &[4]);
        // NAND path should be slightly longer
        assert!(delay_b > delay_a * 0.9);
    }

    #[test]
    fn test_setup_check() {
        let ta = TimingAnalyzer::new(1e-9);
        // Fast path: should meet setup
        assert!(ta.check_setup(100e-12));
        // Very slow path: should violate setup
        assert!(!ta.check_setup(2e-9));
    }

    #[test]
    fn test_hold_check() {
        let ta = TimingAnalyzer::new(1e-9);
        // Fast path: might violate hold
        let hold_ok = ta.check_hold(100e-12);
        // Very fast path: likely violates hold
        let hold_violation = ta.check_hold(1e-12);
        assert!(hold_ok);
        assert!(!hold_violation);
    }

    #[test]
    fn test_setup_slack() {
        let ta = TimingAnalyzer::new(1e-9);
        let slack = ta.setup_slack(500e-12);
        // Slack = 1ns - 0.5ns - 10ps - 20ps = 0.47ns
        assert!((slack - 470e-12).abs() < 1e-12);
    }

    #[test]
    fn test_hold_slack() {
        let ta = TimingAnalyzer::new(1e-9);
        let slack = ta.hold_slack(100e-12);
        // Slack = 100ps - 10ps - 5ps = 85ps
        assert!((slack - 85e-12).abs() < 1e-12);
    }

    #[test]
    fn test_max_frequency() {
        let ta = TimingAnalyzer::new(1e-9);
        let fmax = ta.max_frequency(500e-12);
        // Fmax = 1/(500ps + 20ps + 10ps) = 1/530ps ≈ 1.887 GHz
        assert!(fmax > 1.8e9 && fmax < 2.0e9);
    }

    #[test]
    fn test_slack_report() {
        let ta = TimingAnalyzer::new(1e-9);
        let paths = vec![100e-12, 500e-12, 900e-12];
        let report = ta.slack_report(&paths);
        assert_eq!(report.len(), 3);
        // Path 0: should have positive setup slack
        assert!(report[0].1 > 0.0);
        // Path 2: should have negative (or very small) setup slack
        assert!(report[2].1 < report[0].1);
    }

    #[test]
    fn test_gate_delay_lookup() {
        let ta = TimingAnalyzer::new(1e-9);
        assert!((ta.gate_delay("INV") - 10e-12).abs() < 1e-15);
        assert!((ta.gate_delay("UNKNOWN") - 10e-12).abs() < 1e-15); // default
    }

    #[test]
    fn test_add_custom_delay() {
        let mut ta = TimingAnalyzer::new(1e-9);
        ta.add_gate_delay("CUSTOM", 50e-12);
        assert!((ta.gate_delay("CUSTOM") - 50e-12).abs() < 1e-15);
    }

    #[test]
    fn test_wire_delay() {
        let mut ta = TimingAnalyzer::new(1e-9);
        ta.add_wire_delay(0, 1, 20e-12);
        assert!((ta.wire_delay(0, 1) - 20e-12).abs() < 1e-15);
        // Default wire delay
        assert!((ta.wire_delay(99, 100) - 5e-12).abs() < 1e-15);
    }
}
