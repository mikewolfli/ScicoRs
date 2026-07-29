//! Aerospace & Aerodynamic Simulation (Phase 29).
//!
//! Provides modules for atmospheric modeling, aerodynamics, propulsion,
//! flight control (6DOF), high-altitude environment, thermal protection,
//! and mission analysis.

pub mod physics;
pub mod aerodynamics;
pub mod propulsion;
pub mod flight_ctrl;
pub mod environment;
pub mod thermal_protection;
pub mod analysis;

pub use physics::{
    IsaAtmosphere,
    EARTH_GRAVITATIONAL_PARAMETER, EARTH_MASS, EARTH_RADIUS, EARTH_ROTATION_RATE,
    G0, GAMMA_AIR, ISA_LAPSE_RATE, ISA_SL_DENSITY, ISA_SL_PRESSURE, ISA_SL_TEMP, R_AIR,
};
pub use aerodynamics::{
    airfoil_cd, airfoil_cm, normal_shock_pressure_ratio, oblique_shock_angle,
    prandtl_meyer_angle, thin_airfoil_cl, AircraftAerodynamics,
};
pub use propulsion::{
    characteristic_velocity, isentropic_flow, nozzle_area_ratio, rocket_thrust,
    specific_impulse, thrust_specific_fuel_consumption, turbojet_thrust,
};
pub use flight_ctrl::{
    euler_to_quaternion, quaternion_normalize, quaternion_to_euler, Autopilot, SixDofAircraft,
};
pub use environment::{
    aerodynamic_heating, ambient_temperature, gravity_at_altitude, HighAltitudeAtmosphere,
};
pub use thermal_protection::{
    load_factor, shock_response_sweep, ThermalProtectionSystem, TpsLayer,
};
pub mod hypersonic;
pub mod reentry;
pub use hypersonic::HypersonicFlow;
pub use reentry::ReentryTrajectory;
pub use analysis::{
    breguet_range, lift_to_drag_ratio, rate_of_climb, wing_loading,
};
