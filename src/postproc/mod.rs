// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
#![allow(
    clippy::type_complexity,
    clippy::format_push_string,
    clippy::useless_format
)]

//! Post-Processing & Visualization (Phase 33).
//!
//! Provides data recording/replay, offline analysis, chart/contour/vector
//! visualization, report generation, batch simulation, and HIL support.

pub mod batch;
pub mod dataset;
pub mod hilsupport;
pub mod recorder;
pub mod reporting;
pub mod visualization;

pub use batch::{
    BatchSimManager, BatchTask, BatchTaskStatus, DesignParam, OptimizationLoop, ParameterSweep,
    SolverBenchConfig, SolverBenchmarkResult, bench_solver, benchmark_report, benchmark_speedup,
    run_benchmark_suite, run_diagram_bounded_public,
};
pub use dataset::{
    AdapterError, CLOSED_MARKER, COMPLETE_MARKER, CURRENT_SCHEMA_VERSION, ChunkRecord,
    CoordinateAxis, DatasetAdapter, DatasetManifest, DatasetReadError, DatasetReader,
    DatasetSchema, DatasetWriteError, DatasetWriter, ExportOutcome, ImportSpec, ImportedDataset,
    MANIFEST_FILE, ManifestParameter, MigrationError, MigrationReport, MissingValueEncoding,
    Precision, RedactionPolicy, ResultVariable, Row, RunStatus, SampleLocation, SolverRecord,
    TAGS_DIAGNOSTIC, TAGS_SENSOR_READING, TimeAxis, VariableInfo, crc32, is_current,
    is_marked_complete, is_readable, migrate, read_metadata, variables_with_tag,
};
pub use hilsupport::{
    HilConfig, HilIoChannels, HilIoExchange, HilRunner, HilTransport, LoopbackTransport,
    SimulatedTransport,
};
pub use recorder::{
    DataRecorder, DataReplayer, FieldRecorder3D, FieldSnapshot3D, OfflineAnalysis, RecorderConfig,
};
pub use reporting::{DataExporter, ExportFormat, ReportSection, ReportTable, SimulationReport};
pub use visualization::{
    ChartGenerator, ChartType, ContourGenerator, CurveData, IsoSurface3D, VectorFieldVisualization,
    VolumeSlice3D, vector_field_slice,
};
