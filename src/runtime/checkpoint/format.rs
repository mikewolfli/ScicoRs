// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Versioned checkpoint manifest and payload (Phase 38).
//!
//! A checkpoint is a directory (default) or a single JSON file holding a
//! versioned manifest plus the serialized simulation payload. Writes go through
//! a temporary file/directory that is atomically renamed into place, so an
//! interrupted write never corrupts the last valid checkpoint. Loading validates
//! the schema version, a content hash, version compatibility and size limits.

use crate::core::types::Scalar;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Current on-disk checkpoint schema version.
pub const CHECKPOINT_SCHEMA_VERSION: u32 = 1;

/// Error type for checkpoint operations.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckpointError {
    /// An I/O operation failed.
    Io(String),
    /// The on-disk data could not be parsed.
    Parse(String),
    /// The schema version is newer than this build supports.
    UnsupportedSchema {
        /// Schema version found on disk.
        found: u32,
        /// Schema version this build supports.
        supported: u32,
    },
    /// The payload hash did not match the manifest.
    ChecksumMismatch {
        /// Hash recorded in the manifest.
        expected: u64,
        /// Hash computed from the payload.
        actual: u64,
    },
    /// A required field was missing or invalid.
    Invalid(String),
    /// The checkpoint is incompatible with the current model/solver/plugin.
    Incompatible(String),
    /// A size or capacity bound was exceeded.
    TooLarge {
        /// Offending quantity name.
        what: String,
        /// Observed value.
        value: usize,
        /// Limit.
        limit: usize,
    },
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(m) => write!(f, "checkpoint I/O error: {m}"),
            Self::Parse(m) => write!(f, "checkpoint parse error: {m}"),
            Self::UnsupportedSchema { found, supported } => write!(
                f,
                "checkpoint schema {found} is newer than supported {supported}"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                f,
                "checkpoint payload hash mismatch: manifest={expected:#x}, actual={actual:#x}"
            ),
            Self::Invalid(m) => write!(f, "invalid checkpoint: {m}"),
            Self::Incompatible(m) => write!(f, "incompatible checkpoint: {m}"),
            Self::TooLarge { what, value, limit } => {
                write!(f, "{what} too large: {value} > {limit}")
            }
        }
    }
}

impl std::error::Error for CheckpointError {}

/// Simulation identity recorded in a checkpoint for compatibility checks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelSignature {
    /// Diagram/model name.
    pub model_name: String,
    /// Hash of the model structure (blocks + links).
    pub model_hash: u64,
    /// ODE solver identifier.
    pub solver: String,
    /// Kernel library version.
    pub library_version: String,
    /// Plugin versions keyed by plugin name.
    pub plugin_versions: BTreeMap<String, String>,
}

impl ModelSignature {
    /// Check compatibility with another signature, returning the list of
    /// differences. An empty list means the checkpoint is compatible.
    pub fn differences(&self, other: &ModelSignature) -> Vec<String> {
        let mut diffs = Vec::new();
        if self.model_name != other.model_name {
            diffs.push(format!(
                "model name: checkpoint='{}' current='{}'",
                self.model_name, other.model_name
            ));
        }
        if self.model_hash != other.model_hash {
            diffs.push(format!(
                "model hash: checkpoint={:#x} current={:#x}",
                self.model_hash, other.model_hash
            ));
        }
        if self.solver != other.solver {
            diffs.push(format!(
                "solver: checkpoint='{}' current='{}'",
                self.solver, other.solver
            ));
        }
        if self.library_version != other.library_version {
            diffs.push(format!(
                "library version: checkpoint='{}' current='{}'",
                self.library_version, other.library_version
            ));
        }
        for (name, ver) in &self.plugin_versions {
            match other.plugin_versions.get(name) {
                Some(cur) if cur == ver => {}
                Some(cur) => diffs.push(format!(
                    "plugin '{name}': checkpoint='{ver}' current='{cur}'"
                )),
                None => diffs.push(format!(
                    "plugin '{name}': present in checkpoint ('{ver}') but not current"
                )),
            }
        }
        diffs
    }
}

