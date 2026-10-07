// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Dataset run provenance: versions, parameters, checksums and status.
//!
//! Every dataset carries a **versioned manifest** describing where its numbers
//! came from: the simulation id, a model signature, a run-configuration hash,
//! the parameters (with units), solver tolerances, the RNG seed, the compute
//! backend, library/plugin versions, creation time, run status and — when the
//! run failed — an error summary.
//!
//! The manifest also stores the per-chunk checksums used by the writer/reader to
//! detect corruption. Checksums are real CRC-32 (IEEE 802.3, reflected,
//! polynomial `0xEDB88320`), implemented in this module; see [`crc32`].
//!
//! # Redaction
//!
//! Provenance often contains absolute paths, host names and environment
//! variables. [`RedactionPolicy`] lets a producer strip or mask that
//! information *before* it is written. Redaction is applied structurally rather
//! than by string replacement, so a redacted manifest can never smuggle the
//! original value back in through a neighbouring field.

use crate::core::types::Scalar;

/// CRC-32 (IEEE 802.3) table, generated from the reflected polynomial
/// `0xEDB88320` at compile time. `const fn` so it costs nothing at runtime.
const CRC32_TABLE: [u32; 256] = build_crc32_table();

/// Build the 256-entry CRC-32 lookup table.
const fn build_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0usize;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

/// Compute the CRC-32 (IEEE 802.3) checksum of `data`.
///
/// This is a complete, standard implementation: it initialises to
/// `0xFFFF_FFFF`, processes every byte through the reflected polynomial table,
/// and finalises by XOR with `0xFFFF_FFFF`. For example, `crc32(b"123456789")`
/// equals `0xCBF4_3926`, the standard check value.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = CRC32_TABLE[index] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// Dataset run outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RunStatus {
    /// The run is still in progress; the dataset is not yet complete.
    Running,
    /// The run finished successfully and its data validated.
    Completed,
    /// The run failed; see the manifest's error summary.
    Failed,
    /// The run was stopped before completing (interrupt/checkpoint).
    Aborted,
}

impl RunStatus {
    /// Stable identifier used in files and error messages.
    pub fn name(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Aborted => "aborted",
        }
    }

    /// Whether this status permits the dataset to be marked complete.
    ///
    /// Only a successfully completed run may carry the completion marker; a
    /// failed/aborted run must stay readable as an *incomplete* dataset.
    pub fn allows_completion(self) -> bool {
        matches!(self, RunStatus::Completed)
    }
}

/// Compute the canonical checksum of a schema.
///
/// The schema is serialized and then re-parsed before hashing, so the checksum
/// is identical whether it is computed from a freshly built schema or from one
/// just read back from disk. This matters because JSON float formatting is not
/// byte-stable across a serialization round trip (a computed `0.39999999999999997`
/// is re-emitted as the shorter `0.4`), so hashing the raw serialization of a
/// freshly built schema would spuriously disagree with the same schema after a
/// load.
fn canonical_schema_checksum(schema: &super::schema::DatasetSchema) -> Result<u32, String> {
    let bytes = serde_json::to_vec(schema).map_err(|e| format!("schema serialize error: {}", e))?;
    let normalized: super::schema::DatasetSchema =
        serde_json::from_slice(&bytes).map_err(|e| format!("schema normalize error: {}", e))?;
    let canonical =
        serde_json::to_vec(&normalized).map_err(|e| format!("schema serialize error: {}", e))?;
    Ok(crc32(&canonical))
}

/// A named parameter with its declared unit.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ManifestParameter {
    /// Parameter name.
    pub name: String,
    /// Numeric value.
    pub value: Scalar,
    /// Unit symbol (empty for dimensionless parameters).
    pub unit: String,
}

impl ManifestParameter {
    /// Create a parameter.
    pub fn new(name: &str, value: Scalar, unit: &str) -> Self {
        Self {
            name: name.to_string(),
            value,
            unit: unit.to_string(),
        }
    }
}

