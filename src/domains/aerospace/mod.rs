//! Aerospace & Aerodynamic Simulation (Phase 29).
//!
//! Provides modules for atmospheric modeling, aerodynamics, propulsion,
//! flight control (6DOF), high-altitude environment, thermal protection,
//! and mission analysis.

pub mod aerodynamics;
pub mod analysis;
pub mod environment;
pub mod flight_ctrl;
pub mod physics;
pub mod propulsion;
pub mod thermal_protection;

pub use aerodynamics::{
    AircraftAerodynamics, airfoil_cd, airfoil_cm, normal_shock_pressure_ratio, oblique_shock_angle,
    prandtl_meyer_angle, thin_airfoil_cl,
};
pub use environment::{
    HighAltitudeAtmosphere, aerodynamic_heating, ambient_temperature, gravity_at_altitude,
};
pub use flight_ctrl::{
    Autopilot, SixDofAircraft, euler_to_quaternion, quaternion_normalize, quaternion_to_euler,
};
pub use physics::{
    EARTH_GRAVITATIONAL_PARAMETER, EARTH_MASS, EARTH_RADIUS, EARTH_ROTATION_RATE, G0, GAMMA_AIR,
    ISA_LAPSE_RATE, ISA_SL_DENSITY, ISA_SL_PRESSURE, ISA_SL_TEMP, IsaAtmosphere, R_AIR,
};
pub use propulsion::{
    characteristic_velocity, isentropic_flow, nozzle_area_ratio, rocket_thrust, specific_impulse,
    thrust_specific_fuel_consumption, turbojet_thrust,
};
pub use thermal_protection::{
    ThermalProtectionSystem, TpsLayer, load_factor, shock_response_sweep,
};
pub mod hypersonic;
pub mod reentry;
pub use analysis::{breguet_range, lift_to_drag_ratio, rate_of_climb, wing_loading};
pub use hypersonic::HypersonicFlow;
pub use reentry::ReentryTrajectory;
