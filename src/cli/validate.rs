// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Validation commands for models, libraries, plugins and result datasets.
//!
//! Every command here performs a *real* validation using the crate's own
//! loaders rather than a syntactic approximation: a model is validated by
//! deserializing it and building an engine, a plugin by running its manifest
//! through the compatibility policy, a dataset by opening it through the
//! verified reader. Each returns a [`CommandOutcome`] with a stable exit code.

use crate::bindings::plugins::compatibility::{CompatibilityPolicy, compatibility_of_manifest};
use crate::bindings::plugins::{PluginManager, PluginManifest};
use crate::cli::project::{CommandOutcome, ExitCode};
use crate::postproc::dataset::{DatasetReadError, DatasetReader, is_readable, read_metadata};

/// Validate a diagram/model document and report its structure.
///
/// Delegates to [`crate::cli::project::validate_model`] so the same real
/// validation is used by both `run` and `validate model`.
pub fn validate_model(diagram_path: &str) -> CommandOutcome {
    match crate::cli::project::validate_model(diagram_path) {
        Ok(info) => CommandOutcome::success(
            format!("model '{}' is valid", diagram_path),
            serde_json::json!({
                "status": "valid",
                "kind": "model",
                "detail": info,
            }),
        ),
        Err(e) if e.contains("cannot read") => CommandOutcome::failure(ExitCode::IoError, e),
        Err(e) => CommandOutcome::failure(ExitCode::InvalidInput, e),
    }
}

/// Validate a library directory: every `.json`/`.toml` manifest it contains must
/// parse as a [`PluginManifest`] with a non-empty name and version.
///
/// Returns [`ExitCode::InvalidInput`] listing the offending entries when any
/// manifest is malformed; success only when all entries are valid.
pub fn validate_library(library_dir: &str) -> CommandOutcome {
    let mut manager = PluginManager::new();
    let entries = match manager.load_from_directory(library_dir) {
        Ok(e) => e,
        Err(e) => return CommandOutcome::failure(ExitCode::IoError, e),
    };

    let mut valid = Vec::new();
    let mut errors = Vec::new();
    for path in entries {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(serde_json::json!({ "entry": path, "error": e.to_string() }));
                continue;
            }
        };
        let parsed = if path.ends_with(".toml") {
            PluginManifest::from_toml(&text)
        } else {
            PluginManifest::from_json(&text)
        };
        match parsed {
            Ok(m) => valid.push(m.name),
            Err(e) => errors.push(serde_json::json!({ "entry": path, "error": e })),
        }
    }

    if errors.is_empty() {
        CommandOutcome::success(
            format!(
                "library '{}': {} manifest(s) valid",
                library_dir,
                valid.len()
            ),
            serde_json::json!({
                "status": "valid",
                "kind": "library",
                "manifests": valid,
                "count": valid.len(),
            }),
        )
    } else {
        CommandOutcome::failure(
            ExitCode::InvalidInput,
            format!(
                "library '{}': {} invalid manifest(s)",
                library_dir,
                errors.len()
            ),
        )
        .with_detail(errors)
    }
}

/// Validate a plugin manifest against the compatibility policy.
///
/// Checks: the manifest parses, declares an entry point, and its declared API
/// version is compatible with the host API. Returns the parsed compatibility
/// decision in the status body so a caller can see *why* a plugin is rejected.
pub fn validate_plugin(manifest_path: &str, host_api: ApiVersionSpec) -> CommandOutcome {
    let text = match std::fs::read_to_string(manifest_path) {
        Ok(t) => t,
        Err(e) => {
            return CommandOutcome::failure(
                ExitCode::IoError,
                format!("cannot read '{}': {}", manifest_path, e),
            );
        }
    };
    let manifest = if manifest_path.ends_with(".toml") {
        PluginManifest::from_toml(&text)
    } else {
        PluginManifest::from_json(&text)
    };
    let manifest = match manifest {
        Ok(m) => m,
        Err(e) => return CommandOutcome::failure(ExitCode::InvalidInput, e),
    };

    let host = host_api.to_version();
    let policy = CompatibilityPolicy::default();
    let decision = compatibility_of_manifest(&manifest, host, &policy);

    let body = serde_json::json!({
        "status": if decision.is_compatible() { "compatible" } else { "incompatible" },
        "kind": "plugin",
        "plugin": manifest.name,
        "version": manifest.version,
        "entry_point": manifest.entry_point,
        "api_version": manifest.api_version,
        "reason": decision.reason(),
    });
    if decision.is_compatible() {
        CommandOutcome::success(format!("plugin '{}' is compatible", manifest.name), body)
    } else {
        CommandOutcome::new(
            ExitCode::InvalidInput,
            format!(
                "plugin '{}' is incompatible: {}",
                manifest.name,
                decision.reason()
            ),
            body,
        )
    }
}

