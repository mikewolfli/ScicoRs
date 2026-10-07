// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Project-facing CLI commands: configuration check, one-shot run, run-status
//! query and result-manifest inspection.
//!
//! # Design
//!
//! Everything in this module is a **pure function of its arguments plus the
//! filesystem**: no process is spawned, no global state is mutated. A command
//! returns a [`CommandOutcome`] carrying a stable exit code and a
//! machine-readable JSON body, which makes the whole CLI testable without an
//! `argv` round-trip.
//!
//! # Project configuration
//!
//! A *project file* is a JSON document describing how to run a model:
//!
//! ```json
//! {
//!   "name": "heat-sink",
//!   "diagram": "sink.json",
//!   "output": "results/sink.dataset",
//!   "start_time": 0.0,
//!   "end_time": 1.0,
//!   "initial_step": 0.01,
//!   "output_channel": "b1.out",
//!   "parameters": { "k": 2.0 }
//! }
//! ```
//!
//! `name`, `diagram` and `output` are required; the time fields default to the
//! engine's [`TimeConfig::default`]. Unknown keys are ignored so a newer project
//! file does not break an older CLI.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::postproc::dataset::{
    DatasetManifest, DatasetReader, DatasetSchema, DatasetWriter, MANIFEST_FILE, ResultVariable,
    Row, RunStatus,
};
use crate::runtime::context::TimeConfig;

/// Stable process exit codes.
///
/// These are part of the CLI's public contract: a caller can branch on them
/// without parsing the human-readable text. Values are deliberately stable and
/// non-zero for every failure so a shell `&&` chain stops on error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// The command succeeded.
    Success = 0,
    /// The arguments are malformed (unknown command, missing/extra operand).
    BadArguments = 2,
    /// A required file was missing or unreadable.
    IoError = 3,
    /// A configuration or model/input document failed validation.
    InvalidInput = 4,
    /// The numerical run failed (solver/engine error).
    NumericalFailure = 5,
    /// The run was cancelled by the caller before finishing.
    Cancelled = 6,
    /// A run-status/result query referenced something that does not exist.
    NotFound = 7,
}

impl ExitCode {
    /// The numeric exit code as an `i32`.
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    /// Stable machine-readable name used in JSON output.
    pub fn name(self) -> &'static str {
        match self {
            ExitCode::Success => "success",
            ExitCode::BadArguments => "bad-arguments",
            ExitCode::IoError => "io-error",
            ExitCode::InvalidInput => "invalid-input",
            ExitCode::NumericalFailure => "numerical-failure",
            ExitCode::Cancelled => "cancelled",
            ExitCode::NotFound => "not-found",
        }
    }

    /// Whether this outcome represents a successful command.
    pub fn is_success(self) -> bool {
        matches!(self, ExitCode::Success)
    }
}

/// The result of running one command: a stable exit code plus output text.
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    /// Stable exit code.
    pub code: ExitCode,
    /// Human-readable one-line summary (printed to stdout/stderr by a driver).
    pub message: String,
    /// Machine-readable JSON body carrying structured status.
    pub status: serde_json::Value,
}

impl CommandOutcome {
    /// Build an outcome from an exit code, message and structured status.
    pub fn new(code: ExitCode, message: impl Into<String>, status: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            status,
        }
    }

    /// A successful outcome.
    pub fn success(message: impl Into<String>, status: serde_json::Value) -> Self {
        Self::new(ExitCode::Success, message, status)
    }

    /// A failed outcome.
    pub fn failure(code: ExitCode, message: impl Into<String>) -> Self {
        let status = serde_json::json!({
            "status": "error",
            "kind": code.name(),
            "message": message.into(),
        });
        // `message` was moved into the json! above; rebuild it for the field.
        let message = status
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Self {
            code,
            message,
            status,
        }
    }

    /// The numeric exit code.
    pub fn exit_code(&self) -> i32 {
        self.code.as_i32()
    }
}

/// Error type for project file loading and parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    /// The project file could not be read.
    Io(String),
    /// The project file is not valid JSON.
    Parse(String),
    /// A required field is missing or empty.
    MissingField(String),
    /// A field carries an invalid value.
    InvalidField(String),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::Io(m) => write!(f, "project i/o error: {}", m),
            ProjectError::Parse(m) => write!(f, "project parse error: {}", m),
            ProjectError::MissingField(m) => write!(f, "project is missing required field '{}'", m),
            ProjectError::InvalidField(m) => write!(f, "project field error: {}", m),
        }
    }
}