/// Solver configuration recorded for reproducibility.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SolverRecord {
    /// Solver name/identifier.
    pub name: String,
    /// Relative tolerance, if applicable.
    pub relative_tolerance: Option<Scalar>,
    /// Absolute tolerance, if applicable.
    pub absolute_tolerance: Option<Scalar>,
    /// Maximum iterations/step count, if applicable.
    pub max_iterations: Option<u64>,
}

impl SolverRecord {
    /// Create a solver record with no tolerances set.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            relative_tolerance: None,
            absolute_tolerance: None,
            max_iterations: None,
        }
    }

    /// Set the tolerances.
    pub fn with_tolerances(mut self, relative: Scalar, absolute: Scalar) -> Self {
        self.relative_tolerance = Some(relative);
        self.absolute_tolerance = Some(absolute);
        self
    }

    /// Set the iteration/step budget.
    pub fn with_max_iterations(mut self, max: u64) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// Whether the recorded tolerances are physically sensible (non-negative
    /// and finite).
    pub fn tolerances_are_valid(&self) -> bool {
        let ok = |v: Option<Scalar>| match v {
            Some(x) => x.is_finite() && x >= 0.0,
            None => true,
        };
        ok(self.relative_tolerance) && ok(self.absolute_tolerance)
    }
}

/// Configurable redaction of sensitive provenance fields.
///
/// Redaction is applied by [`DatasetManifest::redact`] before a manifest is
/// persisted. The policy is structural: constructing an empty
/// [`DatasetManifest`] leaves every container empty, and redaction blanks the
/// fields that could carry host- or user-specific information.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct RedactionPolicy {
    /// Replace free-text fields (model signature, error summary, notes) with the
    /// redaction placeholder.
    pub redact_free_text: bool,
    /// Drop library/plugin version entries (which often embed install paths).
    pub drop_library_versions: bool,
    /// Blank the compute backend description.
    pub redact_backend: bool,
}

impl RedactionPolicy {
    /// A policy that redacts everything redactable.
    pub fn strict() -> Self {
        Self {
            redact_free_text: true,
            drop_library_versions: true,
            redact_backend: true,
        }
    }

    /// Placeholder substituted for redacted text.
    pub const PLACEHOLDER: &'static str = "<redacted>";

    /// Whether any redaction is enabled.
    pub fn is_active(&self) -> bool {
        self.redact_free_text || self.drop_library_versions || self.redact_backend
    }
}

/// Per-chunk integrity record stored in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChunkRecord {
    /// Zero-based chunk index within the dataset.
    pub index: u64,
    /// File name of the chunk, relative to the dataset directory.
    pub file: String,
    /// Number of rows (time samples) in the chunk.
    pub rows: usize,
    /// CRC-32 of the chunk file's raw bytes.
    pub checksum: u32,
}

/// The versioned run-provenance manifest of a dataset.
///
/// Construct one with [`DatasetManifest::new`] (which leaves every provenance
/// container empty, never inventing values), fill in what the run knows, and
/// let the writer persist it alongside the chunks.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DatasetManifest {
    /// Manifest format version.
    pub manifest_version: u32,
    /// Simulation identifier assigned by the producer.
    pub simulation_id: String,
    /// Model signature (a stable hash/digest of the model definition).
    pub model_signature: String,
    /// Hash of the run configuration used to produce the data.
    pub run_config_hash: String,
    /// Run parameters with their units.
    pub parameters: Vec<ManifestParameter>,
    /// Solver/tolerance record, if the run used a numerical solver.
    pub solver: Option<SolverRecord>,
    /// RNG seed, if the run was stochastic.
    pub rng_seed: Option<u64>,
    /// Compute backend description (e.g. `"cpu"`, `"gpu:wgt"`).
    pub backend: String,
    /// Library/plugin names and versions active during the run.
    pub library_versions: Vec<(String, String)>,
    /// Creation time, RFC 3339 UTC.
    pub created_at: String,
    /// Run status.
    pub status: RunStatus,
    /// Error summary when the run failed or was aborted (empty otherwise).
    pub error_summary: String,
    /// Checksums of every written chunk, in write order.
    pub chunks: Vec<ChunkRecord>,
    /// CRC-32 over the canonical serialization of the schema, binding the
    /// manifest to the exact schema it describes.
    pub schema_checksum: u32,
    /// Total number of data rows across all chunks.
    pub total_rows: usize,
}