/// A serializable event record (event queue entries).
#[derive(Debug, Clone, PartialEq)]
pub struct EventRecord {
    /// Event identifier.
    pub id: String,
    /// Scheduled time.
    pub time: Scalar,
    /// Event type tag (stable string form).
    pub kind: String,
    /// Priority (higher fires first at the same time).
    pub priority: i32,
    /// Optional target block id.
    pub target: Option<String>,
    /// Scalar payload value, when the event carries a scalar.
    pub scalar: Option<Scalar>,
}

/// A serializable snapshot of the recorder's position, so resumed runs continue
/// appending to the same logical series.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecorderSnapshot {
    /// Output path the recorder was writing to.
    pub output_path: Option<String>,
    /// Number of samples already written.
    pub samples_written: usize,
    /// Signal names recorded so far.
    pub signals: Vec<String>,
}

/// The checkpoint manifest: all metadata needed to judge compatibility before
/// attempting to restore.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointManifest {
    /// On-disk schema version.
    pub schema_version: u32,
    /// Monotonic checkpoint sequence number within a run.
    pub sequence: u64,
    /// Simulation time at which the checkpoint was taken.
    pub sim_time: Scalar,
    /// Engine step count at the time of capture.
    pub step_count: u64,
    /// Creation timestamp (seconds since the Unix epoch, informational).
    pub created_at_unix: u64,
    /// Model / solver / plugin compatibility signature.
    pub signature: ModelSignature,
    /// The RNG seed in effect (for reproducible resume).
    pub rng_seed: u64,
    /// The RNG stream position (number of draws consumed).
    pub rng_draws: u64,
    /// Hash of the payload for corruption detection.
    pub payload_hash: u64,
    /// Byte length of the serialized payload.
    pub payload_len: usize,
    /// Key-value free-form provenance (backend, build id, etc.).
    pub provenance: BTreeMap<String, String>,
}

impl CheckpointManifest {
    /// Create a manifest for a payload, computing its hash and length.
    pub fn new(
        sequence: u64,
        sim_time: Scalar,
        step_count: u64,
        signature: ModelSignature,
        payload: &[u8],
    ) -> Self {
        Self {
            schema_version: CHECKPOINT_SCHEMA_VERSION,
            sequence,
            sim_time,
            step_count,
            created_at_unix: 0,
            signature,
            rng_seed: 0,
            rng_draws: 0,
            payload_hash: hash_bytes(payload),
            payload_len: payload.len(),
            provenance: BTreeMap::new(),
        }
    }
}

/// FNV-1a 64-bit hash, used for payload integrity and model signatures.
///
/// This is a fast, dependency-free, deterministic hash; it detects accidental
/// corruption (the checkpoint use case), not adversarial tampering.
pub fn hash_bytes(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// Hash a string slice.
pub fn hash_str(s: &str) -> u64 {
    hash_bytes(s.as_bytes())
}

/// Compute the path of the temporary staging location next to `base`.
fn staging_path(base: &Path) -> PathBuf {
    sibling_with_suffix(base, ".part")
}

/// Compute the path of the backup location next to `base`.
fn backup_path(base: &Path) -> PathBuf {
    sibling_with_suffix(base, ".old")
}

/// Build a sibling path by appending `suffix` to the file name.
///
/// Appending (rather than replacing the extension with `with_extension`) keeps
/// the mapping injective: `results.01` and `results.02` map to distinct
/// `results.01.old` / `results.02.old` instead of both collapsing to
/// `results.old`.
fn sibling_with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let name = base
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "checkpoint".to_string());
    base.with_file_name(format!("{name}{suffix}"))
}