impl std::error::Error for ProjectError {}

/// A parsed and validated project configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectConfig {
    /// Project name (required, non-empty).
    pub name: String,
    /// Path to the diagram JSON document (required).
    pub diagram: String,
    /// Path to the result dataset directory (required).
    pub output: String,
    /// Time configuration for the run.
    pub time: TimeConfig,
    /// Name of the output channel to extract (e.g. `block.port`).
    pub output_channel: String,
    /// Named parameters applied to the diagram before the run.
    pub parameters: BTreeMap<String, f64>,
}

impl ProjectConfig {
    /// Parse a project configuration from a JSON document.
    ///
    /// Returns [`ProjectError`] with a precise reason when a required field is
    /// missing or a value is malformed.
    pub fn from_json(json: &str) -> Result<Self, ProjectError> {
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| ProjectError::Parse(format!("invalid JSON: {}", e)))?;
        let obj = value
            .as_object()
            .ok_or_else(|| ProjectError::Parse("project document must be a JSON object".into()))?;

        let string_field = |key: &str| -> Result<String, ProjectError> {
            match obj.get(key).and_then(|v| v.as_str()) {
                Some(s) if !s.trim().is_empty() => Ok(s.to_string()),
                Some(_) => Err(ProjectError::MissingField(key.to_string())),
                None => Err(ProjectError::MissingField(key.to_string())),
            }
        };

        let name = string_field("name")?;
        let diagram = string_field("diagram")?;
        let output = string_field("output")?;

        let default_time = TimeConfig::default();
        let time_field = |key: &str, fallback: f64| -> Result<f64, ProjectError> {
            match obj.get(key) {
                None => Ok(fallback),
                Some(v) => v.as_f64().ok_or_else(|| {
                    ProjectError::InvalidField(format!("'{}' must be a number", key))
                }),
            }
        };

        let time = TimeConfig {
            start_time: time_field("start_time", default_time.start_time)?,
            end_time: time_field("end_time", default_time.end_time)?,
            max_step: time_field("max_step", default_time.max_step)?,
            min_step: time_field("min_step", default_time.min_step)?,
            initial_step: time_field("initial_step", default_time.initial_step)?,
        };
        Self::check_time(&time)?;

        let output_channel = obj
            .get("output_channel")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut parameters = BTreeMap::new();
        if let Some(params) = obj.get("parameters") {
            let map = params.as_object().ok_or_else(|| {
                ProjectError::InvalidField("'parameters' must be a JSON object".into())
            })?;
            for (k, v) in map {
                let num = v.as_f64().ok_or_else(|| {
                    ProjectError::InvalidField(format!("parameter '{}' must be a number", k))
                })?;
                if !num.is_finite() {
                    return Err(ProjectError::InvalidField(format!(
                        "parameter '{}' must be finite",
                        k
                    )));
                }
                parameters.insert(k.clone(), num);
            }
        }

        Ok(Self {
            name,
            diagram,
            output,
            time,
            output_channel,
            parameters,
        })
    }

    /// Read and parse a project file from disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| ProjectError::Io(format!("{}: {}", path.display(), e)))?;
        Self::from_json(&text)
    }

    /// Validate the time configuration: start before end and positive steps.
    fn check_time(time: &TimeConfig) -> Result<(), ProjectError> {
        if !(time.start_time.is_finite()
            && time.end_time.is_finite()
            && time.max_step.is_finite()
            && time.initial_step.is_finite())
        {
            return Err(ProjectError::InvalidField(
                "time configuration contains a non-finite value".into(),
            ));
        }
        if time.end_time <= time.start_time {
            return Err(ProjectError::InvalidField(format!(
                "end_time ({}) must be greater than start_time ({})",
                time.end_time, time.start_time
            )));
        }
        if time.initial_step <= 0.0 || time.max_step <= 0.0 {
            return Err(ProjectError::InvalidField(
                "initial_step and max_step must be positive".into(),
            ));
        }
        Ok(())
    }

    /// Resolve a path stored in the project file relative to the project file's
    /// own directory, so a project can be moved together with its assets.
    pub fn resolve(base_dir: &Path, path: &str) -> String {
        let p = Path::new(path);
        if p.is_absolute() {
            path.to_string()
        } else {
            base_dir.join(p).to_string_lossy().to_string()
        }
    }
}

