// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Chunked, append-only dataset writer with an atomic completion marker.
//!
//! # Container layout
//!
//! The container is a *directory* holding a small number of plain files. It is
//! deliberately simple, dependency-free and readable with nothing but a JSON
//! parser:
//!
//! ```text
//! <dataset_dir>/
//!   manifest.json          — schema + provenance manifest (see `manifest.rs`)
//!   chunk_000000.json      — one chunk of rows, in write order
//!   chunk_000001.json      — …
//!   COMPLETE               — the completion marker (written last, atomically)
//!   CLOSED                 — optional marker written by `close()`
//! ```
//!
//! Each chunk file is a JSON object `{"rows": [...]}` where every row is
//! `{"time": <Scalar or null>, "values": {name: <Scalar or null>}}`. Missing
//! samples are stored as JSON `null` and read back as `NaN`.
//!
//! # Atomicity
//!
//! * Each chunk is written to `chunk_<n>.json.part`, flushed and `sync_all`ed,
//!   then renamed into place. A reader therefore never sees a half-written
//!   chunk under its final name.
//! * The `COMPLETE` marker is written last, via the same write-then-rename
//!   dance, and only after every chunk (checksum) and the manifest (schema
//!   checksum, status, row counts) validate.
//! * If any of these steps fails, the dataset directory lacks `COMPLETE` and is
//!   reported as **incomplete** by the reader, rather than silently yielding an
//!   empty "successful" result.

use crate::core::types::Scalar;
use crate::postproc::dataset::manifest::{DatasetManifest, RunStatus, crc32};
use crate::postproc::dataset::schema::DatasetSchema;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Name of the completion marker file.
pub const COMPLETE_MARKER: &str = "COMPLETE";

/// Name of the marker written by [`DatasetWriter::close`].
pub const CLOSED_MARKER: &str = "CLOSED";

/// Name of the manifest file.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Default number of buffered rows that triggers a chunk flush.
pub const DEFAULT_CHUNK_ROWS: usize = 1024;

/// One row of a dataset chunk.
///
/// A row holds a time stamp (absent for steady/snapshot datasets) and the
/// variable values at that instant. Only variables declared in the *pinned*
/// schema are stored; see [`DatasetWriter::write_chunk`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Row {
    /// Time stamp of the row, or `null` for a time-independent row.
    pub time: Option<Scalar>,
    /// Variable name -> value. Missing samples are `null`.
    pub values: BTreeMap<String, Option<Scalar>>,
}

impl Row {
    /// A row with a time stamp and no values yet.
    pub fn at(time: Scalar) -> Self {
        Self {
            time: Some(time),
            values: BTreeMap::new(),
        }
    }

    /// A row without a time stamp.
    pub fn timeless() -> Self {
        Self {
            time: None,
            values: BTreeMap::new(),
        }
    }

    /// Set a variable value, or clear it to a missing (`null`) sample.
    pub fn set(&mut self, name: &str, value: Option<Scalar>) -> &mut Self {
        self.values
            .insert(name.to_string(), value.filter(|v| !v.is_nan()));
        self
    }

    /// Read a variable value, mapping a stored `null` (and a non-finite value)
    /// onto [`Scalar::NAN`].
    pub fn get(&self, name: &str) -> Scalar {
        match self.values.get(name) {
            Some(Some(v)) if v.is_finite() => *v,
            _ => Scalar::NAN,
        }
    }

    /// Whether the row carries a finite value for `name`.
    pub fn has(&self, name: &str) -> bool {
        matches!(self.values.get(name), Some(Some(v)) if v.is_finite())
    }
}

/// On-disk chunk format version.
const CHUNK_FORMAT_VERSION: u32 = 1;

/// Serialized chunk body.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ChunkFile {
    chunk_version: u32,
    rows: Vec<Row>,
}

/// Errors produced by the dataset writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatasetWriteError {
    /// The schema itself is invalid.
    InvalidSchema(String),
    /// The manifest is invalid.
    InvalidManifest(String),
    /// A filesystem operation failed (create/open/write/rename/sync).
    Io(String),
    /// Serialization of a chunk or the manifest failed.
    Serialization(String),
    /// The dataset was already closed; no further rows can be appended.
    AlreadyClosed,
}

