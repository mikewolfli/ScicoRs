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
//! - **bio_medical** (Phase 23): Physiological systems & biomedical simulation.
//! - **chemical** (Phase 24): Chemical reactions & process engineering simulation.
//! - **structural** (Phase 25): Structural mechanics & finite element FEA simulation.
//! - **thermal** (Phase 26): Thermodynamics & heat transfer simulation.
//! - **fluid** (Phase 27): Fluid dynamics & CFD simulation.
//! - **multibody** (Phase 28): Multibody dynamics & mechanical system simulation.
//! - **aerospace** (Phase 29): Aerospace & aerodynamic simulation.
//! - **quantum** (Phase 30): Quantum physics & quantum computing simulation.
//! - **astrophysics** (Phase 31): Astrophysics & celestial orbit simulation.

pub mod acoustic;
pub mod aerospace;
pub mod analog;
pub mod astrophysics;
pub mod bio_medical;
pub mod cellbio;
pub mod chemical;
pub mod digital;
pub mod emag;
pub mod fluid;
pub mod molbio;
pub mod multibody;
pub mod optical;
pub mod pcb;
pub mod powerelec;
pub mod quantum;
pub mod structural;
pub mod tcad;
pub mod thermal;