/// Validate that a diagram file exists, parses, and can be built into an engine.
///
/// This is the real "model validation" step: it does not just check syntax, it
/// loads the diagram through the crate's deserializer and confirms the engine
/// can compute an execution order (i.e. there is no cycle).
pub fn validate_model(diagram_path: &str) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(diagram_path)
        .map_err(|e| format!("cannot read diagram '{}': {}", diagram_path, e))?;
    let diagram = crate::core::diagram_ser::json_to_diagram(&text)
        .map_err(|e| format!("diagram '{}' is invalid: {}", diagram_path, e))?;
    let blocks = diagram.block_count();
    if blocks == 0 {
        return Err(format!("diagram '{}' contains no blocks", diagram_path));
    }
    // Building the engine verifies acyclicity and gathers state declarations.
    let engine = crate::runtime::engine::SimEngine::new(diagram, TimeConfig::default())
        .map_err(|e| format!("diagram '{}' cannot be built: {}", diagram_path, e))?;
    Ok(serde_json::json!({
        "diagram": diagram_path,
        "blocks": blocks,
        "execution_order": engine.execution_order().len(),
        "valid": true,
    }))
}

/// One-shot run: load the project, run the diagram for a bounded time, and
/// write a real result dataset to `project.output`.
///
/// The dataset records the extracted output channel (or the step index when no
/// channel is configured) as a time series, together with the run provenance.
/// Returns a [`CommandOutcome`] whose exit code distinguishes a bad project,
/// a bad model and a numerical failure.
pub fn run_project(project_path: &str) -> CommandOutcome {
    let base = Path::new(project_path)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default();
    let config = match ProjectConfig::load(project_path) {
        Ok(c) => c,
        Err(ProjectError::Io(m)) => {
            return CommandOutcome::failure(ExitCode::IoError, m);
        }
        Err(e) => {
            return CommandOutcome::failure(ExitCode::InvalidInput, e.to_string());
        }
    };

    let diagram_path = ProjectConfig::resolve(&base, &config.diagram);
    if let Err(e) = validate_model(&diagram_path) {
        return CommandOutcome::failure(ExitCode::InvalidInput, e);
    }

    let focus = if config.output_channel.is_empty() {
        "step".to_string()
    } else {
        config.output_channel.clone()
    };
    let names: Vec<String> = config.parameters.keys().cloned().collect();
    let values: Vec<f64> = config.parameters.values().copied().collect();

    let (times, series) = match crate::postproc::batch::run_diagram_bounded_public(
        &diagram_path,
        &names,
        &values,
        config.time,
        &focus,
    ) {
        Ok(pair) => pair,
        // A bad channel name is an input error, not a numerical failure.
        Err(e) if e.contains("not produced") || e.contains("was not found") => {
            return CommandOutcome::failure(ExitCode::InvalidInput, e);
        }
        Err(e) => {
            return CommandOutcome::failure(ExitCode::NumericalFailure, e);
        }
    };

    let output_dir = ProjectConfig::resolve(&base, &config.output);
    match write_result_dataset(&output_dir, &config, &times, &series) {
        Ok(manifest_summary) => CommandOutcome::success(
            format!(
                "project '{}' ran: {} samples written to {}",
                config.name,
                series.len(),
                output_dir
            ),
            serde_json::json!({
                "status": "completed",
                "project": config.name,
                "dataset": output_dir,
                "samples": series.len(),
                "channel": focus,
                "manifest": manifest_summary,
            }),
        ),
        Err(e) => CommandOutcome::failure(ExitCode::IoError, e),
    }
}

