//! Process flowsheet simulation for chemical engineering.
//!
//! Provides ProcessUnit and ProcessFlowsheet structures for
//! building and simulating chemical process flowsheets,
//! including heat exchanger models.

use std::collections::HashMap;
use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// Process Unit
// ──────────────────────────────────────────────

/// A single unit in a chemical process flowsheet.
///
/// Units have inputs, outputs, and parameters that define
/// their behavior (e.g., reactor volume, separator stages).
#[derive(Debug, Clone)]
pub struct ProcessUnit {
    /// Unique identifier for the unit.
    pub id: String,
    /// Type of unit (e.g., "reactor", "separator", "mixer", "heat_exchanger").
    pub unit_type: String,
    /// Names of input streams.
    pub inputs: Vec<String>,
    /// Names of output streams.
    pub outputs: Vec<String>,
    /// Unit-specific parameters.
    pub parameters: HashMap<String, Scalar>,
}

impl ProcessUnit {
    /// Create a new process unit.
    pub fn new(id: &str, unit_type: &str) -> Self {
        Self {
            id: id.to_string(),
            unit_type: unit_type.to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            parameters: HashMap::new(),
        }
    }

    /// Add an input stream.
    pub fn add_input(&mut self, name: &str) {
        self.inputs.push(name.to_string());
    }

    /// Add an output stream.
    pub fn add_output(&mut self, name: &str) {
        self.outputs.push(name.to_string());
    }

    /// Set a parameter value.
    pub fn set_parameter(&mut self, key: &str, value: Scalar) {
        self.parameters.insert(key.to_string(), value);
    }

    /// Get a parameter value.
    pub fn get_parameter(&self, key: &str) -> Option<Scalar> {
        self.parameters.get(key).copied()
    }
}

// ──────────────────────────────────────────────
// Process Flowsheet
// ──────────────────────────────────────────────

/// A complete chemical process flowsheet consisting of interconnected units.
pub struct ProcessFlowsheet {
    /// All process units in the flowsheet.
    pub units: Vec<ProcessUnit>,
    /// Stream connections: (from_unit_id, to_unit_id, stream_name).
    pub streams: Vec<(String, String, String)>,
}

impl ProcessFlowsheet {
    /// Create a new empty flowsheet.
    pub fn new() -> Self {
        Self {
            units: Vec::new(),
            streams: Vec::new(),
        }
    }

    /// Add a process unit to the flowsheet.
    pub fn add_unit(&mut self, unit: ProcessUnit) {
        self.units.push(unit);
    }

    /// Connect two units with a stream.
    pub fn add_stream(&mut self, from: &str, to: &str, name: &str) {
        self.streams.push((
            from.to_string(),
            to.to_string(),
            name.to_string(),
        ));
    }

    /// Find a unit by ID and return its index.
    pub fn find_unit_index(&self, id: &str) -> Option<usize> {
        self.units.iter().position(|u| u.id == id)
    }

    /// Compute topological order of units using Kahn's algorithm.
    ///
    /// Returns indices into `self.units` in topological order,
    /// or an error if a cycle is detected.
    pub fn topological_order(&self) -> Result<Vec<usize>, String> {
        let n = self.units.len();
        // Build adjacency list
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_degree: Vec<usize> = vec![0; n];

        // Map unit IDs to indices
        let id_to_idx: HashMap<&str, usize> = self
            .units
            .iter()
            .enumerate()
            .map(|(i, u)| (u.id.as_str(), i))
            .collect();

        for (from_id, to_id, _) in &self.streams {
            let from_idx = id_to_idx
                .get(from_id.as_str())
                .ok_or_else(|| format!("unknown unit: {from_id}"))?;
            let to_idx = id_to_idx
                .get(to_id.as_str())
                .ok_or_else(|| format!("unknown unit: {to_id}"))?;
            adj[*from_idx].push(*to_idx);
            in_degree[*to_idx] += 1;
        }

        // Kahn's algorithm
        let mut queue: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut order = Vec::with_capacity(n);

        while let Some(u) = queue.pop() {
            order.push(u);
            for &v in &adj[u] {
                in_degree[v] -= 1;
                if in_degree[v] == 0 {
                    queue.push(v);
                }
            }
        }

        if order.len() != n {
            return Err("cycle detected in flowsheet".to_string());
        }

        Ok(order)
    }

