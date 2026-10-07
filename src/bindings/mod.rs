// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
#![allow(
    clippy::type_complexity,
    clippy::format_push_string,
    clippy::useless_format,
    clippy::needless_question_mark
)]

//! Cross-Platform & Script Bindings (Phase 34).
//!
//! Provides Python scripting interface, plugin system, CAD/CAE data
//! interfaces (STEP, STL, mesh), cross-platform abstractions, and
//! cloud/distributed deployment support. Phase 41 adds a stable C ABI facade
//! ([`c_api`]) and plugin compatibility/diagnostics.

pub mod c_api;
pub mod data_io;
pub mod platform;
pub mod plugins;
pub mod python;

pub use c_api::{
    SCICO_C_API_VERSION, SCICO_MAX_OUTPUTS, ScicoHandle, ScicoStatus, scico_api_version,
    scico_config, scico_create, scico_free, scico_output_count, scico_read_output, scico_run,
    scico_status_from_code, scico_step_count,
};

pub use data_io::{
    MeshData, MeshElement, MeshFormat, StlMesh, StlTriangle, export_mesh, export_stl, import_mesh,
    import_stl,
};
pub use platform::{
    CloudConfig, DistributedRunner, DistributedTask, Platform, TaskPartition, current_platform,
    normalize_path,
};
pub use plugins::{
    ApiVersion, BlockRegistry, Capability, CompatDecision, CompatibilityPolicy, ExtendedManifest,
    PlatformRequirement, Plugin, PluginContribution, PluginFailure, PluginLoader, PluginManager,
    PluginManifest, PluginStage, PostProcessor, PostProcessorRegistry, SolverRegistry,
    SourcePolicy, StagedPlugin, check_compatibility, compatibility_of_manifest, detect_conflicts,
};
pub use python::{
    connect_blocks, get_result_data, get_simulation_status, pause_simulation, query_library,
    read_signal, register_custom_block, resume_simulation, run_simulation, set_block_parameter,
};
