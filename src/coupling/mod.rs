#![allow(
    clippy::type_complexity,
    clippy::needless_question_mark,
    clippy::format_push_string,
    clippy::useless_format
)]

//! Multi-Physics Coupling Bus (Phase 32).
//!
//! Provides a unified coupling bus for multi-physics simulation:
//! - Physics field type registry
//! - Field mapping & interpolation between meshes
//! - Cross-scale coupling (nano → micro → meter → cosmic)
//! - Convergence control & coupling iteration scheduling

pub mod bus;
pub mod convergence;
pub mod cross_scale;
pub mod field_mapping;

pub use bus::FieldMappingMethod;
pub use bus::{
    CouplingBus, CouplingInterface, FieldData, PhysicsField, QuantityType, TimeSyncMode,
};
pub use convergence::{ConvergenceCriteria, CouplingScheduler, TimeSyncManager};
pub use cross_scale::{CrossScaleCoupling, RveHomogenization, ScaleBridge, ScaleLevel};
pub use field_mapping::{FieldMapper, RbfType};

// Integration tests (only compiled in test mode)
#[cfg(test)]
mod integration_tests;