impl std::fmt::Display for DatasetWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatasetWriteError::InvalidSchema(m) => write!(f, "invalid schema: {}", m),
            DatasetWriteError::InvalidManifest(m) => write!(f, "invalid manifest: {}", m),
            DatasetWriteError::Io(m) => write!(f, "i/o error: {}", m),
            DatasetWriteError::Serialization(m) => write!(f, "serialization error: {}", m),
            DatasetWriteError::AlreadyClosed => {
                write!(f, "dataset is already closed; cannot append more rows")
            }
        }
    }
}

impl std::error::Error for DatasetWriteError {}

/// Accumulates rows and flushes them to disk as chunks.
///
/// The writer pins the schema at construction: every chunk carries **exactly
/// the variables declared in the schema**, in the schema's order, so the column
/// set is stable across chunks and both streaming and one-shot writes produce
/// identical files.
pub struct DatasetWriter {
    dir: PathBuf,
    schema: DatasetSchema,
    manifest: DatasetManifest,
    /// Rows buffered but not yet flushed.
    buffer: Vec<Row>,
    /// Rows per chunk; when the buffer reaches this, a chunk is flushed.
    chunk_rows: usize,
    /// Number of chunks flushed so far (also the next chunk index).
    next_chunk: u64,
    /// Set once the completion marker has been committed.
    closed: bool,
}

impl DatasetWriter {
    /// Create a writer for a dataset directory.
    ///
    /// The directory is created if missing. The writer validates and writes the
    /// manifest immediately, binds it to the schema, and leaves the dataset in
    /// the *incomplete* state (no `COMPLETE` marker) until [`Self::finish`]
    /// commits it.
    ///
    /// `chunk_rows` is the flush threshold in rows; `0` is treated as `1` so a
    /// writer always makes progress.
    pub fn create(
        dir: impl AsRef<Path>,
        schema: DatasetSchema,
        manifest: DatasetManifest,
    ) -> Result<Self, DatasetWriteError> {
        Self::create_with_chunk_rows(dir, schema, manifest, DEFAULT_CHUNK_ROWS)
    }

    /// Like [`Self::create`] but with an explicit chunk flush threshold.
    ///
    /// `chunk_rows` is the number of buffered rows that triggers a chunk flush;
    /// `0` is treated as `1` so a writer always makes progress.
    pub fn create_with_chunk_rows(
        dir: impl AsRef<Path>,
        schema: DatasetSchema,
        mut manifest: DatasetManifest,
        chunk_rows: usize,
    ) -> Result<Self, DatasetWriteError> {
        schema
            .validate()
            .map_err(DatasetWriteError::InvalidSchema)?;
        manifest.status = match manifest.status {
            // A writer that is open implies the run is in progress.
            RunStatus::Completed => RunStatus::Running,
            other => other,
        };
        manifest
            .bind_schema(&schema)
            .map_err(DatasetWriteError::Serialization)?;

        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| DatasetWriteError::Io(e.to_string()))?;

