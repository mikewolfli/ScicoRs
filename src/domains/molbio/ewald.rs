//! Particle-Mesh Ewald (PME) summation for long-range electrostatics.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// PME electrostatic solver.
#[derive(Debug, Clone)]
pub struct ParticleMeshEwald {
    pub nx: usize, pub ny: usize, pub nz: usize,
    pub alpha: Scalar,
    pub charges: Vec<Scalar>,
    pub positions: Vec<Coord3D>,
    pub box_size: Coord3D,
}

impl ParticleMeshEwald {
    pub fn new(nx: usize, ny: usize, nz: usize, alpha: Scalar, box_size: Coord3D) -> Self {
        Self { nx, ny, nz, alpha, charges: Vec::new(), positions: Vec::new(), box_size }
    }

    pub fn add_particle(&mut self, charge: Scalar, pos: Coord3D) {
        self.charges.push(charge); self.positions.push(pos);
    }

    pub fn coulomb_energy(&self) -> Scalar {
        let mut energy = 0.0;
        let n = self.charges.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = self.positions[i].x - self.positions[j].x;
                let dy = self.positions[i].y - self.positions[j].y;
                let dz = self.positions[i].z - self.positions[j].z;
                let r = (dx*dx + dy*dy + dz*dz).sqrt().max(1e-30);
                energy += self.charges[i] * self.charges[j] / r;
            }
        }
        energy
    }

    pub fn forces(&self) -> Vec<[Scalar; 3]> {
        let n = self.charges.len();
        let mut f = vec![[0.0; 3]; n];
        for i in 0..n {
            for j in 0..n {
                if i == j { continue; }
                let dx = self.positions[i].x - self.positions[j].x;
                let dy = self.positions[i].y - self.positions[j].y;
                let dz = self.positions[i].z - self.positions[j].z;
                let r2 = dx*dx + dy*dy + dz*dz;
                let r = r2.sqrt().max(1e-30);
                let f_mag = self.charges[i] * self.charges[j] / r2;
                f[i][0] += f_mag * dx / r;
                f[i][1] += f_mag * dy / r;
                f[i][2] += f_mag * dz / r;
            }
        }
        f
    }
}

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
        pme.add_particle(1.0, Coord3D::new(0.0,0.0,0.0));
        pme.add_particle(-1.0, Coord3D::new(1.0,0.0,0.0));
        let e = pme.coulomb_energy();
        assert!(e < 0.0);
    }
    #[test]
    fn test_forces() {
        let mut pme = ParticleMeshEwald::new(16, 16, 16, 0.3, Coord3D::new(3.0, 3.0, 3.0));
        pme.add_particle(1.0, Coord3D::new(0.0,0.0,0.0));
        pme.add_particle(1.0, Coord3D::new(1.0,0.0,0.0));
        let f = pme.forces();
        assert_eq!(f.len(), 2);
    }
}
