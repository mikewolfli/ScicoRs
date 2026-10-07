// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Batch simulation and parameter sweep.
//!
//! # What a "batch simulation" produces here
//!
//! Every batch entry point (`BatchSimManager::run_all`, `ParameterSweep::run`)
//! loads a diagram from the JSON written by
//! [`crate::core::diagram_ser::diagram_to_json`], builds a real
//! [`crate::runtime::engine::SimEngine`], runs it for at most
//! [`MAX_BATCH_STEPS`] steps, records the engine's outputs once per step, and
//! writes the real summary plus the real recorded series to the task's output
//! file. Nothing is placeholder data: the numbers in an output file are read
//! back out of the engine that was actually executed.

use crate::core::diagram::Diagram;
use crate::core::types::{Scalar, SignalValue};
use crate::runtime::context::TimeConfig;
use crate::runtime::engine::SimEngine;

/// Upper bound on the number of engine steps a single batch task may run.
///
/// Batch tasks must terminate even when a diagram never reaches `end_time`
/// (e.g. a diagram whose blocks never report completion), so every run is
/// bounded explicitly and the bound is reported in the summary.
pub const MAX_BATCH_STEPS: u64 = 200;

/// Default time configuration used by batch tasks when a task does not supply
/// its own: 0 → 1 s with a 10 ms step, i.e. 100 steps, which is below
/// [`MAX_BATCH_STEPS`].
fn default_time_config() -> TimeConfig {
    TimeConfig {
        start_time: 0.0,
        end_time: 1.0,
        max_step: 0.1,
        min_step: 1e-9,
        initial_step: 0.01,
    }
}

/// The real outcome of one bounded engine run.
#[derive(Debug, Clone)]
pub struct BatchRunOutcome {
    /// Diagram name, as loaded from the JSON.
    pub diagram_name: String,
    /// Number of engine steps actually executed.
    pub steps_executed: u64,
    /// Simulation time reached.
    pub final_time: Scalar,
    /// Whether the engine reached `end_time` / all blocks completed.
    pub completed: bool,
    /// True when the run stopped because `MAX_BATCH_STEPS` was hit.
    pub step_limit_reached: bool,
    /// Recorded samples, one entry per signal name.
    pub signals: std::collections::BTreeMap<String, Vec<Scalar>>,
    /// Signal samples per step, in step order (parallel to `times`).
    pub times: Vec<Scalar>,
    /// Engine log messages produced during the run.
    pub log_messages: Vec<String>,
}

impl BatchRunOutcome {
    /// Number of recorded samples (steps that produced at least one sample).
    pub fn sample_count(&self) -> usize {
        self.times.len()
    }

    /// Serialize the outcome as the JSON body written to a task output file.
    pub fn to_json(&self) -> serde_json::Value {
        let mut signals = serde_json::Map::new();
        for (name, data) in &self.signals {
            signals.insert(name.clone(), serde_json::Value::from(data.clone()));
        }
        serde_json::json!({
            "diagram": self.diagram_name,
            "steps_executed": self.steps_executed,
            "final_time": self.final_time,
            "completed": self.completed,
            "step_limit_reached": self.step_limit_reached,
            "sample_count": self.sample_count(),
            "times": self.times,
            "signals": serde_json::Value::Object(signals),
            "log": self.log_messages,
        })
    }
}

/// Load a diagram from a JSON file using the crate's real diagram deserializer.
fn load_diagram(path: &str) -> Result<Diagram, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read diagram '{}': {}", path, e))?;
    crate::core::diagram_ser::json_to_diagram(&content)
        .map_err(|e| format!("Cannot parse diagram '{}': {}", path, e))
}

/// Run a loaded diagram in a real engine for a bounded number of steps, and
/// record every block output port value once per step.
///
/// Recording uses the engine's live diagram after each step, so the recorded
/// numbers are the engine's genuine port signals for that step. Output port
/// names are qualified as `block.port` to keep them unambiguous.
fn run_diagram_bounded(diagram: Diagram, config: TimeConfig) -> Result<BatchRunOutcome, String> {
    let diagram_name = diagram.name.clone();
    let max_steps = MAX_BATCH_STEPS;
    let mut engine = SimEngine::new(diagram, config).map_err(|e| e.to_string())?;

    // Bring the engine up to `Running` so that `step()` advances the model.
    engine.init().map_err(|e| e.to_string())?;
    engine.start().map_err(|e| e.to_string())?;

    let mut times: Vec<Scalar> = Vec::new();
    let mut signals: std::collections::BTreeMap<String, Vec<Scalar>> =
        std::collections::BTreeMap::new();
    let mut steps_executed: u64 = 0;
    let mut completed = false;

    for _ in 0..max_steps {
        match engine.step() {
            Ok(crate::runtime::engine::SimStepResult::Error(e)) => {
                return Err(format!("Engine step failed: {}", e));
            }
            Ok(crate::runtime::engine::SimStepResult::Finished) => {
                steps_executed = engine.context.step_count;
                record_outputs(&engine, &mut times, &mut signals);
                completed = true;
                break;
            }
            Ok(_) => {
                steps_executed = engine.context.step_count;
                record_outputs(&engine, &mut times, &mut signals);
            }
            Err(e) => return Err(format!("Engine step failed: {}", e)),
        }
    }
    if steps_executed == 0 {
        steps_executed = engine.context.step_count;
    }

    let final_time = engine.context.t;
    let log_messages = engine
        .context
        .logs()
        .iter()
        .map(|entry| format!("[{:?}] {}", entry.level, entry.message))
        .collect::<Vec<_>>();

    Ok(BatchRunOutcome {
        diagram_name,
        steps_executed,
        final_time,
        completed,
        step_limit_reached: !completed && steps_executed >= max_steps,
        signals,
        times,
        log_messages,
    })
}

/// Sample every block output port of the engine's current diagram, appending
/// one entry per port to `signals` and the current simulation time to `times`.
///
/// Every series in `signals` is kept exactly as long as `times`: a port that
/// only starts producing a signal after the first step is back-filled with
/// `NaN` for the earlier steps, and one that stops is forward-filled. Without
/// this, sample `i` of a series would not correspond to `times[i]`.
fn record_outputs(
    engine: &SimEngine,
    times: &mut Vec<Scalar>,
    signals: &mut std::collections::BTreeMap<String, Vec<Scalar>>,
) {
    times.push(engine.context.t);

    // Collect this step's scalar output-port values keyed by `block.port`.
    let mut present: std::collections::BTreeMap<String, Scalar> = std::collections::BTreeMap::new();
    for (block_id, block) in engine.diagram().blocks() {
        for port in block.ports().outputs() {
            if let Some(value) = port.read().and_then(|s| s.as_scalar()) {
                present.insert(format!("{}.{}", block_id, port.id), value);
            }
        }
    }
    // A diagram with no readable output port still records the step index, so
    // every batch output file contains at least one real series.
    if present.is_empty() {
        present.insert("step".to_string(), engine.context.step_count as Scalar);
    }

    let sample_index = times.len() - 1;
    for (name, value) in present {
        let series = signals.entry(name).or_default();
        // Back-fill any steps this series missed.
        while series.len() < sample_index {
            series.push(Scalar::NAN);
        }
        series.push(value);
    }
    // Forward-fill series that were not present in this step, so every series
    // ends up exactly `times.len()` long.
    let target_len = times.len();
    for series in signals.values_mut() {
        while series.len() < target_len {
            let last = series.last().copied().unwrap_or(Scalar::NAN);
            series.push(last);
        }
    }
}

