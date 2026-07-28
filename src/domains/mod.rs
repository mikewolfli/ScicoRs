//! Domain-Specific Simulation Blocks (Phase 13-31).
//!
//! This module provides domain-specific simulation modules built on the
//! core Block/Diagram/Engine framework. Each submodule corresponds to one
//! or more roadmap phases.
//!
//! # Current Modules
//!
//! - **tcad** (Phase 13): Semiconductor device physics / TCAD simulation.
//! - **analog** (Phase 14): SPICE-level analog circuit simulation.
//! - **digital** (Phase 15): Digital logic & RTL simulation.
//! - **molbio** (Phase 16): Molecular dynamics & biomolecular simulation.
//! - **cellbio** (Phase 17): Cell culture & tissue growth simulation.
//! - **optical** (Phase 18): Optics & photonics simulation.
//! - **acoustic** (Phase 19): Acoustics & vibration simulation.
//! - **pcb** (Phase 20): Board-level circuit & PCB simulation.
//! - **powerelec** (Phase 21): Power electronics & motor drive simulation.
//! - **emag** (Phase 22): Electromagnetic field & RF/microwave simulation.

pub mod acoustic;
pub mod analog;
pub mod cellbio;
pub mod digital;
pub mod emag;
pub mod molbio;
pub mod optical;
pub mod pcb;
pub mod powerelec;
pub mod tcad;