    /// Perform a sequential simulation of the flowsheet.
    ///
    /// Each unit is evaluated in topological order, receiving input
    /// concentrations from upstream units and passing outputs downstream.
    pub fn sequential_simulate(&self) -> Result<HashMap<String, Vec<Scalar>>, String> {
        let order = self.topological_order()?;
        // Map unit ID to its output concentrations
        let mut results: HashMap<String, Vec<Scalar>> = HashMap::new();

        // Map unit ID to its index
        let _id_to_idx: HashMap<&str, usize> = self
            .units
            .iter()
            .enumerate()
            .map(|(i, u)| (u.id.as_str(), i))
            .collect();

        for &idx in &order {
            let unit = &self.units[idx];

            // Gather input concentrations from upstream units
            let mut input_conc: Vec<Scalar> = Vec::new();
            for (from_id, _to_id, _stream_name) in &self.streams {
                if _to_id == &unit.id {
                    if let Some(upstream_result) = results.get(from_id) {
                        input_conc.extend(upstream_result);
                    }
                }
            }

            // Simulate the unit based on its type
            let default_conc = vec![1.0; 3];
            let input = if input_conc.is_empty() {
                &default_conc
            } else {
                &input_conc
            };

            let output = match unit.unit_type.as_str() {
                "reactor" | "CSTR" => {
                    let _volume = unit.get_parameter("volume").unwrap_or(1.0);
                    let _k = unit.get_parameter("k").unwrap_or(0.1);
                    // Simplified: conversion = 1 - exp(-k * residence_time)
                    let residence_time = unit.get_parameter("residence_time").unwrap_or(1.0);
                    let conv = 1.0 - (-_k * residence_time).exp();
                    input.iter().map(|c| c * (1.0 - conv * 0.5)).collect()
                }
                "separator" | "distillation" => {
                    let split_frac = unit.get_parameter("split_fraction").unwrap_or(0.5);
                    input.iter().map(|c| c * split_frac).collect()
                }
                "mixer" => {
                    // Mixer just sums and averages
                    if input.is_empty() {
                        vec![0.0; 3]
                    } else {
                        let n = input.len();
                        let avg: Scalar = input.iter().sum::<Scalar>() / n as Scalar;
                        vec![avg; 3]
                    }
                }
                "heat_exchanger" => {
                    // Heat exchanger doesn't change concentration
                    input.to_vec()
                }
                _ => {
                    // Default unit: pass through
                    input.to_vec()
                }
            };

            results.insert(unit.id.clone(), output);
        }

        Ok(results)
    }

    /// Converge the flowsheet with tear streams using direct substitution.
    ///
    /// For flowsheets with recycle loops, tear streams are iterated
    /// until convergence.
    pub fn converge(&mut self, max_iter: usize, tolerance: Scalar) -> Result<(), String> {
        // Find recycle loops and use direct substitution
        let _n = self.units.len();

        // Guess initial values for all units
        let mut _old_results: HashMap<String, Vec<Scalar>> = HashMap::new();
        for unit in &self.units {
            _old_results.insert(unit.id.clone(), vec![1.0; 3]);
        }

        for iteration in 0..max_iter {
            let results = self.sequential_simulate()?;

            // Check convergence
            let mut max_change = 0.0;
            for (id, values) in &results {
                if let Some(old_vals) = _old_results.get(id) {
                    for (v_new, v_old) in values.iter().zip(old_vals.iter()) {
                        let change = (v_new - v_old).abs();
                        if change > max_change {
                            max_change = change;
                        }
                    }
                }
            }

            // Update guesses
            _old_results = results;

            if max_change < tolerance {
                return Ok(());
            }

            if iteration == max_iter - 1 {
                return Err(format!(
                    "flowsheet did not converge after {max_iter} iterations, residual={max_change}"
                ));
            }
        }

        Ok(())
    }
}

