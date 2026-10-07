// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! User-facing command-line toolchain (Phase 41).
//!
//! This module is the *library* form of the SCIcoRS CLI. It exposes a
//! [`dispatch`] function that turns an `argv` vector into a
//! [`CommandOutcome`] (stable exit code + machine-readable JSON status) without
//! ever spawning a process. A thin [`cli_main`] wrapper is provided for a host
//! binary to call, but the crate ships no `[[bin]]` target: the toolchain is a
//! reusable library surface, and the same dispatch is what a future binary (or
//! a Python/C host) would drive.
//!
//! # Commands
//!
//! | Command | Purpose |
//! |---------|---------|
//! | `check <project.json>` | validate project config, model and output paths |
//! | `run <project.json>` | one-shot bounded run writing a result dataset |
//! | `status <dataset_dir>` | query a run's status / resume point |
//! | `results <dataset_dir>` | inspect a result dataset's manifest |
//! | `cancel <dataset_dir> [t]` | cancel a run, recording a resume point |
//! | `resume <project.json>` | resume a cancelled run |
//! | `validate model <diagram.json>` | validate a model |
//! | `validate library <dir>` | validate a library directory of manifests |
//! | `validate plugin <manifest> [host_version]` | validate plugin compatibility |
//! | `validate dataset <dir>` | validate a result dataset |
//! | `validate compat <dir>` | report dataset schema compatibility |
//! | `version` | print the toolchain and API version |
//!
//! # Exit codes
//!
//! Exit codes are stable and non-zero on every failure (see [`ExitCode`]).
//! `--help`/`--version` are recognised, and `--json` may appear anywhere to
//! force JSON-only output.

pub mod project;
pub mod validate;

pub use project::{
    CANCEL_MARKER, CommandOutcome, ExitCode, ProjectConfig, ProjectError, RESUME_MARKER,
    cancel_run, exit_code_for_status, inspect_results, query_run_status, resume_run, run_project,
    run_summary_status,
};
pub use validate::{
    ApiVersionSpec, validate_dataset, validate_dataset_compatibility, validate_library,
    validate_model, validate_plugin,
};

/// The API version this CLI and its bindings speak, `major.minor`.
///
/// This is the *stable* versioned surface referenced by the compatibility
/// policy: plugins declare the API version they were built against and the
/// loader rejects incompatible ones. Deprecations follow the policy in
/// [`crate::bindings::plugins::compatibility`].
pub const CLI_API_VERSION: ApiVersionSpec = ApiVersionSpec { major: 1, minor: 0 };

/// A parsed CLI command.
///
/// Parsing is separated from execution so that argument errors are decided (and
/// tested) without touching the filesystem.
#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    /// Print usage.
    Help,
    /// Print the toolchain and API version.
    Version,
    /// Validate a project file, its model and its output location.
    Check {
        /// Path to the project file.
        project: String,
    },
    /// One-shot run of a project.
    Run {
        /// Path to the project file.
        project: String,
    },
    /// Query the status of a dataset directory.
    Status {
        /// Dataset directory.
        dataset: String,
    },
    /// Inspect a result dataset's manifest.
    Results {
        /// Dataset directory.
        dataset: String,
    },
    /// Cancel a run at a given time.
    Cancel {
        /// Dataset directory.
        dataset: String,
        /// Time the run reached when cancelled.
        at_time: f64,
    },
    /// Resume a cancelled run.
    Resume {
        /// Path to the project file.
        project: String,
    },
    /// Validate a model.
    ValidateModel {
        /// Diagram document path.
        diagram: String,
    },
    /// Validate a library directory.
    ValidateLibrary {
        /// Library directory.
        dir: String,
    },
    /// Validate a plugin manifest.
    ValidatePlugin {
        /// Manifest path.
        manifest: String,
        /// Host API version (parsed).
        host_api: ApiVersionSpec,
    },
    /// Validate a result dataset.
    ValidateDataset {
        /// Dataset directory.
        dir: String,
    },
    /// Report dataset schema compatibility.
    ValidateCompat {
        /// Dataset directory.
        dir: String,
    },
}

