#![allow(clippy::type_complexity, clippy::format_push_string, clippy::useless_format, clippy::needless_question_mark)]

//! Cross-Platform & Script Bindings (Phase 34).
//!
//! Provides Python scripting interface, plugin system, CAD/CAE data
//! interfaces (STEP, STL, mesh), cross-platform abstractions, and
//! cloud/distributed deployment support.

pub mod data_io;
pub mod platform;
pub mod plugins;
pub mod python;

pub use data_io::{MeshData, MeshElement, MeshFormat, StlMesh, StlTriangle,
    export_mesh, export_stl, import_mesh, import_stl};
pub use platform::{CloudConfig, DistributedRunner, DistributedTask, Platform, TaskPartition, current_platform, normalize_path};
pub use plugins::{BlockRegistry, Plugin, PluginManager, PluginManifest, PostProcessor, PostProcessorRegistry, SolverRegistry};
pub use python::{connect_blocks, get_result_data, get_simulation_status, pause_simulation, query_library, read_signal, register_custom_block, resume_simulation, run_simulation, set_block_parameter};