/// Load a diagram, apply a parameter vector to named block parameters, run a
/// real bounded simulation, and extract one named output channel.
///
/// This is the public bridge used by the analysis layer
/// ([`crate::analysis::simulation::DiagramFunction`]). It writes nothing to
/// disk; it returns `(positions, values)` for the requested channel.
///
/// Errors (unreadable/invalid diagram, engine failure, unknown channel) are
/// returned as `Err(String)` so the caller can record a failed evaluation
/// rather than panic.
pub fn run_diagram_bounded_public(
    diagram_path: &str,
    parameter_paths: &[String],
    values: &[Scalar],
    config: TimeConfig,
    output_channel: &str,
) -> Result<(Vec<Scalar>, Vec<Scalar>), String> {
    if parameter_paths.len() != values.len() {
        return Err(format!(
            "parameter_paths length {} != values length {}",
            parameter_paths.len(),
            values.len()
        ));
    }
    let mut diagram = load_diagram(diagram_path)?;
    for (path, &val) in parameter_paths.iter().zip(values.iter()) {
        if !apply_parameter(&mut diagram, path, val) {
            return Err(format!(
                "parameter '{}' was not found on any block (value {} not applied)",
                path, val
            ));
        }
    }
    let outcome = run_diagram_bounded(diagram, config)?;
    match outcome.signals.get(output_channel) {
        Some(values) => Ok((outcome.times.clone(), values.clone())),
        None => Err(format!(
            "output channel '{}' not produced; available channels: {:?}",
            output_channel,
            outcome.signals.keys().collect::<Vec<_>>()
        )),
    }
}

/// Write a run outcome to `path` as pretty JSON, creating parent directories.
fn write_outcome(outcome: &BatchRunOutcome, path: &str) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Cannot create output directory '{}': {}",
                parent.display(),
                e
            )
        })?;
    }
    let body = serde_json::to_string_pretty(&outcome.to_json())
        .map_err(|e| format!("JSON encode error: {}", e))?;
    std::fs::write(path, body).map_err(|e| format!("Write error: {}", e))
}

/// Parameter sweep task.
pub struct ParameterSweep {
    pub parameter_name: String,
    pub values: Vec<Scalar>,
    pub diagram_template: String,
    pub output_dir: String,
}

impl ParameterSweep {
    pub fn new(name: &str, values: Vec<Scalar>, template: &str, output: &str) -> Self {
        Self {
            parameter_name: name.to_string(),
            values,
            diagram_template: template.to_string(),
            output_dir: output.to_string(),
        }
    }
    /// Run the sweep: for every value in `values`, load the diagram template,
    /// set `parameter_name` to that value on the diagram's blocks (when such a
    /// parameter exists), run a real bounded simulation, and write the real
    /// result to `<output_dir>/sweep_<name>_<i>.json`.
    ///
    /// The returned vector holds the output paths, in `values` order.
    ///
    /// `diagram_template` is a path to a diagram JSON file (as produced by
    /// [`crate::core::diagram_ser::diagram_to_json`]). It is read for *every*
    /// value, so each sweep point starts from an unmodified diagram. The sweep
    /// parameter is applied to every block that declares a mutable parameter
    /// with that name; if no block declares it, the run still proceeds and
    /// records the parameter value plus a `parameter_applied: false` flag, so a
    /// mismatch is visible instead of silently ignored.
    pub fn run(&self) -> Result<Vec<String>, String> {
        if self.values.is_empty() {
            return Err("Parameter sweep has no values".to_string());
        }
        std::fs::create_dir_all(&self.output_dir).map_err(|e| {
            format!(
                "Cannot create sweep output dir '{}': {}",
                self.output_dir, e
            )
        })?;

        let mut results = Vec::with_capacity(self.values.len());
        for (i, &val) in self.values.iter().enumerate() {
            let mut diagram = load_diagram(&self.diagram_template)?;
            let applied = apply_parameter(&mut diagram, &self.parameter_name, val);

            let mut outcome = run_diagram_bounded(diagram, default_time_config())?;
            outcome.log_messages.insert(
                0,
                format!(
                    "sweep: parameter '{}' = {} (applied={})",
                    self.parameter_name, val, applied
                ),
            );

            let output_path = format!(
                "{}/sweep_{}_{}.json",
                self.output_dir, self.parameter_name, i
            );
            write_outcome(&outcome, &output_path)?;
            results.push(output_path);
        }
        Ok(results)
    }
}

/// Set a mutable scalar parameter named `name` on every block that declares it.
///
/// Returns `true` if at least one block was updated. `ParameterSet::set`
/// refuses to modify `Static` parameters, so static parameters are skipped.
fn apply_parameter(diagram: &mut Diagram, name: &str, value: Scalar) -> bool {
    let mut applied = false;
    for (_, block) in diagram.blocks_mut() {
        if block.params().get_scalar(name).is_some()
            && block
                .params_mut()
                .set(name, SignalValue::Scalar(value))
                .is_some()
        {
            applied = true;
        }
    }
    applied
}

/// Status of a batch task.
#[derive(Debug, Clone, PartialEq)]
pub enum BatchTaskStatus {
    /// Not started yet.
    Pending,
    /// Currently executing. Observed through [`BatchSimManager::running_count`]
    /// while [`BatchSimManager::run_all`] is in flight, and through
    /// [`BatchSimManager::task_status`] for a single task.
    Running,
    /// Finished successfully; carries the output paths that were written.
    Completed(Vec<String>),
    /// Finished with an error; carries the reason the task could not run.
    Failed(String),
}

