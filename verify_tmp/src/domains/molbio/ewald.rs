//! Particle-Mesh Ewald (PME) summation for long-range electrostatics.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// PME electrostatic solver.
#[derive(Debug, Clone)]
pub struct ParticleMeshEwald {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub alpha: Scalar,
    pub charges: Vec<Scalar>,
    pub positions: Vec<Coord3D>,
    pub box_size: Coord3D,
}

impl ParticleMeshEwald {
    pub fn new(nx: usize, ny: usize, nz: usize, alpha: Scalar, box_size: Coord3D) -> Self {
        Self {
            nx,
            ny,
            nz,
            alpha,
            charges: Vec::new(),
            positions: Vec::new(),
            box_size,
        }
    }

    pub fn add_particle(&mut self, charge: Scalar, pos: Coord3D) {
        self.charges.push(charge);
        self.positions.push(pos);
    }

    pub fn coulomb_energy(&self) -> Scalar {
        let n = self.charges.len();
        let energy_one = |i: usize| -> Scalar {
            let mut e = 0.0;
            for j in (i + 1)..n {
                let dx = self.positions[i].x - self.positions[j].x;
                let dy = self.positions[i].y - self.positions[j].y;
                let dz = self.positions[i].z - self.positions[j].z;
                let r = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-30);
                e += self.charges[i] * self.charges[j] / r;
            }
            e
        };
        if n >= PAIR_PAR_MIN {
            use rayon::prelude::*;
            (0..n).into_par_iter().map(energy_one).sum()
        } else {
            (0..n).map(energy_one).sum()
        }
    }

    pub fn forces(&self) -> Vec<[Scalar; 3]> {
        let n = self.charges.len();
        let mut f = vec![[0.0; 3]; n];
        let row = |i: usize, fi: &mut [Scalar; 3]| {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let dx = self.positions[i].x - self.positions[j].x;
                let dy = self.positions[i].y - self.positions[j].y;
                let dz = self.positions[i].z - self.positions[j].z;
                let r2 = dx * dx + dy * dy + dz * dz;
                let r = r2.sqrt().max(1e-30);
                let f_mag = self.charges[i] * self.charges[j] / r2;
                fi[0] += f_mag * dx / r;
                fi[1] += f_mag * dy / r;
                fi[2] += f_mag * dz / r;
            }
        };
        // Each row `i` writes only its own force entry → embarrassingly
        // parallel with no shared-write race; small systems stay serial.
        if n >= PAIR_PAR_MIN {
            use rayon::prelude::*;
            f.par_iter_mut().enumerate().for_each(|(i, fi)| row(i, fi));
        } else {
            for (i, fi) in f.iter_mut().enumerate() {
                row(i, fi);
            }
        }
        f
    }
}

/// Particle count at which the pairwise loops switch to rayon.
const PAIR_PAR_MIN: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pme_new() {
        let pme = ParticleMeshEwald::new(16, 16, 16, 0.3, Coord3D::new(3.0, 3.0, 3.0));
        assert_eq!(pme.nx, 16);
    }
    #[test]
    fn test_coulomb_energy() {
        let mut pme = ParticleMeshEwald::new(16, 16, 16, 0.3, Coord3D::new(3.0, 3.0, 3.0));
        pme.add_particle(1.0, Coord3D::new(0.0, 0.0, 0.0));
        pme.add_particle(-1.0, Coord3D::new(1.0, 0.0, 0.0));
        let e = pme.coulomb_energy();
        assert!(e < 0.0);
    }
    #[test]
    fn test_forces() {
        let mut pme = ParticleMeshEwald::new(16, 16, 16, 0.3, Coord3D::new(3.0, 3.0, 3.0));
        pme.add_particle(1.0, Coord3D::new(0.0, 0.0, 0.0));
        pme.add_particle(1.0, Coord3D::new(1.0, 0.0, 0.0));
        let f = pme.forces();
        assert_eq!(f.len(), 2);
    }

    #[test]
    fn test_parallel_forces_match_serial_reference() {
        // 70 particles ≥ PAIR_PAR_MIN → rayon path; verify against an
        // independent serial reference (forces + energy).
        let n = 70;
        let mut pme = ParticleMeshEwald::new(16, 16, 16, 0.3, Coord3D::new(30.0, 30.0, 30.0));
        for i in 0..n {
            let x = (i % 10) as Scalar * 2.0;
            let y = ((i / 10) % 7) as Scalar * 2.0;
            let z = (i / 70) as Scalar * 2.0;
            let q = if i % 2 == 0 { 1.0 } else { -1.0 };
            pme.add_particle(q, Coord3D::new(x, y, z));
        }
        assert!(n >= PAIR_PAR_MIN);
        let f = pme.forces();
        let e = pme.coulomb_energy();
        // Serial reference.
        let mut want = vec![[0.0; 3]; n];
        let mut want_e = 0.0;
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let dx = pme.positions[i].x - pme.positions[j].x;
                let dy = pme.positions[i].y - pme.positions[j].y;
                let dz = pme.positions[i].z - pme.positions[j].z;
                let r2 = dx * dx + dy * dy + dz * dz;
                let r = r2.sqrt().max(1e-30);
                let f_mag = pme.charges[i] * pme.charges[j] / r2;
                want[i][0] += f_mag * dx / r;
                want[i][1] += f_mag * dy / r;
                want[i][2] += f_mag * dz / r;
            }
            for j in (i + 1)..n {
                let dx = pme.positions[i].x - pme.positions[j].x;
                let dy = pme.positions[i].y - pme.positions[j].y;
                let dz = pme.positions[i].z - pme.positions[j].z;
                let r = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-30);
                want_e += pme.charges[i] * pme.charges[j] / r;
            }
        }
        for i in 0..n {
            assert!(
                (f[i][0] - want[i][0]).abs() < 1e-9
                    && (f[i][1] - want[i][1]).abs() < 1e-9
                    && (f[i][2] - want[i][2]).abs() < 1e-9,
                "parallel ewald force mismatch at {i}"
            );
        }
        assert!((e - want_e).abs() < 1e-9, "parallel ewald energy mismatch");
    }
}
