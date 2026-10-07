// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Dataset reader: open a finished dataset and query it by variable, time
//! window and spatial region.
//!
//! Opening a dataset validates its [completion marker](super::writer::COMPLETE_MARKER),
//! its schema/manifest checksums and **every** chunk checksum. A dataset that
//! was left incomplete by a crashed writer, or whose bytes were corrupted, is
//! reported as an error rather than being read back as a shorter-but-looking-
//! successful result.
//!
//! The reader answers the questions the acceptance criteria name:
//!
//! * list variables, their units and coordinate axes ([`DatasetReader::variables`]);
//! * read one variable's whole series ([`DatasetReader::variable_array`]);
//! * read a time window ([`DatasetReader::time_window`]);
//! * read a spatial sub-region ([`DatasetReader::spatial_region`]);
//! * convert a read series into another compatible unit
//!   ([`DatasetReader::variable_in_unit`]).

use crate::core::types::Scalar;
use crate::postproc::dataset::manifest::{DatasetManifest, crc32};
use crate::postproc::dataset::schema::{DatasetSchema, ResultVariable};
use crate::postproc::dataset::writer::{CLOSED_MARKER, COMPLETE_MARKER, MANIFEST_FILE, Row};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Serialized chunk body (mirrors the writer's private representation).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ChunkFile {
    chunk_version: u32,
    rows: Vec<Row>,
}

/// Supported chunk format version.
const SUPPORTED_CHUNK_VERSION: u32 = 1;

/// Errors produced by the dataset reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatasetReadError {
    /// A filesystem operation failed.
    Io(String),
    /// The manifest could not be parsed or is structurally invalid.
    Manifest(String),
    /// The schema could not be parsed or is structurally invalid.
    Schema(String),
    /// The schema version is newer than this crate understands.
    UnsupportedSchemaVersion(String),
    /// The dataset directory has no completion marker: it is incomplete.
    Incomplete(String),
    /// A chunk's bytes do not match the checksum recorded in the manifest.
    ChecksumMismatch {
        /// Chunk file name.
        file: String,
        /// Checksum recorded in the manifest.
        expected: u32,
        /// Checksum computed from the bytes actually read.
        actual: u32,
    },
    /// A chunk file is missing or malformed.
    CorruptChunk(String),
    /// A requested variable does not exist in the dataset.
    UnknownVariable(String),
}

impl std::fmt::Display for DatasetReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatasetReadError::Io(m) => write!(f, "i/o error: {}", m),
            DatasetReadError::Manifest(m) => write!(f, "manifest error: {}", m),
            DatasetReadError::Schema(m) => write!(f, "schema error: {}", m),
            DatasetReadError::UnsupportedSchemaVersion(m) => {
                write!(f, "unsupported schema version: {}", m)
            }
            DatasetReadError::Incomplete(m) => {
                write!(f, "incomplete dataset (no completion marker): {}", m)
            }
            DatasetReadError::ChecksumMismatch {
                file,
                expected,
                actual,
            } => write!(
                f,
                "checksum mismatch reading '{}': expected {:08x}, got {:08x}",
                file, expected, actual
            ),
            DatasetReadError::CorruptChunk(m) => write!(f, "corrupt chunk: {}", m),
            DatasetReadError::UnknownVariable(m) => write!(f, "unknown variable: {}", m),
        }
    }
}

impl std::error::Error for DatasetReadError {}

/// A dataset opened for reading, with its schema, manifest and rows loaded and
/// verified.
#[derive(Debug)]
pub struct DatasetReader {
    dir: PathBuf,
    schema: DatasetSchema,
    manifest: DatasetManifest,
    /// All rows, in chunk order then within-chunk order — the canonical,
    /// defined row order.
    rows: Vec<Row>,
    /// Completion marker present.
    completed: bool,
    /// Closed marker present (writer stopped, may be incomplete).
    closed: bool,
}

