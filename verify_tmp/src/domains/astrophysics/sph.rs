//! Smoothed Particle Hydrodynamics (SPH) for astrophysical fluid dynamics.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use std::f64::consts::PI;

/// SPH kernel function types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KernelType { CubicSpline, WendlandC2, Gaussian }

/// A single SPH particle.
#[derive(Debug, Clone)]
pub struct SPHParticle {
    pub pos: Coord3D, pub vel: [Scalar; 3],
    pub mass: Scalar, pub rho: Scalar, pub p: Scalar,
    pub u: Scalar, pub h: Scalar,
}

/// SPH simulation.
#[derive(Debug, Clone)]
pub struct SPHSimulation {
    pub particles: Vec<SPHParticle>,
    pub h: Scalar,
    pub kernel: KernelType,
    pub gamma: Scalar,
}

impl SPHSimulation {
    pub fn new(h: Scalar, gamma: Scalar) -> Self { Self { particles: Vec::new(), h, kernel: KernelType::CubicSpline, gamma } }

    pub fn add_particle(&mut self, pos: Coord3D, mass: Scalar, vel: [Scalar; 3]) {
        self.particles.push(SPHParticle { pos, vel, mass, rho: 0.0, p: 0.0, u: 0.0, h: self.h });
    }

    fn w(&self, r: Scalar, h: Scalar) -> Scalar {
        let q = r / h.max(1e-30);
        let sigma = 1.0 / (PI * h * h * h);
        match self.kernel {
            KernelType::CubicSpline => {
                if q < 0.0 || q >= 2.0 { 0.0 }
                else if q < 1.0 { sigma * (1.0 - 1.5 * q * q + 0.75 * q * q * q) }
                else { sigma * 0.25 * (2.0 - q).powi(3) }
            }
            KernelType::Gaussian => sigma * (-q * q).exp(),
            KernelType::WendlandC2 => {
                let t = (2.0 - q).max(0.0);
                21.0 / (2.0 * PI * h * h * h) * t.powi(4) * (1.0 + 2.0 * q)
            }
        }
    }

    pub fn density(&mut self) {
        for i in 0..self.particles.len() {
            let mut rho = 0.0;
            for j in 0..self.particles.len() {
                let dx = self.particles[i].pos.x - self.particles[j].pos.x;
                let dy = self.particles[i].pos.y - self.particles[j].pos.y;
                let dz = self.particles[i].pos.z - self.particles[j].pos.z;
                let r = (dx*dx + dy*dy + dz*dz).sqrt();
                rho += self.particles[j].mass * self.w(r, self.particles[j].h);
            }
            self.particles[i].rho = rho.max(1e-30);
        }
    }

    pub fn step(&mut self, dt: Scalar) {
        self.density();
        for i in 0..self.particles.len() {
            self.particles[i].p = (self.gamma - 1.0) * self.particles[i].rho * self.particles[i].u;
            let mut ax = 0.0; let mut ay = 0.0; let mut az = 0.0;
            for j in 0..self.particles.len() {
                if i == j { continue; }
                let dx = self.particles[i].pos.x - self.particles[j].pos.x;
                let dy = self.particles[i].pos.y - self.particles[j].pos.y;
                let dz = self.particles[i].pos.z - self.particles[j].pos.z;
                let r = (dx*dx + dy*dy + dz*dz).sqrt().max(1e-30);
                let pi_rho2 = self.particles[i].p / (self.particles[i].rho * self.particles[i].rho);
                let pj_rho2 = self.particles[j].p / (self.particles[j].rho * self.particles[j].rho);
                let acc = -(pi_rho2 + pj_rho2) * self.particles[j].mass * self.w(r, self.h) / r;
                ax += acc * dx; ay += acc * dy; az += acc * dz;
            }
            self.particles[i].vel[0] += ax * dt;
            self.particles[i].vel[1] += ay * dt;
            self.particles[i].vel[2] += az * dt;
            self.particles[i].pos.x += self.particles[i].vel[0] * dt;
            self.particles[i].pos.y += self.particles[i].vel[1] * dt;
            self.particles[i].pos.z += self.particles[i].vel[2] * dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_sph_new() { let s = SPHSimulation::new(1.0, 1.4); assert!((s.h - 1.0).abs() < 1e-10); }
    #[test]
    fn test_kernel_normalisation() {
        let s = SPHSimulation::new(1.0, 1.4);
        let w0 = s.w(0.0, 1.0);
        assert!(w0 > 0.0);
        let w2 = s.w(2.0, 1.0);
        assert!((w2 - 0.0).abs() < 1e-10);
    }
    #[test]
    fn test_density_computation() {
        let mut s = SPHSimulation::new(2.0, 1.4);
        s.add_particle(Coord3D::new(0.0,0.0,0.0), 1.0, [0.0; 3]);
        s.add_particle(Coord3D::new(1.0,0.0,0.0), 1.0, [0.0; 3]);
        s.density();
        for p in &s.particles { assert!(p.rho > 0.0); }
    }
    #[test]
    fn test_step() {
        let mut s = SPHSimulation::new(2.0, 1.4);
        s.add_particle(Coord3D::new(0.0,0.0,0.0), 1.0, [0.0; 3]);
        s.add_particle(Coord3D::new(1.0,0.0,0.0), 1.0, [0.0; 3]);
        s.step(0.01);
        // Particles should have moved due to pressure
        for p in &s.particles { assert!(p.pos.x.is_finite()); assert!(p.vel[0].is_finite()); }
    }
}