impl DatasetManifest {
    /// Manifest format version produced by this crate.
    pub const MANIFEST_VERSION: u32 = 1;

    /// Create an empty manifest at the current version.
    ///
    /// Every provenance container starts empty: an untouched manifest makes no
    /// claims about versions, parameters or backends it did not observe.
    pub fn new(simulation_id: &str) -> Self {
        Self {
            manifest_version: Self::MANIFEST_VERSION,
            simulation_id: simulation_id.to_string(),
            model_signature: String::new(),
            run_config_hash: String::new(),
            parameters: Vec::new(),
            solver: None,
            rng_seed: None,
            backend: String::new(),
            library_versions: Vec::new(),
            created_at: String::new(),
            status: RunStatus::Running,
            error_summary: String::new(),
            chunks: Vec::new(),
            schema_checksum: 0,
            total_rows: 0,
        }
    }

    /// Set the model signature.
    pub fn with_model_signature(mut self, signature: &str) -> Self {
        self.model_signature = signature.to_string();
        self
    }

    /// Set the run-configuration hash.
    pub fn with_run_config_hash(mut self, hash: &str) -> Self {
        self.run_config_hash = hash.to_string();
        self
    }

    /// Set the compute backend description.
    pub fn with_backend(mut self, backend: &str) -> Self {
        self.backend = backend.to_string();
        self
    }

    /// Set the RNG seed.
    pub fn with_rng_seed(mut self, seed: u64) -> Self {
        self.rng_seed = Some(seed);
        self
    }

    /// Set the solver record.
    pub fn with_solver(mut self, solver: SolverRecord) -> Self {
        self.solver = Some(solver);
        self
    }

    /// Set the creation time.
    pub fn with_created_at(mut self, created_at: &str) -> Self {
        self.created_at = created_at.to_string();
        self
    }

    /// Record a library/plugin version.
    pub fn add_library_version(mut self, name: &str, version: &str) -> Self {
        self.library_versions
            .push((name.to_string(), version.to_string()));
        self
    }

    /// Record a run parameter.
    pub fn add_parameter(mut self, name: &str, value: Scalar, unit: &str) -> Self {
        self.parameters
            .push(ManifestParameter::new(name, value, unit));
        self
    }