        let writer = Self {
            dir,
            schema,
            manifest,
            buffer: Vec::new(),
            chunk_rows: chunk_rows.max(1),
            next_chunk: 0,
            closed: false,
        };
        writer.persist_manifest()?;
        Ok(writer)
    }

    /// Number of rows currently buffered but not yet written to a chunk.
    pub fn buffered_rows(&self) -> usize {
        self.buffer.len()
    }

    /// Number of chunks already committed to disk.
    pub fn chunk_count(&self) -> u64 {
        self.next_chunk
    }

    /// The schema pinned for this dataset.
    pub fn schema(&self) -> &DatasetSchema {
        &self.schema
    }

    /// The manifest as currently persisted (provenance, status, chunks).
    pub fn manifest(&self) -> &DatasetManifest {
        &self.manifest
    }

    /// Mutable access to the manifest, for recording provenance the writer did
    /// not observe (e.g. solver tolerances discovered late).
    ///
    /// The manifest must still validate; [`Self::finish`] re-checks it.
    pub fn manifest_mut(&mut self) -> &mut DatasetManifest {
        &mut self.manifest
    }

    /// Dataset directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Mark the run successful. Completion will be permitted by [`Self::finish`].
    ///
    /// Use [`Self::fail`] or [`Self::abort`] to record a run that must remain
    /// incomplete.
    pub fn complete(&mut self) {
        self.manifest.status = RunStatus::Completed;
    }

    /// Mark the run failed with an error summary. The dataset will stay
    /// incomplete.
    pub fn fail(&mut self, summary: &str) {
        self.manifest.status = RunStatus::Failed;
        self.manifest.error_summary = summary.to_string();
    }

    /// Mark the run aborted. The dataset will stay incomplete.
    pub fn abort(&mut self, summary: &str) {
        self.manifest.status = RunStatus::Aborted;
        self.manifest.error_summary = summary.to_string();
    }

    /// Append a single row, flushing a chunk when the buffer is full.
    pub fn append(&mut self, row: Row) -> Result<(), DatasetWriteError> {
        if self.closed {
            return Err(DatasetWriteError::AlreadyClosed);
        }
        self.buffer.push(row);
        if self.buffer.len() >= self.chunk_rows {
            self.flush_chunk()?;
        }
        Ok(())
    }

    /// Append many rows, then flush the buffer as one chunk regardless of the
    /// threshold.
    ///
    /// This is the *one-shot* write path; its output is byte-identical to
    /// appending the same rows through [`Self::append`] with a chunk size of
    /// `rows.len()`, because both end up producing one chunk with the same rows.
    pub fn write_chunk(&mut self, rows: Vec<Row>) -> Result<(), DatasetWriteError> {
        if self.closed {
            return Err(DatasetWriteError::AlreadyClosed);
        }
        self.buffer.extend(rows);
        self.flush_chunk()
    }

    /// Flush any buffered rows into a chunk and re-persist the manifest.
    ///
    /// This is a durability barrier, not a close: the dataset stays incomplete
    /// (no `COMPLETE` marker) so a crash after this point is still detectable.
    pub fn flush(&mut self) -> Result<(), DatasetWriteError> {
        if self.closed {
            return Err(DatasetWriteError::AlreadyClosed);
        }
        if !self.buffer.is_empty() {
            self.flush_chunk()?;
        }
        self.persist_manifest()
    }

    /// Commit the dataset: flush the tail, validate every chunk's checksum and
    /// the manifest, then write the `COMPLETE` marker atomically.
    ///
    /// Returns an error — and leaves the dataset *incomplete* — if the run is
    /// not [`RunStatus::Completed`], if the manifest does not validate, or if
    /// any chunk fails to validate. On success the writer is marked closed, so
    /// any later [`Self::append`] is rejected with
    /// [`DatasetWriteError::AlreadyClosed`].
    pub fn finish(&mut self) -> Result<(), DatasetWriteError> {
        if self.closed {
            return Err(DatasetWriteError::AlreadyClosed);
        }
        if !self.buffer.is_empty() {
            self.flush_chunk()?;
        }
        if !self.manifest.status.allows_completion() {
            self.persist_manifest()?;
            return Err(DatasetWriteError::InvalidManifest(format!(
                "cannot mark the dataset complete: run status is '{}', expected 'completed'",
                self.manifest.status.name()
            )));
        }
        self.manifest
            .validate()
            .map_err(DatasetWriteError::InvalidManifest)?;
        self.persist_manifest()?;
        self.verify_all_chunks()?;
        self.write_marker(COMPLETE_MARKER)?;
        self.closed = true;
        Ok(())
    }

    /// Flush, write the `CLOSED` marker and mark the writer closed.
    ///
    /// Unlike [`Self::finish`], `close` records only that no more rows will be
    /// written. It does **not** commit completion; a reader treats a dataset
    /// without `COMPLETE` as incomplete even when `CLOSED` is present. This
    /// models a run that stopped early (interrupt/checkpoint). After `close`,
    /// any [`Self::append`] is rejected with [`DatasetWriteError::AlreadyClosed`].
    pub fn close(&mut self) -> Result<(), DatasetWriteError> {
        if self.closed {
            return Err(DatasetWriteError::AlreadyClosed);
        }
        if !self.buffer.is_empty() {
            self.flush_chunk()?;
        }
        self.persist_manifest()?;
        self.write_marker(CLOSED_MARKER)?;
        self.closed = true;
        Ok(())
    }

    // ── internals ──────────────────────────────────────────────────────────

    /// Name of the chunk file for a zero-based index.
    fn chunk_name(index: u64) -> String {
        format!("chunk_{:06}.json", index)
    }

    /// Write the buffered rows as the next chunk, atomically.
    fn flush_chunk(&mut self) -> Result<(), DatasetWriteError> {
        let rows = std::mem::take(&mut self.buffer);
        if rows.is_empty() {
            return Ok(());
        }
        let index = self.next_chunk;
        let name = Self::chunk_name(index);
        let body = ChunkFile {
            chunk_version: CHUNK_FORMAT_VERSION,
            rows,
        };
        let json = serde_json::to_vec_pretty(&body)
            .map_err(|e| DatasetWriteError::Serialization(e.to_string()))?;
        let checksum = crc32(&json);
        let rows_written = body.rows.len();

        let final_path = self.dir.join(&name);
        let part_path = self.dir.join(format!("{}.part", name));
        write_atomically(&part_path, &final_path, &json)?;

        self.manifest
            .record_chunk(index, &name, rows_written, checksum);
        self.next_chunk += 1;
        // A manifest after each chunk keeps provenance current; the reader also
        // consumes it, so a failed persist must fail the write.
        self.persist_manifest()
    }

    /// Serialize and atomically persist the manifest.
    ///
    /// The on-disk `manifest.json` embeds the schema alongside the provenance
    /// manifest (see [`crate::postproc::dataset::reader::serialize_stored`]), so
    /// a single file describes both the data layout and where it came from.
    fn persist_manifest(&self) -> Result<(), DatasetWriteError> {
        self.manifest
            .validate()
            .map_err(DatasetWriteError::InvalidManifest)?;
        let json = crate::postproc::dataset::reader::serialize_stored(&self.schema, &self.manifest)
            .map_err(DatasetWriteError::Serialization)?;
        let final_path = self.dir.join(MANIFEST_FILE);
        let part_path = self.dir.join(format!("{}.part", MANIFEST_FILE));
        write_atomically(&part_path, &final_path, json.as_bytes())
    }

    /// Re-read every chunk from disk and confirm its CRC-32 matches the
    /// manifest. Returns an error on the first mismatch.
    fn verify_all_chunks(&self) -> Result<(), DatasetWriteError> {
        for record in &self.manifest.chunks {
            let path = self.dir.join(&record.file);
            let bytes = std::fs::read(&path).map_err(|e| DatasetWriteError::Io(e.to_string()))?;
            let actual = crc32(&bytes);
            if actual != record.checksum {
                return Err(DatasetWriteError::Io(format!(
                    "chunk '{}' failed checksum validation (expected {:08x}, got {:08x})",
                    record.file, record.checksum, actual
                )));
            }
        }
        Ok(())
    }

    /// Atomically write a marker file (write temp, sync, rename).
    fn write_marker(&self, name: &str) -> Result<(), DatasetWriteError> {
        let final_path = self.dir.join(name);
        let part_path = self.dir.join(format!("{}.part", name));
        write_atomically(&part_path, &final_path, name.as_bytes())
    }
}