impl DatasetReader {
    /// Open and fully validate a dataset directory.
    ///
    /// Validation order, most fundamental first:
    ///
    /// 1. the directory and `manifest.json` exist and parse;
    /// 2. the schema version is supported ([`DatasetSchema::validate`]);
    /// 3. the manifest is structurally valid and still matches the schema;
    /// 4. every chunk named by the manifest is present and its CRC-32 matches;
    /// 5. the `COMPLETE` marker is present and the manifest status allows it.
    ///
    /// A missing completion marker yields [`DatasetReadError::Incomplete`], so
    /// a partially written dataset can never be mistaken for a valid one.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, DatasetReadError> {
        let dir = dir.as_ref().to_path_buf();
        if !dir.is_dir() {
            return Err(DatasetReadError::Io(format!(
                "'{}' is not a dataset directory",
                dir.display()
            )));
        }

        let manifest_path = dir.join(MANIFEST_FILE);
        let manifest_text = std::fs::read_to_string(&manifest_path)
            .map_err(|e| DatasetReadError::Io(format!("{}: {}", manifest_path.display(), e)))?;
        // The manifest stores the schema alongside the provenance, so one file
        // carries both halves of the dataset descriptor.
        let stored: StoredManifest = serde_json::from_str(&manifest_text)
            .map_err(|e| DatasetReadError::Manifest(e.to_string()))?;
        let schema = stored.schema;
        let manifest = stored.manifest;

        schema
            .validate()
            .map_err(DatasetReadError::UnsupportedSchemaVersion)?;
        manifest.validate().map_err(DatasetReadError::Manifest)?;
        if !manifest.schema_matches(&schema) {
            return Err(DatasetReadError::Schema(
                "manifest schema checksum does not match the stored schema".to_string(),
            ));
        }

        // Read and verify every chunk before trusting any of it.
        let mut rows: Vec<Row> = Vec::new();
        for record in &manifest.chunks {
            let path = dir.join(&record.file);
            let bytes = std::fs::read(&path)
                .map_err(|e| DatasetReadError::CorruptChunk(format!("{}: {}", record.file, e)))?;
            let actual = crc32(&bytes);
            if actual != record.checksum {
                return Err(DatasetReadError::ChecksumMismatch {
                    file: record.file.clone(),
                    expected: record.checksum,
                    actual,
                });
            }
            let chunk: ChunkFile = serde_json::from_slice(&bytes)
                .map_err(|e| DatasetReadError::CorruptChunk(format!("{}: {}", record.file, e)))?;
            if chunk.chunk_version != SUPPORTED_CHUNK_VERSION {
                return Err(DatasetReadError::CorruptChunk(format!(
                    "{}: unsupported chunk version {}",
                    record.file, chunk.chunk_version
                )));
            }
            rows.extend(chunk.rows);
        }

        let completed = dir.join(COMPLETE_MARKER).exists();
        let closed = dir.join(CLOSED_MARKER).exists();
        if !completed {
            return Err(DatasetReadError::Incomplete(format!(
                "'{}' has no '{}' marker ({} chunk(s) present, status '{}')",
                dir.display(),
                COMPLETE_MARKER,
                manifest.chunks.len(),
                manifest.status.name()
            )));
        }
        if !manifest.status.allows_completion() {
            return Err(DatasetReadError::Incomplete(format!(
                "completion marker present but run status is '{}'",
                manifest.status.name()
            )));
        }

