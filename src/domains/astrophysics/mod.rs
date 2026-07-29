//! Astrophysics & Celestial Orbit Simulation (Phase 31).
//!
//! Provides celestial body models, orbital mechanics (Keplerian elements,
//! two-body/N-body propagators), gravity solvers, cosmology (ΛCDM model),
//! spacecraft trajectory design, and orbital analysis tools.

#![allow(clippy::approx_constant, clippy::excessive_precision)]

pub mod analysis;
pub mod celestial_body;
pub mod cosmology;
pub mod gr_weak_field;
pub mod gravity;
pub mod magnetohydrodynamics;
pub mod orbital;
pub mod physics;
pub mod spacecraft;
pub mod sph;
pub mod stellar_evolution;

pub use analysis::{
    collision_probability, eccentricity_vector, orbital_angular_momentum, orbital_energy,
    orbital_lifetime, visibility_window,
};
pub use celestial_body::{
    CelestialBody, CelestialBodyType, earth, jupiter, mars, mercury, moon, neptune, saturn, sun,
    uranus, venus,
};
pub use cosmology::{
    comoving_distance, einstein_radius, hubble_parameter, luminosity_distance, nfw_profile,
    scale_factor, universe_age,
};
pub use gr_weak_field::GRCorrection;
pub use gravity::{
    gravitational_acceleration, gravitational_force, gravitational_potential_energy,
    hill_sphere_radius, lagrange_l1_distance, nbody_accelerations, tidal_force,
};
pub use magnetohydrodynamics::Mhd2D;
pub use orbital::{
    J2PrecessionRate, KeplerianElements, NBodySolver, TwoBodyPropagator, j2_precession_rate,
};
pub use physics::*;
pub use spacecraft::{
    gravity_assist_delta_v, hohmann_transfer_delta_v, lambert_solver, launch_window,
    rendezvous_maneuver, station_keeping_budget,
};
pub use sph::SPHSimulation;
pub use stellar_evolution::StellarStructure;
