//! Simplified molecular docking scoring functions.
//!
//! Provides a docking score based on shape complementarity (Lennard-Jones
//! attractive term), electrostatic interactions, and desolvation penalty.

use crate::core::types::Scalar;
use crate::domains::molbio::forcefield::{CoulombPotential, LennardJones, Vec3};

/// Docking score function using weighted terms.
#[derive(Debug, Clone)]
pub struct DockingScore {
    /// Weight for shape complementarity term.
    pub shape_weight: Scalar,
    /// Weight for electrostatic term.
    pub electrostatic_weight: Scalar,
    /// Weight for desolvation penalty.
    pub desolvation_weight: Scalar,
}

impl Default for DockingScore {
    fn default() -> Self {
        Self {
            shape_weight: 1.0,
            electrostatic_weight: 0.5,
            desolvation_weight: 0.2,
        }
    }
}

impl DockingScore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute the total docking score.
    ///
    /// Lower (more negative) scores indicate better binding.
    pub fn score(
        &self,
        ligand_coords: &[Vec3],
        ligand_charges: &[Scalar],
        receptor_coords: &[Vec3],
        receptor_charges: &[Scalar],
    ) -> Scalar {
        let shape = self.shape_score(ligand_coords, receptor_coords);
        let electrostatic = self.electrostatic_score(ligand_coords, ligand_charges, receptor_coords, receptor_charges);
        let desolvation = self.desolvation_penalty(ligand_coords, ligand_charges);

        self.shape_weight * shape
            + self.electrostatic_weight * electrostatic
            + self.desolvation_weight * desolvation
    }

    /// Shape complementarity score using LJ attractive component.
    ///
    /// Computes the attractive part of Lennard-Jones 12-6 potential
    /// between all ligand-receptor atom pairs.
    pub fn shape_score(
        &self,
        ligand_coords: &[Vec3],
        receptor_coords: &[Vec3],
    ) -> Scalar {
        let mut score = 0.0;
        // Use a uniform LJ parameter for shape scoring
        let lj = LennardJones::new(3.5, 0.1);

        for lig in ligand_coords {
            for rec in receptor_coords {
                let r = lig.distance(rec);
                // Only consider attractive region (r > sigma)
                if r > 3.0 && r < 8.0 {
                    let energy = lj.energy(r);
                    score += energy.min(0.0); // only attractive part
                }
            }
        }
        score
    }

    /// Electrostatic score using Coulomb's law.
    pub fn electrostatic_score(
        &self,
        ligand_coords: &[Vec3],
        ligand_charges: &[Scalar],
        receptor_coords: &[Vec3],
        receptor_charges: &[Scalar],
    ) -> Scalar {
        let coulomb = CoulombPotential::new(4.0); // solvated environment
        let mut score = 0.0;

        for (i, lig) in ligand_coords.iter().enumerate() {
            let qi = if i < ligand_charges.len() { ligand_charges[i] } else { 0.0 };
            for (j, rec) in receptor_coords.iter().enumerate() {
                let qj = if j < receptor_charges.len() { receptor_charges[j] } else { 0.0 };
                let r = lig.distance(rec);
                if r < 8.0 {
                    score += coulomb.energy(qi, qj, r);
                }
            }
        }
        score
    }

    /// Desolvation penalty based on atomic solvation parameters.
    fn desolvation_penalty(
        &self,
        ligand_coords: &[Vec3],
        ligand_charges: &[Scalar],
    ) -> Scalar {
        // Simple desolvation: penalize burying of charged/polar groups
        let mut penalty = 0.0;
        for (i, _pos) in ligand_coords.iter().enumerate() {
            let charge = if i < ligand_charges.len() {
                ligand_charges[i].abs()
            } else {
                0.0
            };
            // Buried charged atoms incur a penalty proportional to charge magnitude
            penalty += charge * 0.5;
        }
        penalty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_score() {
        let lig = vec![Vec3::new(0.0, 0.0, 0.0)];
        let rec = vec![Vec3::new(4.0, 0.0, 0.0)];
        let score = DockingScore::new();
        let s = score.shape_score(&lig, &rec);
        // Attractive region should give negative score
        assert!(s <= 0.0);
    }

    #[test]
    fn test_electrostatic_score() {
        let lig = vec![Vec3::new(0.0, 0.0, 0.0)];
        let rec = vec![Vec3::new(4.0, 0.0, 0.0)];
        let score = DockingScore::new();
        // Opposite charges attract → negative
        let s = score.electrostatic_score(&lig, &[1.0], &rec, &[-1.0]);
        assert!(s < 0.0);
        // Like charges repel → positive
        let s2 = score.electrostatic_score(&lig, &[1.0], &rec, &[1.0]);
        assert!(s2 > 0.0);
    }

    #[test]
    fn test_total_score() {
        let lig = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];
        let rec = vec![Vec3::new(5.0, 0.0, 0.0), Vec3::new(6.0, 0.0, 0.0)];
        let score = DockingScore::new();
        let s = score.score(&lig, &[0.5, -0.3], &rec, &[-0.2, 0.4]);
        // Score should be finite
        assert!(s.is_finite());
    }
}