        Ok(Self {
            dir,
            schema,
            manifest,
            rows,
            completed,
            closed,
        })
    }

    /// The dataset directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The verified schema.
    pub fn schema(&self) -> &DatasetSchema {
        &self.schema
    }

    /// The verified manifest.
    pub fn manifest(&self) -> &DatasetManifest {
        &self.manifest
    }

    /// Whether the completion marker was present.
    pub fn is_complete(&self) -> bool {
        self.completed
    }

    /// Whether a `CLOSED` marker was present.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Number of data rows read.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Describe every variable in the dataset, in defined order.
    ///
    /// Returns `(name, quantity, unit_symbol, location, per-sample value count)`.
    pub fn variables(&self) -> Vec<VariableInfo> {
        self.schema
            .variables
            .iter()
            .map(|v| VariableInfo {
                name: v.name.clone(),
                quantity: v.quantity.clone(),
                unit_symbol: v.unit_symbol.clone(),
                location: v.location,
                values_per_sample: v.values_per_sample(),
            })
            .collect()
    }

    /// Read a variable's whole series as one value per row.
    ///
    /// Missing samples read back as [`Scalar::NAN`].
    pub fn variable_array(&self, name: &str) -> Result<Vec<Scalar>, DatasetReadError> {
        self.schema
            .variable(name)
            .ok_or_else(|| DatasetReadError::UnknownVariable(name.to_string()))?;
        Ok(self.rows.iter().map(|r| r.get(name)).collect())
    }

    /// Read a variable converted into another compatible unit, transforming the
    /// numbers.
    pub fn variable_in_unit(
        &self,
        name: &str,
        target_symbol: &str,
        target_scale: Scalar,
        target_offset: Scalar,
    ) -> Result<Vec<Scalar>, DatasetReadError> {
        let var = self
            .schema
            .variable(name)
            .ok_or_else(|| DatasetReadError::UnknownVariable(name.to_string()))?;
        let target = ResultVariable::scalar(name, &var.quantity, target_symbol).with_unit(
            target_symbol,
            target_scale,
            target_offset,
        );
        let raw = self.variable_array(name)?;
        var.convert_values_to(&raw, &target)
            .map_err(DatasetReadError::Schema)
    }

    /// The series of time stamps, in row order (missing where a row is
    /// timeless).
    pub fn time_series(&self) -> Vec<Scalar> {
        self.rows
            .iter()
            .map(|r| r.time.unwrap_or(Scalar::NAN))
            .collect()
    }

    /// Read a variable restricted to rows whose time falls in `[start, end]`
    /// (inclusive), together with the matching time stamps.
    ///
    /// Rows without a time stamp are excluded from a time window.
    pub fn time_window(
        &self,
        name: &str,
        start: Scalar,
        end: Scalar,
    ) -> Result<(Vec<Scalar>, Vec<Scalar>), DatasetReadError> {
        self.schema
            .variable(name)
            .ok_or_else(|| DatasetReadError::UnknownVariable(name.to_string()))?;
        let mut times = Vec::new();
        let mut values = Vec::new();
        for row in &self.rows {
            if let Some(t) = row.time {
                if t >= start && t <= end {
                    times.push(t);
                    values.push(row.get(name));
                }
            }
        }
        Ok((times, values))
    }

    /// Read rows whose coordinate on `axis_name` falls in `[low, high]`
    /// (inclusive), for the named variable.
    ///
    /// The spatial position of row `i` is taken from the schema's coordinate
    /// axis: a row is a *cell* if the variable is cell-centred or the axis has
    /// `rows - 1` entries, otherwise a *node*. This means the two classic
    /// layouts (node-centred axis of length `N`; cell-centred axis of length
    /// `N - 1`) are both handled without extra configuration.
    pub fn spatial_region(
        &self,
        name: &str,
        axis_name: &str,
        low: Scalar,
        high: Scalar,
    ) -> Result<Vec<Scalar>, DatasetReadError> {
        self.schema
            .variable(name)
            .ok_or_else(|| DatasetReadError::UnknownVariable(name.to_string()))?;
        let axis = self
            .schema
            .coordinates
            .iter()
            .find(|c| c.name == axis_name)
            .ok_or_else(|| {
                DatasetReadError::Schema(format!("unknown coordinate axis '{}'", axis_name))
            })?;

        // Determine layout: cell-centred axes hold one fewer coordinate than the
        // rows (cell k spans node k..k+1 and is located at node k).
        let n = self.rows.len();
        let node_centred = axis.length == n;
        if !(node_centred || axis.length + 1 == n) {
            return Err(DatasetReadError::Schema(format!(
                "coordinate axis '{}' (length {}) is incompatible with {} data rows",
                axis_name, axis.length, n
            )));
        }

        let mut out = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            let coord = axis.values[i.min(axis.values.len().saturating_sub(1))];
            if !(coord >= low && coord <= high) {
                continue;
            }
            out.push(row.get(name));
        }
        Ok(out)
    }

    /// All rows, for callers that need the raw records (e.g. adapters).
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Variables keyed by name, for adapter convenience.
    pub fn variable_map(&self) -> BTreeMap<String, VariableInfo> {
        self.variables()
            .into_iter()
            .map(|v| (v.name.clone(), v))
            .collect()
    }
}