/// A small CLI-facing API version specification (`major.minor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiVersionSpec {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
}

impl ApiVersionSpec {
    /// Create a version spec.
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Convert into the plugin compatibility layer's version type.
    pub fn to_version(self) -> crate::bindings::plugins::compatibility::ApiVersion {
        crate::bindings::plugins::compatibility::ApiVersion::new(self.major, self.minor)
    }

    /// Parse `"major.minor"`, rejecting anything else.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut parts = text.split('.');
        let major = parts
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| format!("invalid API version '{}': expected 'major.minor'", text))?;
        let minor = parts
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| format!("invalid API version '{}': expected 'major.minor'", text))?;
        if parts.next().is_some() {
            return Err(format!(
                "invalid API version '{}': too many components",
                text
            ));
        }
        Ok(Self { major, minor })
    }
}

/// Validate a result dataset directory using the verified reader.
///
/// A missing completion marker is reported as [`ExitCode::Cancelled`] (a run
/// that never finished), a schema/checksum failure as [`ExitCode::InvalidInput`].
pub fn validate_dataset(dataset_dir: &str) -> CommandOutcome {
    if !std::path::Path::new(dataset_dir).is_dir() {
        return CommandOutcome::failure(
            ExitCode::NotFound,
            format!("'{}' is not a dataset directory", dataset_dir),
        );
    }
    match DatasetReader::open(dataset_dir) {
        Ok(reader) => {
            let m = reader.manifest();
            CommandOutcome::success(
                format!(
                    "dataset '{}' is valid: {} row(s)",
                    dataset_dir,
                    reader.row_count()
                ),
                serde_json::json!({
                    "status": "valid",
                    "kind": "dataset",
                    "simulation_id": m.simulation_id,
                    "run_status": m.status.name(),
                    "rows": reader.row_count(),
                    "chunks": m.chunks.len(),
                    "schema_version": reader.schema().schema_version,
                }),
            )
        }
        Err(DatasetReadError::Incomplete(m)) => CommandOutcome::failure(ExitCode::Cancelled, m),
        Err(DatasetReadError::Io(m)) => CommandOutcome::failure(ExitCode::IoError, m),
        Err(e) => CommandOutcome::failure(ExitCode::InvalidInput, e.to_string()),
    }
}

/// Report a dataset's schema version and whether it is readable by this build.
///
/// Uses [`read_metadata`] and [`is_readable`] from the migration layer so the
/// answer reflects the real, supported version range rather than a guess.
pub fn validate_dataset_compatibility(dataset_dir: &str) -> CommandOutcome {
    match read_metadata(dataset_dir) {
        Ok((schema, manifest)) => {
            let readable = is_readable(schema.schema_version);
            let body = serde_json::json!({
                "status": if readable { "readable" } else { "unreadable" },
                "kind": "dataset-compatibility",
                "schema_version": schema.schema_version,
                "readable": readable,
                "simulation_id": manifest.simulation_id,
            });
            if readable {
                CommandOutcome::success(
                    format!("dataset schema v{} is readable", schema.schema_version),
                    body,
                )
            } else {
                CommandOutcome::new(
                    ExitCode::InvalidInput,
                    format!(
                        "dataset schema v{} is not supported by this build",
                        schema.schema_version
                    ),
                    body,
                )
            }
        }
        Err(e) => CommandOutcome::failure(ExitCode::IoError, e.to_string()),
    }
}

