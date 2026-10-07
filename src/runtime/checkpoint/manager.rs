// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! High-level checkpoint store and run-outcome classification (Phase 38).
//!
//! [`CheckpointStore`] bundles the manifest, snapshot and compatibility policy
//! into a single save/load workflow, computing the payload and hash for the
//! caller. [`RunOutcome`] gives the six distinct end states a long simulation can
//! reach, so a run never reports "success" for a cancelled or divergent task.

use super::compatibility::{Compatibility, CompatibilityPolicy, check_snapshot_compatibility};
use super::format::{
    CheckpointError, CheckpointManifest, ModelSignature, load_checkpoint, write_checkpoint_atomic,
};
use super::snapshot::SimulationSnapshot;
use crate::core::types::Scalar;
use std::path::{Path, PathBuf};

/// The distinct ways a simulation run can end.
///
/// These are deliberately separate: only [`Self::Completed`] is a successful
/// run. Every other variant preserves the last consistent state so a checkpoint
/// can resume the work.
#[derive(Debug, Clone, PartialEq)]
pub enum RunOutcome {
    /// The run reached its end time / all blocks completed.
    Completed {
        /// Final simulation time.
        final_time: Scalar,
        /// Steps executed.
        steps: u64,
    },
    /// The user requested cancellation at a safe stop point.
    Cancelled {
        /// Time at which the run stopped.
        at_time: Scalar,
    },
    /// A resource budget was exceeded.
    ResourceLimit {
        /// Time at which the run stopped.
        at_time: Scalar,
        /// The reason the budget was exceeded.
        detail: String,
    },
    /// The model diverged or encountered a numerical failure.
    NumericalFailure {
        /// Time at which the run stopped.
        at_time: Scalar,
        /// The numerical error detail.
        detail: String,
    },
    /// An I/O error occurred (writing output, reading input).
    IoFailure {
        /// The I/O error detail.
        detail: String,
    },
    /// The run exceeded its wall-clock timeout.
    TimedOut {
        /// Time at which the run stopped.
        at_time: Scalar,
    },
}

impl RunOutcome {
    /// Whether the run completed normally. Only `Completed` returns `true`.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    /// A short, stable status identifier for reporting.
    pub fn status_str(&self) -> &'static str {
        match self {
            Self::Completed { .. } => "completed",
            Self::Cancelled { .. } => "cancelled",
            Self::ResourceLimit { .. } => "resource-limit",
            Self::NumericalFailure { .. } => "numerical-failure",
            Self::IoFailure { .. } => "io-failure",
            Self::TimedOut { .. } => "timeout",
        }
    }

    /// The last consistent simulation time, when the run produced one.
    pub fn last_time(&self) -> Option<Scalar> {
        match self {
            Self::Completed { final_time, .. } => Some(*final_time),
            Self::Cancelled { at_time }
            | Self::ResourceLimit { at_time, .. }
            | Self::NumericalFailure { at_time, .. }
            | Self::TimedOut { at_time } => Some(*at_time),
            Self::IoFailure { .. } => None,
        }
    }
}

/// A store bound to a run directory, tracking the checkpoint sequence.
#[derive(Debug, Clone)]
pub struct CheckpointStore {
    /// Directory holding the checkpoint (one per sequence number is not used;
    /// the current checkpoint is overwritten atomically).
    base_dir: PathBuf,
    /// Signature used for every manifest written by this store.
    signature: ModelSignature,
    /// Compatibility policy applied on load.
    policy: CompatibilityPolicy,
    /// Next sequence number to assign.
    next_sequence: u64,
    /// Maximum accepted payload size on load.
    max_payload_bytes: usize,
}

impl CheckpointStore {
    /// Create a store rooted at `base_dir`.
    pub fn new(base_dir: impl Into<PathBuf>, signature: ModelSignature) -> Self {
        Self {
            base_dir: base_dir.into(),
            signature,
            policy: CompatibilityPolicy::default(),
            next_sequence: 0,
            max_payload_bytes: 512 * 1024 * 1024,
        }
    }