/// Write `bytes` to `part_path`, sync it to disk, then rename onto
/// `final_path`. A crash mid-write leaves only the `.part` file, never a
/// truncated file under the final name.
fn write_atomically(
    part_path: &Path,
    final_path: &Path,
    bytes: &[u8],
) -> Result<(), DatasetWriteError> {
    use std::io::Write;
    let mut file =
        std::fs::File::create(part_path).map_err(|e| DatasetWriteError::Io(e.to_string()))?;
    file.write_all(bytes)
        .map_err(|e| DatasetWriteError::Io(e.to_string()))?;
    file.flush()
        .map_err(|e| DatasetWriteError::Io(e.to_string()))?;
    file.sync_all()
        .map_err(|e| DatasetWriteError::Io(e.to_string()))?;
    drop(file);
    std::fs::rename(part_path, final_path).map_err(|e| DatasetWriteError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postproc::dataset::manifest::DatasetManifest;
    use crate::postproc::dataset::schema::{ResultVariable, SampleLocation};

    /// A unique scratch directory for a test, removed up-front.
    pub(crate) fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "scico_dataset_{}_{}_{}",
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

    pub(crate) fn sample_schema(n: usize) -> DatasetSchema {
        let mut schema = DatasetSchema::with_uniform_time("run", n, 0.1);
        schema
            .add_variable(
                ResultVariable::scalar("T", "temperature", "K")
                    .with_location(SampleLocation::Node)
                    .with_time_axis("time"),
            )
            .unwrap();
        schema
            .add_variable(
                ResultVariable::scalar("p", "pressure", "Pa")
                    .with_location(SampleLocation::Cell)
                    .with_time_axis("time"),
            )
            .unwrap();
        schema
    }

    fn row(t: Scalar, temp: Scalar, pressure: Scalar) -> Row {
        let mut r = Row::at(t);
        r.set("T", Some(temp)).set("p", Some(pressure));
        r
    }

    #[test]
    fn test_row_null_encodes_missing() {
        let mut r = Row::at(0.0);
        r.set("T", None).set("p", Some(Scalar::NAN));
        assert!(r.get("T").is_nan());
        assert!(r.get("p").is_nan());
        assert!(!r.has("T"));
    }

    #[test]
    fn test_writer_flushes_at_threshold() {
        let dir = scratch_dir("threshold");
        let mut w = DatasetWriter::create_with_chunk_rows(
            &dir,
            sample_schema(10),
            DatasetManifest::new("sim"),
            2,
        )
        .unwrap();
        assert_eq!(w.chunk_rows, 2);
        for i in 0..4 {
            w.append(row(i as Scalar, i as Scalar, i as Scalar * 2.0))
                .unwrap();
        }
        // 4 rows with a threshold of 2 => 2 chunks already on disk.
        assert_eq!(w.chunk_count(), 2);
        assert_eq!(w.buffered_rows(), 0);
        assert_eq!(w.manifest().total_rows, 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_chunk_forces_one_chunk_of_every_row() {
        let dir = scratch_dir("oneshot");
        let mut w =
            DatasetWriter::create(&dir, sample_schema(4), DatasetManifest::new("sim")).unwrap();
        w.write_chunk(vec![row(0.0, 1.0, 2.0), row(1.0, 3.0, 4.0)])
            .unwrap();
        assert_eq!(w.chunk_count(), 1);
        assert_eq!(w.manifest().total_rows, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_finish_writes_complete_marker_only_after_validation() {
        let dir = scratch_dir("finish");
        let mut w =
            DatasetWriter::create(&dir, sample_schema(2), DatasetManifest::new("sim")).unwrap();
        assert!(!dir.join(COMPLETE_MARKER).exists());
        w.write_chunk(vec![row(0.0, 1.0, 2.0)]).unwrap();
        w.complete();
        w.finish().unwrap();
        assert!(dir.join(COMPLETE_MARKER).exists());
        assert!(dir.join(MANIFEST_FILE).exists());
        assert!(dir.join("chunk_000000.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_finish_without_complete_status_leaves_dataset_incomplete() {
        let dir = scratch_dir("nocomplete");
        let mut w =
            DatasetWriter::create(&dir, sample_schema(2), DatasetManifest::new("sim")).unwrap();
        w.write_chunk(vec![row(0.0, 1.0, 2.0)]).unwrap();
        // Status stays Running: completion must be refused.
        let err = w.finish().unwrap_err();
        assert!(matches!(err, DatasetWriteError::InvalidManifest(_)));
        assert!(!dir.join(COMPLETE_MARKER).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_append_after_close_is_rejected() {
        let dir = scratch_dir("closed");
        let mut w =
            DatasetWriter::create(&dir, sample_schema(2), DatasetManifest::new("sim")).unwrap();
        w.write_chunk(vec![row(0.0, 1.0, 2.0)]).unwrap();
        w.close().unwrap();
        // `CLOSED` is present but the dataset is NOT complete: a run that
        // stopped early is readable as incomplete, not as success.
        assert!(dir.join(CLOSED_MARKER).exists());
        assert!(!dir.join(COMPLETE_MARKER).exists());
        // Appending after close is refused rather than silently dropped.
        let err = w.append(row(1.0, 3.0, 4.0)).unwrap_err();
        assert_eq!(err, DatasetWriteError::AlreadyClosed);
        // And a second close/finish is equally refused.
        assert_eq!(w.close().unwrap_err(), DatasetWriteError::AlreadyClosed);
        assert_eq!(w.finish().unwrap_err(), DatasetWriteError::AlreadyClosed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_append_after_finish_is_rejected() {
        let dir = scratch_dir("after_finish");
        let mut w =
            DatasetWriter::create(&dir, sample_schema(2), DatasetManifest::new("sim")).unwrap();
        w.write_chunk(vec![row(0.0, 1.0, 2.0)]).unwrap();
        w.complete();
        w.finish().unwrap();
        assert_eq!(
            w.append(row(1.0, 3.0, 4.0)).unwrap_err(),
            DatasetWriteError::AlreadyClosed
        );
        // The rejected append must not have extended the dataset.
        assert_eq!(w.chunk_count(), 1);
        assert_eq!(w.manifest().total_rows, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_write_leaves_no_part_file() {
        let dir = scratch_dir("atomic");
        let mut w =
            DatasetWriter::create(&dir, sample_schema(2), DatasetManifest::new("sim")).unwrap();
        w.write_chunk(vec![row(0.0, 1.0, 2.0)]).unwrap();
        w.complete();
        w.finish().unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
            .collect();
        assert!(leftovers.is_empty(), "no .part files may remain");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