/// A compact description of one dataset variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableInfo {
    /// Variable name.
    pub name: String,
    /// Physical quantity.
    pub quantity: String,
    /// Unit symbol as stored.
    pub unit_symbol: String,
    /// Sampling location.
    pub location: crate::postproc::dataset::schema::SampleLocation,
    /// Values per time sample.
    pub values_per_sample: usize,
}

/// The manifest file actually written: it embeds the schema so a single JSON
/// file describes both the data layout and its provenance.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct StoredManifest {
    schema: DatasetSchema,
    manifest: DatasetManifest,
}

/// Serialize a `(schema, manifest)` pair into the on-disk manifest body.
pub(crate) fn serialize_stored(
    schema: &DatasetSchema,
    manifest: &DatasetManifest,
) -> Result<String, String> {
    let stored = StoredManifest {
        schema: schema.clone(),
        manifest: manifest.clone(),
    };
    serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())
}

/// Read just the manifest+metadata of a dataset without loading chunks.
///
/// Useful for inspecting provenance of an incomplete dataset: the checksums are
/// *not* verified here, so a caller can report the run status of a dataset that
/// [`DatasetReader::open`] would reject.
pub fn read_metadata(
    dir: impl AsRef<Path>,
) -> Result<(DatasetSchema, DatasetManifest), DatasetReadError> {
    let dir = dir.as_ref();
    let text = std::fs::read_to_string(dir.join(MANIFEST_FILE))
        .map_err(|e| DatasetReadError::Io(e.to_string()))?;
    let stored: StoredManifest =
        serde_json::from_str(&text).map_err(|e| DatasetReadError::Manifest(e.to_string()))?;
    Ok((stored.schema, stored.manifest))
}