/// Error parsing a CLI argument vector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliParseError {
    /// No command was supplied.
    MissingCommand,
    /// The command name is not recognised.
    UnknownCommand(String),
    /// A required operand is missing.
    MissingOperand {
        /// The command that needed the operand.
        command: String,
        /// The operand's role.
        operand: String,
    },
    /// An operand failed to parse.
    InvalidOperand(String),
    /// Too many operands were supplied.
    TooManyOperands(String),
}

impl std::fmt::Display for CliParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliParseError::MissingCommand => write!(f, "no command supplied"),
            CliParseError::UnknownCommand(c) => write!(f, "unknown command '{}'", c),
            CliParseError::MissingOperand { command, operand } => {
                write!(f, "command '{}' requires a {}", command, operand)
            }
            CliParseError::InvalidOperand(m) => write!(f, "invalid operand: {}", m),
            CliParseError::TooManyOperands(c) => write!(f, "too many operands for '{}'", c),
        }
    }
}

impl std::error::Error for CliParseError {}

/// Parse an argument vector (excluding the program name) into a [`CliCommand`].
///
/// This is a pure function: no filesystem or process access.
pub fn parse_command(args: &[String]) -> Result<CliCommand, CliParseError> {
    // Strip a leading program name if the caller passed the whole argv.
    let args: Vec<String> = args.iter().filter(|a| *a != "--json").cloned().collect();

    let first = args.first().ok_or(CliParseError::MissingCommand)?;
    match first.as_str() {
        "-h" | "--help" | "help" => Ok(CliCommand::Help),
        "-V" | "--version" | "version" => Ok(CliCommand::Version),
        "check" => Ok(CliCommand::Check {
            project: operand(&args, 1, "check", "project path")?,
        }),
        "run" => Ok(CliCommand::Run {
            project: operand(&args, 1, "run", "project path")?,
        }),
        "status" => Ok(CliCommand::Status {
            dataset: operand(&args, 1, "status", "dataset directory")?,
        }),
        "results" => Ok(CliCommand::Results {
            dataset: operand(&args, 1, "results", "dataset directory")?,
        }),
        "cancel" => {
            let dataset = operand(&args, 1, "cancel", "dataset directory")?;
            let at_time = match args.get(2) {
                Some(t) => t
                    .parse::<f64>()
                    .map_err(|_| CliParseError::InvalidOperand(format!("cancel time '{}'", t)))?,
                None => 0.0,
            };
            Ok(CliCommand::Cancel { dataset, at_time })
        }
        "resume" => Ok(CliCommand::Resume {
            project: operand(&args, 1, "resume", "project path")?,
        }),
        "validate" => parse_validate(&args),
        other => Err(CliParseError::UnknownCommand(other.to_string())),
    }
}

/// Parse the `validate <subject> <path> [host_api]` sub-grammar.
fn parse_validate(args: &[String]) -> Result<CliCommand, CliParseError> {
    let subject = args.get(1).ok_or_else(|| CliParseError::MissingOperand {
        command: "validate".into(),
        operand: "subject (model|library|plugin|dataset|compat)".into(),
    })?;
    match subject.as_str() {
        "model" => Ok(CliCommand::ValidateModel {
            diagram: operand(args, 2, "validate model", "diagram path")?,
        }),
        "library" => Ok(CliCommand::ValidateLibrary {
            dir: operand(args, 2, "validate library", "library directory")?,
        }),
        "plugin" => {
            let manifest = operand(args, 2, "validate plugin", "manifest path")?;
            let host_api = match args.get(3) {
                Some(v) => ApiVersionSpec::parse(v).map_err(CliParseError::InvalidOperand)?,
                None => CLI_API_VERSION,
            };
            Ok(CliCommand::ValidatePlugin { manifest, host_api })
        }
        "dataset" => Ok(CliCommand::ValidateDataset {
            dir: operand(args, 2, "validate dataset", "dataset directory")?,
        }),
        "compat" => Ok(CliCommand::ValidateCompat {
            dir: operand(args, 2, "validate compat", "dataset directory")?,
        }),
        other => Err(CliParseError::UnknownCommand(format!("validate {}", other))),
    }
}

