// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Phase 39 — self-describing result datasets, provenance and data
//! compatibility.
//!
//! This module upgrades simulation output from "export a file" to a
//! *composable, queryable, auditable and long-term readable* dataset. It
//! extends the existing recorder/report/export facilities rather than
//! introducing a second, incompatible data outlet: adapters bridge the two.
//!
//! # Structure
//!
//! * [`schema`] — versioned description of result variables, dimensions,
//!   coordinate axes and units (with real unit conversion).
//! * [`manifest`] — run provenance: simulation id, model signature, run-config
//!   hash, parameters, solver tolerances, RNG seed, backend, library versions,
//!   creation time, status, error summary, per-chunk CRC-32 checksums.
//! * [`writer`] — chunked/append writes with atomic completion marker.
//! * [`reader`] — read by variable, time window and spatial region after full
//!   validation of every checksum.
//! * [`migration`] — schema version upgrade and compatibility strategy.
//! * [`adapter`] — CSV/JSON import/export with explicitly recorded loss.
//!
//! # Container
//!
//! The first-version container is a directory containing `manifest.json`, one
//! `chunk_<n>.json` per writer flush, and a `COMPLETE` marker committed last.
//! It is intentionally dependency-free and self-contained: a dataset is
//! readable with nothing but a JSON parser. The stable [`schema`] is decoupled
//! from this container so a future, differently-compressed container can be
//! dropped in without changing the schema or manifest.
//!
//! # Example
//!
//! ```no_run
//! use scico_rs::postproc::dataset::{
//!     DatasetManifest, DatasetReader, DatasetSchema, DatasetWriter, ResultVariable, Row,
//! };
//!
//! # fn main() -> Result<(), String> {
//! let mut schema = DatasetSchema::with_uniform_time("run", 3, 0.1);
//! schema.add_variable(
//!     ResultVariable::scalar("T", "temperature", "K").with_time_axis("time"),
//! )?;
//!
//! let dir = std::env::temp_dir().join("scico_doc_example");
//! let mut writer =
//!     DatasetWriter::create(&dir, schema, DatasetManifest::new("sim-1"))
//!         .map_err(|e| e.to_string())?;
//! for i in 0..3 {
//!     let mut row = Row::at(i as f64 * 0.1);
//!     row.set("T", Some(300.0 + i as f64));
//!     writer.append(row).map_err(|e| e.to_string())?;
//! }
//! writer.complete();
//! writer.finish().map_err(|e| e.to_string())?;
//!
//! let reader = DatasetReader::open(&dir).map_err(|e| e.to_string())?;
//! let temps = reader.variable_array("T").map_err(|e| e.to_string())?;
//! assert_eq!(temps.len(), 3);
//! # std::fs::remove_dir_all(&dir).ok();
//! # Ok(())
//! # }
//! ```

pub mod adapter;
pub mod manifest;
pub mod migration;
pub mod reader;
pub mod schema;
pub mod writer;

// ── Schema ────────────────────────────────────────────────────────────────
pub use schema::{
    CURRENT_SCHEMA_VERSION, CoordinateAxis, DatasetSchema, MIN_SUPPORTED_SCHEMA_VERSION,
    MissingValueEncoding, Precision, ResultVariable, SampleLocation, TAGS_DIAGNOSTIC,
    TAGS_SENSOR_READING, TimeAxis,
};

// ── Manifest ──────────────────────────────────────────────────────────────
pub use manifest::{
    ChunkRecord, DatasetManifest, ManifestParameter, RedactionPolicy, RunStatus, SolverRecord,
    crc32,
};

// ── Writer ────────────────────────────────────────────────────────────────
pub use writer::{
    CLOSED_MARKER, COMPLETE_MARKER, DatasetWriteError, DatasetWriter, MANIFEST_FILE, Row,
};

// ── Reader ────────────────────────────────────────────────────────────────
pub use reader::{
    DatasetReadError, DatasetReader, VariableInfo, is_marked_complete, read_metadata,
};

// ── Migration ─────────────────────────────────────────────────────────────
pub use migration::{
    MigrationError, MigrationReport, TAG_SENSOR_READING as MIGRATION_TAG_SENSOR_READING,
    is_current, is_readable, migrate, variables_with_tag,
};

// ── Adapters ──────────────────────────────────────────────────────────────
pub use adapter::{AdapterError, DatasetAdapter, ExportOutcome, ImportSpec, ImportedDataset};
