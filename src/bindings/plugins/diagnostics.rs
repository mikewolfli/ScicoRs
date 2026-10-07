// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Plugin load/init/run failure diagnostics.
//!
//! When a plugin fails, the *stage* it failed at and the *plugin it belongs to*
//! are as important as the underlying error. This module captures both in a
//! [`PluginFailure`] record so a failure is always attributable to a specific
//! `(plugin, entry, version, stage)` and carries the underlying error message.
//!
//! # Isolation
//!
//! A failed plugin must not pollute other plugins or global state. Loading is
//! therefore performed into a **staging area** ([`PluginLoader::load`]): the
//! candidate's blocks are collected and only merged into the shared registries
//! once the whole load (resolve → verify → init) has succeeded. When any stage
//! fails, the staged contributions are dropped and nothing is committed, so a
//! broken plugin leaves the host exactly as it was.

use std::collections::BTreeMap;

use super::PluginManifest;
use super::compatibility::{
    CompatDecision, CompatibilityPolicy, ExtendedManifest, check_compatibility,
};

/// The stage at which a plugin operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginStage {
    /// The manifest could not be located or parsed.
    Manifest,
    /// Compatibility/version checking rejected the plugin.
    Compatibility,
    /// The dynamic library could not be opened / the entry symbol is missing.
    Load,
    /// The plugin's `initialize` failed.
    Init,
    /// The plugin's registration or run hook failed.
    Run,
}

impl PluginStage {
    /// Stable name used in error messages and diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            PluginStage::Manifest => "manifest",
            PluginStage::Compatibility => "compatibility",
            PluginStage::Load => "load",
            PluginStage::Init => "init",
            PluginStage::Run => "run",
        }
    }
}

impl std::fmt::Display for PluginStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A structured, attributable plugin failure.
///
/// Every field needed to locate the fault is present: which plugin, its entry
/// point, its declared version, the pipeline stage, and the underlying error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginFailure {
    /// Plugin name (from the manifest; `"<unknown>"` when the manifest could not
    /// even be read).
    pub plugin: String,
    /// Entry point the plugin declared (empty when unknown).
    pub entry_point: String,
    /// Plugin version (empty when unknown).
    pub version: String,
    /// Stage at which the failure occurred.
    pub stage: PluginStage,
    /// The underlying error message.
    pub error: String,
}

impl PluginFailure {
    /// Build a failure record.
    pub fn new(
        plugin: &str,
        entry_point: &str,
        version: &str,
        stage: PluginStage,
        error: impl Into<String>,
    ) -> Self {
        Self {
            plugin: plugin.to_string(),
            entry_point: entry_point.to_string(),
            version: version.to_string(),
            stage,
            error: error.into(),
        }
    }

    /// Build a failure from a manifest and a stage.
    pub fn from_manifest(
        manifest: &PluginManifest,
        stage: PluginStage,
        error: impl Into<String>,
    ) -> Self {
        Self::new(
            &manifest.name,
            &manifest.entry_point,
            &manifest.version,
            stage,
            error,
        )
    }

    /// A single diagnostic line: `"plugin 'p' (v1.0, entry 'libp.so') failed at
    /// load: <error>"`.
    pub fn diagnostic(&self) -> String {
        format!(
            "plugin '{}' (v{}, entry '{}') failed at {}: {}",
            self.plugin,
            if self.version.is_empty() {
                "?"
            } else {
                &self.version
            },
            if self.entry_point.is_empty() {
                "<none>"
            } else {
                &self.entry_point
            },
            self.stage,
            self.error
        )
    }

    /// Structured, machine-readable representation of the failure.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "plugin": self.plugin,
            "version": self.version,
            "entry_point": self.entry_point,
            "stage": self.stage.name(),
            "error": self.error,
        })
    }
}

impl std::fmt::Display for PluginFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.diagnostic())
    }
}

impl std::error::Error for PluginFailure {}

/// A staged set of contributions a plugin would make to the host.
///
/// Nothing here touches shared state until the load succeeds: the loader builds
/// a `PluginContribution`, and only [`PluginLoader::commit`] merges it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginContribution {
    /// Block type names the plugin registers.
    pub block_types: Vec<String>,
    /// Solver names the plugin registers.
    pub solvers: Vec<String>,
    /// Post-processor names the plugin registers.
    pub postprocessors: Vec<String>,
}

