//! SPICE-Level Analog Circuit Simulation (Phase 14).
//!
//! Provides Modified Nodal Analysis (MNA) matrix builder/solver, passive
//! and active device element stamps, and DC/AC/transient/noise analysis.
//!
//! # Modules
//!
//! - **mna**: Modified Nodal Analysis matrix builder and solver
//! - **devices**: R, C, L, diode, MOSFET, BJT, op-amp element stamps
//! - **analysis**: DC op-point, DC sweep, AC sweep, transient analysis
//! - **noise**: Basic noise analysis (thermal, shot, flicker)

pub mod analysis;
pub mod devices;
pub mod mna;
pub mod noise;

pub use analysis::{
    AcResult, AcSweepConfig, AnalysisType, DcOpResult, DcSweepConfig, FreqScale, NoiseConfig,
    TransientConfig, TransientResult, run_ac_sweep, run_dc_op, run_dc_sweep, run_transient,
};
pub use devices::{
    BjtStamp, CapacitorBlock, CapacitorStamp, CurrentSourceStamp, DiodeBlock, DiodeStamp,
    InductorBlock, InductorStamp, MosfetStamp, OpAmpStamp, ResistorBlock, ResistorStamp,
    VoltageSourceStamp,
};
pub use mna::{MnaMatrix, MnaSolution, solve_mna};
pub use noise::{flicker_noise_psd, shot_noise_psd, thermal_noise_psd};