/// Write a checkpoint directory atomically and crash-safely.
///
/// The manifest is written to `<dir>/manifest.json` and the payload to
/// `<dir>/payload.bin` inside a `<dir>.part` staging directory. The swap keeps a
/// valid checkpoint readable at *every* instant:
///
/// 1. Read the existing checkpoint (if any) into memory as a fallback.
/// 2. Rename `dir` → `<dir>.old` (only if `dir` exists).
/// 3. Rename `<dir>.part` → `dir`.
/// 4. Remove `<dir>.old`.
///
/// If the process dies between steps 2 and 3, `dir` is momentarily absent but
/// `<dir>.old` holds the previous valid checkpoint; [`load_checkpoint`]
/// transparently falls back to `<dir>.old`, so the last valid checkpoint is
/// never lost. If step 3 cannot be completed (e.g. an I/O error), the backup is
/// renamed back to `dir` so the old checkpoint remains in place.
pub fn write_checkpoint_atomic(
    dir: &Path,
    manifest: &CheckpointManifest,
    payload: &[u8],
) -> Result<(), CheckpointError> {
    let staging = staging_path(dir);
    let backup = backup_path(dir);
    // Crash recovery: if a previous write died between the two renames, `dir` is
    // absent but `<dir>.old` holds the last valid checkpoint. Restore it in place
    // before overwriting, so the backup directory is not orphaned.
    if !dir.exists() && backup.exists() {
        std::fs::rename(&backup, dir).map_err(|e| CheckpointError::Io(e.to_string()))?;
    }
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| CheckpointError::Io(e.to_string()))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| CheckpointError::Io(e.to_string()))?;

    let payload_path = staging.join("payload.bin");
    std::fs::write(&payload_path, payload).map_err(|e| CheckpointError::Io(e.to_string()))?;

    let manifest_path = staging.join("manifest.json");
    let json = manifest_to_json(manifest);
    std::fs::write(&manifest_path, json).map_err(|e| CheckpointError::Io(e.to_string()))?;

    // The staging directory is fully written and closed before any rename.
    if dir.exists() {
        if backup.exists() {
            std::fs::remove_dir_all(&backup).map_err(|e| CheckpointError::Io(e.to_string()))?;
        }
        std::fs::rename(dir, &backup).map_err(|e| CheckpointError::Io(e.to_string()))?;
        match std::fs::rename(&staging, dir) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&backup);
            }
            Err(e) => {
                // Roll back so the previous checkpoint is restored in place.
                let _ = std::fs::rename(&backup, dir);
                return Err(CheckpointError::Io(e.to_string()));
            }
        }
    } else {
        std::fs::rename(&staging, dir).map_err(|e| CheckpointError::Io(e.to_string()))?;
    }
    Ok(())
}

/// Load a checkpoint from a directory, validating schema, hash and size bounds.
///
/// If `dir` is absent but its crash-recovery sibling `<dir>.old` exists (the
/// window between the two renames in [`write_checkpoint_atomic`]), the backup is
/// read instead so a valid checkpoint is never lost.
///
/// `max_payload_bytes` bounds the accepted payload; pass `usize::MAX` to disable.
pub fn load_checkpoint(
    dir: &Path,
    max_payload_bytes: usize,
) -> Result<(CheckpointManifest, Vec<u8>), CheckpointError> {
    let dir = if dir.join("manifest.json").exists() {
        dir.to_path_buf()
    } else {
        let backup = backup_path(dir);
        if backup.join("manifest.json").exists() {
            backup
        } else {
            dir.to_path_buf()
        }
    };
    let dir = dir.as_path();
    let manifest_path = dir.join("manifest.json");
    let manifest_raw = std::fs::read_to_string(&manifest_path)
        .map_err(|e| CheckpointError::Io(format!("reading {}: {e}", manifest_path.display())))?;
    let manifest = manifest_from_json(&manifest_raw)?;

    if manifest.schema_version > CHECKPOINT_SCHEMA_VERSION {
        return Err(CheckpointError::UnsupportedSchema {
            found: manifest.schema_version,
            supported: CHECKPOINT_SCHEMA_VERSION,
        });
    }
    if manifest.payload_len > max_payload_bytes {
        return Err(CheckpointError::TooLarge {
            what: "payload".to_string(),
            value: manifest.payload_len,
            limit: max_payload_bytes,
        });
    }

    let payload_path = dir.join("payload.bin");
    let payload = std::fs::read(&payload_path)
        .map_err(|e| CheckpointError::Io(format!("reading {}: {e}", payload_path.display())))?;

    if payload.len() != manifest.payload_len {
        return Err(CheckpointError::Invalid(format!(
            "payload length {} != manifest {}",
            payload.len(),
            manifest.payload_len
        )));
    }
    let actual = hash_bytes(&payload);
    if actual != manifest.payload_hash {
        return Err(CheckpointError::ChecksumMismatch {
            expected: manifest.payload_hash,
            actual,
        });
    }
    Ok((manifest, payload))
}