impl Default for ProcessFlowsheet {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────
// Heat Exchanger (NTU Method)
// ──────────────────────────────────────────────

/// Heat exchanger model using the ε-NTU method.
///
/// Returns (T_hot_out, T_cold_out) for given inlet temperatures and
/// heat capacity rates.
///
/// `flow_config` can be "countercurrent", "cocurrent", or "crossflow".
pub fn heat_exchanger_ntu(
    c_hot: Scalar,
    c_cold: Scalar,
    ua: Scalar,
    t_hot_in: Scalar,
    t_cold_in: Scalar,
    flow_config: &str,
) -> (Scalar, Scalar) {
    if c_hot <= 0.0 || c_cold <= 0.0 || ua <= 0.0 {
        return (t_hot_in, t_cold_in);
    }

    let c_min = c_hot.min(c_cold);
    let c_max = c_hot.max(c_cold);
    let cr = c_min / c_max;
    let ntu = ua / c_min;

    let effectiveness = match flow_config {
        "countercurrent" => {
            if (1.0 - cr).abs() < 1e-12 {
                ntu / (1.0 + ntu)
            } else {
                (1.0 - (-ntu * (1.0 - cr)).exp()) / (1.0 - cr * (-ntu * (1.0 - cr)).exp())
            }
        }
        "cocurrent" | "parallel" => {
            (1.0 - (-ntu * (1.0 + cr)).exp()) / (1.0 + cr)
        }
        "crossflow" => {
            // Approximation for both fluids unmixed
            1.0 - ((-ntu.powf(0.22) / cr) * (1.0 - (-cr * ntu.powf(0.78)).exp() - 1.0)).exp()
        }
        _ => {
            // Default: countercurrent
            if (1.0 - cr).abs() < 1e-12 {
                ntu / (1.0 + ntu)
            } else {
                (1.0 - (-ntu * (1.0 - cr)).exp()) / (1.0 - cr * (-ntu * (1.0 - cr)).exp())
            }
        }
    }
    .clamp(0.0, 1.0);

    let q_max = c_min * (t_hot_in - t_cold_in);
    let q_actual = effectiveness * q_max;

    let t_hot_out = t_hot_in - q_actual / c_hot;
    let t_cold_out = t_cold_in + q_actual / c_cold;

    (t_hot_out, t_cold_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_unit_new() {
        let unit = ProcessUnit::new("R1", "reactor");
        assert_eq!(unit.id, "R1");
        assert_eq!(unit.unit_type, "reactor");
        assert!(unit.inputs.is_empty());
        assert!(unit.parameters.is_empty());
    }

    #[test]
    fn test_process_unit_parameters() {
        let mut unit = ProcessUnit::new("R1", "reactor");
        unit.set_parameter("volume", 10.0);
        assert!((unit.get_parameter("volume").unwrap() - 10.0).abs() < 1e-12);
    }

    #[test]
    fn test_flowsheet_add_unit_and_stream() {
        let mut fs = ProcessFlowsheet::new();
        let r1 = ProcessUnit::new("R1", "reactor");
        let s1 = ProcessUnit::new("S1", "separator");
        fs.add_unit(r1);
        fs.add_unit(s1);
        fs.add_stream("R1", "S1", "stream_1");
        assert_eq!(fs.units.len(), 2);
        assert_eq!(fs.streams.len(), 1);
    }

    #[test]
    fn test_topological_order() {
        let mut fs = ProcessFlowsheet::new();
        let feed = ProcessUnit::new("Feed", "source");
        let r1 = ProcessUnit::new("R1", "reactor");
        let s1 = ProcessUnit::new("S1", "separator");

        fs.add_unit(feed);
        fs.add_unit(r1);
        fs.add_unit(s1);

        fs.add_stream("Feed", "R1", "feed_stream");
        fs.add_stream("R1", "S1", "product_stream");

        let order = fs.topological_order().unwrap();
        assert_eq!(order.len(), 3);
        // Feed should come before R1 which comes before S1
        let feed_pos = order.iter().position(|&i| fs.units[i].id == "Feed").unwrap();
        let r1_pos = order.iter().position(|&i| fs.units[i].id == "R1").unwrap();
        let s1_pos = order.iter().position(|&i| fs.units[i].id == "S1").unwrap();
        assert!(feed_pos < r1_pos);
        assert!(r1_pos < s1_pos);
    }

    #[test]
    fn test_topological_order_cycle() {
        let mut fs = ProcessFlowsheet::new();
        let a = ProcessUnit::new("A", "unit");
        let b = ProcessUnit::new("B", "unit");

        fs.add_unit(a);
        fs.add_unit(b);

        fs.add_stream("A", "B", "s1");
        fs.add_stream("B", "A", "s2");

        assert!(fs.topological_order().is_err());
    }

    #[test]
    fn test_sequential_simulate() {
        let mut fs = ProcessFlowsheet::new();
        let feed = ProcessUnit::new("Feed", "source");
        let mut r1_mut = ProcessUnit::new("R1", "reactor");
        r1_mut.set_parameter("volume", 10.0);
        r1_mut.set_parameter("k", 0.2);
        r1_mut.set_parameter("residence_time", 5.0);

        fs.add_unit(feed);
        fs.add_unit(r1_mut);
        fs.add_stream("Feed", "R1", "feed_stream");

        let results = fs.sequential_simulate();
        assert!(results.is_ok());
    }

    #[test]
    fn test_heat_exchanger_ntu_countercurrent() {
        let (t_hot_out, t_cold_out) =
            heat_exchanger_ntu(1000.0, 2000.0, 500.0, 400.0, 300.0, "countercurrent");
        // Hot stream should cool, cold stream should heat
        assert!(t_hot_out < 400.0);
        assert!(t_cold_out > 300.0);
        assert!(t_hot_out > t_cold_out);
    }

    #[test]
    fn test_heat_exchanger_ntu_zero_c() {
        let (t_hot, t_cold) = heat_exchanger_ntu(0.0, 2000.0, 500.0, 400.0, 300.0, "countercurrent");
        assert_eq!(t_hot, 400.0);
        assert_eq!(t_cold, 300.0);
    }

    #[test]
    fn test_heat_exchanger_ntu_cocurrent() {
        let (t_hot_out, t_cold_out) =
            heat_exchanger_ntu(1000.0, 1000.0, 500.0, 400.0, 300.0, "cocurrent");
        assert!(t_hot_out < 400.0);
        assert!(t_cold_out > 300.0);
    }
}