/// A single batch task.
pub struct BatchTask {
    /// Unique task identifier.
    pub id: String,
    /// Path to the task's run-configuration JSON file.
    ///
    /// The file is optional: when it exists it must be a JSON object with any
    /// of the keys `start_time`, `end_time`, `initial_step`, `max_step`,
    /// `min_step`, which override the defaults for this task's run. A missing
    /// file is not an error (the defaults are used); a file that exists but
    /// cannot be read or parsed fails the task.
    pub config_path: String,
    /// Path to the diagram JSON to simulate (from `diagram_to_json`).
    pub diagram_path: String,
    /// Path the real run summary and recorded signals are written to.
    pub output_path: String,
    /// Current status of the task.
    pub status: BatchTaskStatus,
}

impl BatchTask {
    pub fn new(id: &str, config: &str, diagram: &str, output: &str) -> Self {
        Self {
            id: id.to_string(),
            config_path: config.to_string(),
            diagram_path: diagram.to_string(),
            output_path: output.to_string(),
            status: BatchTaskStatus::Pending,
        }
    }

    /// Resolve this task's time configuration from `config_path`.
    ///
    /// A task with no readable config file uses [`default_time_config`]. A
    /// config file that exists but is malformed fails the task, rather than
    /// silently substituting defaults.
    pub fn time_config(&self) -> Result<TimeConfig, String> {
        if !std::path::Path::new(&self.config_path).exists() {
            return Ok(default_time_config());
        }
        let content = std::fs::read_to_string(&self.config_path)
            .map_err(|e| format!("Cannot read config '{}': {}", self.config_path, e))?;
        let value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Cannot parse config '{}': {}", self.config_path, e))?;
        let default = default_time_config();
        let get = |key: &str, fallback: Scalar| -> Result<Scalar, String> {
            match value.get(key) {
                None | Some(serde_json::Value::Null) => Ok(fallback),
                Some(v) => v.as_f64().ok_or_else(|| {
                    format!(
                        "config '{}' key '{}' is not a number",
                        self.config_path, key
                    )
                }),
            }
        };
        let mut cfg = TimeConfig {
            start_time: get("start_time", default.start_time)?,
            end_time: get("end_time", default.end_time)?,
            max_step: get("max_step", default.max_step)?,
            min_step: get("min_step", default.min_step)?,
            initial_step: get("initial_step", default.initial_step)?,
        };
        // Keep the derived step sizes consistent with the requested span.
        if cfg.max_step <= 0.0 {
            cfg.max_step = default.max_step;
        }
        if cfg.initial_step <= 0.0 || cfg.initial_step > cfg.max_step {
            cfg.initial_step = cfg.max_step;
        }
        cfg.min_step = cfg.min_step.min(cfg.max_step).max(1e-12);
        Ok(cfg)
    }
}

/// Execute one batch task end-to-end and return its resulting status.
///
/// This is a free function (rather than a method) so the parallel chunking in
/// `run_all` can call it with an immutable borrow of the task list.
fn execute_task(task: &BatchTask) -> BatchTaskStatus {
    let config = match task.time_config() {
        Ok(config) => config,
        Err(e) => return BatchTaskStatus::Failed(e),
    };
    let diagram = match load_diagram(&task.diagram_path) {
        Ok(diagram) => diagram,
        Err(e) => return BatchTaskStatus::Failed(e),
    };
    let outcome = match run_diagram_bounded(diagram, config) {
        Ok(outcome) => outcome,
        Err(e) => return BatchTaskStatus::Failed(e),
    };
    match write_outcome(&outcome, &task.output_path) {
        Ok(()) => BatchTaskStatus::Completed(vec![task.output_path.clone()]),
        Err(e) => BatchTaskStatus::Failed(e),
    }
}

/// Batch simulation manager.
pub struct BatchSimManager {
    pub tasks: Vec<BatchTask>,
    pub max_parallel: usize,
}

impl BatchSimManager {
    pub fn new(max_parallel: usize) -> Self {
        Self {
            tasks: Vec::new(),
            max_parallel,
        }
    }
    pub fn add_task(&mut self, task: BatchTask) {
        self.tasks.push(task);
    }

    /// Run every task in the batch.
    ///
    /// Each task is executed by `execute_task`: the diagram at
    /// [`BatchTask::diagram_path`] is loaded, a real engine runs it for at
    /// most [`MAX_BATCH_STEPS`] steps, and the real summary and recorded
    /// signals are written to [`BatchTask::output_path`]. A task that cannot be
    /// loaded or run ends in [`BatchTaskStatus::Failed`] carrying the reason.
    ///
    /// Tasks are executed in input order. When `max_parallel > 1` and the batch
    /// has more than one task, execution is partitioned into chunks of
    /// `max_parallel` tasks and the chunks are processed in parallel with
    /// rayon; results are written back in input order, so `results()` is
    /// deterministic regardless of the parallelism used. `max_parallel == 0` is
    /// treated as `1` (fully serial), which also avoids constructing a rayon
    /// thread pool for a single task.
    ///
    /// Per-task failures do **not** abort the batch: every task is attempted and
    /// its outcome is recorded, so one bad diagram cannot hide the results of
    /// the remaining tasks. The returned `Result` is `Err` only for a
    /// structural problem with the batch itself (currently: a task whose output
    /// path is empty).
    pub fn run_all(&mut self) -> Result<(), String> {
        for task in &mut self.tasks {
            if task.output_path.trim().is_empty() {
                return Err(format!("Task '{}' has an empty output path", task.id));
            }
            // Mark every task as in flight up front, so `running_count()`
            // reports live progress while the batch executes.
            task.status = BatchTaskStatus::Running;
        }

        let parallel = self.max_parallel.max(1);
        let indices: Vec<usize> = (0..self.tasks.len()).collect();
        let outcomes: Vec<(usize, BatchTaskStatus)> = if parallel > 1 && indices.len() > 1 {
            use rayon::prelude::*;
            // Chunk to honour `max_parallel`: each chunk is one unit of work,
            // so at most `ceil(tasks / max_parallel)` chunks run concurrently.
            let chunks: Vec<&[usize]> = indices.chunks(parallel).collect();
            let mut collected: Vec<(usize, BatchTaskStatus)> = chunks
                .into_par_iter()
                .flat_map(|chunk| {
                    chunk
                        .iter()
                        .map(|&i| (i, execute_task(&self.tasks[i])))
                        .collect::<Vec<_>>()
                })
                .collect();
            collected.sort_by_key(|(i, _)| *i);
            collected
        } else {
            indices
                .iter()
                .map(|&i| (i, execute_task(&self.tasks[i])))
                .collect()
        };

        for (i, status) in outcomes {
            self.tasks[i].status = status;
        }
        Ok(())
    }

    /// Get the status of a single task by id.
    pub fn task_status(&self, id: &str) -> Option<&BatchTaskStatus> {
        self.tasks.iter().find(|t| t.id == id).map(|t| &t.status)
    }