impl CommandOutcome {
    /// Attach a structured `detail` array to a failed outcome's status body.
    pub fn with_detail(mut self, detail: Vec<serde_json::Value>) -> Self {
        if let Some(obj) = self.status.as_object_mut() {
            obj.insert("detail".into(), serde_json::Value::Array(detail));
        }
        self
    }
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
            "scico_cli_validate_{}_{}_{}",
            tag,
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_diagram(dir: &std::path::Path, name: &str) -> String {
        use crate::core::block::SimpleBlock;
        use crate::core::diagram::Diagram;
        let mut diagram = Diagram::new("v");
        diagram.add_block(Box::new(SimpleBlock::new("b1", "Source")));
        let json = crate::core::diagram_ser::diagram_to_json(&diagram).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, json).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    fn validate_model_ok_and_bad() {
        let dir = scratch_dir("model");
        let good = write_diagram(&dir, "d.json");
        assert_eq!(validate_model(&good).code, ExitCode::Success);

        std::fs::write(dir.join("bad.json"), "{ not json }").unwrap();
        assert_eq!(
            validate_model(dir.join("bad.json").to_str().unwrap()).code,
            ExitCode::InvalidInput
        );
        assert_eq!(
            validate_model(dir.join("missing.json").to_str().unwrap()).code,
            ExitCode::IoError
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_library_reports_valid_and_invalid_manifests() {
        let dir = scratch_dir("library");
        std::fs::write(
            dir.join("ok.json"),
            r#"{"name":"p","version":"1.0","entry_point":"libp.so"}"#,
        )
        .unwrap();
        let ok = validate_library(dir.to_str().unwrap());
        assert_eq!(ok.code, ExitCode::Success, "{}", ok.message);
        assert_eq!(ok.status["count"].as_u64().unwrap(), 1);

        std::fs::write(dir.join("bad.json"), "{}").unwrap();
        let bad = validate_library(dir.to_str().unwrap());
        assert_eq!(bad.code, ExitCode::InvalidInput);
        assert!(bad.status["detail"].as_array().unwrap().len() == 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_library_missing_directory_is_io_error() {
        let dir = scratch_dir("library_missing");
        let outcome = validate_library(dir.join("nope").to_str().unwrap());
        assert_eq!(outcome.code, ExitCode::IoError);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_version_spec_parses_and_rejects() {
        assert_eq!(
            ApiVersionSpec::parse("1.2").unwrap(),
            ApiVersionSpec::new(1, 2)
        );
        assert!(ApiVersionSpec::parse("1").is_err());
        assert!(ApiVersionSpec::parse("1.2.3").is_err());
        assert!(ApiVersionSpec::parse("x.y").is_err());
    }

    #[test]
    fn validate_plugin_accepts_compatible_and_rejects_missing_entry() {
        let dir = scratch_dir("plugin");
        let host = ApiVersionSpec::new(1, 0);

        let good = dir.join("good.json");
        std::fs::write(
            &good,
            r#"{"name":"p","version":"1.0","api_version":"1.0","entry_point":"libp.so"}"#,
        )
        .unwrap();
        let ok = validate_plugin(good.to_str().unwrap(), host);
        assert_eq!(ok.code, ExitCode::Success, "{}", ok.message);
        assert_eq!(ok.status["status"], "compatible");

        // Missing entry point -> incompatible/rejected (missing entry reason).
        let no_entry = dir.join("noentry.json");
        std::fs::write(
            &no_entry,
            r#"{"name":"q","version":"1.0","api_version":"1.0"}"#,
        )
        .unwrap();
        let rejected = validate_plugin(no_entry.to_str().unwrap(), host);
        assert_eq!(rejected.code, ExitCode::InvalidInput);
        assert_eq!(rejected.status["status"], "incompatible");

        // Version mismatch.
        let wrong = dir.join("wrong.json");
        std::fs::write(
            &wrong,
            r#"{"name":"r","version":"1.0","api_version":"2.0","entry_point":"libr.so"}"#,
        )
        .unwrap();
        assert_eq!(
            validate_plugin(wrong.to_str().unwrap(), host).code,
            ExitCode::InvalidInput
        );

        assert_eq!(
            validate_plugin(dir.join("absent.json").to_str().unwrap(), host).code,
            ExitCode::IoError
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_dataset_accepts_complete_and_flags_incomplete() {
        use crate::postproc::dataset::{DatasetManifest, DatasetSchema, DatasetWriter, Row};

        let complete_dir = scratch_dir("ds_complete");
        let mut schema = DatasetSchema::with_uniform_time("s", 1, 0.1);
        schema
            .add_variable(
                crate::postproc::dataset::ResultVariable::scalar("x", "temperature", "K")
                    .with_time_axis("time"),
            )
            .unwrap();
        let mut w =
            DatasetWriter::create(&complete_dir, schema, DatasetManifest::new("sim")).unwrap();
        let mut row = Row::at(0.0);
        row.set("x", Some(300.0));
        w.write_chunk(vec![row]).unwrap();
        w.complete();
        w.finish().unwrap();
        assert_eq!(
            validate_dataset(complete_dir.to_str().unwrap()).code,
            ExitCode::Success
        );
        assert_eq!(
            validate_dataset_compatibility(complete_dir.to_str().unwrap()).code,
            ExitCode::Success
        );

        let incomplete_dir = scratch_dir("ds_incomplete");
        let mut w2 = DatasetWriter::create(
            &incomplete_dir,
            DatasetSchema::with_uniform_time("s", 1, 0.1),
            DatasetManifest::new("sim2"),
        )
        .unwrap();
        w2.write_chunk(vec![Row::at(0.0)]).unwrap();
        drop(w2);
        assert_eq!(
            validate_dataset(incomplete_dir.to_str().unwrap()).code,
            ExitCode::Cancelled
        );

        let _ = std::fs::remove_dir_all(&complete_dir);
        let _ = std::fs::remove_dir_all(&incomplete_dir);
    }

    #[test]
    fn validate_dataset_missing_is_not_found_style_io_error() {
        let dir = scratch_dir("ds_missing");
        let outcome = validate_dataset(dir.join("nope").to_str().unwrap());
        assert_eq!(outcome.code, ExitCode::NotFound);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