    /// Override the compatibility policy applied on load.
    pub fn with_policy(mut self, policy: CompatibilityPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Override the maximum payload size accepted on load.
    pub fn with_max_payload_bytes(mut self, bytes: usize) -> Self {
        self.max_payload_bytes = bytes;
        self
    }

    /// The directory this store writes into.
    pub fn dir(&self) -> &Path {
        &self.base_dir
    }

    /// Save a snapshot, returning the written manifest.
    pub fn save(
        &mut self,
        snapshot: &SimulationSnapshot,
    ) -> Result<CheckpointManifest, CheckpointError> {
        let payload = snapshot.to_bytes();
        let mut manifest = CheckpointManifest::new(
            self.next_sequence,
            snapshot.sim_time,
            snapshot.step_count,
            self.signature.clone(),
            &payload,
        );
        manifest.rng_seed = snapshot.rng_seed;
        manifest.rng_draws = snapshot.rng_draws;
        write_checkpoint_atomic(&self.base_dir, &manifest, &payload)?;
        self.next_sequence += 1;
        Ok(manifest)
    }

    /// Load the current checkpoint and check compatibility before returning it.
    ///
    /// An incompatible checkpoint is rejected with the differences, so no caller
    /// can silently restore incompatible state.
    pub fn load(&self) -> Result<(CheckpointManifest, SimulationSnapshot), CheckpointError> {
        let (manifest, payload) = load_checkpoint(&self.base_dir, self.max_payload_bytes)?;
        let snapshot = SimulationSnapshot::from_bytes(&payload)?;
        match check_snapshot_compatibility(&snapshot, &self.signature, &self.policy) {
            Compatibility::Compatible => {}
            Compatibility::CompatibleWithWarning(_) => {}
            Compatibility::Incompatible(diffs) => {
                return Err(CheckpointError::Incompatible(diffs.join("; ")));
            }
        }
        Ok((manifest, snapshot))
    }

    /// Check compatibility of an on-disk checkpoint without fully decoding it.
    pub fn check_only(&self) -> Result<Compatibility, CheckpointError> {
        let (manifest, _payload) = load_checkpoint(&self.base_dir, self.max_payload_bytes)?;
        Ok(super::compatibility::check_manifest_compatibility(
            &manifest,
            &self.signature,
            &self.policy,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sig() -> ModelSignature {
        ModelSignature {
            model_name: "run".to_string(),
            model_hash: 7,
            solver: "rk4".to_string(),
            library_version: "0.2.0".to_string(),
            plugin_versions: BTreeMap::new(),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("scico-store-{name}-{}", std::process::id()));
        p
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = temp_dir("roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = CheckpointStore::new(&dir, sig());
        let mut snap = SimulationSnapshot::new(sig());
        snap.sim_time = 3.0;
        snap.step_count = 300;
        snap.continuous_x = vec![1.0, 2.0];
        let m = store.save(&snap).unwrap();
        assert_eq!(m.sequence, 0);
        let (m2, snap2) = store.load().unwrap();
        assert_eq!(m2.sequence, 0);
        assert_eq!(snap2, snap);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sequence_increments_and_overwrites() {
        let dir = temp_dir("seq");
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = CheckpointStore::new(&dir, sig());
        let mut snap = SimulationSnapshot::new(sig());
        snap.step_count = 1;
        let m0 = store.save(&snap).unwrap();
        snap.step_count = 2;
        let m1 = store.save(&snap).unwrap();
        assert_eq!(m0.sequence, 0);
        assert_eq!(m1.sequence, 1);
        // Loading returns the latest.
        let (m, s) = store.load().unwrap();
        assert_eq!(m.sequence, 1);
        assert_eq!(s.step_count, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn incompatible_signature_rejected_on_load() {
        let dir = temp_dir("incompat");
        let _ = std::fs::remove_dir_all(&dir);
        {
            let mut store = CheckpointStore::new(&dir, sig());
            let snap = SimulationSnapshot::new(sig());
            store.save(&snap).unwrap();
        }
        // A store with a different solver must reject the checkpoint.
        let mut other_sig = sig();
        other_sig.solver = "bdf2".to_string();
        let store2 = CheckpointStore::new(&dir, other_sig);
        let err = store2.load().unwrap_err();
        assert!(matches!(err, CheckpointError::Incompatible(_)));
        let compat = store2.check_only().unwrap();
        assert!(!compat.can_restore());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn payload_size_cap_is_enforced() {
        let dir = temp_dir("cap");
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = CheckpointStore::new(&dir, sig()).with_max_payload_bytes(8);
        let mut snap = SimulationSnapshot::new(sig());
        // A snapshot with several state values produces a payload > 8 bytes.
        snap.continuous_x = vec![1.0, 2.0, 3.0, 4.0];
        store.save(&snap).unwrap();
        let err = store.load().unwrap_err();
        assert!(matches!(err, CheckpointError::TooLarge { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_outcome_classification() {
        assert!(
            RunOutcome::Completed {
                final_time: 1.0,
                steps: 10
            }
            .is_success()
        );
        assert!(!RunOutcome::Cancelled { at_time: 1.0 }.is_success());
        assert_eq!(
            RunOutcome::Cancelled { at_time: 1.0 }.status_str(),
            "cancelled"
        );
        assert_eq!(
            RunOutcome::NumericalFailure {
                at_time: 0.5,
                detail: "div".to_string()
            }
            .status_str(),
            "numerical-failure"
        );
        assert_eq!(
            RunOutcome::IoFailure {
                detail: "x".to_string()
            }
            .last_time(),
            None
        );
        assert_eq!(RunOutcome::TimedOut { at_time: 2.0 }.last_time(), Some(2.0));
    }
}
