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
        let electrostatic = self.electrostatic_score(
            ligand_coords,
            ligand_charges,
            receptor_coords,
            receptor_charges,
        );
        let desolvation = self.desolvation_penalty(ligand_coords, ligand_charges);

        self.shape_weight * shape
            + self.electrostatic_weight * electrostatic
            + self.desolvation_weight * desolvation
    }

    /// Shape complementarity score using LJ attractive component.
    ///
    /// Computes the attractive part of Lennard-Jones 12-6 potential
    /// between all ligand-receptor atom pairs. Each ligand atom's partial
    /// score is independent → large pair counts run on rayon (sum-reduce).
    pub fn shape_score(&self, ligand_coords: &[Vec3], receptor_coords: &[Vec3]) -> Scalar {
        /// Pair products at which rayon pays for itself.
        const PAR_MIN_PAIRS: usize = 16_384;

        let lj = LennardJones::new(3.5, 0.1);
        let lig_sum = |lig: &Vec3| -> Scalar {
            let mut s = 0.0;
            for rec in receptor_coords {
                let r = lig.distance(rec);
                // Only consider attractive region (r > sigma)
                if r > 3.0 && r < 8.0 {
                    s += lj.energy(r).min(0.0); // only attractive part
                }
            }
            s
        };

        if ligand_coords.len() * receptor_coords.len() >= PAR_MIN_PAIRS {
            use rayon::prelude::*;
            ligand_coords.par_iter().map(lig_sum).sum()
        } else {
            ligand_coords.iter().map(lig_sum).sum()
        }
    }

    /// Electrostatic score using Coulomb's law.
    ///
    /// Each ligand atom's partial score is independent → large pair counts
    /// run on rayon (sum-reduce).
    pub fn electrostatic_score(
        &self,
        ligand_coords: &[Vec3],
        ligand_charges: &[Scalar],
        receptor_coords: &[Vec3],
        receptor_charges: &[Scalar],
    ) -> Scalar {
        /// Pair products at which rayon pays for itself.
        const PAR_MIN_PAIRS: usize = 16_384;

        let coulomb = CoulombPotential::new(4.0); // solvated environment
        let lig_sum = |(i, lig): (usize, &Vec3)| -> Scalar {
            let qi = if i < ligand_charges.len() {
                ligand_charges[i]
            } else {
                0.0
            };
            let mut s = 0.0;
            for (j, rec) in receptor_coords.iter().enumerate() {
                let qj = if j < receptor_charges.len() {
                    receptor_charges[j]
                } else {
                    0.0
                };
                let r = lig.distance(rec);
                if r < 8.0 {
                    s += coulomb.energy(qi, qj, r);
                }
            }
            s
        };

        if ligand_coords.len() * receptor_coords.len() >= PAR_MIN_PAIRS {
            use rayon::prelude::*;
            ligand_coords.par_iter().enumerate().map(lig_sum).sum()
        } else {
            ligand_coords.iter().enumerate().map(lig_sum).sum()
        }
    }

    /// Desolvation penalty based on atomic solvation parameters.
    fn desolvation_penalty(&self, ligand_coords: &[Vec3], ligand_charges: &[Scalar]) -> Scalar {
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

    #[test]
    fn test_score_parallel_matches_serial_reference() {
        // 200×100 = 20,000 pairs > PAR_MIN_PAIRS=16,384 → rayon path; verify
        // against the original serial pair loops (parallel reduction reorders
        // floating-point sums, so use a relative tolerance).
        let n_lig = 200;
        let n_rec = 100;
        let lig: Vec<Vec3> = (0..n_lig)
            .map(|i| Vec3::new((i as Scalar) * 0.2, 0.0, 0.0))
            .collect();
        let rec: Vec<Vec3> = (0..n_rec)
            .map(|i| Vec3::new(2.0 + (i as Scalar) * 0.2, 1.0, 0.0))
            .collect();
        let lig_q: Vec<Scalar> = (0..n_lig)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let rec_q: Vec<Scalar> = (0..n_rec)
            .map(|i| if i % 3 == 0 { 1.0 } else { -0.5 })
            .collect();
        let d = DockingScore::new();

        let s_par = d.shape_score(&lig, &rec);
        let s_ref: Scalar = {
            let lj = LennardJones::new(3.5, 0.1);
            let mut s = 0.0;
            for l in &lig {
                for r in &rec {
                    let dist = l.distance(r);
                    if dist > 3.0 && dist < 8.0 {
                        s += lj.energy(dist).min(0.0);
                    }
                }
            }
            s
        };
        assert!(
            (s_par - s_ref).abs() <= 1e-9 * s_ref.abs().max(1.0),
            "shape score mismatch: {s_par} vs {s_ref}"
        );

        let e_par = d.electrostatic_score(&lig, &lig_q, &rec, &rec_q);
        let e_ref: Scalar = {
            let coulomb = CoulombPotential::new(4.0);
            let mut s = 0.0;
            for (i, l) in lig.iter().enumerate() {
                let qi = if i < lig_q.len() { lig_q[i] } else { 0.0 };
                for (j, r) in rec.iter().enumerate() {
                    let qj = if j < rec_q.len() { rec_q[j] } else { 0.0 };
                    let dist = l.distance(r);
                    if dist < 8.0 {
                        s += coulomb.energy(qi, qj, dist);
                    }
                }
            }
            s
        };
        assert!(
            (e_par - e_ref).abs() <= 1e-9 * e_ref.abs().max(1.0),
            "electrostatic score mismatch: {e_par} vs {e_ref}"
        );
    }
}
