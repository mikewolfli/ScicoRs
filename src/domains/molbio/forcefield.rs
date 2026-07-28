//! Molecular force field components.
//!
//! Provides individual energy term structs and a composite ForceField
//! for computing molecular mechanics energies and forces.
//!
//! Energy terms: bond stretching (HarmonicBond), angle bending (HarmonicAngle),
//! dihedral torsion (PeriodicDihedral), van der Waals (LennardJones), electrostatic (CoulombPotential).

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// Vec3 — Lightweight 3D Vector
// ──────────────────────────────────────────────

/// A 3D vector for molecular coordinates (Angstrom).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: Scalar,
    pub y: Scalar,
    pub z: Scalar,
}

impl Vec3 {
    pub fn new(x: Scalar, y: Scalar, z: Scalar) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }

    pub fn distance(&self, other: &Vec3) -> Scalar {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn dot(&self, other: &Vec3) -> Scalar {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(&self, other: &Vec3) -> Vec3 {
        Vec3 {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    pub fn norm(&self) -> Scalar {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalized(&self) -> Vec3 {
        let n = self.norm();
        if n <= 1e-15 {
            Vec3::zero()
        } else {
            Vec3::new(self.x / n, self.y / n, self.z / n)
        }
    }

    pub fn subtract(&self, other: &Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    pub fn add(&self, other: &Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    pub fn scale(&self, s: Scalar) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

// ──────────────────────────────────────────────
// HarmonicBond
// ──────────────────────────────────────────────

/// Harmonic bond stretching potential: E = k * (r - b0)².
#[derive(Debug, Clone, Copy)]
pub struct HarmonicBond {
    /// Force constant (kcal/(mol·Å²)).
    pub k: Scalar,
    /// Equilibrium bond length (Å).
    pub b0: Scalar,
}

impl HarmonicBond {
    pub fn new(k: Scalar, b0: Scalar) -> Self {
        Self { k, b0 }
    }

    /// Energy = k * (r - b0)².
    pub fn energy(&self, r: Scalar) -> Scalar {
        let dr = r - self.b0;
        self.k * dr * dr
    }

    /// Force = -dE/dr = -2 * k * (r - b0).
    pub fn force(&self, r: Scalar) -> Scalar {
        -2.0 * self.k * (r - self.b0)
    }
}

// ──────────────────────────────────────────────
// HarmonicAngle
// ──────────────────────────────────────────────

/// Harmonic angle bending potential: E = k * (θ - θ₀)².
#[derive(Debug, Clone, Copy)]
pub struct HarmonicAngle {
    /// Force constant (kcal/(mol·rad²)).
    pub k: Scalar,
    /// Equilibrium angle (rad).
    pub theta0: Scalar,
}

impl HarmonicAngle {
    pub fn new(k: Scalar, theta0: Scalar) -> Self {
        Self { k, theta0 }
    }

    /// Energy = k * (θ - θ₀)².
    pub fn energy(&self, theta: Scalar) -> Scalar {
        let dt = theta - self.theta0;
        self.k * dt * dt
    }

    /// Force = -dE/dθ = -2 * k * (θ - θ₀).
    pub fn force(&self, theta: Scalar) -> Scalar {
        -2.0 * self.k * (theta - self.theta0)
    }
}

// ──────────────────────────────────────────────
// PeriodicDihedral
// ──────────────────────────────────────────────

/// Periodic dihedral torsion potential: E = Vn * (1 + cos(n·φ - γ)).
#[derive(Debug, Clone, Copy)]
pub struct PeriodicDihedral {
    /// Barrier height (kcal/mol).
    pub vn: Scalar,
    /// Periodicity.
    pub n: i32,
    /// Phase shift (rad).
    pub gamma: Scalar,
}

impl PeriodicDihedral {
    pub fn new(vn: Scalar, n: i32, gamma: Scalar) -> Self {
        Self { vn, n, gamma }
    }

    /// Energy = Vn * (1 + cos(n·φ - γ)).
    pub fn energy(&self, phi: Scalar) -> Scalar {
        self.vn * (1.0 + (self.n as Scalar * phi - self.gamma).cos())
    }

    /// Force = -dE/dφ = Vn * n * sin(n·φ - γ).
    pub fn force(&self, phi: Scalar) -> Scalar {
        self.vn * (self.n as Scalar) * (self.n as Scalar * phi - self.gamma).sin()
    }
}

// ──────────────────────────────────────────────
// LennardJones
// ──────────────────────────────────────────────

/// Lennard-Jones 12-6 potential: E = 4·ε·[(σ/r)¹² - (σ/r)⁶].
#[derive(Debug, Clone, Copy)]
pub struct LennardJones {
    /// Zero-energy distance (Å).
    pub sigma: Scalar,
    /// Well depth (kcal/mol).
    pub epsilon: Scalar,
}

impl LennardJones {
    pub fn new(sigma: Scalar, epsilon: Scalar) -> Self {
        Self { sigma, epsilon }
    }

    /// Energy = 4·ε·[(σ/r)¹² - (σ/r)⁶].
    /// Returns 0 for r < 0.5 Å (hard core repulsion cap).
    pub fn energy(&self, r: Scalar) -> Scalar {
        if r < 0.5 {
            return 1e6; // repulsive core
        }
        let sr = self.sigma / r;
        let sr6 = sr.powi(6);
        let sr12 = sr6 * sr6;
        4.0 * self.epsilon * (sr12 - sr6)
    }

    /// Force = -dE/dr = 24·ε·[2·(σ/r)¹² - (σ/r)⁶] / r.
    pub fn force(&self, r: Scalar) -> Scalar {
        if r < 0.5 {
            return 1e6;
        }
        let sr = self.sigma / r;
        let sr6 = sr.powi(6);
        let sr12 = sr6 * sr6;
        -24.0 * self.epsilon * (2.0 * sr12 - sr6) / r
    }

    /// Lorentz-Berthelot combining rules.
    pub fn combine_lorentz_berthelot(&self, other: &LennardJones) -> LennardJones {
        let sigma = (self.sigma + other.sigma) * 0.5;
        let epsilon = (self.epsilon * other.epsilon).sqrt();
        LennardJones::new(sigma, epsilon)
    }
}

// ──────────────────────────────────────────────
// CoulombPotential
// ──────────────────────────────────────────────

/// Coulomb electrostatic potential: E = (1/(4·π·ε₀·εr)) · (qi·qj/r).
#[derive(Debug, Clone, Copy)]
pub struct CoulombPotential {
    /// Relative dielectric constant.
    pub epsilon_r: Scalar,
}

impl CoulombPotential {
    pub fn new(epsilon_r: Scalar) -> Self {
        Self { epsilon_r }
    }

    /// Default with epsilon_r = 1.0 (vacuum).
    pub fn vacuum() -> Self {
        Self { epsilon_r: 1.0 }
    }

    /// Energy in kcal/mol.
    /// Conversion: 1/(4·π·ε₀) = 332.06371 (kcal·Å/(mol·e²)).
    pub fn energy(&self, qi: Scalar, qj: Scalar, r: Scalar) -> Scalar {
        if r < 0.5 {
            return 1e6; // prevent singularity
        }
        let coulomb_const = 332.06371; // kcal·Å/(mol·e²)
        coulomb_const * qi * qj / (self.epsilon_r * r)
    }

    /// Force = -dE/dr = (1/(4·π·ε₀·εr)) · (qi·qj/r²).
    pub fn force(&self, qi: Scalar, qj: Scalar, r: Scalar) -> Scalar {
        if r < 0.5 {
            return -1e6;
        }
        let coulomb_const = 332.06371;
        coulomb_const * qi * qj / (self.epsilon_r * r * r)
    }
}

// ──────────────────────────────────────────────
// ForceField
// ──────────────────────────────────────────────

/// Complete molecular force field aggregating all energy terms.
#[derive(Debug, Clone)]
pub struct ForceField {
    /// Bond terms: (atom_i, atom_j, parameters).
    pub bonds: Vec<(usize, usize, HarmonicBond)>,
    /// Angle terms: (atom_i, atom_j, atom_k, parameters).
    pub angles: Vec<(usize, usize, usize, HarmonicAngle)>,
    /// Dihedral terms: (atom_i, atom_j, atom_k, atom_l, parameters).
    pub dihedrals: Vec<(usize, usize, usize, usize, PeriodicDihedral)>,
    /// Lennard-Jones parameters per atom: (atom_idx, params).
    pub lj_params: Vec<(usize, LennardJones)>,
    /// Partial charges per atom: (atom_idx, charge in e).
    pub charges: Vec<(usize, Scalar)>,
    /// Coulomb potential parameters.
    pub coulomb: CoulombPotential,
}

impl ForceField {
    pub fn new() -> Self {
        Self {
            bonds: Vec::new(),
            angles: Vec::new(),
            dihedrals: Vec::new(),
            lj_params: Vec::new(),
            charges: Vec::new(),
            coulomb: CoulombPotential::new(1.0),
        }
    }

    pub fn add_bond(&mut self, i: usize, j: usize, bond: HarmonicBond) {
        self.bonds.push((i, j, bond));
    }

    pub fn add_angle(&mut self, i: usize, j: usize, k: usize, angle: HarmonicAngle) {
        self.angles.push((i, j, k, angle));
    }

    pub fn add_dihedral(
        &mut self,
        i: usize,
        j: usize,
        k: usize,
        l: usize,
        dihedral: PeriodicDihedral,
    ) {
        self.dihedrals.push((i, j, k, l, dihedral));
    }

    pub fn add_lj(&mut self, atom: usize, lj: LennardJones) {
        self.lj_params.push((atom, lj));
    }

    pub fn add_charge(&mut self, atom: usize, charge: Scalar) {
        self.charges.push((atom, charge));
    }

    /// Compute total potential energy for given coordinates.
    pub fn total_energy(&self, coords: &[Vec3]) -> Scalar {
        let mut e = 0.0;

        // Bond energy
        for &(i, j, ref bond) in &self.bonds {
            let r = coords[i].distance(&coords[j]);
            e += bond.energy(r);
        }

        // Angle energy
        for &(i, j, k, ref angle) in &self.angles {
            let theta = compute_angle(&coords[i], &coords[j], &coords[k]);
            e += angle.energy(theta);
        }

        // Dihedral energy
        for &(i, j, k, l, ref dihedral) in &self.dihedrals {
            let phi = compute_dihedral(&coords[i], &coords[j], &coords[k], &coords[l]);
            e += dihedral.energy(phi);
        }

        // Lennard-Jones energy (pairwise, excluding 1-2 and 1-3 pairs)
        let excluded = build_exclusion_list(&self.bonds, &self.angles, coords.len());
        for i in 0..coords.len() {
            let lj_i = self.lj_params.iter().find(|(idx, _)| *idx == i).map(|(_, p)| *p);
            if lj_i.is_none() {
                continue;
            }
            let lj_i = lj_i.unwrap();
            for j in (i + 1)..coords.len() {
                if excluded[i].contains(&j) {
                    continue;
                }
                let lj_j = self.lj_params.iter().find(|(idx, _)| *idx == j).map(|(_, p)| *p);
                if lj_j.is_none() {
                    continue;
                }
                let lj_ij = lj_i.combine_lorentz_berthelot(&lj_j.unwrap());
                let r = coords[i].distance(&coords[j]);
                e += lj_ij.energy(r);
            }
        }

        // Electrostatic energy (pairwise, excluding 1-2 and 1-3 pairs)
        for i in 0..coords.len() {
            let qi = self.charges.iter().find(|(idx, _)| *idx == i).map(|(_, q)| *q);
            if qi.is_none() || qi.unwrap().abs() < 1e-10 {
                continue;
            }
            let qi = qi.unwrap();
            for j in (i + 1)..coords.len() {
                if excluded[i].contains(&j) {
                    continue;
                }
                let qj = self.charges.iter().find(|(idx, _)| *idx == j).map(|(_, q)| *q);
                if qj.is_none() || qj.unwrap().abs() < 1e-10 {
                    continue;
                }
                let qj = qj.unwrap();
                let r = coords[i].distance(&coords[j]);
                e += self.coulomb.energy(qi, qj, r);
            }
        }

        e
    }

    /// Compute forces on all atoms (-dE/dr).
    pub fn compute_forces(&self, coords: &[Vec3]) -> Vec<Vec3> {
        let n = coords.len();
        let mut forces = vec![Vec3::zero(); n];

        // Bond forces
        for &(i, j, ref bond) in &self.bonds {
            let rij = coords[j].subtract(&coords[i]);
            let r = rij.norm();
            if r < 1e-15 {
                continue;
            }
            let f_mag = bond.force(r);
            let dir = rij.scale(1.0 / r);
            forces[i] = forces[i].add(&dir.scale(-f_mag));
            forces[j] = forces[j].add(&dir.scale(f_mag));
        }

        // Lennard-Jones forces (pairwise)
        let excluded = build_exclusion_list(&self.bonds, &self.angles, n);
        for i in 0..n {
            let lj_i = self.lj_params.iter().find(|(idx, _)| *idx == i).map(|(_, p)| *p);
            if lj_i.is_none() {
                continue;
            }
            let lj_i = lj_i.unwrap();
            for j in (i + 1)..n {
                if excluded[i].contains(&j) {
                    continue;
                }
                let lj_j = self.lj_params.iter().find(|(idx, _)| *idx == j).map(|(_, p)| *p);
                if lj_j.is_none() {
                    continue;
                }
                let lj_ij = lj_i.combine_lorentz_berthelot(&lj_j.unwrap());
                let rij = coords[j].subtract(&coords[i]);
                let r = rij.norm();
                if r < 1e-15 {
                    continue;
                }
                let f_mag = lj_ij.force(r);
                let dir = rij.scale(1.0 / r);
                forces[i] = forces[i].add(&dir.scale(-f_mag));
                forces[j] = forces[j].add(&dir.scale(f_mag));
            }
        }

        // Coulomb forces (pairwise)
        for i in 0..n {
            let qi = self.charges.iter().find(|(idx, _)| *idx == i).map(|(_, q)| *q);
            if qi.is_none() || qi.unwrap().abs() < 1e-10 {
                continue;
            }
            let qi = qi.unwrap();
            for j in (i + 1)..n {
                if excluded[i].contains(&j) {
                    continue;
                }
                let qj = self.charges.iter().find(|(idx, _)| *idx == j).map(|(_, q)| *q);
                if qj.is_none() || qj.unwrap().abs() < 1e-10 {
                    continue;
                }
                let qj = qj.unwrap();
                let rij = coords[j].subtract(&coords[i]);
                let r = rij.norm();
                if r < 1e-15 {
                    continue;
                }
                let f_mag = self.coulomb.force(qi, qj, r);
                let dir = rij.scale(1.0 / r);
                forces[i] = forces[i].add(&dir.scale(-f_mag));
                forces[j] = forces[j].add(&dir.scale(f_mag));
            }
        }

        forces
    }
}

impl Default for ForceField {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────
// Helper Functions
// ──────────────────────────────────────────────

/// Compute the angle (radians) between vectors a-b and c-b.
fn compute_angle(a: &Vec3, b: &Vec3, c: &Vec3) -> Scalar {
    let ba = a.subtract(b);
    let bc = c.subtract(b);
    let dot = ba.dot(&bc);
    let norm = ba.norm() * bc.norm();
    if norm < 1e-15 {
        0.0
    } else {
        (dot / norm).clamp(-1.0, 1.0).acos()
    }
}

/// Compute the dihedral angle (radians) from four atoms.
fn compute_dihedral(a: &Vec3, b: &Vec3, c: &Vec3, d: &Vec3) -> Scalar {
    let b1 = a.subtract(b);
    let b2 = c.subtract(b);
    let b3 = d.subtract(c);
    let n1 = b1.cross(&b2);
    let n2 = b2.cross(&b3);
    let dot = n1.dot(&n2);
    let norm = n1.norm() * n2.norm();
    if norm < 1e-15 {
        0.0
    } else {
        let cos_phi = (dot / norm).clamp(-1.0, 1.0);
        let sign = (n1.cross(&n2)).dot(&b2).signum();
        sign * cos_phi.acos()
    }
}

/// Build exclusion lists for non-bonded interactions.
/// Excludes 1-2 (bonded) and 1-3 (angle) pairs.
fn build_exclusion_list(
    bonds: &[(usize, usize, HarmonicBond)],
    angles: &[(usize, usize, usize, HarmonicAngle)],
    n_atoms: usize,
) -> Vec<Vec<usize>> {
    let mut excluded = vec![Vec::new(); n_atoms];

    // Exclude 1-2 pairs (bonded)
    for &(i, j, _) in bonds {
        excluded[i].push(j);
        excluded[j].push(i);
    }

    // Exclude 1-3 pairs (angle-connected)
    for &(i, _j, k, _) in angles {
        if !excluded[i].contains(&k) {
            excluded[i].push(k);
        }
        if !excluded[k].contains(&i) {
            excluded[k].push(i);
        }
    }

    excluded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec3_distance() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(3.0, 4.0, 0.0);
        assert!((a.distance(&b) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_vec3_dot() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        assert!(a.dot(&b).abs() < 1e-12);
    }

    #[test]
    fn test_vec3_cross() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        let c = a.cross(&b);
        assert!((c.x - 0.0).abs() < 1e-12);
        assert!((c.y - 0.0).abs() < 1e-12);
        assert!((c.z - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_harmonic_bond_energy() {
        let bond = HarmonicBond::new(310.0, 1.525);
        let e = bond.energy(1.525); // at equilibrium
        assert!(e.abs() < 1e-10);
        let e2 = bond.energy(1.625); // stretched
        assert!(e2 > 0.0);
    }

    #[test]
    fn test_harmonic_angle_energy() {
        let angle = HarmonicAngle::new(100.0, 1.911); // ~109.5° in rad
        let e = angle.energy(1.911);
        assert!(e.abs() < 1e-10);
        let e2 = angle.energy(2.0);
        assert!(e2 > 0.0);
    }

    #[test]
    fn test_periodic_dihedral_energy() {
        let dih = PeriodicDihedral::new(0.5, 3, 0.0);
        let e = dih.energy(0.0); // cos(0)=1, E = Vn*(1+1) = 1.0
        assert!((e - 1.0).abs() < 1e-10);
        let e2 = dih.energy(std::f64::consts::PI / 3.0); // cos(π)= -1, E = 0
        assert!(e2.abs() < 1e-10);
    }

    #[test]
    fn test_lennard_jones_energy() {
        let lj = LennardJones::new(3.0, 0.2);
        let e_at_sigma = lj.energy(3.0); // at sigma, E = 0
        assert!(e_at_sigma.abs() < 1e-10);
        let e_min = lj.energy(3.0 * 2.0_f64.powf(1.0 / 6.0)); // minimum
        assert!(e_min < 0.0);
    }

    #[test]
    fn test_lj_combine_rule() {
        let lj1 = LennardJones::new(3.0, 0.2);
        let lj2 = LennardJones::new(4.0, 0.4);
        let combined = lj1.combine_lorentz_berthelot(&lj2);
        assert!((combined.sigma - 3.5).abs() < 1e-10);
        assert!((combined.epsilon - (0.2_f64 * 0.4_f64).sqrt()).abs() < 1e-10);
    }

    #[test]
    fn test_coulomb_energy() {
        let coul = CoulombPotential::vacuum();
        let e = coul.energy(1.0, -1.0, 2.0);
        assert!(e < 0.0); // opposite charges attract → negative energy
    }

    #[test]
    fn test_forcefield_total_energy() {
        let mut ff = ForceField::new();
        // Two atoms with a bond
        let coords = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0)];
        ff.add_bond(0, 1, HarmonicBond::new(310.0, 1.525));
        let e = ff.total_energy(&coords);
        // At equilibrium, bond energy ~ 0
        assert!(e.abs() < 1.0);
    }

    #[test]
    fn test_forcefield_forces() {
        let mut ff = ForceField::new();
        let coords = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.6, 0.0, 0.0)];
        ff.add_bond(0, 1, HarmonicBond::new(310.0, 1.525));
        let forces = ff.compute_forces(&coords);
        // Forces should be equal and opposite
        assert!((forces[0].x + forces[1].x).abs() < 1e-10);
        // Stretched bond → atoms pulled inward
        assert!(forces[0].x > 0.0); // atom 0 pulled toward 1
        assert!(forces[1].x < 0.0); // atom 1 pulled toward 0
    }
}