    /// Set the run status.
    pub fn with_status(mut self, status: RunStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the error summary (used for failed/aborted runs).
    pub fn with_error_summary(mut self, summary: &str) -> Self {
        self.error_summary = summary.to_string();
        self
    }

    /// Record the CRC-32 of a written chunk and add to the totals.
    ///
    /// The checksum must be computed by the caller over the chunk's raw bytes;
    /// [`crate::postproc::dataset::crc32`] is provided for that purpose.
    pub fn record_chunk(&mut self, index: u64, file: &str, rows: usize, checksum: u32) {
        self.chunks.push(ChunkRecord {
            index,
            file: file.to_string(),
            rows,
            checksum,
        });
        self.total_rows += rows;
    }

    /// Bind the manifest to a schema by storing the schema's canonical
    /// serialization checksum.
    ///
    /// Returns an error if the schema cannot be serialized.
    pub fn bind_schema(&mut self, schema: &super::schema::DatasetSchema) -> Result<(), String> {
        self.schema_checksum = canonical_schema_checksum(schema)?;
        Ok(())
    }

    /// Verify the manifest still matches `schema`.
    pub fn schema_matches(&self, schema: &super::schema::DatasetSchema) -> bool {
        match canonical_schema_checksum(schema) {
            Ok(checksum) => checksum == self.schema_checksum,
            Err(_) => false,
        }
    }

    /// Look up a recorded chunk by index.
    pub fn chunk(&self, index: u64) -> Option<&ChunkRecord> {
        self.chunks.iter().find(|c| c.index == index)
    }

    /// Apply a redaction policy in place, returning the list of field names
    /// that were redacted (for an audit note that itself carries no secrets).
    pub fn redact(&mut self, policy: &RedactionPolicy) -> Vec<String> {
        let mut redacted = Vec::new();
        if policy.redact_free_text {
            if !self.model_signature.is_empty() {
                self.model_signature = RedactionPolicy::PLACEHOLDER.to_string();
                redacted.push("model_signature".to_string());
            }
            if !self.error_summary.is_empty() {
                self.error_summary = RedactionPolicy::PLACEHOLDER.to_string();
                redacted.push("error_summary".to_string());
            }
        }
        if policy.drop_library_versions && !self.library_versions.is_empty() {
            self.library_versions.clear();
            redacted.push("library_versions".to_string());
        }
        if policy.redact_backend && !self.backend.is_empty() {
            self.backend = RedactionPolicy::PLACEHOLDER.to_string();
            redacted.push("backend".to_string());
        }
        redacted
    }

    /// Validate the manifest's internal consistency and run status.
    pub fn validate(&self) -> Result<(), String> {
        if self.manifest_version != Self::MANIFEST_VERSION {
            return Err(format!(
                "unsupported manifest version {} (expected {})",
                self.manifest_version,
                Self::MANIFEST_VERSION
            ));
        }
        if self.simulation_id.trim().is_empty() {
            return Err("manifest has an empty simulation id".to_string());
        }
        if let Some(solver) = &self.solver {
            if !solver.tolerances_are_valid() {
                return Err(format!(
                    "solver '{}' has invalid (negative or non-finite) tolerances",
                    solver.name
                ));
            }
        }
        if self.status == RunStatus::Failed && self.error_summary.trim().is_empty() {
            return Err("a failed run must carry an error summary".to_string());
        }
        let mut indices: Vec<u64> = self.chunks.iter().map(|c| c.index).collect();
        indices.sort_unstable();
        for pair in indices.windows(2) {
            if pair[0] == pair[1] {
                return Err(format!("duplicate chunk index {}", pair[0]));
            }
        }
        let sum: usize = self.chunks.iter().map(|c| c.rows).sum();
        if sum != self.total_rows {
            return Err(format!(
                "manifest declares {} total rows but its chunks hold {}",
                self.total_rows, sum
            ));
        }
        Ok(())
    }

    /// Serialize the manifest as pretty JSON.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("manifest serialize error: {}", e))
    }

    /// Parse a manifest from JSON.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("manifest parse error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postproc::dataset::schema::{DatasetSchema, ResultVariable};

    #[test]
    fn test_crc32_known_check_value() {
        // The standard CRC-32 check value for "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn test_crc32_matches_zlib_reference_vectors() {
        // Reference values from the zlib/PNG CRC-32 implementation.
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }

    #[test]
    fn test_crc32_detects_single_bit_flip() {
        let original = b"scicors-dataset-chunk";
        let mut corrupted = *original;
        corrupted[3] ^= 0x01;
        assert_ne!(crc32(original), crc32(&corrupted));
    }

    #[test]
    fn test_manifest_round_trip_json() {
        let manifest = DatasetManifest::new("sim-42")
            .with_model_signature("sha256:deadbeef")
            .with_run_config_hash("cfg-123")
            .with_backend("cpu")
            .with_rng_seed(7)
            .with_solver(SolverRecord::new("rk4").with_tolerances(1e-6, 1e-9))
            .with_created_at("2026-10-08T00:00:00Z")
            .add_parameter("k", 1.5, "1/s")
            .add_library_version("scico_rs", "0.2.0");
        let json = manifest.to_json().unwrap();
        let back = DatasetManifest::from_json(&json).unwrap();
        assert_eq!(manifest, back);
    }