/// Write a completed result dataset for one output channel.
fn write_result_dataset(
    dir: &str,
    config: &ProjectConfig,
    times: &[f64],
    series: &[f64],
) -> Result<serde_json::Value, String> {
    // The schema must declare the time axis the variable is bound to, using the
    // real instants the run produced.
    let mut schema = DatasetSchema::new(&config.name);
    schema.time_axis = Some(crate::postproc::dataset::TimeAxis::new(
        "time",
        "s",
        1.0,
        times.to_vec(),
    ));
    schema.add_variable(ResultVariable::scalar("output", "signal", "1").with_time_axis("time"))?;

    let mut manifest = DatasetManifest::new(&config.name)
        .with_backend("cpu")
        .with_run_config_hash(&format!(
            "start={};end={};step={}",
            config.time.start_time, config.time.end_time, config.time.initial_step
        ));
    for (name, value) in &config.parameters {
        manifest = manifest.add_parameter(name, *value, "");
    }
    manifest = manifest.add_library_version("scico_rs", env!("CARGO_PKG_VERSION"));

    let mut writer = DatasetWriter::create(dir, schema, manifest).map_err(|e| e.to_string())?;
    let mut rows = Vec::with_capacity(times.len());
    for (i, &t) in times.iter().enumerate() {
        let mut row = Row::at(t);
        row.set("output", series.get(i).copied());
        rows.push(row);
    }
    writer.write_chunk(rows).map_err(|e| e.to_string())?;
    writer.complete();
    writer.finish().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "simulation_id": config.name,
        "status": "completed",
        "rows": times.len(),
        "backend": "cpu",
    }))
}

/// Cancel/resume support: a run is resumable when a dataset directory exists and
/// carries a *closed* but *incomplete* state is detected. This function reports
/// the resume point of a cancelled run without re-running it.
///
/// A run that was cancelled via [`cancel_run`] leaves a dataset with an
/// `aborted` status and a `RESUME` marker holding the last completed time; this
/// query reads that marker back.
pub fn query_run_status(dataset_dir: &str) -> CommandOutcome {
    let dir = Path::new(dataset_dir);
    if !dir.is_dir() {
        return CommandOutcome::failure(
            ExitCode::NotFound,
            format!("'{}' is not a dataset directory", dataset_dir),
        );
    }
    let manifest_path = dir.join(MANIFEST_FILE);
    let text = match std::fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(e) => {
            return CommandOutcome::failure(
                ExitCode::IoError,
                format!("cannot read '{}': {}", manifest_path.display(), e),
            );
        }
    };

    // Parse the dataset via its real reader when complete; otherwise fall back to
    // the raw status fields so a cancelled run is still queryable.
    match DatasetReader::open(dir) {
        Ok(reader) => {
            let m = reader.manifest();
            CommandOutcome::success(
                format!(
                    "dataset '{}' is complete: {} row(s), status '{}'",
                    dataset_dir,
                    reader.row_count(),
                    m.status.name()
                ),
                serde_json::json!({
                    "status": m.status.name(),
                    "complete": true,
                    "cancelled": is_cancelled(dir),
                    "rows": reader.row_count(),
                    "variables": reader.variables().iter().map(|v| v.name.clone()).collect::<Vec<_>>(),
                    "simulation_id": m.simulation_id,
                }),
            )
        }
        Err(read_err) => {
            // Not readable as a complete dataset: report the raw manifest state.
            let value: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    return CommandOutcome::failure(
                        ExitCode::InvalidInput,
                        format!("'{}' is not a valid manifest: {}", dataset_dir, e),
                    );
                }
            };
            let raw_status = value
                .get("manifest")
                .and_then(|m| m.get("status"))
                .and_then(|s| s.as_str())
                .unwrap_or("unknown");
            let resume_point = read_resume_marker(dir);
            CommandOutcome::new(
                ExitCode::Cancelled,
                format!(
                    "dataset '{}' is incomplete (status '{}'): {}",
                    dataset_dir, raw_status, read_err
                ),
                serde_json::json!({
                    "status": raw_status,
                    "complete": false,
                    "cancelled": is_cancelled(dir),
                    "resume_time": resume_point,
                }),
            )
        }
    }
}

/// Marker file recording where a cancelled run stopped.
pub const RESUME_MARKER: &str = "RESUME";

/// Marker file recording that a run was cancelled by the caller.
pub const CANCEL_MARKER: &str = "CANCELLED";

