// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Checkpoint compatibility checks (Phase 38).
//!
//! Before restoring, a checkpoint's model/solver/plugin signature is compared
//! against the running configuration. Incompatible checkpoints are **rejected**
//! with an explicit list of differences; solver history and events are never
//! silently discarded in order to pretend a resume succeeded.

use super::format::{CheckpointManifest, ModelSignature};
use super::snapshot::SimulationSnapshot;

/// The outcome of a compatibility check.
#[derive(Debug, Clone, PartialEq)]
pub enum Compatibility {
    /// The checkpoint can be restored exactly.
    Compatible,
    /// The checkpoint can be restored but with documented, acceptable drift
    /// (e.g. a newer library patch version with the same major API).
    CompatibleWithWarning(Vec<String>),
    /// The checkpoint must be rejected.
    Incompatible(Vec<String>),
}

impl Compatibility {
    /// Whether restoration may proceed (compatible, possibly with warnings).
    pub fn can_restore(&self) -> bool {
        !matches!(self, Self::Incompatible(_))
    }

    /// The list of differences, empty when fully compatible.
    pub fn differences(&self) -> &[String] {
        match self {
            Self::Compatible => &[],
            Self::CompatibleWithWarning(d) | Self::Incompatible(d) => d,
        }
    }
}

/// Policy controlling how strictly checkpoints are matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityPolicy {
    /// Require the model hash to match exactly.
    pub require_model_hash: bool,
    /// Require the solver identifier to match exactly.
    pub require_solver: bool,
    /// Require all recorded plugin versions to match exactly.
    pub require_plugins: bool,
    /// Accept a differing library version whose major component is unchanged.
    pub allow_minor_library_drift: bool,
}

impl Default for CompatibilityPolicy {
    fn default() -> Self {
        Self {
            require_model_hash: true,
            require_solver: true,
            require_plugins: true,
            allow_minor_library_drift: true,
        }
    }
}

/// Extract the leading numeric major version from a dotted version string.
fn major_of(version: &str) -> Option<u64> {
    version
        .split('.')
        .next()
        .and_then(|s| s.trim().parse::<u64>().ok())
}

/// Check a manifest's signature against the current signature under `policy`.
pub fn check_manifest_compatibility(
    manifest: &CheckpointManifest,
    current: &ModelSignature,
    policy: &CompatibilityPolicy,
) -> Compatibility {
    check_signature_compatibility(&manifest.signature, current, policy)
}

/// Check a snapshot's embedded signature against the current signature.
pub fn check_snapshot_compatibility(
    snapshot: &SimulationSnapshot,
    current: &ModelSignature,
    policy: &CompatibilityPolicy,
) -> Compatibility {
    check_signature_compatibility(&snapshot.signature, current, policy)
}