impl PluginContribution {
    /// Whether the contribution is empty (no registrations).
    pub fn is_empty(&self) -> bool {
        self.block_types.is_empty() && self.solvers.is_empty() && self.postprocessors.is_empty()
    }

    /// Total number of registered names across all categories.
    pub fn len(&self) -> usize {
        self.block_types.len() + self.solvers.len() + self.postprocessors.len()
    }
}

/// The result of a successful staged load: a contribution plus a deprecation
/// decision if applicable.
#[derive(Debug, Clone, PartialEq)]
pub struct StagedPlugin {
    /// The plugin's name.
    pub name: String,
    /// The staged contribution (not yet committed).
    pub contribution: PluginContribution,
    /// The compatibility decision reached during loading.
    pub decision: CompatDecision,
}

/// Drives the load pipeline and enforces isolation.
///
/// The loader owns a *shared* registry view (the committed state) and produces
/// staged contributions. A stage failure yields a [`PluginFailure`] and leaves
/// the committed state untouched.
#[derive(Debug, Default)]
pub struct PluginLoader {
    /// Committed contributions, keyed by plugin name.
    committed: BTreeMap<String, PluginContribution>,
    /// All failures observed so far, in order.
    failures: Vec<PluginFailure>,
    /// Non-fatal notices (e.g. deprecation migration notes), in load order.
    notices: Vec<String>,
}

impl PluginLoader {
    /// Create an empty loader.
    pub fn new() -> Self {
        Self {
            committed: BTreeMap::new(),
            failures: Vec::new(),
            notices: Vec::new(),
        }
    }

    /// The set of committed block type names (the host's live registry view).
    pub fn committed_block_types(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self
            .committed
            .values()
            .flat_map(|c| c.block_types.iter().map(|s| s.as_str()))
            .collect();
        out.sort_unstable();
        out
    }

    /// Names of the plugins that were successfully committed.
    pub fn loaded_plugins(&self) -> Vec<&str> {
        self.committed.keys().map(|s| s.as_str()).collect()
    }

    /// All failures observed, in order.
    pub fn failures(&self) -> &[PluginFailure] {
        &self.failures
    }

    /// Non-fatal notices recorded during loading (e.g. deprecation notes).
    pub fn notices(&self) -> &[String] {
        &self.notices
    }

    /// The most recent failure, if any.
    pub fn last_failure(&self) -> Option<&PluginFailure> {
        self.failures.last()
    }

    /// Load a plugin described by `manifest`, using `supplier` to provide the
    /// staged contribution and the init/run hooks.
    ///
    /// Pipeline: compatibility → staged contribution → init → registration.
    /// The first stage that fails records a [`PluginFailure`] with the stage,
    /// and nothing is committed. On success the contribution is merged into the
    /// committed state atomically.
    pub fn load<F>(
        &mut self,
        manifest: &ExtendedManifest,
        policy: &CompatibilityPolicy,
        supplier: F,
    ) -> Result<(), PluginFailure>
    where
        F: FnOnce()
            -> Result<(PluginContribution, Box<dyn FnOnce() -> Result<(), String>>), String>,
    {
        // Stage 1: compatibility.
        let decision = check_compatibility(manifest, policy);
        if let CompatDecision::Incompatible { reason } = &decision {
            let failure = PluginFailure::from_manifest(
                &manifest.base,
                PluginStage::Compatibility,
                reason.clone(),
            );
            self.failures.push(failure.clone());
            return Err(failure);
        }

        // Stage 2: produce the staged contribution (may itself fail, e.g. a
        // missing entry symbol is reported here).
        let (contribution, init) = match supplier() {
            Ok(pair) => pair,
            Err(e) => {
                let failure = PluginFailure::from_manifest(&manifest.base, PluginStage::Load, e);
                self.failures.push(failure.clone());
                return Err(failure);
            }
        };

        // Stage 3: init hook.
        if let Err(e) = init() {
            let failure = PluginFailure::from_manifest(&manifest.base, PluginStage::Init, e);
            self.failures.push(failure.clone());
            return Err(failure);
        }

        // Commit atomically: only now does shared state change.
        self.committed
            .insert(manifest.base.name.clone(), contribution.clone());
        // A compatible-but-deprecated decision is non-fatal, but its migration
        // note must be surfaced rather than silently dropped.
        if let CompatDecision::Deprecated { note, .. } = &decision {
            self.notices
                .push(format!("plugin '{}': {}", manifest.base.name, note));
        }
        Ok(())
    }