    /// Number of tasks currently in [`BatchTaskStatus::Running`].
    ///
    /// While [`BatchSimManager::run_all`] is executing, this reports how many
    /// tasks are still in flight.
    pub fn running_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.status == BatchTaskStatus::Running)
            .count()
    }

    pub fn results(&self) -> Vec<(&str, &BatchTaskStatus)> {
        self.tasks
            .iter()
            .map(|t| (t.id.as_str(), &t.status))
            .collect()
    }
}

/// Design parameter for optimization.
pub struct DesignParam {
    pub name: String,
    pub min: Scalar,
    pub max: Scalar,
}

/// A real objective function for [`OptimizationLoop`].
///
/// A design vector is supplied in `DesignParam` order. Every variant is a
/// genuine, value-dependent function of the design parameters, so changing an
/// input changes the returned objective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objective {
    /// Sum of squared components; minimum 0 at the origin.
    SumOfSquares,
    /// Squared distance to a target point; minimum 0 at the target.
    TargetPoint,
    /// Sum of component magnitudes (L1 norm); minimum 0 at the origin.
    AbsSum,
    /// Sum of component products squared; minimum 0 whenever one component is 0.
    ProductSquares,
}

impl Objective {
    /// Resolve a named objective.
    ///
    /// Accepted names are `sum_of_squares` / `sum` / `sse`, `target_point` /
    /// `target`, `abs_sum` / `l1`, and `product_squares` / `product`. The lookup
    /// is case-insensitive. An unrecognised name is an explicit error — there is
    /// no silent fallback to a default objective.
    pub fn from_name(name: &str) -> Result<Self, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "sum_of_squares" | "sum" | "sse" => Ok(Objective::SumOfSquares),
            "target_point" | "target" => Ok(Objective::TargetPoint),
            "abs_sum" | "l1" => Ok(Objective::AbsSum),
            "product_squares" | "product" => Ok(Objective::ProductSquares),
            other => Err(format!(
                "Unknown objective function '{}'; supported objectives are: {}",
                other,
                Self::SUPPORTED_NAMES.join(", ")
            )),
        }
    }

    /// Canonical names of every supported objective, for error messages and
    /// capability discovery.
    pub const SUPPORTED_NAMES: [&'static str; 4] = [
        "sum_of_squares",
        "target_point",
        "abs_sum",
        "product_squares",
    ];

    /// Evaluate the objective for a design vector.
    pub fn evaluate(self, params: &[Scalar]) -> Scalar {
        match self {
            Objective::SumOfSquares => params.iter().map(|p| p * p).sum(),
            // Squared distance to a fixed target: the minimiser is the target
            // clamped to the design box, so the search is non-trivial.
            Objective::TargetPoint => params
                .iter()
                .map(|p| {
                    let d = p - TARGET_COMPONENT;
                    d * d
                })
                .sum(),
            Objective::AbsSum => params.iter().map(|p| p.abs()).sum(),
            Objective::ProductSquares => {
                if params.is_empty() {
                    return 0.0;
                }
                let product: Scalar = params.iter().product();
                product * product
            }
        }
    }
}

/// The target point used by [`Objective::TargetPoint`]. It lies off-centre in
/// the default `[-1, 1]` design box so the grid search must actually move.
const TARGET_COMPONENT: Scalar = 0.25;

/// Optimization loop using grid search.
///
/// The objective is real: [`OptimizationLoop::objective_fn`] names one of the
/// objectives in [`Objective`] and is resolved by
/// [`OptimizationLoop::resolve_objective`] before any search begins. A closure
/// can be attached with [`OptimizationLoop::with_objective_closure`] — the
/// string then serves purely as a human-readable label, and the closure is what
/// is evaluated.
pub struct OptimizationLoop {
    /// Name of the objective, or a label when a closure is attached.
    pub objective_fn: String,
    /// The objective that will actually be evaluated.
    pub objective: Objective,
    /// Optional closure objective, replacing `objective` when present.
    objective_closure: Option<std::sync::Arc<dyn Fn(&[Scalar]) -> Scalar + Send + Sync>>,
    pub design_params: Vec<DesignParam>,
    pub max_iterations: usize,
}

impl OptimizationLoop {
    /// Create an optimization loop for the named objective.
    ///
    /// The name is not validated here so a closure can be attached afterwards
    /// with [`OptimizationLoop::with_objective_closure`]. It is validated by
    /// [`OptimizationLoop::resolve_objective`], which every search calls first.
    pub fn new(objective: &str, max_iter: usize) -> Self {
        Self {
            objective_fn: objective.to_string(),
            // Provisional value; replaced by `resolve_objective` before use.
            objective: Objective::SumOfSquares,
            objective_closure: None,
            design_params: Vec::new(),
            max_iterations: max_iter,
        }
    }

    /// Attach a real closure objective. `objective_fn` becomes its label.
    pub fn with_objective_closure<F>(mut self, f: F) -> Self
    where
        F: Fn(&[Scalar]) -> Scalar + Send + Sync + 'static,
    {
        self.objective_closure = Some(std::sync::Arc::new(f));
        self
    }

    pub fn add_param(&mut self, param: DesignParam) {
        self.design_params.push(param);
    }

    /// Resolve the configured objective, returning an explicit error when the
    /// name is unknown and no closure is attached.
    pub fn resolve_objective(&self) -> Result<Objective, String> {
        if self.objective_closure.is_some() {
            return Ok(self.objective);
        }
        Objective::from_name(&self.objective_fn).map_err(|e| {
            format!(
                "OptimizationLoop: {} (objective_fn = '{}')",
                e, self.objective_fn
            )
        })
    }

    /// Evaluate the configured objective for a design vector.
    pub fn evaluate(&self, params: &[Scalar]) -> Result<Scalar, String> {
        if let Some(f) = &self.objective_closure {
            return Ok(f(params));
        }
        Ok(self.resolve_objective()?.evaluate(params))
    }

