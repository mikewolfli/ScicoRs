//! Semiconductor Device Physics / TCAD Simulation (Phase 13).
//!
//! Provides transistor-level physics models for semiconductor device
//! simulation: MOSFET, BJT, carrier transport, IV/CV characterization,
//! and basic process simulation.
//!
//! # Modules
//!
//! - **physics**: Physical constants, carrier transport models
//! - **mosfet**: MOSFET DC/AC models (Shichman-Hodges Level 1)
//! - **bjt**: BJT Ebers-Moll model
//! - **iv_curve**: IV/CV curve computation utilities
//! - **process**: Basic process simulation (diffusion, implant, oxidation)

pub mod bjt;
pub mod iv_curve;
pub mod mosfet;
pub mod physics;
pub mod process;

pub use bjt::{BjtBlock, BjtModel, bjt_base_current, bjt_collector_current};
pub use iv_curve::{mosfet_cv_curve, mosfet_iv_curve, mosfet_transfer_curve};
pub use mosfet::{MosfetBlock, MosfetModel, mosfet_drain_current, mosfet_gds, mosfet_gm};
pub use physics::{
    EPSILON_0, K_B, MobilityModel, Q, T_300K, V_T_300K, built_in_potential, depletion_width,
    drift_diffusion_current, thermal_voltage,
};
pub use process::{OxidationAmbient, diffusion_profile, implant_range, oxide_thickness};

pub mod bsim;
pub mod reliability;
