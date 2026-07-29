//! Thermodynamics & Heat Transfer Simulation (Phase 26).
//!
//! Provides heat conduction solvers (1D/2D steady and transient), convection
//! models (natural/forced), thermal radiation (Stefan-Boltzmann, view factors),
//! phase change (melting, solidification, evaporation), multi-physics thermal
//! coupling, and thermal system analysis (heat sinks, heat pipes, radiators,
//! cooling systems).

pub mod analysis;
pub mod conduction;
pub mod conduction_3d;
pub mod conjugate_heat;
pub mod convection;
pub mod coupling;
pub mod phase_change;
pub mod physics;
pub mod radiation;
pub mod radiation_3d;

pub use analysis::{
    cooling_cop, heat_pipe_effective_k, heatsink_thermal_resistance, temperature_gradient,
};
pub use conduction::{
    BoundaryCondition, HeatConduction1D, HeatConduction2D, ThermalResistance, fourier_law_1d,
};
pub use conduction_3d::{BoundaryCondition3D, HeatConduction3D};
pub use conjugate_heat::ConjugateHeatTransfer;
pub use convection::{
    convection_coefficient, forced_convection_nu_laminar, forced_convection_nu_turbulent,
    grashof_number, natural_convection_nu, nucleate_boiling_h,
};
pub use coupling::{convective_heat_transfer, friction_heating, joule_heating, thermal_strain};
pub use phase_change::{PhaseChange1D, evaporation_rate};
pub use physics::{
    AIR_DYNAMIC_VISCOSITY, AIR_THERMAL_CONDUCTIVITY, ALUMINUM_THERMAL_CONDUCTIVITY,
    COPPER_THERMAL_CONDUCTIVITY, G, SIGMA_SB, WATER_DYNAMIC_VISCOSITY, WATER_FUSION_LATENT_HEAT,
    WATER_THERMAL_CONDUCTIVITY, WATER_VAPORIZATION_LATENT_HEAT,
};
pub use radiation::{
    radiation_exchange, stefan_boltzmann, view_factor_parallel_disks,
    view_factor_perpendicular_rectangles,
};
pub use radiation_3d::{DomQuadrature, DomRadiation3D};
