//! Chemical Reactions & Process Engineering Simulation (Phase 24).
//!
//! Provides simulation models for chemical reaction engineering including
//! reaction kinetics, reactor design (CSTR, PFR, Batch), separation processes,
//! combustion, and flowsheet simulation.
//!
//! # Modules
//!
//! - **physics**: Fundamental physical constants and property functions
//! - **kinetics**: Reaction kinetics (Arrhenius, rate laws, equilibrium)
//! - **reactor**: Reactor models (CSTR, PFR, Batch)
//! - **separation**: Distillation, absorption, extraction
//! - **combustion**: Flame temperature, flame speed, explosive limits
//! - **flowsheet**: Process flowsheet simulation
//! - **analysis**: Conversion, yield, selectivity, enthalpy analysis

pub mod analysis;
pub mod combustion;
pub mod flowsheet;
pub mod kinetics;
pub mod physics;
pub mod reactor;
pub mod separation;

pub use analysis::{conversion, reaction_enthalpy, selectivity, yield_ratio};
pub use combustion::{
    adiabatic_flame_temperature, auto_catalytic_conversion, explosive_limits,
    laminar_flame_speed,
};
pub use flowsheet::{heat_exchanger_ntu, ProcessFlowsheet, ProcessUnit};
pub use kinetics::{
    arrhenius_rate, equilibrium_constant, half_life_first_order, reaction_rate,
    reversible_rate, ReactionKinetics,
};
pub use reactor::{BatchReactor, Cstr, Pfr};
pub use separation::{
    absorption_factor, distribution_coefficient, fenske_equation, minimum_reflux_ratio,
    rachford_rice,
};

pub mod catalytic_reactor;
pub mod distillation_column;
pub mod safety_relief;