/// Whether a directory currently carries a valid completion marker (a cheap
/// existence/status check that does not read any chunk).
pub fn is_marked_complete(dir: impl AsRef<Path>) -> bool {
    dir.as_ref().join(COMPLETE_MARKER).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postproc::dataset::manifest::{DatasetManifest, RunStatus};
    use crate::postproc::dataset::schema::{
        CoordinateAxis, DatasetSchema, ResultVariable, SampleLocation,
    };
    use crate::postproc::dataset::writer::{DatasetWriter, Row};

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "scico_reader_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn schema_with_coords(n: usize) -> DatasetSchema {
        let mut schema = DatasetSchema::with_uniform_time("run", n, 1.0);
        schema
            .add_coordinate(CoordinateAxis::uniform("x", 0.0, 1.0, n))
            .unwrap();
        schema
            .add_variable(
                ResultVariable::scalar("T", "temperature", "K")
                    .with_location(SampleLocation::Node)
                    .with_coordinates(&["x"])
                    .with_time_axis("time"),
            )
            .unwrap();
        schema
    }

    /// Build a finished dataset of `n` rows.
    fn build(dir: &Path, n: usize) -> Vec<(Scalar, Scalar)> {
        let mut w =
            DatasetWriter::create(dir, schema_with_coords(n), DatasetManifest::new("sim-1"))
                .unwrap();
        let mut model = Vec::new();
        for i in 0..n {
            let t = i as Scalar;
            let temp = 300.0 + i as Scalar;
            let mut r = Row::at(t);
            r.set("T", Some(temp));
            w.append(r).unwrap();
            model.push((t, temp));
        }
        w.complete();
        w.finish().unwrap();
        model
    }

    #[test]
    fn test_open_reads_metadata_variables_and_units() {
        let dir = scratch_dir("open");
        let model = build(&dir, 4);
        let reader = DatasetReader::open(&dir).unwrap();
        assert!(reader.is_complete());
        assert_eq!(reader.row_count(), 4);
        assert_eq!(reader.manifest().simulation_id, "sim-1");

        let vars = reader.variables();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "T");
        assert_eq!(vars[0].unit_symbol, "K");
        assert_eq!(vars[0].quantity, "temperature");

        let series = reader.variable_array("T").unwrap();
        let expected: Vec<Scalar> = model.iter().map(|(_, v)| *v).collect();
        assert_eq!(series, expected);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_round_trip_after_close_is_readable() {
        let dir = scratch_dir("reopen");
        build(&dir, 3);
        // Re-open in a fresh reader: simulates reading after the process closed.
        let reader = DatasetReader::open(&dir).unwrap();
        assert_eq!(
            reader.variable_array("T").unwrap(),
            vec![300.0, 301.0, 302.0]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_time_window_selects_inclusive_range() {
        let dir = scratch_dir("window");
        build(&dir, 6);
        let reader = DatasetReader::open(&dir).unwrap();
        let (times, values) = reader.time_window("T", 2.0, 4.0).unwrap();
        assert_eq!(times, vec![2.0, 3.0, 4.0]);
        assert_eq!(values, vec![302.0, 303.0, 304.0]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_spatial_region_selects_sub_region() {
        let dir = scratch_dir("region");
        build(&dir, 5);
        let reader = DatasetReader::open(&dir).unwrap();
        // Axis x = [0,1,2,3,4], node-centred (length == rows).
        let sub = reader.spatial_region("T", "x", 1.0, 3.0).unwrap();
        assert_eq!(sub, vec![301.0, 302.0, 303.0]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_variable_in_unit_converts_numbers() {
        let dir = scratch_dir("units");
        build(&dir, 3);
        let reader = DatasetReader::open(&dir).unwrap();
        // Treat the stored values as kelvin and read back degrees Celsius.
        let celsius = reader.variable_in_unit("T", "°C", 1.0, 273.15).unwrap();
        assert!((celsius[0] - (300.0 - 273.15)).abs() < 1e-9);
        assert!((celsius[1] - (301.0 - 273.15)).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_unknown_variable_is_rejected() {
        let dir = scratch_dir("unknown");
        build(&dir, 2);
        let reader = DatasetReader::open(&dir).unwrap();
        assert!(matches!(
            reader.variable_array("nope"),
            Err(DatasetReadError::UnknownVariable(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_missing_completion_marker_is_incomplete() {
        let dir = scratch_dir("incomplete");
        build(&dir, 2);
        std::fs::remove_file(dir.join(COMPLETE_MARKER)).unwrap();
        let err = DatasetReader::open(&dir).unwrap_err();
        assert!(
            matches!(err, DatasetReadError::Incomplete(_)),
            "got {err:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_corrupt_chunk_checksum_is_detected() {
        let dir = scratch_dir("corrupt");
        build(&dir, 3);
        let chunk = dir.join("chunk_000000.json");
        let mut bytes = std::fs::read(&chunk).unwrap();
        // Flip a byte without touching any length/JSON structure.
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0x01;
        std::fs::write(&chunk, &bytes).unwrap();
        let err = DatasetReader::open(&dir).unwrap_err();
        assert!(
            matches!(err, DatasetReadError::ChecksumMismatch { .. }),
            "got {err:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_metadata_of_incomplete_dataset_is_still_readable() {
        let dir = scratch_dir("meta");
        let mut w =
            DatasetWriter::create(&dir, schema_with_coords(2), DatasetManifest::new("sim-x"))
                .unwrap();
        w.write_chunk(vec![Row::at(0.0)]).unwrap();
        w.abort("user interrupt");
        w.close().unwrap();
        // Full open rejects it...
        assert!(DatasetReader::open(&dir).is_err());
        // ...but metadata inspection reports the real status.
        let (_schema, manifest) = read_metadata(&dir).unwrap();
        assert_eq!(manifest.status, RunStatus::Aborted);
        assert_eq!(manifest.error_summary, "user interrupt");
        assert!(!is_marked_complete(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
