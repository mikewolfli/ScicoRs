// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Simulation abstraction for the analysis layer.
//!
//! The analysis algorithms are pure functions of a [`SimulationFunction`]:
//! given a parameter vector, produce an output time series. This keeps the
//! numerics testable against analytic problems while still allowing real
//! bounded simulations through [`DiagramFunction`].

use crate::core::types::Scalar;
use std::collections::BTreeMap;

/// The output of one model evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationRecord {
    /// Time (or 1-D sampling coordinate) of each output sample.
    pub positions: Vec<Scalar>,
    /// Named output channels, each parallel to `positions`.
    pub channels: BTreeMap<String, Vec<Scalar>>,
    /// A stable, traceable identifier for this evaluation (for provenance).
    pub run_id: String,
    /// Whether the evaluation produced a valid result. A `false` here means the
    /// output must not be treated as a real measurement.
    pub valid: bool,
    /// Optional diagnostic message (e.g. why an evaluation was invalid).
    pub message: Option<String>,
}

impl SimulationRecord {
    /// A valid record with a single channel.
    pub fn single(
        channel: &str,
        positions: Vec<Scalar>,
        values: Vec<Scalar>,
        run_id: &str,
    ) -> Self {
        let mut channels = BTreeMap::new();
        channels.insert(channel.to_string(), values);
        Self {
            positions,
            channels,
            run_id: run_id.to_string(),
            valid: true,
            message: None,
        }
    }

    /// A failed record that carries why it failed.
    pub fn failed(run_id: &str, message: &str) -> Self {
        Self {
            positions: Vec::new(),
            channels: BTreeMap::new(),
            run_id: run_id.to_string(),
            valid: false,
            message: Some(message.to_string()),
        }
    }

    /// Read a channel by name.
    pub fn channel(&self, name: &str) -> Option<&Vec<Scalar>> {
        self.channels.get(name)
    }

    /// Linearly interpolate a channel at position `t`. Returns `None` when the
    /// channel is absent or `t` is outside the sampled range.
    pub fn sample_at(&self, channel: &str, t: Scalar) -> Option<Scalar> {
        let ys = self.channel(channel)?;
        let xs = &self.positions;
        if xs.len() != ys.len() || xs.is_empty() {
            return None;
        }
        if t < xs[0] || t > *xs.last().unwrap() {
            return None;
        }
        // Binary search for the bracketing interval.
        let idx = xs.partition_point(|&x| x < t);
        if idx == 0 {
            return Some(ys[0]);
        }
        if idx >= xs.len() {
            return Some(*ys.last().unwrap());
        }
        let (x0, x1) = (xs[idx - 1], xs[idx]);
        let (y0, y1) = (ys[idx - 1], ys[idx]);
        if (x1 - x0).abs() < 1e-300 {
            return Some(y0);
        }
        let w = (t - x0) / (x1 - x0);
        Some(y0 * (1.0 - w) + y1 * w)
    }
}

/// A parameterized model the analysis layer can evaluate.
pub trait SimulationFunction {
    /// Evaluate the model at the given parameter vector, returning a labelled
    /// [`SimulationRecord`]. Failures are represented by `valid = false` rather
    /// than panics.
    fn evaluate(&self, params: &[Scalar], run_id: &str) -> SimulationRecord;

    /// Number of parameters this function consumes.
    fn param_count(&self) -> usize;

    /// A human-readable description for provenance metadata.
    fn describe(&self) -> String;
}

/// A closure-backed [`SimulationFunction`] — the simplest adapter for analytic
/// test problems and user code.
pub struct FnSimulationFunction<F>
where
    F: Fn(&[Scalar], &str) -> SimulationRecord,
{
    param_count: usize,
    description: String,
    eval: F,
}

impl<F> FnSimulationFunction<F>
where
    F: Fn(&[Scalar], &str) -> SimulationRecord,
{
    /// Wrap a closure as a simulation function.
    pub fn new(param_count: usize, description: &str, eval: F) -> Self {
        Self {
            param_count,
            description: description.to_string(),
            eval,
        }
    }
}