/// Fetch operand `index`, erroring with a descriptive parse error when absent.
fn operand(
    args: &[String],
    index: usize,
    command: &str,
    role: &str,
) -> Result<String, CliParseError> {
    args.get(index)
        .cloned()
        .ok_or_else(|| CliParseError::MissingOperand {
            command: command.to_string(),
            operand: role.to_string(),
        })
}

/// Dispatch an argument vector to a command and execute it.
///
/// This is the single entry point used by tests and by [`cli_main`]. It returns
/// a [`CommandOutcome`] so the exit code and status are observable without
/// launching a process. Argument errors yield [`ExitCode::BadArguments`].
pub fn dispatch(args: &[String]) -> CommandOutcome {
    match parse_command(args) {
        Ok(command) => execute(&command),
        Err(e) => CommandOutcome::failure(ExitCode::BadArguments, e.to_string()),
    }
}

/// Execute a parsed command.
pub fn execute(command: &CliCommand) -> CommandOutcome {
    match command {
        CliCommand::Help => CommandOutcome::success(
            "usage: scico <check|run|status|results|cancel|resume|validate|version> [args]",
            serde_json::json!({ "status": "ok", "help": true, "commands": HELP_COMMANDS }),
        ),
        CliCommand::Version => CommandOutcome::success(
            format!(
                "scico_rs {} (cli api {}.{})",
                env!("CARGO_PKG_VERSION"),
                CLI_API_VERSION.major,
                CLI_API_VERSION.minor
            ),
            serde_json::json!({
                "status": "ok",
                "crate_version": env!("CARGO_PKG_VERSION"),
                "api_major": CLI_API_VERSION.major,
                "api_minor": CLI_API_VERSION.minor,
            }),
        ),
        CliCommand::Check { project } => check_project(project),
        CliCommand::Run { project } => run_project(project),
        CliCommand::Status { dataset } => query_run_status(dataset),
        CliCommand::Results { dataset } => inspect_results(dataset),
        CliCommand::Cancel { dataset, at_time } => cancel_run(dataset, *at_time, "user request"),
        CliCommand::Resume { project } => resume_run(project),
        CliCommand::ValidateModel { diagram } => validate_model(diagram),
        CliCommand::ValidateLibrary { dir } => validate_library(dir),
        CliCommand::ValidatePlugin { manifest, host_api } => validate_plugin(manifest, *host_api),
        CliCommand::ValidateDataset { dir } => validate_dataset(dir),
        CliCommand::ValidateCompat { dir } => validate_dataset_compatibility(dir),
    }
}

/// The list of supported commands, used by the help output.
pub const HELP_COMMANDS: &[&str] = &[
    "check <project.json>",
    "run <project.json>",
    "status <dataset_dir>",
    "results <dataset_dir>",
    "cancel <dataset_dir> [time]",
    "resume <project.json>",
    "validate model <diagram.json>",
    "validate library <dir>",
    "validate plugin <manifest> [host_api_version]",
    "validate dataset <dir>",
    "validate compat <dir>",
    "version",
];

/// Validate a project file: config parses, the model validates, and the output
/// location is usable. This is the "project config check" command.
///
/// The check is real: it loads and parses the project, validates the referenced
/// diagram by building an engine, and confirms the output's parent directory can
/// be created.
pub fn check_project(project_path: &str) -> CommandOutcome {
    let base = std::path::Path::new(project_path)
        .parent()
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let config = match ProjectConfig::load(project_path) {
        Ok(c) => c,
        Err(ProjectError::Io(m)) => return CommandOutcome::failure(ExitCode::IoError, m),
        Err(e) => return CommandOutcome::failure(ExitCode::InvalidInput, e.to_string()),
    };

    let diagram_path = ProjectConfig::resolve(&base, &config.diagram);
    let model = match crate::cli::project::validate_model(&diagram_path) {
        Ok(info) => info,
        Err(e) => return CommandOutcome::failure(ExitCode::InvalidInput, e),
    };

    let output_dir = ProjectConfig::resolve(&base, &config.output);
    if let Some(parent) = std::path::Path::new(&output_dir).parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return CommandOutcome::failure(
            ExitCode::IoError,
            format!(
                "cannot create output directory '{}': {}",
                parent.display(),
                e
            ),
        );
    }

    CommandOutcome::success(
        format!("project '{}' is valid", config.name),
        serde_json::json!({
            "status": "valid",
            "project": config.name,
            "model": model,
            "output": output_dir,
            "parameters": config.parameters,
            "time": {
                "start": config.time.start_time,
                "end": config.time.end_time,
                "initial_step": config.time.initial_step,
            },
        }),
    )
}