/// Serialize a manifest to JSON (deterministic key order via `BTreeMap`).
fn manifest_to_json(m: &CheckpointManifest) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    s.push('{');
    let _ = write!(s, "\"schema_version\":{},", m.schema_version);
    let _ = write!(s, "\"sequence\":{},", m.sequence);
    let _ = write!(s, "\"sim_time\":{},", m.sim_time);
    let _ = write!(s, "\"step_count\":{},", m.step_count);
    let _ = write!(s, "\"created_at_unix\":{},", m.created_at_unix);
    let _ = write!(s, "\"rng_seed\":{},", m.rng_seed);
    let _ = write!(s, "\"rng_draws\":{},", m.rng_draws);
    let _ = write!(s, "\"payload_hash\":{},", m.payload_hash);
    let _ = write!(s, "\"payload_len\":{},", m.payload_len);
    let _ = write!(s, "\"signature\":{{");
    let _ = write!(
        s,
        "\"model_name\":\"{}\",",
        escape_json(&m.signature.model_name)
    );
    let _ = write!(s, "\"model_hash\":{},", m.signature.model_hash);
    let _ = write!(s, "\"solver\":\"{}\",", escape_json(&m.signature.solver));
    let _ = write!(
        s,
        "\"library_version\":\"{}\"",
        escape_json(&m.signature.library_version)
    );
    if !m.signature.plugin_versions.is_empty() {
        let _ = write!(s, ",\"plugin_versions\":{{");
        let mut first = true;
        for (k, v) in &m.signature.plugin_versions {
            if !first {
                s.push(',');
            }
            first = false;
            let _ = write!(s, "\"{}\":\"{}\"", escape_json(k), escape_json(v));
        }
        s.push('}');
    }
    s.push('}');
    if !m.provenance.is_empty() {
        let _ = write!(s, ",\"provenance\":{{");
        let mut first = true;
        for (k, v) in &m.provenance {
            if !first {
                s.push(',');
            }
            first = false;
            let _ = write!(s, "\"{}\":\"{}\"", escape_json(k), escape_json(v));
        }
        s.push('}');
    }
    s.push('}');
    s
}

