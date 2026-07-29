//! Physiological Systems & Biomedical Simulation (Phase 23).
//!
//! Provides models for tissue mechanics, hemodynamics, cardiac electrophysiology,
//! pharmacokinetics / pharmacodynamics (PK/PD), neural action potentials,
//! tumor growth kinetics, and biomedical analysis tools.

pub mod analysis;
pub mod cardiac_electromechanics;
pub mod circulatory_network;
pub mod hemodynamics;
pub mod neural;
pub mod oncology;
pub mod pharmacokinetics;
pub mod physics;
pub mod tissue;
pub mod tissue_diffusion;
pub use tissue_diffusion::TissueDiffusion2D;
pub use analysis::{body_surface_area, cardiac_output, egfr_ckd_epi, perfusion_pressure};
pub use cardiac_electromechanics::CardiacModel;
pub use circulatory_network::{ArterialSegment, CirculatoryNetwork};
pub use hemodynamics::{HodgkinHuxley, VesselSegment, WindkesselModel, pulse_wave_velocity};
pub use neural::NeuronModel;
pub use oncology::TumorModel;
pub use tissue::{
    TissueMaterial, TissueMechanics, cortical_bone, trabecular_bone, skeletal_muscle,
    articular_cartilage, artery_wall,
};
pub use pharmacokinetics::{CompartmentModel, PkPdParams, emax_model};