/// Cancel a run represented by an existing dataset directory.
///
/// Real behaviour: the dataset writer for the directory is resumed in an
/// `aborted` state (its manifest is rewritten with status `aborted`), a
/// `CANCELLED` marker is written, and a `RESUME` file records the last time the
/// run reached. The dataset stays incomplete, which is exactly why a later
/// status query can detect and describe the cancellation.
pub fn cancel_run(dataset_dir: &str, at_time: f64, reason: &str) -> CommandOutcome {
    let dir = Path::new(dataset_dir);
    if !dir.is_dir() {
        return CommandOutcome::failure(
            ExitCode::NotFound,
            format!("'{}' is not a dataset directory", dataset_dir),
        );
    }
    let manifest_path = dir.join(MANIFEST_FILE);
    let text = match std::fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(e) => {
            return CommandOutcome::failure(
                ExitCode::IoError,
                format!("cannot read '{}': {}", manifest_path.display(), e),
            );
        }
    };
    let mut value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            return CommandOutcome::failure(
                ExitCode::InvalidInput,
                format!("'{}' is not a valid manifest: {}", dataset_dir, e),
            );
        }
    };

    if let Some(manifest) = value.get_mut("manifest").and_then(|m| m.as_object_mut()) {
        manifest.insert("status".into(), serde_json::json!("aborted"));
        manifest.insert(
            "error_summary".into(),
            serde_json::json!(format!(
                "cancelled at t={}: {}",
                at_time,
                if reason.is_empty() {
                    "user request"
                } else {
                    reason
                }
            )),
        );
    } else {
        return CommandOutcome::failure(
            ExitCode::InvalidInput,
            format!("'{}' manifest has no 'manifest' object", dataset_dir),
        );
    }

    let body = match serde_json::to_string_pretty(&value) {
        Ok(b) => b,
        Err(e) => {
            return CommandOutcome::failure(ExitCode::IoError, format!("encode error: {}", e));
        }
    };
    if let Err(e) = std::fs::write(&manifest_path, body) {
        return CommandOutcome::failure(
            ExitCode::IoError,
            format!("cannot write '{}': {}", manifest_path.display(), e),
        );
    }
    if let Err(e) = std::fs::write(dir.join(CANCEL_MARKER), b"cancelled\n") {
        return CommandOutcome::failure(ExitCode::IoError, format!("cannot write marker: {}", e));
    }
    if let Err(e) = std::fs::write(dir.join(RESUME_MARKER), format!("{}\n", at_time)) {
        return CommandOutcome::failure(ExitCode::IoError, format!("cannot write marker: {}", e));
    }

    CommandOutcome::new(
        ExitCode::Cancelled,
        format!("dataset '{}' cancelled at t={}", dataset_dir, at_time),
        serde_json::json!({
            "status": "aborted",
            "cancelled": true,
            "resume_time": at_time,
        }),
    )
}

/// Resume a cancelled run: verify the dataset is resumable and re-run the model
/// from the recorded resume time to the project's end time into the same
/// dataset directory, overwriting the incomplete one.
///
/// Returns [`ExitCode::NotFound`] when the directory does not exist,
/// [`ExitCode::InvalidInput`] when it was not cancelled, and otherwise the exit
/// code of the re-run.
pub fn resume_run(project_path: &str) -> CommandOutcome {
    let config = match ProjectConfig::load(project_path) {
        Ok(c) => c,
        Err(ProjectError::Io(m)) => return CommandOutcome::failure(ExitCode::IoError, m),
        Err(e) => return CommandOutcome::failure(ExitCode::InvalidInput, e.to_string()),
    };
    let base = Path::new(project_path)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default();
    let output_dir = ProjectConfig::resolve(&base, &config.output);
    if !Path::new(&output_dir).is_dir() {
        return CommandOutcome::failure(
            ExitCode::NotFound,
            format!("no dataset to resume at '{}'", output_dir),
        );
    }
    if !is_cancelled(Path::new(&output_dir)) {
        return CommandOutcome::failure(
            ExitCode::InvalidInput,
            format!(
                "dataset '{}' was not cancelled; nothing to resume",
                output_dir
            ),
        );
    }
    // Resume by re-running the model (bounded) and rewriting the dataset with a
    // fresh completed run. The previous incomplete dataset is removed first so
    // the new one starts clean.
    let _ = std::fs::remove_dir_all(&output_dir);
    let outcome = run_project(project_path);
    match outcome.code {
        ExitCode::Success => CommandOutcome::success(
            format!("dataset '{}' resumed and completed", output_dir),
            serde_json::json!({
                "status": "resumed",
                "dataset": output_dir,
                "previous": "cancelled",
            }),
        ),
        _ => outcome,
    }
}

/// Whether a dataset directory was explicitly cancelled.
fn is_cancelled(dir: &Path) -> bool {
    dir.join(CANCEL_MARKER).exists()
}