/// Parse a manifest from JSON, rejecting malformed input.
fn manifest_from_json(raw: &str) -> Result<CheckpointManifest, CheckpointError> {
    let v: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| CheckpointError::Parse(e.to_string()))?;
    let get_u64 = |key: &str| -> Result<u64, CheckpointError> {
        v.get(key)
            .and_then(|x| x.as_u64())
            .ok_or_else(|| CheckpointError::Invalid(format!("missing/invalid field '{key}'")))
    };
    let get_f64 = |key: &str| -> Result<Scalar, CheckpointError> {
        v.get(key)
            .and_then(|x| x.as_f64())
            .ok_or_else(|| CheckpointError::Invalid(format!("missing/invalid field '{key}'")))
    };
    let sig = v
        .get("signature")
        .ok_or_else(|| CheckpointError::Invalid("missing 'signature'".to_string()))?;
    let mut plugin_versions = BTreeMap::new();
    if let Some(map) = sig.get("plugin_versions").and_then(|x| x.as_object()) {
        for (k, val) in map {
            if let Some(sv) = val.as_str() {
                plugin_versions.insert(k.clone(), sv.to_string());
            }
        }
    }
    let mut provenance = BTreeMap::new();
    if let Some(map) = v.get("provenance").and_then(|x| x.as_object()) {
        for (k, val) in map {
            if let Some(sv) = val.as_str() {
                provenance.insert(k.clone(), sv.to_string());
            }
        }
    }
    Ok(CheckpointManifest {
        schema_version: get_u64("schema_version")? as u32,
        sequence: get_u64("sequence")?,
        sim_time: get_f64("sim_time")?,
        step_count: get_u64("step_count")?,
        created_at_unix: v
            .get("created_at_unix")
            .and_then(|x| x.as_u64())
            .unwrap_or(0),
        signature: ModelSignature {
            model_name: sig
                .get("model_name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            model_hash: sig.get("model_hash").and_then(|x| x.as_u64()).unwrap_or(0),
            solver: sig
                .get("solver")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            library_version: sig
                .get("library_version")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            plugin_versions,
        },
        rng_seed: v.get("rng_seed").and_then(|x| x.as_u64()).unwrap_or(0),
        rng_draws: v.get("rng_draws").and_then(|x| x.as_u64()).unwrap_or(0),
        payload_hash: get_u64("payload_hash")?,
        payload_len: get_u64("payload_len")? as usize,
        provenance,
    })
}

