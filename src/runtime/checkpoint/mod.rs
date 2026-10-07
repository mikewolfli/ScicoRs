// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Checkpointing subsystem (Phase 38).
//!
//! Provides versioned, atomically-written checkpoints containing the full
//! simulation state ([`snapshot::SimulationSnapshot`]), a validated manifest
//! ([`format::CheckpointManifest`]) and compatibility checks
//! ([`compatibility`]).
//!
//! # Structure
//!
//! * [`mod@format`] — versioned manifest, atomic write/load, hashing.
//! * [`snapshot`] — state/event/RNG/recorder capture and restore.
//! * [`compatibility`] — model/solver/plugin compatibility policy.

pub mod compatibility;
pub mod format;
pub mod manager;
pub mod snapshot;

pub use compatibility::{
    Compatibility, CompatibilityPolicy, check_manifest_compatibility,
    check_signature_compatibility, check_snapshot_compatibility,
};
pub use format::{
    CHECKPOINT_SCHEMA_VERSION, CheckpointError, CheckpointManifest, EventRecord, ModelSignature,
    RecorderSnapshot, hash_bytes, hash_str, load_checkpoint, write_checkpoint_atomic,
};
pub use manager::{CheckpointStore, RunOutcome};
pub use snapshot::{SNAPSHOT_VERSION, SimulationSnapshot};

#[cfg(test)]
mod integration_tests;