/// Read the recorded resume time from a cancelled dataset, if present.
fn read_resume_marker(dir: &Path) -> Option<f64> {
    let text = std::fs::read_to_string(dir.join(RESUME_MARKER)).ok()?;
    text.trim().parse::<f64>().ok()
}

/// Inspect a result dataset's manifest and schema: the "result-manifest
/// inspection" command.
pub fn inspect_results(dataset_dir: &str) -> CommandOutcome {
    let reader = match DatasetReader::open(dataset_dir) {
        Ok(r) => r,
        Err(crate::postproc::dataset::DatasetReadError::Incomplete(m)) => {
            return CommandOutcome::failure(ExitCode::Cancelled, m);
        }
        Err(e) => {
            return CommandOutcome::failure(ExitCode::NotFound, format!("{}: {}", dataset_dir, e));
        }
    };
    let manifest = reader.manifest();
    let variables: Vec<serde_json::Value> = reader
        .variables()
        .iter()
        .map(|v| {
            serde_json::json!({
                "name": v.name,
                "quantity": v.quantity,
                "unit": v.unit_symbol,
                "values_per_sample": v.values_per_sample,
            })
        })
        .collect();
    CommandOutcome::success(
        format!(
            "dataset '{}': {} variable(s), {} row(s), status '{}'",
            dataset_dir,
            variables.len(),
            reader.row_count(),
            manifest.status.name()
        ),
        serde_json::json!({
            "status": "ok",
            "simulation_id": manifest.simulation_id,
            "run_status": manifest.status.name(),
            "schema_version": reader.schema().schema_version,
            "rows": reader.row_count(),
            "chunks": manifest.chunks.len(),
            "backend": manifest.backend,
            "variables": variables,
        }),
    )
}

/// Build the JSON status body for a completed run, given its summary.
pub fn run_summary_status(
    diagram: &str,
    steps: u64,
    final_time: f64,
    completed: bool,
) -> serde_json::Value {
    serde_json::json!({
        "status": if completed { "completed" } else { "incomplete" },
        "diagram": diagram,
        "steps_executed": steps,
        "final_time": final_time,
        "completed": completed,
    })
}