    /// Run a plugin's registered operation, attributing any failure to the
    /// `run` stage. A failure is recorded and returned, but does not roll back
    /// the already-committed contribution of *other* plugins.
    pub fn run_plugin<F>(
        &mut self,
        manifest: &PluginManifest,
        op: F,
    ) -> Result<String, PluginFailure>
    where
        F: FnOnce() -> Result<String, String>,
    {
        match op() {
            Ok(v) => Ok(v),
            Err(e) => {
                let failure = PluginFailure::from_manifest(manifest, PluginStage::Run, e);
                self.failures.push(failure.clone());
                Err(failure)
            }
        }
    }

    /// Drop the record of the most recent failure (used by a caller that has
    /// handled it). Committed state is unaffected.
    pub fn clear_last_failure(&mut self) -> Option<PluginFailure> {
        self.failures.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::plugins::PluginManifest;
    use crate::bindings::plugins::compatibility::ApiVersion;

    fn manifest(name: &str, api: &str, entry: &str) -> PluginManifest {
        PluginManifest {
            name: name.to_string(),
            version: "1.0".to_string(),
            author: "a".to_string(),
            description: String::new(),
            api_version: api.to_string(),
            entry_point: entry.to_string(),
        }
    }

    fn contribution(block: &str) -> PluginContribution {
        PluginContribution {
            block_types: vec![block.to_string()],
            solvers: Vec::new(),
            postprocessors: Vec::new(),
        }
    }

    #[test]
    fn failure_diagnostic_names_plugin_entry_version_stage() {
        let f = PluginFailure::new(
            "thermal",
            "libthermal.so",
            "2.1",
            PluginStage::Init,
            "device not ready",
        );
        let d = f.diagnostic();
        assert!(d.contains("thermal"));
        assert!(d.contains("libthermal.so"));
        assert!(d.contains("2.1"));
        assert!(d.contains("init"));
        assert!(d.contains("device not ready"));
        assert_eq!(f.to_json()["stage"], "init");
    }

    #[test]
    fn failure_from_manifest_copies_identity_fields() {
        let f = PluginFailure::from_manifest(
            &manifest("p", "1.0", "libp.so"),
            PluginStage::Load,
            "symbol missing",
        );
        assert_eq!(f.plugin, "p");
        assert_eq!(f.entry_point, "libp.so");
        assert_eq!(f.version, "1.0");
        assert_eq!(f.stage, PluginStage::Load);
    }

    #[test]
    fn successful_load_commits_contribution() {
        let mut loader = PluginLoader::new();
        let m = ExtendedManifest::from_base(manifest("p", "1.0", "libp.so"));
        let result = loader.load(&m, &CompatibilityPolicy::default(), || {
            Ok((contribution("Constant"), Box::new(|| Ok(()))))
        });
        assert!(result.is_ok());
        assert_eq!(loader.loaded_plugins(), vec!["p"]);
        assert_eq!(loader.committed_block_types(), vec!["Constant"]);
        assert!(loader.failures().is_empty());
    }

    #[test]
    fn deprecation_note_is_surfaced_as_a_notice() {
        // A compatible-but-deprecated plugin loads successfully *and* its
        // migration note is recorded (not silently dropped).
        let mut loader = PluginLoader::new();
        let m = ExtendedManifest::from_base(manifest("old", "1.2", "libold.so"));
        // Host API 1.3 accepts plugin API 1.2; minor 2 is deprecated with a note.
        let policy = CompatibilityPolicy::new(ApiVersion::new(1, 3))
            .deprecate_minor(2, "migrate to api 2.0");
        let result = loader.load(&m, &policy, || {
            Ok((contribution("Legacy"), Box::new(|| Ok(()))))
        });
        assert!(result.is_ok());
        assert_eq!(loader.loaded_plugins(), vec!["old"]);
        assert_eq!(loader.notices().len(), 1, "notices={:?}", loader.notices());
        assert!(loader.notices()[0].contains("old"));
        assert!(loader.notices()[0].contains("migrate to api 2.0"));
    }

    #[test]
    fn version_mismatch_fails_at_compatibility_and_commits_nothing() {
        let mut loader = PluginLoader::new();
        let m = ExtendedManifest::from_base(manifest("bad", "2.0", "libbad.so"));
        let err = loader
            .load(&m, &CompatibilityPolicy::default(), || {
                Ok((contribution("X"), Box::new(|| Ok(()))))
            })
            .unwrap_err();
        assert_eq!(err.stage, PluginStage::Compatibility);
        assert!(loader.loaded_plugins().is_empty());
        assert!(loader.committed_block_types().is_empty());
    }

    #[test]
    fn missing_entry_fails_at_load_stage() {
        let mut loader = PluginLoader::new();
        let m = ExtendedManifest::from_base(manifest("p", "1.0", "libp.so"));
        let err = loader
            .load(&m, &CompatibilityPolicy::default(), || {
                Err("entry symbol 'plugin_init' not found".to_string())
            })
            .unwrap_err();
        assert_eq!(err.stage, PluginStage::Load);
        assert!(err.error.contains("not found"));
        assert!(loader.loaded_plugins().is_empty());
    }

    #[test]
    fn init_error_fails_at_init_stage_and_leaves_host_clean() {
        let mut loader = PluginLoader::new();
        let m = ExtendedManifest::from_base(manifest("p", "1.0", "libp.so"));
        let err = loader
            .load(&m, &CompatibilityPolicy::default(), || {
                Ok((
                    contribution("X"),
                    Box::new(|| Err("init exploded".to_string())),
                ))
            })
            .unwrap_err();
        assert_eq!(err.stage, PluginStage::Init);
        // The staged contribution must NOT have been committed.
        assert!(loader.committed_block_types().is_empty());
        assert!(loader.loaded_plugins().is_empty());
    }

    #[test]
    fn a_failed_plugin_does_not_pollute_a_later_successful_one() {
        let mut loader = PluginLoader::new();
        let policy = CompatibilityPolicy::default();

        // First plugin fails at init.
        let bad = ExtendedManifest::from_base(manifest("bad", "1.0", "libbad.so"));
        let _ = loader.load(&bad, &policy, || {
            Ok((contribution("Bad"), Box::new(|| Err("boom".into()))))
        });

        // Second plugin succeeds and is the only one committed.
        let good = ExtendedManifest::from_base(manifest("good", "1.0", "libgood.so"));
        assert!(
            loader
                .load(&good, &policy, || {
                    Ok((contribution("Good"), Box::new(|| Ok(()))))
                })
                .is_ok()
        );
        assert_eq!(loader.loaded_plugins(), vec!["good"]);
        assert_eq!(loader.committed_block_types(), vec!["Good"]);
        assert_eq!(loader.failures().len(), 1);
        assert_eq!(loader.last_failure().unwrap().plugin, "bad");
    }

    #[test]
    fn run_error_is_attributed_to_the_run_stage() {
        let mut loader = PluginLoader::new();
        let m = manifest("p", "1.0", "libp.so");
        let err = loader
            .run_plugin(&m, || Err("stack overflow".to_string()))
            .unwrap_err();
        assert_eq!(err.stage, PluginStage::Run);
        assert_eq!(err.plugin, "p");

        let ok = loader.run_plugin(&m, || Ok("fine".to_string())).unwrap();
        assert_eq!(ok, "fine");
    }

    #[test]
    fn failure_record_can_be_cleared_without_touching_state() {
        let mut loader = PluginLoader::new();
        let m = manifest("p", "1.0", "libp.so");
        let _ = loader.run_plugin(&m, || Err("x".into()));
        assert_eq!(loader.failures().len(), 1);
        assert!(loader.clear_last_failure().is_some());
        assert!(loader.failures().is_empty());
    }

    #[test]
    fn contribution_size_accounting() {
        let c = PluginContribution {
            block_types: vec!["a".into(), "b".into()],
            solvers: vec!["s".into()],
            postprocessors: Vec::new(),
        };
        assert_eq!(c.len(), 3);
        assert!(!c.is_empty());
        assert!(PluginContribution::default().is_empty());
    }

    #[test]
    fn deprecated_but_compatible_plugin_still_loads() {
        let mut loader = PluginLoader::new();
        let policy = CompatibilityPolicy::default().deprecate_minor(0, "upgrade to 1.1");
        let m = ExtendedManifest::from_base(manifest("legacy", "1.0", "libl.so"));
        assert!(
            loader
                .load(&m, &policy, || {
                    Ok((contribution("L"), Box::new(|| Ok(()))))
                })
                .is_ok()
        );
        assert_eq!(loader.loaded_plugins(), vec!["legacy"]);
    }
}
