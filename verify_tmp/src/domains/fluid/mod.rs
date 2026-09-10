//! Fluid Dynamics & CFD Simulation (Phase 27).
//!
//! Provides fluid dynamics simulation models including Navier-Stokes
//! solvers, flow regime classification, aerodynamics, hydraulics, and
//! general flow analysis functions.

pub mod aerodynamics;
pub mod analysis;
pub mod compressible_ns;
pub mod flow_regimes;
pub mod hydraulics;
pub mod multiphase;
pub mod navier_stokes;
pub mod navier_stokes_3d;
pub mod physics;
pub mod turbulence;

pub use aerodynamics::{
    drag_coefficient, drag_force, dynamic_pressure, lift_coefficient, lift_force,
    turbulent_boundary_layer_thickness,
};
pub use analysis::{hydraulic_diameter, mass_flow, pressure_coefficient, volumetric_flow};
pub use compressible_ns::CompressibleNS2D;
pub use flow_regimes::{
    FlowRegime, bubble_terminal_velocity, darcy_friction_factor, flow_regime, homogeneous_density,
    mixing_length_turbulent_viscosity, pipe_pressure_drop,
};
pub use hydraulics::{manning_flow, orifice_flow, water_hammer_pressure, weir_flow};
pub use multiphase::VofSolver2D;
pub use navier_stokes::{NavierStokes2D, WallCondition, mach_number, reynolds_number};
pub use navier_stokes_3d::{NavierStokes3D, WallCondition3D};
pub use physics::{
    AIR_DENSITY_STP, AIR_GAMMA, AIR_GAS_CONSTANT, G, HighTempAirProps, WATER_DENSITY,
    WATER_VISCOSITY, high_temp_air_properties, kinematic_viscosity, plasma_frequency,
};
pub use turbulence::{KEpsilon, Smagorinsky, TurbulenceModel};
