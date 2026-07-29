//! Cell Culture & Tissue Growth Simulation (Phase 17).
//!
//! Provides simulation models for cell culture and tissue growth including
//! cell proliferation/apoptosis, nutrient diffusion, pH/O2/temperature effects,
//! colony growth morphology, and bioreactor dynamics.
//!
//! # Modules
//!
//! - **physics**: Biophysical constants, medium property functions
//! - **cell_model**: Cell lifecycle (CellState, Cell, CellPopulation)
//! - **media**: Culture media model (CultureMedia, MediumComponent)
//! - **growth**: 3D lattice reaction-diffusion tissue growth model
//! - **bioreactor**: Bioreactor dynamic model (batch/fed-batch/continuous/perfusion)
//! - **analysis**: Growth curve, Monod kinetics, metabolic rates

pub mod analysis;
pub mod bioreactor;
pub mod cell_model;
pub mod growth;
pub mod media;
pub mod physics;

pub use analysis::{
    MetabolicAnalysis, cell_viability_factor, doubling_time, metabolic_rates,
    michaelis_menten_uptake, monod_growth_rate, specific_growth_rate,
};
pub use bioreactor::{Bioreactor, BioreactorMode};
pub use cell_model::{Cell, CellPopulation, CellState, CellUpdateResult};
pub use growth::{GridModel, TissueMorphology, analyze_tissue_morphology, detect_necrotic_core};
pub use media::{CultureMedia, MediumComponent};
pub use physics::{
    DIFFUSION_WATER_37C, GLUCOSE_DIFFUSION_COEFFICIENT, MAX_CELL_DENSITY, O2_DIFFUSION_COEFFICIENT,
    TYPICAL_CELL_DIAMETER, TYPICAL_DOUBLING_TIME, TYPICAL_SEEDING_DENSITY, diffusion_coefficient,
    water_density, water_viscosity,
};

pub mod immune_model;