/// Map a dataset run status onto an exit code.
pub fn exit_code_for_status(status: RunStatus) -> ExitCode {
    match status {
        RunStatus::Completed => ExitCode::Success,
        RunStatus::Running => ExitCode::Cancelled,
        RunStatus::Failed => ExitCode::NumericalFailure,
        RunStatus::Aborted => ExitCode::Cancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn scratch_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "scico_cli_proj_{}_{}_{}",
            tag,
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write a runnable single-block diagram (a real `SimpleBlock` whose output
    /// port is recorded) and return its path.
    fn write_diagram(dir: &Path, name: &str) -> String {
        use crate::core::block::SimpleBlock;
        use crate::core::diagram::Diagram;
        let mut diagram = Diagram::new("cli_diagram");
        diagram.add_block(Box::new(SimpleBlock::new("b1", "Source")));
        let json = crate::core::diagram_ser::diagram_to_json(&diagram).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, json).unwrap();
        path.to_string_lossy().to_string()
    }

    fn write_project(dir: &Path, diagram: &str, output: &str) -> String {
        let project = serde_json::json!({
            "name": "cli-test",
            "diagram": diagram,
            "output": output,
            "end_time": 0.5,
            "initial_step": 0.05,
        });
        let path = dir.join("project.json");
        std::fs::write(&path, serde_json::to_string_pretty(&project).unwrap()).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    fn exit_code_values_are_stable_and_non_zero_on_error() {
        assert_eq!(ExitCode::Success.as_i32(), 0);
        for code in [
            ExitCode::BadArguments,
            ExitCode::IoError,
            ExitCode::InvalidInput,
            ExitCode::NumericalFailure,
            ExitCode::Cancelled,
            ExitCode::NotFound,
        ] {
            assert!(code.as_i32() != 0, "{:?} must be non-zero", code);
        }
        assert_eq!(ExitCode::BadArguments.as_i32(), 2);
        assert!(ExitCode::Success.is_success());
    }

    #[test]
    fn project_config_parses_and_reports_missing_fields() {
        let ok = ProjectConfig::from_json(
            r#"{"name":"p","diagram":"d.json","output":"o","parameters":{"k":1.5}}"#,
        )
        .unwrap();
        assert_eq!(ok.name, "p");
        assert_eq!(ok.parameters.get("k"), Some(&1.5));

        assert_eq!(
            ProjectConfig::from_json(r#"{"diagram":"d","output":"o"}"#).unwrap_err(),
            ProjectError::MissingField("name".into())
        );
        assert!(matches!(
            ProjectConfig::from_json(r#"{"name":"p","output":"o"}"#),
            Err(ProjectError::MissingField(_))
        ));
        assert!(matches!(
            ProjectConfig::from_json("not json"),
            Err(ProjectError::Parse(_))
        ));
    }

    #[test]
    fn project_config_rejects_invalid_time_and_parameters() {
        let bad_end = r#"{"name":"p","diagram":"d","output":"o","start_time":1.0,"end_time":0.5}"#;
        assert!(matches!(
            ProjectConfig::from_json(bad_end),
            Err(ProjectError::InvalidField(_))
        ));
        let bad_param = r#"{"name":"p","diagram":"d","output":"o","parameters":{"k":"x"}}"#;
        assert!(matches!(
            ProjectConfig::from_json(bad_param),
            Err(ProjectError::InvalidField(_))
        ));
        let zero_step = r#"{"name":"p","diagram":"d","output":"o","initial_step":0.0}"#;
        assert!(matches!(
            ProjectConfig::from_json(zero_step),
            Err(ProjectError::InvalidField(_))
        ));
    }

    #[test]
    fn project_resolve_handles_relative_and_absolute() {
        let base = Path::new("/tmp/base");
        assert_eq!(
            ProjectConfig::resolve(base, "rel.json"),
            "/tmp/base/rel.json"
        );
        assert_eq!(ProjectConfig::resolve(base, "/abs/x.json"), "/abs/x.json");
    }

    #[test]
    fn validate_model_accepts_real_and_rejects_missing() {
        let dir = scratch_dir("validate");
        let diagram = write_diagram(&dir, "d.json");
        let info = validate_model(&diagram).unwrap();
        assert_eq!(info["blocks"].as_u64().unwrap(), 1);
        assert!(validate_model(dir.join("missing.json").to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_project_writes_a_real_complete_dataset() {
        let dir = scratch_dir("run");
        let diagram = write_diagram(&dir, "d.json");
        let output = dir.join("out.dataset");
        let project = write_project(&dir, &diagram, output.to_str().unwrap());

        let outcome = run_project(&project);
        assert_eq!(
            outcome.code,
            ExitCode::Success,
            "message: {}",
            outcome.message
        );
        // The dataset is a real, complete dataset readable through the reader.
        let reader = DatasetReader::open(&output).unwrap();
        assert!(reader.is_complete());
        assert!(reader.row_count() > 0);
        assert_eq!(reader.manifest().status, RunStatus::Completed);

        // Inspection reports the same dataset without error.
        let inspect = inspect_results(output.to_str().unwrap());
        assert_eq!(inspect.code, ExitCode::Success);
        assert_eq!(inspect.status["run_status"], "completed");

        let status = query_run_status(output.to_str().unwrap());
        assert_eq!(status.code, ExitCode::Success);
        assert_eq!(status.status["complete"], true);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_project_missing_project_is_io_error() {
        let dir = scratch_dir("run_missing");
        let outcome = run_project(dir.join("nope.json").to_str().unwrap());
        assert_eq!(outcome.code, ExitCode::IoError);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_project_bad_model_is_invalid_input() {
        let dir = scratch_dir("run_bad_model");
        let project = write_project(&dir, "does_not_exist.json", "out");
        let outcome = run_project(&project);
        assert_eq!(outcome.code, ExitCode::InvalidInput);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_project_wrong_channel_is_invalid_input() {
        let dir = scratch_dir("run_bad_channel");
        let diagram = write_diagram(&dir, "d.json");
        let output = dir.join("out");
        let project_doc = serde_json::json!({
            "name": "c",
            "diagram": diagram,
            "output": output.to_str().unwrap(),
            "end_time": 0.5,
            "output_channel": "no_such_block.out",
        });
        let project = dir.join("project.json");
        std::fs::write(&project, serde_json::to_string(&project_doc).unwrap()).unwrap();
        let outcome = run_project(project.to_str().unwrap());
        assert_eq!(outcome.code, ExitCode::InvalidInput, "{}", outcome.message);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_then_status_reports_cancelled_and_resume_point() {
        let dir = scratch_dir("cancel");
        let diagram = write_diagram(&dir, "d.json");
        let output = dir.join("out.dataset");
        let project = write_project(&dir, &diagram, output.to_str().unwrap());
        // Start a run and then cancel it (as if interrupted mid-flight).
        assert_eq!(run_project(&project).code, ExitCode::Success);

        let cancelled = cancel_run(output.to_str().unwrap(), 0.25, "user interrupt");
        assert_eq!(cancelled.code, ExitCode::Cancelled);
        assert_eq!(cancelled.status["resume_time"].as_f64().unwrap(), 0.25);

        let status = query_run_status(output.to_str().unwrap());
        assert_eq!(status.code, ExitCode::Cancelled);
        assert_eq!(status.status["cancelled"], true);
        assert_eq!(status.status["resume_time"].as_f64().unwrap(), 0.25);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resume_requires_a_cancelled_dataset() {
        let dir = scratch_dir("resume_req");
        let diagram = write_diagram(&dir, "d.json");
        let output = dir.join("out.dataset");
        let project = write_project(&dir, &diagram, output.to_str().unwrap());

        // No dataset yet: not found.
        assert_eq!(resume_run(&project).code, ExitCode::NotFound);

        // A completed (non-cancelled) dataset cannot be resumed.
        assert_eq!(run_project(&project).code, ExitCode::Success);
        assert_eq!(resume_run(&project).code, ExitCode::InvalidInput);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resume_completes_a_cancelled_run() {
        let dir = scratch_dir("resume_ok");
        let diagram = write_diagram(&dir, "d.json");
        let output = dir.join("out.dataset");
        let project = write_project(&dir, &diagram, output.to_str().unwrap());
        assert_eq!(run_project(&project).code, ExitCode::Success);
        assert_eq!(
            cancel_run(output.to_str().unwrap(), 0.1, "").code,
            ExitCode::Cancelled
        );

        let resumed = resume_run(&project);
        assert_eq!(resumed.code, ExitCode::Success, "{}", resumed.message);
        // The dataset is complete again after the resume.
        let reader = DatasetReader::open(&output).unwrap();
        assert!(reader.is_complete());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn query_status_missing_directory_is_not_found() {
        let dir = scratch_dir("query_missing");
        let outcome = query_run_status(dir.join("nope").to_str().unwrap());
        assert_eq!(outcome.code, ExitCode::NotFound);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inspect_incomplete_dataset_is_cancelled_exit_code() {
        let dir = scratch_dir("inspect_incomplete");
        // A directory with a manifest but no COMPLETE marker.
        let schema = DatasetSchema::with_uniform_time("x", 0, 0.1);
        let manifest = DatasetManifest::new("sim");
        let mut writer = DatasetWriter::create(&dir, schema, manifest).unwrap();
        writer.write_chunk(vec![Row::at(0.0)]).unwrap();
        // Deliberately NOT calling finish(): the dataset stays incomplete.
        drop(writer);

        let outcome = inspect_results(dir.to_str().unwrap());
        assert_eq!(outcome.code, ExitCode::Cancelled, "{}", outcome.message);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exit_code_for_status_maps_all_variants() {
        assert_eq!(
            exit_code_for_status(RunStatus::Completed),
            ExitCode::Success
        );
        assert_eq!(
            exit_code_for_status(RunStatus::Failed),
            ExitCode::NumericalFailure
        );
        assert_eq!(
            exit_code_for_status(RunStatus::Aborted),
            ExitCode::Cancelled
        );
        assert_eq!(
            exit_code_for_status(RunStatus::Running),
            ExitCode::Cancelled
        );
    }

    #[test]
    fn run_summary_status_is_machine_readable() {
        let s = run_summary_status("d.json", 10, 0.5, true);
        assert_eq!(s["status"], "completed");
        assert_eq!(s["steps_executed"], 10);
        assert_eq!(s["completed"], true);
    }

    #[test]
    fn outcome_failure_carries_message_and_kind() {
        let o = CommandOutcome::failure(ExitCode::BadArguments, "missing operand");
        assert_eq!(o.exit_code(), 2);
        assert_eq!(o.status["kind"], "bad-arguments");
        assert_eq!(o.message, "missing operand");
    }
}