/// `main`-style entry point: dispatch, print the message and status, and return
/// the numeric exit code.
///
/// This is provided for a host binary (the crate defines no `[[bin]]`) and for
/// embedding in a C/Python host. It writes to stdout/stderr and returns the
/// process exit code the host should use.
pub fn cli_main(args: &[String]) -> i32 {
    let outcome = dispatch(args);
    if outcome.code.is_success() {
        println!("{}", outcome.message);
    } else {
        eprintln!("error: {}", outcome.message);
    }
    outcome.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn scratch_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "scico_cli_dispatch_{}_{}_{}",
            tag,
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    fn write_diagram(dir: &std::path::Path, name: &str) -> String {
        use crate::core::block::SimpleBlock;
        use crate::core::diagram::Diagram;
        let mut diagram = Diagram::new("d");
        diagram.add_block(Box::new(SimpleBlock::new("b1", "Source")));
        let json = crate::core::diagram_ser::diagram_to_json(&diagram).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, json).unwrap();
        path.to_string_lossy().to_string()
    }

    fn write_project(dir: &std::path::Path, diagram: &str, output: &str) -> String {
        let project = serde_json::json!({
            "name": "cli",
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
    fn parse_help_version_and_unknown() {
        assert_eq!(parse_command(&args(&["--help"])).unwrap(), CliCommand::Help);
        assert_eq!(
            parse_command(&args(&["version"])).unwrap(),
            CliCommand::Version
        );
        assert_eq!(
            parse_command(&[]).unwrap_err(),
            CliParseError::MissingCommand
        );
        assert!(matches!(
            parse_command(&args(&["frobnicate"])),
            Err(CliParseError::UnknownCommand(_))
        ));
    }

    #[test]
    fn parse_requires_operands() {
        assert!(matches!(
            parse_command(&args(&["run"])),
            Err(CliParseError::MissingOperand { .. })
        ));
        assert!(matches!(
            parse_command(&args(&["validate"])),
            Err(CliParseError::MissingOperand { .. })
        ));
        assert!(matches!(
            parse_command(&args(&["validate", "plugin"])),
            Err(CliParseError::MissingOperand { .. })
        ));
    }

    #[test]
    fn parse_cancel_time_and_plugin_host_version() {
        assert_eq!(
            parse_command(&args(&["cancel", "/tmp/x", "1.5"])).unwrap(),
            CliCommand::Cancel {
                dataset: "/tmp/x".into(),
                at_time: 1.5
            }
        );
        assert!(matches!(
            parse_command(&args(&["cancel", "/tmp/x", "notanumber"])),
            Err(CliParseError::InvalidOperand(_))
        ));
        assert_eq!(
            parse_command(&args(&["validate", "plugin", "m.json", "1.0"])).unwrap(),
            CliCommand::ValidatePlugin {
                manifest: "m.json".into(),
                host_api: ApiVersionSpec::new(1, 0)
            }
        );
    }

    #[test]
    fn parse_json_flag_is_ignored() {
        let parsed = parse_command(&args(&["--json", "status", "/tmp/d"])).unwrap();
        assert_eq!(
            parsed,
            CliCommand::Status {
                dataset: "/tmp/d".into()
            }
        );
    }

    #[test]
    fn dispatch_bad_args_returns_non_zero_exit_code() {
        assert_eq!(dispatch(&args(&["bogus"])).code, ExitCode::BadArguments);
        assert_eq!(dispatch(&[]).exit_code(), 2);
    }

    #[test]
    fn dispatch_version_is_success() {
        let outcome = dispatch(&args(&["version"]));
        assert_eq!(outcome.code, ExitCode::Success);
        assert_eq!(outcome.status["api_major"].as_u64().unwrap(), 1);
    }

    #[test]
    fn dispatch_check_validates_a_real_project() {
        let dir = scratch_dir("check");
        let diagram = write_diagram(&dir, "d.json");
        let project = write_project(&dir, &diagram, dir.join("out").to_str().unwrap());
        let outcome = dispatch(&args(&["check", &project]));
        assert_eq!(outcome.code, ExitCode::Success, "{}", outcome.message);
        assert_eq!(outcome.status["status"], "valid");
        // The output parent directory was actually created by the check.
        assert!(dir.join("out").parent().unwrap().exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatch_run_then_status_then_cancel_then_resume_exit_codes() {
        let dir = scratch_dir("lifecycle");
        let diagram = write_diagram(&dir, "d.json");
        let output = dir.join("out.dataset");
        let project = write_project(&dir, &diagram, output.to_str().unwrap());
        let out_str = output.to_str().unwrap();

        // Success path.
        assert_eq!(dispatch(&args(&["run", &project])).code, ExitCode::Success);
        assert_eq!(
            dispatch(&args(&["status", out_str])).code,
            ExitCode::Success
        );
        assert_eq!(
            dispatch(&args(&["results", out_str])).code,
            ExitCode::Success
        );

        // Cancel yields the cancelled exit code and a resume point.
        let cancelled = dispatch(&args(&["cancel", out_str, "0.3"]));
        assert_eq!(cancelled.code, ExitCode::Cancelled);
        assert_eq!(cancelled.status["resume_time"].as_f64().unwrap(), 0.3);

        // Status now reports cancellation.
        let status = dispatch(&args(&["status", out_str]));
        assert_eq!(status.code, ExitCode::Cancelled);
        assert_eq!(status.status["cancelled"], true);

        // Resume completes.
        assert_eq!(
            dispatch(&args(&["resume", &project])).code,
            ExitCode::Success
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatch_run_with_bad_project_is_invalid_input() {
        let dir = scratch_dir("badproj");
        std::fs::write(dir.join("project.json"), "{").unwrap();
        let outcome = dispatch(&args(&["run", dir.join("project.json").to_str().unwrap()]));
        assert_eq!(outcome.code, ExitCode::InvalidInput);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatch_validate_subcommands() {
        let dir = scratch_dir("validates");
        let diagram = write_diagram(&dir, "d.json");
        assert_eq!(
            dispatch(&args(&["validate", "model", &diagram])).code,
            ExitCode::Success
        );

        std::fs::write(
            dir.join("p.json"),
            r#"{"name":"p","version":"1.0","api_version":"1.0","entry_point":"libp.so"}"#,
        )
        .unwrap();
        assert_eq!(
            dispatch(&args(&[
                "validate",
                "plugin",
                dir.join("p.json").to_str().unwrap(),
                "1.0"
            ]))
            .code,
            ExitCode::Success
        );
        // Incompatible host version.
        assert_eq!(
            dispatch(&args(&[
                "validate",
                "plugin",
                dir.join("p.json").to_str().unwrap(),
                "2.0"
            ]))
            .code,
            ExitCode::InvalidInput
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_main_returns_the_exit_code() {
        assert_eq!(cli_main(&args(&["version"])), 0);
        assert_eq!(cli_main(&args(&["nope"])), 2);
    }

    #[test]
    fn help_lists_supported_commands() {
        let help = execute(&CliCommand::Help);
        assert_eq!(help.code, ExitCode::Success);
        let commands = help.status["commands"].as_array().unwrap();
        assert!(commands.iter().any(|c| c == "run <project.json>"));
        assert!(commands.iter().any(|c| c == "status <dataset_dir>"));
    }
}