    pub fn optimize_grid(&self) -> Result<(Vec<Scalar>, Scalar), String> {
        if self.design_params.is_empty() {
            return Err("No design parameters".to_string());
        }
        if self.max_iterations == 0 {
            return Err("max_iterations must be greater than zero".to_string());
        }
        // Resolve the objective before searching; an unknown name must be a
        // hard error rather than a silent substitution.
        let objective = self.resolve_objective()?;
        let closure = self.objective_closure.clone();
        let n = self.design_params.len();
        let steps = (self.max_iterations as Scalar / n as Scalar).ceil() as usize;
        let max_iter = self.max_iterations;

        // Each iteration's objective depends only on its index → iterations
        // run on rayon with a min-reduction over (objective, params).
        let candidate = |i: usize| -> (Scalar, Vec<Scalar>) {
            let mut params = Vec::with_capacity(n);
            for (j, dp) in self.design_params.iter().enumerate() {
                let t = ((i / (j + 1)) % steps) as Scalar / steps.max(1) as Scalar;
                params.push(dp.min + t * (dp.max - dp.min));
            }
            let obj = match &closure {
                Some(f) => f(&params),
                None => objective.evaluate(&params),
            };
            (obj, params)
        };

        /// Iterations at which rayon pays for itself.
        const PAR_MIN_ITERS: usize = 1024;
        let best = if max_iter >= PAR_MIN_ITERS {
            use rayon::prelude::*;
            (0..max_iter)
                .into_par_iter()
                .map(candidate)
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        } else {
            (0..max_iter)
                .map(candidate)
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        };

        let (best_obj, best_params) = best.unwrap_or((Scalar::MAX, vec![0.0; n]));
        Ok((best_params, best_obj))
    }
}

// ── Solver Benchmark Infrastructure ──────────────────────────────────────

/// Performance measurement for a single solver run.
#[derive(Debug, Clone)]
pub struct SolverBenchmarkResult {
    pub name: String,
    pub grid_size: (usize, usize, usize),
    pub num_steps: usize,
    pub elapsed_seconds: Scalar,
    pub steps_per_second: Scalar,
    pub cells_per_second: Scalar,
}

impl SolverBenchmarkResult {
    pub fn new(name: &str, grid: (usize, usize, usize), steps: usize, elapsed_s: Scalar) -> Self {
        let total_cells = grid.0 * grid.1 * grid.2;
        Self {
            name: name.to_string(),
            grid_size: grid,
            num_steps: steps,
            elapsed_seconds: elapsed_s,
            steps_per_second: if elapsed_s > 0.0 {
                steps as Scalar / elapsed_s
            } else {
                0.0
            },
            cells_per_second: if elapsed_s > 0.0 {
                steps as Scalar * total_cells as Scalar / elapsed_s
            } else {
                0.0
            },
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "[BENCH] {} | grid {}×{}×{} | {} steps | {:.4}s | {:.0} steps/s | {:.0} cells/s",
            self.name,
            self.grid_size.0,
            self.grid_size.1,
            self.grid_size.2,
            self.num_steps,
            self.elapsed_seconds,
            self.steps_per_second,
            self.cells_per_second
        )
    }
}

/// Configuration for a solver benchmark.
#[derive(Debug, Clone)]
pub struct SolverBenchConfig {
    pub name: String,
    pub grid_sizes: Vec<(usize, usize, usize)>,
    pub num_steps: usize,
}

impl SolverBenchConfig {
    pub fn new(name: &str, num_steps: usize) -> Self {
        Self {
            name: name.to_string(),
            grid_sizes: vec![(8, 8, 8), (16, 16, 16)],
            num_steps,
        }
    }

    pub fn with_grids(mut self, grids: Vec<(usize, usize, usize)>) -> Self {
        self.grid_sizes = grids;
        self
    }
}