/// Core compatibility comparison.
pub fn check_signature_compatibility(
    checkpoint: &ModelSignature,
    current: &ModelSignature,
    policy: &CompatibilityPolicy,
) -> Compatibility {
    let mut hard: Vec<String> = Vec::new();
    let mut soft: Vec<String> = Vec::new();

    if policy.require_model_hash && checkpoint.model_hash != current.model_hash {
        hard.push(format!(
            "model hash mismatch: checkpoint={:#x} current={:#x}",
            checkpoint.model_hash, current.model_hash
        ));
    }
    if checkpoint.model_name != current.model_name {
        hard.push(format!(
            "model name mismatch: checkpoint='{}' current='{}'",
            checkpoint.model_name, current.model_name
        ));
    }
    if policy.require_solver && checkpoint.solver != current.solver {
        hard.push(format!(
            "solver mismatch: checkpoint='{}' current='{}'",
            checkpoint.solver, current.solver
        ));
    }

    // Library version: exact match is fine; a differing version is a hard error
    // unless the major component matches and minor drift is allowed.
    if checkpoint.library_version != current.library_version {
        let same_major = matches!(
            (major_of(&checkpoint.library_version), major_of(&current.library_version)),
            (Some(a), Some(b)) if a == b
        );
        if policy.allow_minor_library_drift && same_major {
            soft.push(format!(
                "library version drift: checkpoint='{}' current='{}' (same major)",
                checkpoint.library_version, current.library_version
            ));
        } else {
            hard.push(format!(
                "library version mismatch: checkpoint='{}' current='{}'",
                checkpoint.library_version, current.library_version
            ));
        }
    }

    // Plugins.
    if policy.require_plugins {
        for (name, ver) in &checkpoint.plugin_versions {
            match current.plugin_versions.get(name) {
                Some(cur) if cur == ver => {}
                Some(cur) => hard.push(format!(
                    "plugin '{name}' version mismatch: checkpoint='{ver}' current='{cur}'"
                )),
                None => hard.push(format!(
                    "plugin '{name}' required by checkpoint ('{ver}') is not loaded"
                )),
            }
        }
        for name in current.plugin_versions.keys() {
            if !checkpoint.plugin_versions.contains_key(name) {
                soft.push(format!(
                    "plugin '{name}' is loaded now but absent from checkpoint"
                ));
            }
        }
    }

    if !hard.is_empty() {
        Compatibility::Incompatible(hard)
    } else if !soft.is_empty() {
        Compatibility::CompatibleWithWarning(soft)
    } else {
        Compatibility::Compatible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_sig() -> ModelSignature {
        ModelSignature {
            model_name: "mdl".to_string(),
            model_hash: 100,
            solver: "rk4".to_string(),
            library_version: "1.4.0".to_string(),
            plugin_versions: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn identical_is_compatible() {
        let s = base_sig();
        let p = CompatibilityPolicy::default();
        assert_eq!(
            check_signature_compatibility(&s, &s, &p),
            Compatibility::Compatible
        );
    }

    #[test]
    fn model_hash_mismatch_is_hard() {
        let a = base_sig();
        let mut b = base_sig();
        b.model_hash = 999;
        let c = check_signature_compatibility(&a, &b, &CompatibilityPolicy::default());
        assert!(!c.can_restore());
        assert!(c.differences().iter().any(|d| d.contains("model hash")));
    }

    #[test]
    fn solver_mismatch_is_hard() {
        let a = base_sig();
        let mut b = base_sig();
        b.solver = "bdf2".to_string();
        let c = check_signature_compatibility(&a, &b, &CompatibilityPolicy::default());
        assert!(!c.can_restore());
    }

    #[test]
    fn minor_library_drift_allowed_same_major() {
        let a = base_sig();
        let mut b = base_sig();
        b.library_version = "1.5.2".to_string();
        let c = check_signature_compatibility(&a, &b, &CompatibilityPolicy::default());
        assert!(c.can_restore());
        assert!(matches!(c, Compatibility::CompatibleWithWarning(_)));
    }

    #[test]
    fn major_library_mismatch_is_hard() {
        let a = base_sig();
        let mut b = base_sig();
        b.library_version = "2.0.0".to_string();
        let c = check_signature_compatibility(&a, &b, &CompatibilityPolicy::default());
        assert!(!c.can_restore());
    }

    #[test]
    fn plugin_mismatch_is_hard() {
        let mut a = base_sig();
        a.plugin_versions
            .insert("optics".to_string(), "1.0".to_string());
        let b = base_sig(); // current has no optics plugin
        let c = check_signature_compatibility(&a, &b, &CompatibilityPolicy::default());
        assert!(!c.can_restore());
        assert!(c.differences().iter().any(|d| d.contains("optics")));
    }

    #[test]
    fn extra_current_plugin_is_soft() {
        let a = base_sig();
        let mut b = base_sig();
        b.plugin_versions
            .insert("thermal".to_string(), "2.0".to_string());
        let c = check_signature_compatibility(&a, &b, &CompatibilityPolicy::default());
        assert!(c.can_restore());
        assert!(matches!(c, Compatibility::CompatibleWithWarning(_)));
    }

    #[test]
    fn relaxed_policy_ignores_hash() {
        let a = base_sig();
        let mut b = base_sig();
        b.model_hash = 4242;
        let policy = CompatibilityPolicy {
            require_model_hash: false,
            ..Default::default()
        };
        assert!(check_signature_compatibility(&a, &b, &policy).can_restore());
    }
}
