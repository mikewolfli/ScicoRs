//! Multibody Dynamics Simulation (Phase 28).
//!
//! Rigid and flexible body dynamics with kinematic constraints, collision
//! detection, and numerical integration for mechanical system simulation.
//!
//! # Submodules
//!
//! - `physics` — Gravitational constant, rigid-body properties
//! - `body` — RigidBody, FlexibleBody, Quaternion
//! - `constraints` — Joint types, constraint Jacobian, Lagrange multiplier solver
//! - `collision` — AABB, sphere-sphere, contact forces, friction, impulse
//! - `dynamics` — MultibodySystem with semi-implicit Euler integration
//! - `analysis` — COM, momentum, trajectory, linkage analysis

pub mod analysis;
pub mod body;
pub mod collision;
pub mod constraints;
pub mod dynamics;
pub mod physics;

pub use analysis::{
    average_speed, center_of_mass, linkage_ratio, rms_velocity, total_angular_momentum,
    total_kinetic_energy, total_mass, total_momentum, trajectory_length,
};
pub use body::{FlexibleBody, Quaternion, RigidBody};
pub use collision::{
    collision_impulse, contact_force_spring_damper, friction_force, sphere_sphere_collision,
    Aabb, CollisionResult, CollisionShape,
};
pub use constraints::{Constraint, ConstraintJacobian, ConstraintSolver, ConstraintType};
pub use dynamics::{ExternalForce, MultibodySystem};
pub use physics::{RigidBodyProperties, GRAVITY};

pub mod articulated_body;
pub mod contact_dynamics;
pub mod flexible_body;