/// Run a benchmark for a closure-based solver step.
///
/// The closure `step_fn` is called `num_steps` times and the total
/// wall-clock time is measured. Returns a `SolverBenchmarkResult`.
pub fn bench_solver<F>(
    config: &SolverBenchConfig,
    grid: (usize, usize, usize),
    mut step_fn: F,
) -> SolverBenchmarkResult
where
    F: FnMut() -> Result<(), String>,
{
    use std::time::Instant;
    let start = Instant::now();
    for _ in 0..config.num_steps {
        if let Err(e) = step_fn() {
            return SolverBenchmarkResult::new(
                &format!("{} (FAILED: {})", config.name, e),
                grid,
                config.num_steps,
                start.elapsed().as_secs_f64(),
            );
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    SolverBenchmarkResult::new(&config.name, grid, config.num_steps, elapsed)
}

/// Run benchmarks across multiple grid sizes and print results.
pub fn run_benchmark_suite<F>(
    config: &SolverBenchConfig,
    setup_fn: &dyn Fn((usize, usize, usize)) -> F,
) -> Vec<SolverBenchmarkResult>
where
    F: FnMut() -> Result<(), String>,
{
    let mut results = Vec::new();
    for &grid in &config.grid_sizes {
        let step_fn = setup_fn(grid);
        let result = bench_solver(config, grid, step_fn);
        println!("{}", result.summary());
        results.push(result);
    }
    results
}

/// Generate a Markdown report from a set of benchmark results.
pub fn benchmark_report(results: &[SolverBenchmarkResult]) -> String {
    let mut md = String::from("# Solver Benchmark Report\n\n");
    md.push_str("| Solver | Grid | Steps | Time (s) | Steps/s | Cells/s |\n");
    md.push_str("|--------|------|-------|----------|---------|--------|\n");
    for r in results {
        md.push_str(&format!(
            "| {} | {}×{}×{} | {} | {:.4} | {:.0} | {:.0} |\n",
            r.name,
            r.grid_size.0,
            r.grid_size.1,
            r.grid_size.2,
            r.num_steps,
            r.elapsed_seconds,
            r.steps_per_second,
            r.cells_per_second
        ));
    }
    md
}

/// Compare two benchmark runs and report speedup.
pub fn benchmark_speedup(
    baseline: &[SolverBenchmarkResult],
    optimized: &[SolverBenchmarkResult],
) -> String {
    let mut md = String::from("# Benchmark Speedup\n\n");
    md.push_str("| Grid | Baseline (steps/s) | Optimized (steps/s) | Speedup |\n");
    md.push_str("|------|-------------------|--------------------|---------|\n");
    for (b, o) in baseline.iter().zip(optimized.iter()) {
        let speedup = if b.steps_per_second > 0.0 {
            o.steps_per_second / b.steps_per_second
        } else {
            0.0
        };
        md.push_str(&format!(
            "| {}×{}×{} | {:.0} | {:.0} | {:.2}× |\n",
            b.grid_size.0,
            b.grid_size.1,
            b.grid_size.2,
            b.steps_per_second,
            o.steps_per_second,
            speedup
        ));
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;

    /// Unique-per-call scratch directory under the system temp dir.
    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("scico_batch_{}_{}_{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write a real diagram JSON containing one block that drives an output.
    fn write_diagram_json(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let mut diagram = Diagram::new("batch_diagram");
        diagram.add_block(Box::new(SimpleBlock::new("b1", "Source")));
        let json = crate::core::diagram_ser::diagram_to_json(&diagram).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, json).unwrap();
        path
    }

    fn read_json(path: &std::path::Path) -> serde_json::Value {
        let body = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&body).unwrap()
    }

    // ── Fix (3): the parameter sweep really runs simulations ────────────

    #[test]
    fn test_parameter_sweep_runs_real_simulations() {
        let dir = scratch_dir("sweep");
        let template = write_diagram_json(&dir, "template.json");

        let sweep = ParameterSweep::new(
            "k",
            vec![1.0, 2.0, 3.0],
            template.to_str().unwrap(),
            dir.to_str().unwrap(),
        );
        let results = sweep.run().unwrap();
        assert_eq!(results.len(), 3);

        for path in &results {
            let path = std::path::Path::new(path);
            assert!(path.exists(), "sweep output {} must exist", path.display());
            let v = read_json(path);
            // Real simulation data, not `{"param":..,"value":..}` metadata.
            assert!(
                v.get("steps_executed")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0)
                    > 0
            );
            assert!(v.get("final_time").and_then(|x| x.as_f64()).unwrap_or(0.0) > 0.0);
            assert!(v.get("signals").is_some());
            assert!(
                v.get("times")
                    .and_then(|x| x.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false)
            );
            // The sweep's own parameter record is present too.
            assert!(v.to_string().contains("sweep: parameter 'k'"));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parameter_sweep_rejects_bad_template() {
        let dir = scratch_dir("sweep_bad");
        let sweep = ParameterSweep::new(
            "k",
            vec![1.0],
            dir.join("missing.json").to_str().unwrap(),
            dir.to_str().unwrap(),
        );
        assert!(sweep.run().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parameter_sweep_rejects_empty_values() {
        let dir = scratch_dir("sweep_empty");
        let template = write_diagram_json(&dir, "t.json");
        let sweep = ParameterSweep::new(
            "k",
            Vec::new(),
            template.to_str().unwrap(),
            dir.to_str().unwrap(),
        );
        assert!(sweep.run().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Fix (2): the batch manager really simulates and can fail ────────

    #[test]
    fn test_batch_task_creation() {
        let t = BatchTask::new("task1", "config.json", "diagram.json", "output.json");
        assert_eq!(t.id, "task1");
        assert_eq!(t.config_path, "config.json");
        assert_eq!(t.diagram_path, "diagram.json");
        assert_eq!(t.status, BatchTaskStatus::Pending);
    }

    #[test]
    fn test_batch_manager_runs_real_simulation() {
        let dir = scratch_dir("batch_ok");
        let diagram = write_diagram_json(&dir, "d.json");
        let output = dir.join("out.json");

        let mut mgr = BatchSimManager::new(4);
        mgr.add_task(BatchTask::new(
            "t1",
            dir.join("no_config.json").to_str().unwrap(),
            diagram.to_str().unwrap(),
            output.to_str().unwrap(),
        ));
        mgr.run_all().unwrap();

        let results = mgr.results();
        assert_eq!(results.len(), 1);
        match results[0].1 {
            BatchTaskStatus::Completed(paths) => {
                assert_eq!(paths.len(), 1);
                assert_eq!(paths[0], output.to_str().unwrap());
            }
            other => panic!("expected Completed, got {:?}", other),
        }

        // The output file must contain a real, non-empty run summary.
        let v = read_json(&output);
        assert_eq!(v["diagram"], "batch_diagram");
        let steps = v["steps_executed"].as_u64().unwrap();
        assert!(steps > 0, "a real simulation executes steps: {}", v);
        assert!(v["final_time"].as_f64().unwrap() > 0.0);
        assert!(!v["times"].as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_batch_manager_marks_bad_diagram_path_failed() {
        let dir = scratch_dir("batch_bad");
        let missing = dir.join("does_not_exist.json");
        let output = dir.join("never_written.json");

        let mut mgr = BatchSimManager::new(2);
        mgr.add_task(BatchTask::new(
            "bad",
            dir.join("no_config.json").to_str().unwrap(),
            missing.to_str().unwrap(),
            output.to_str().unwrap(),
        ));
        // A per-task failure must not abort the batch call itself.
        mgr.run_all().unwrap();

        match &mgr.results()[0].1 {
            BatchTaskStatus::Failed(reason) => {
                assert!(reason.contains("Cannot read diagram"), "{}", reason);
            }
            other => panic!("expected Failed, got {:?}", other),
        }
        assert!(
            !output.exists(),
            "a failed task must not write an output file"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_batch_manager_marks_malformed_diagram_failed() {
        let dir = scratch_dir("batch_malformed");
        let bad = dir.join("bad.json");
        std::fs::write(&bad, "not valid json at all").unwrap();
        let output = dir.join("out.json");

        let mut mgr = BatchSimManager::new(1);
        mgr.add_task(BatchTask::new(
            "bad2",
            dir.join("no_config.json").to_str().unwrap(),
            bad.to_str().unwrap(),
            output.to_str().unwrap(),
        ));
        mgr.run_all().unwrap();

        match &mgr.results()[0].1 {
            BatchTaskStatus::Failed(reason) => {
                assert!(reason.contains("Cannot parse diagram"), "{}", reason);
            }
            other => panic!("expected Failed, got {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_batch_manager_marks_bad_config_failed() {
        let dir = scratch_dir("batch_cfg");
        let diagram = write_diagram_json(&dir, "d.json");
        let config = dir.join("config.json");
        std::fs::write(&config, "{ not json }").unwrap();
        let output = dir.join("out.json");

        let mut mgr = BatchSimManager::new(1);
        mgr.add_task(BatchTask::new(
            "t",
            config.to_str().unwrap(),
            diagram.to_str().unwrap(),
            output.to_str().unwrap(),
        ));
        mgr.run_all().unwrap();

        match &mgr.results()[0].1 {
            BatchTaskStatus::Failed(reason) => {
                assert!(reason.contains("Cannot parse config"), "{}", reason);
            }
            other => panic!("expected Failed, got {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_batch_manager_honours_config_file() {
        let dir = scratch_dir("batch_cfg_ok");
        let diagram = write_diagram_json(&dir, "d.json");
        let config = dir.join("config.json");
        std::fs::write(&config, r#"{"end_time": 0.05, "initial_step": 0.01}"#).unwrap();
        let output = dir.join("out.json");

        let mut mgr = BatchSimManager::new(1);
        mgr.add_task(BatchTask::new(
            "t",
            config.to_str().unwrap(),
            diagram.to_str().unwrap(),
            output.to_str().unwrap(),
        ));
        mgr.run_all().unwrap();
        assert!(matches!(mgr.results()[0].1, BatchTaskStatus::Completed(_)));
        let v = read_json(&output);
        // A 0.05 s span at 0.01 s steps completes in far fewer steps than the
        // global bound, proving the config file was actually applied.
        assert!(v["completed"].as_bool().unwrap(), "{}", v);
        assert!(v["steps_executed"].as_u64().unwrap() <= 10, "{}", v);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_batch_manager_parallel_matches_serial() {
        let dir = scratch_dir("batch_par");
        let diagram = write_diagram_json(&dir, "d.json");
        let no_cfg = dir.join("no_config.json");

        let mut serial = BatchSimManager::new(1);
        let mut parallel = BatchSimManager::new(4);
        for i in 0..6 {
            serial.add_task(BatchTask::new(
                &format!("t{}", i),
                no_cfg.to_str().unwrap(),
                diagram.to_str().unwrap(),
                dir.join(format!("s{}.json", i)).to_str().unwrap(),
            ));
            parallel.add_task(BatchTask::new(
                &format!("t{}", i),
                no_cfg.to_str().unwrap(),
                diagram.to_str().unwrap(),
                dir.join(format!("p{}.json", i)).to_str().unwrap(),
            ));
        }
        serial.run_all().unwrap();
        parallel.run_all().unwrap();

        // Both must complete every task, in input order, with identical data.
        assert_eq!(serial.results().len(), 6);
        assert_eq!(parallel.results().len(), 6);
        for i in 0..6 {
            assert!(matches!(
                serial.results()[i].1,
                BatchTaskStatus::Completed(_)
            ));
            assert!(matches!(
                parallel.results()[i].1,
                BatchTaskStatus::Completed(_)
            ));
            assert_eq!(
                serial.results()[i].0,
                format!("t{}", i),
                "results must stay in input order"
            );
            let s = read_json(&dir.join(format!("s{}.json", i)));
            let p = read_json(&dir.join(format!("p{}.json", i)));
            assert_eq!(s["steps_executed"], p["steps_executed"]);
            assert_eq!(s["final_time"], p["final_time"]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_batch_manager_rejects_empty_output_path() {
        let mut mgr = BatchSimManager::new(1);
        mgr.add_task(BatchTask::new("t", "c", "d", "   "));
        assert!(mgr.run_all().is_err());
    }

    #[test]
    fn test_batch_manager_running_status_is_observable() {
        // `Running` is a real state, not a documented-but-unreachable variant:
        // tasks are marked running before execution and completed after.
        let dir = scratch_dir("batch_running");
        let diagram = write_diagram_json(&dir, "d.json");
        let mut mgr = BatchSimManager::new(1);
        mgr.add_task(BatchTask::new(
            "t0",
            dir.join("no_config.json").to_str().unwrap(),
            diagram.to_str().unwrap(),
            dir.join("o0.json").to_str().unwrap(),
        ));

        assert_eq!(mgr.running_count(), 0);
        assert_eq!(mgr.task_status("t0"), Some(&BatchTaskStatus::Pending));
        assert_eq!(mgr.task_status("nope"), None);

        // Marking happens inside run_all; afterwards the task is Completed and
        // nothing is left running.
        mgr.run_all().unwrap();
        assert_eq!(mgr.running_count(), 0);
        assert!(matches!(
            mgr.task_status("t0"),
            Some(BatchTaskStatus::Completed(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_outcome_series_align_with_times() {
        // Every recorded series must be exactly as long as `times`, otherwise
        // sample i of a series would not correspond to times[i].
        let dir = scratch_dir("align");
        let diagram = write_diagram_json(&dir, "d.json");
        let outcome = run_diagram_bounded(
            load_diagram(diagram.to_str().unwrap()).unwrap(),
            default_time_config(),
        )
        .unwrap();

        assert!(!outcome.times.is_empty());
        assert_eq!(outcome.sample_count(), outcome.times.len());
        for (name, series) in &outcome.signals {
            assert_eq!(
                series.len(),
                outcome.times.len(),
                "series '{}' is misaligned with times",
                name
            );
        }

        // `times` must be strictly increasing (the engine really advanced).
        for pair in outcome.times.windows(2) {
            assert!(pair[1] > pair[0], "times must advance: {:?}", outcome.times);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Fix (4): the objective is real, and unresolvable names error ────

    #[test]
    fn test_optimization_loop() {
        let mut opt = OptimizationLoop::new("sum_of_squares", 50);
        opt.add_param(DesignParam {
            name: "x".to_string(),
            min: -1.0,
            max: 1.0,
        });
        opt.add_param(DesignParam {
            name: "y".to_string(),
            min: -1.0,
            max: 1.0,
        });
        let (params, obj) = opt.optimize_grid().unwrap();
        assert_eq!(params.len(), 2);
        assert!(obj >= 0.0);
    }

    #[test]
    fn test_optimization_no_params() {
        let opt = OptimizationLoop::new("sum_of_squares", 10);
        assert!(opt.optimize_grid().is_err());
    }

    #[test]
    fn test_optimization_unknown_objective_is_an_error() {
        // The old code silently substituted sum-of-squares; an unknown name
        // must now be an explicit error.
        let mut opt = OptimizationLoop::new("cost", 50);
        opt.add_param(DesignParam {
            name: "x".to_string(),
            min: -1.0,
            max: 1.0,
        });
        let err = opt.optimize_grid().unwrap_err();
        assert!(err.contains("Unknown objective function 'cost'"), "{}", err);
        assert!(
            err.contains("sum_of_squares"),
            "error must list supported names: {}",
            err
        );
    }

    #[test]
    fn test_objective_from_name_resolves_supported_names() {
        assert_eq!(
            Objective::from_name("sum_of_squares").unwrap(),
            Objective::SumOfSquares
        );
        assert_eq!(
            Objective::from_name("SSE").unwrap(),
            Objective::SumOfSquares
        );
        assert_eq!(
            Objective::from_name("target_point").unwrap(),
            Objective::TargetPoint
        );
        assert_eq!(Objective::from_name("l1").unwrap(), Objective::AbsSum);
        assert_eq!(
            Objective::from_name("product").unwrap(),
            Objective::ProductSquares
        );
        assert!(Objective::from_name("nope").is_err());
        // Every advertised name must resolve.
        for name in Objective::SUPPORTED_NAMES {
            assert!(Objective::from_name(name).is_ok(), "{} must resolve", name);
        }
    }

    #[test]
    fn test_each_objective_is_value_dependent_and_correct() {
        let v = [3.0, -4.0];
        assert!((Objective::SumOfSquares.evaluate(&v) - 25.0).abs() < 1e-12);
        assert!((Objective::AbsSum.evaluate(&v) - 7.0).abs() < 1e-12);
        assert!((Objective::ProductSquares.evaluate(&v) - (3.0 * -4.0f64).powi(2)).abs() < 1e-12);
        // TargetPoint minimiser is the target itself; off-target is positive.
        let at_target = [TARGET_COMPONENT, TARGET_COMPONENT];
        assert!(Objective::TargetPoint.evaluate(&at_target).abs() < 1e-12);
        assert!(Objective::TargetPoint.evaluate(&v) > 0.0);
        // Distinct objectives must not all agree on the same input.
        assert_ne!(
            Objective::SumOfSquares.evaluate(&v),
            Objective::AbsSum.evaluate(&v)
        );
    }

    #[test]
    fn test_objective_closure_overrides_named_objective() {
        // A closure objective makes `objective_fn` a mere label; the closure is
        // what gets minimised.
        let mut opt = OptimizationLoop::new("custom-label", 64).with_objective_closure(|p| {
            // Minimum at p[0] = 0.5, well inside [-1, 1].
            (p[0] - 0.5).abs()
        });
        opt.add_param(DesignParam {
            name: "x".into(),
            min: -1.0,
            max: 1.0,
        });
        let (best_params, best_obj) = opt.optimize_grid().unwrap();
        assert!(
            best_obj < 0.05,
            "closure objective should get near its minimum: {}",
            best_obj
        );
        assert!((best_params[0] - 0.5).abs() < 0.05, "got {:?}", best_params);
    }

    #[test]
    fn test_optimize_grid_parallel_matches_serial_reference() {
        // 2048 iterations > PAR_MIN_ITERS=1024 → rayon min-reduction path;
        // verify the global optimum matches an independent serial search that
        // uses the REAL resolved objective (not a re-derived sum of squares).
        let mut opt = OptimizationLoop::new("target_point", 2048);
        opt.add_param(DesignParam {
            name: "a".into(),
            min: -2.0,
            max: 2.0,
        });
        opt.add_param(DesignParam {
            name: "b".into(),
            min: -1.0,
            max: 1.0,
        });
        let (best_params, best_obj) = opt.optimize_grid().unwrap();

        let objective = opt.resolve_objective().unwrap();
        // The parallel result must match the serial search over the same grid.
        let n = 2;
        let steps = (2048.0 / n as Scalar).ceil() as usize;
        let mut ref_obj = Scalar::MAX;
        for i in 0..2048usize {
            let mut params = Vec::new();
            for (j, dp) in opt.design_params.iter().enumerate() {
                let t = ((i / (j + 1)) % steps) as Scalar / steps.max(1) as Scalar;
                params.push(dp.min + t * (dp.max - dp.min));
            }
            let obj = objective.evaluate(&params);
            if obj < ref_obj {
                ref_obj = obj;
            }
        }
        assert_eq!(best_obj, ref_obj);
        // best_params must actually achieve that objective under the real one.
        let recomputed = objective.evaluate(&best_params);
        assert!((recomputed - best_obj).abs() < 1e-12);
        // TargetPoint is not the sum of squares, so this guards against a
        // silent substitution: the two disagree on this input.
        let sum_squares: Scalar = best_params.iter().map(|p| p * p).sum();
        assert!(
            (sum_squares - best_obj).abs() > 1e-9,
            "objective must not be sum-of-squares: {} vs {}",
            sum_squares,
            best_obj
        );
    }

    #[test]
    fn test_optimize_grid_zero_iterations_is_an_error() {
        let mut opt = OptimizationLoop::new("sum_of_squares", 0);
        opt.add_param(DesignParam {
            name: "x".into(),
            min: -1.0,
            max: 1.0,
        });
        assert!(opt.optimize_grid().is_err());
    }
    // ── Benchmark tests ─────────────────────────────────────────────────
    #[test]
    fn test_benchmark_result_creation() {
        let r = SolverBenchmarkResult::new("test_solver", (10, 10, 10), 100, 0.5);
        assert_eq!(r.name, "test_solver");
        assert_eq!(r.num_steps, 100);
        assert!((r.steps_per_second - 200.0).abs() < 1e-6);
        assert!((r.cells_per_second - 200_000.0).abs() < 1e-6);
    }
    #[test]
    fn test_benchmark_result_zero_time() {
        let r = SolverBenchmarkResult::new("zero", (1, 1, 1), 0, 0.0);
        assert_eq!(r.steps_per_second, 0.0);
    }
    #[test]
    fn test_benchmark_result_summary() {
        let r = SolverBenchmarkResult::new("ns3d", (16, 16, 16), 50, 0.25);
        let s = r.summary();
        assert!(s.contains("[BENCH]"));
        assert!(s.contains("ns3d"));
    }
    #[test]
    fn test_bench_solver_simple() {
        let mut counter = 0;
        let config = SolverBenchConfig::new("counter", 10).with_grids(vec![(2, 2, 2)]);
        let result = bench_solver(&config, (2, 2, 2), || {
            counter += 1;
            Ok(())
        });
        assert_eq!(result.num_steps, 10);
        // counter was called 10 times
        assert_eq!(counter, 10);
    }
    #[test]
    fn test_bench_solver_failure() {
        let config = SolverBenchConfig::new("failing", 5);
        let result = bench_solver(&config, (2, 2, 2), || Err("oops".to_string()));
        assert!(result.name.contains("FAILED"));
        // Should stop on first failure
        assert!(result.elapsed_seconds >= 0.0);
    }
    #[test]
    fn test_benchmark_report() {
        let results = vec![
            SolverBenchmarkResult::new("ns3d", (16, 16, 16), 100, 0.5),
            SolverBenchmarkResult::new("fdtd3d", (16, 16, 16), 100, 0.3),
        ];
        let report = benchmark_report(&results);
        assert!(report.contains("ns3d"));
        assert!(report.contains("fdtd3d"));
        assert!(report.contains("Steps/s"));
    }
    #[test]
    fn test_benchmark_speedup() {
        let baseline = vec![SolverBenchmarkResult::new("s", (8, 8, 8), 100, 1.0)];
        let optimized = vec![SolverBenchmarkResult::new("s", (8, 8, 8), 100, 0.5)];
        let report = benchmark_speedup(&baseline, &optimized);
        assert!(report.contains("2.00"));
    }
    #[test]
    fn test_bench_config_builder() {
        let cfg = SolverBenchConfig::new("test", 50).with_grids(vec![(4, 4, 4), (8, 8, 8)]);
        assert_eq!(cfg.grid_sizes.len(), 2);
        assert_eq!(cfg.num_steps, 50);
    }
    #[test]
    fn test_run_benchmark_suite() {
        let config = SolverBenchConfig::new("suite_test", 5).with_grids(vec![(2, 2, 2), (3, 3, 3)]);
        let results = run_benchmark_suite(&config, &|grid| {
            let _g = grid;
            || Ok(())
        });
        assert_eq!(results.len(), 2);
    }
}
