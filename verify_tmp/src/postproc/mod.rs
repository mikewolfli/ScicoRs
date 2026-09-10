#![allow(clippy::type_complexity, clippy::format_push_string, clippy::useless_format)]

//! Post-Processing & Visualization (Phase 33).
//!
//! Provides data recording/replay, offline analysis, chart/contour/vector
//! visualization, report generation, batch simulation, and HIL support.

pub mod batch;
pub mod hilsupport;
pub mod recorder;
pub mod reporting;
pub mod visualization;

pub use batch::{
    BatchSimManager, BatchTask, BatchTaskStatus, DesignParam, OptimizationLoop, ParameterSweep,
    SolverBenchConfig, SolverBenchmarkResult, bench_solver, benchmark_report, benchmark_speedup,
    run_benchmark_suite,
};
pub use hilsupport::{HilConfig, HilIoChannels, HilRunner};
pub use recorder::{DataRecorder, DataReplayer, FieldRecorder3D, FieldSnapshot3D, OfflineAnalysis, RecorderConfig};
pub use reporting::{DataExporter, ExportFormat, ReportSection, ReportTable, SimulationReport};
pub use visualization::{
    ChartGenerator, ChartType, ContourGenerator, CurveData, IsoSurface3D, VectorFieldVisualization,
    VolumeSlice3D, vector_field_slice,
};