impl<F> SimulationFunction for FnSimulationFunction<F>
where
    F: Fn(&[Scalar], &str) -> SimulationRecord,
{
    fn evaluate(&self, params: &[Scalar], run_id: &str) -> SimulationRecord {
        (self.eval)(params, run_id)
    }

    fn param_count(&self) -> usize {
        self.param_count
    }

    fn describe(&self) -> String {
        self.description.clone()
    }
}

/// A [`SimulationFunction`] that runs a real bounded diagram simulation.
///
/// This bridges the analysis layer to the existing batch/engine infrastructure:
/// the function holds a diagram JSON template and a time configuration, applies
/// the parameter vector to a fresh copy of the diagram, runs the bounded engine,
/// and returns the recorded channels. Execution failures become
/// [`SimulationRecord::failed`] rather than panics.
#[derive(Debug, Clone)]
pub struct DiagramFunction {
    /// Path to the diagram JSON template.
    pub diagram_path: String,
    /// Parameter paths, in model order, mapped to block parameters.
    pub parameter_paths: Vec<String>,
    /// Name of the output channel to extract (must match a recorded signal).
    pub output_channel: String,
    /// Time configuration for the bounded run.
    pub time_config: crate::runtime::context::TimeConfig,
}

impl DiagramFunction {
    /// Create a diagram-backed simulation function.
    pub fn new(
        diagram_path: &str,
        parameter_paths: Vec<String>,
        output_channel: &str,
        time_config: crate::runtime::context::TimeConfig,
    ) -> Self {
        Self {
            diagram_path: diagram_path.to_string(),
            parameter_paths,
            output_channel: output_channel.to_string(),
            time_config,
        }
    }
}

impl SimulationFunction for DiagramFunction {
    fn evaluate(&self, params: &[Scalar], run_id: &str) -> SimulationRecord {
        use crate::postproc::batch::run_diagram_bounded_public;

        if params.len() != self.parameter_paths.len() {
            return SimulationRecord::failed(
                run_id,
                &format!(
                    "expected {} parameters, got {}",
                    self.parameter_paths.len(),
                    params.len()
                ),
            );
        }
        // Load a fresh diagram and apply the parameter vector. A diagram that
        // cannot be loaded/produced is a failed evaluation, not a panic.
        let result = run_diagram_bounded_public(
            &self.diagram_path,
            &self.parameter_paths,
            params,
            self.time_config,
            &self.output_channel,
        );
        match result {
            Ok((positions, values)) => {
                SimulationRecord::single(&self.output_channel, positions, values, run_id)
            }
            Err(msg) => SimulationRecord::failed(run_id, &msg),
        }
    }

    fn param_count(&self) -> usize {
        self.parameter_paths.len()
    }

    fn describe(&self) -> String {
        format!(
            "DiagramFunction({}, channel={})",
            self.diagram_path, self.output_channel
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_sampling_interpolates() {
        let rec = SimulationRecord::single("y", vec![0.0, 1.0, 2.0], vec![0.0, 10.0, 20.0], "r0");
        assert_eq!(rec.sample_at("y", 0.0), Some(0.0));
        assert_eq!(rec.sample_at("y", 1.5), Some(15.0));
        assert_eq!(rec.sample_at("y", 3.0), None);
        assert_eq!(rec.sample_at("missing", 1.0), None);
    }

    #[test]
    fn closure_function_adapter() {
        let f = FnSimulationFunction::new(1, "linear", |p, id| {
            SimulationRecord::single("y", vec![0.0, 1.0], vec![p[0], 2.0 * p[0]], id)
        });
        let rec = f.evaluate(&[3.0], "run1");
        assert!(rec.valid);
        assert_eq!(rec.channel("y").unwrap(), &vec![3.0, 6.0]);
        assert_eq!(f.param_count(), 1);
    }

    #[test]
    fn failed_record_has_no_channels() {
        let r = SimulationRecord::failed("r", "boom");
        assert!(!r.valid);
        assert!(r.channel("y").is_none());
        assert_eq!(r.message.as_deref(), Some("boom"));
    }
}