    #[test]
    fn test_manifest_new_invents_nothing() {
        let m = DatasetManifest::new("s");
        assert!(m.parameters.is_empty());
        assert!(m.library_versions.is_empty());
        assert!(m.solver.is_none());
        assert!(m.rng_seed.is_none());
        assert_eq!(m.backend, "");
        assert_eq!(m.status, RunStatus::Running);
    }

    #[test]
    fn test_manifest_bind_schema_detects_change() {
        let mut schema = DatasetSchema::new("run");
        schema
            .add_variable(ResultVariable::scalar("a", "q", "m"))
            .unwrap();
        let mut manifest = DatasetManifest::new("sim");
        manifest.bind_schema(&schema).unwrap();
        assert!(manifest.schema_matches(&schema));

        schema
            .add_variable(ResultVariable::scalar("b", "q", "m"))
            .unwrap();
        assert!(!manifest.schema_matches(&schema));
    }

    #[test]
    fn test_record_chunk_accumulates_rows() {
        let mut m = DatasetManifest::new("sim");
        m.record_chunk(0, "chunk_0.json", 3, crc32(b"chunk0"));
        m.record_chunk(1, "chunk_1.json", 2, crc32(b"chunk1"));
        assert_eq!(m.total_rows, 5);
        assert_eq!(m.chunk(1).unwrap().rows, 2);
        m.validate().unwrap();
    }

    #[test]
    fn test_validate_rejects_failed_run_without_error_summary() {
        let m = DatasetManifest::new("sim").with_status(RunStatus::Failed);
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_row_count_mismatch() {
        let mut m = DatasetManifest::new("sim");
        m.record_chunk(0, "chunk_0.json", 3, 123);
        m.total_rows = 999;
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_only_completed_status_allows_completion() {
        assert!(RunStatus::Completed.allows_completion());
        assert!(!RunStatus::Running.allows_completion());
        assert!(!RunStatus::Failed.allows_completion());
        assert!(!RunStatus::Aborted.allows_completion());
    }

    #[test]
    fn test_redaction_removes_sensitive_text_and_backend() {
        let mut m = DatasetManifest::new("sim")
            .with_model_signature("/home/alice/private/model.yaml")
            .with_backend("gpu:/opt/cuda/lib")
            .with_error_summary("failed at /home/alice/work")
            .add_library_version("plugin", "/home/alice/libplugin.so");
        let redacted = m.redact(&RedactionPolicy::strict());
        assert!(redacted.contains(&"model_signature".to_string()));
        assert!(redacted.contains(&"library_versions".to_string()));
        assert!(redacted.contains(&"backend".to_string()));
        assert_eq!(m.model_signature, RedactionPolicy::PLACEHOLDER);
        assert_eq!(m.backend, RedactionPolicy::PLACEHOLDER);
        assert_eq!(m.error_summary, RedactionPolicy::PLACEHOLDER);
        assert!(m.library_versions.is_empty());
        // The secret path must not survive anywhere in the serialized form.
        let json = m.to_json().unwrap();
        assert!(!json.contains("alice"));
        assert!(!json.contains("cuda"));
    }

    #[test]
    fn test_default_redaction_policy_is_a_no_op() {
        let policy = RedactionPolicy::default();
        assert!(!policy.is_active());
        let mut m = DatasetManifest::new("sim").with_backend("cpu");
        assert!(m.redact(&policy).is_empty());
        assert_eq!(m.backend, "cpu");
    }

    #[test]
    fn test_solver_record_rejects_negative_tolerance() {
        let m = DatasetManifest::new("sim")
            .with_solver(SolverRecord::new("rk4").with_tolerances(-1e-6, 1e-9));
        assert!(m.validate().is_err());
    }
}
