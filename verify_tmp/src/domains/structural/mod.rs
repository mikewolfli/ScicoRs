//! Structural Mechanics & Finite Element FEA Simulation (Phase 25).
//!
//! Provides material mechanical properties, finite-element types (beam,
//! truss, shell, solid, spring), an FEM solver (static/modal/buckling),
//! structural dynamics (SDOF, fatigue), contact mechanics, and analysis
//! tools (safety factor, Euler buckling, beam deflection).

pub mod analysis;
pub mod contact;
pub mod dynamics;
pub mod elements;
pub mod explicit_dynamics;
pub mod fem_solver;
pub mod nonlinear_fea;
pub mod physics;

pub use analysis::{
    axial_stress, beam_deflection_simple, beam_deflection_udl, bending_stress, euler_buckling_load,
    safety_factor,
};
pub use contact::{
    bolt_preload, coulomb_friction, hertz_contact_stress, is_in_contact, point_to_point_distance,
};
pub use dynamics::{SdofSystem, miner_damage, sn_curve};
pub use elements::{BeamElement, ShellElement, SolidElement, SpringElement, TrussElement};
pub use explicit_dynamics::ExplicitDynamics;
pub use fem_solver::{FemElement, FemSystem};
pub use nonlinear_fea::NonlinearFem;
pub use physics::{
    MaterialProperties, aluminum_6061, concrete_30mpa, hookes_law_1d, hookes_law_3d,
    steel_structural, titanium_ti6al4v, von_mises_stress,
};