/// Minimal JSON string escaping consistent with serde's output.
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig() -> ModelSignature {
        ModelSignature {
            model_name: "test-model".to_string(),
            model_hash: 0xABCD,
            solver: "rk4".to_string(),
            library_version: "0.2.0".to_string(),
            plugin_versions: BTreeMap::new(),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("scico-ckpt-test-{name}-{}", std::process::id()));
        p
    }

    #[test]
    fn hash_is_deterministic_and_sensitive() {
        assert_eq!(hash_bytes(b"hello"), hash_bytes(b"hello"));
        assert_ne!(hash_bytes(b"hello"), hash_bytes(b"hellp"));
        assert_eq!(hash_str("hello"), hash_bytes(b"hello"));
    }

    #[test]
    fn roundtrip_write_load() {
        let dir = temp_dir("roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let payload = b"simulation state bytes".to_vec();
        let m = CheckpointManifest::new(3, 1.5, 150, sig(), &payload);
        write_checkpoint_atomic(&dir, &m, &payload).unwrap();
        let (loaded, data) = load_checkpoint(&dir, usize::MAX).unwrap();
        assert_eq!(loaded.sequence, 3);
        assert_eq!(loaded.step_count, 150);
        assert!((loaded.sim_time - 1.5).abs() < 1e-12);
        assert_eq!(data, payload);
        assert_eq!(loaded.signature.model_name, "test-model");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_payload_detected() {
        let dir = temp_dir("corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        let payload = b"original".to_vec();
        let m = CheckpointManifest::new(1, 0.0, 0, sig(), &payload);
        write_checkpoint_atomic(&dir, &m, &payload).unwrap();
        // Corrupt the payload after writing.
        std::fs::write(dir.join("payload.bin"), b"tampered").unwrap();
        let err = load_checkpoint(&dir, usize::MAX).unwrap_err();
        assert!(matches!(
            err,
            CheckpointError::ChecksumMismatch { .. } | CheckpointError::Invalid(_)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsupported_future_schema_rejected() {
        let dir = temp_dir("future");
        let _ = std::fs::remove_dir_all(&dir);
        let payload = b"x".to_vec();
        let mut m = CheckpointManifest::new(1, 0.0, 0, sig(), &payload);
        m.schema_version = CHECKPOINT_SCHEMA_VERSION + 5;
        write_checkpoint_atomic(&dir, &m, &payload).unwrap();
        let err = load_checkpoint(&dir, usize::MAX).unwrap_err();
        assert!(matches!(err, CheckpointError::UnsupportedSchema { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn size_bound_enforced() {
        let dir = temp_dir("size");
        let _ = std::fs::remove_dir_all(&dir);
        let payload = vec![0u8; 100];
        let m = CheckpointManifest::new(1, 0.0, 0, sig(), &payload);
        write_checkpoint_atomic(&dir, &m, &payload).unwrap();
        let err = load_checkpoint(&dir, 10).unwrap_err();
        assert!(matches!(err, CheckpointError::TooLarge { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_overwrite_keeps_valid_checkpoint() {
        let dir = temp_dir("overwrite");
        let _ = std::fs::remove_dir_all(&dir);
        let p1 = b"first".to_vec();
        let m1 = CheckpointManifest::new(1, 1.0, 10, sig(), &p1);
        write_checkpoint_atomic(&dir, &m1, &p1).unwrap();
        let p2 = b"second".to_vec();
        let m2 = CheckpointManifest::new(2, 2.0, 20, sig(), &p2);
        write_checkpoint_atomic(&dir, &m2, &p2).unwrap();
        let (loaded2, data2) = load_checkpoint(&dir, usize::MAX).unwrap();
        assert_eq!(loaded2.sequence, 2);
        assert_eq!(data2, p2);
        // No staging or backup leftovers after a successful overwrite.
        assert!(!staging_path(&dir).exists());
        assert!(!backup_path(&dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn signature_differences_are_reported() {
        let a = sig();
        let mut b = sig();
        b.solver = "bdf2".to_string();
        let diffs = a.differences(&b);
        assert_eq!(diffs.len(), 1);
        assert!(diffs[0].contains("solver"));

        let mut c = sig();
        c.plugin_versions
            .insert("optics".to_string(), "1.0".to_string());
        // `differences` reports the checkpoint's requirements that the current
        // build does not satisfy — the compatibility-relevant direction.
        let diffs2 = c.differences(&a);
        assert!(diffs2.iter().any(|d| d.contains("optics")));
    }

    #[test]
    fn backup_path_does_not_collide_for_dotted_names() {
        // `results.01` and `results.02` must map to distinct siblings (the old
        // `with_extension` behaviour collapsed both to `results.old`).
        let d1 = std::path::PathBuf::from("/tmp/results.01");
        let d2 = std::path::PathBuf::from("/tmp/results.02");
        assert_ne!(backup_path(&d1), backup_path(&d2));
        assert_eq!(
            backup_path(&d1),
            std::path::PathBuf::from("/tmp/results.01.old")
        );
        assert_eq!(
            staging_path(&d1),
            std::path::PathBuf::from("/tmp/results.01.part")
        );
    }

    #[test]
    fn crash_between_renames_still_loads_backup() {
        // Simulate a crash in the write window: `dir` renamed to `<dir>.old` but
        // the staging rename to `dir` never happened. Loading must recover the
        // previous valid checkpoint from the backup instead of failing.
        let dir = temp_dir("crashwindow");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(backup_path(&dir));
        let payload = b"valid checkpoint".to_vec();
        let m = CheckpointManifest::new(7, 2.0, 70, sig(), &payload);
        write_checkpoint_atomic(&dir, &m, &payload).unwrap();

        // Reproduce the intermediate crash state.
        let backup = backup_path(&dir);
        std::fs::rename(&dir, &backup).unwrap();
        assert!(!dir.exists());

        // `load_checkpoint` falls back to the backup and still succeeds.
        let (loaded, data) = load_checkpoint(&dir, usize::MAX).unwrap();
        assert_eq!(loaded.sequence, 7);
        assert_eq!(data, payload);

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&backup);
    }

    #[test]
    fn manifest_json_escapes_strings() {
        let mut s = sig();
        s.model_name = "quote\"and\\slash".to_string();
        let m = CheckpointManifest::new(1, 0.0, 0, s, b"x");
        let json = manifest_to_json(&m);
        let parsed = manifest_from_json(&json).unwrap();
        assert_eq!(parsed.signature.model_name, "quote\"and\\slash");
    }
}
