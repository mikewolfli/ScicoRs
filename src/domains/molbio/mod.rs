//! Molecular Dynamics & Biomolecular Simulation (Phase 16).
//!
//! Provides molecular dynamics simulation capabilities including:
//! protein folding, molecular docking, DNA/RNA structure modeling,
//! force field computation, and trajectory analysis.
//!
//! # Modules
//!
//! - **physics**: Physical constants, element mass table, bond parameters
//! - **molecule**: Molecular structure (Atom, Bond, Residue, Molecule), PDB I/O
//! - **forcefield**: Molecular force field (bond, angle, dihedral, LJ, Coulomb)
//! - **dynamics**: MD engine (Velocity Verlet, Langevin, energy minimization)
//! - **docking**: Simplified molecular docking scoring
//! - **analysis**: Trajectory analysis (RMSD, Rg, MSD, hydrogen bonds, SAS)

pub mod analysis;
pub mod docking;
pub mod dynamics;
pub mod forcefield;
pub mod molecule;
pub mod physics;

pub use analysis::{
    compute_dihedral_angle, compute_rmsd, detect_hydrogen_bonds, mean_squared_displacement,
    radius_of_gyration, solvent_accessible_surface,
};
pub use docking::DockingScore;
pub use dynamics::{
    EnergyMinimizer, Integrator, MdResult, MinimizationResult, MolecularDynamics, SimParams,
};
pub use forcefield::{
    CoulombPotential, ForceField, HarmonicAngle, HarmonicBond, LennardJones, PeriodicDihedral, Vec3,
};
pub use molecule::{Atom, Bond, Molecule, Residue};
pub use physics::{ANGSTROM, AVOGADRO, KB, KCAL_TO_KJ, QE, T_300K, bond_parameters, element_mass};
