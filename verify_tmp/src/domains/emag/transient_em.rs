//! Transient electromagnetic simulation: phasors, plane waves, FDTD 1D.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Phasor representation of a sinusoidal quantity.
#[derive(Debug, Clone, Copy)]
pub struct Phasor {
    pub magnitude: Scalar,
    pub phase: Scalar,
}

impl Phasor {
    pub fn new(magnitude: Scalar, phase: Scalar) -> Self { Self { magnitude, phase } }
    pub fn real(&self) -> Scalar { self.magnitude * f64::cos(self.phase) }
    pub fn imag(&self) -> Scalar { self.magnitude * f64::sin(self.phase) }
}

/// Plane electromagnetic wave.
#[derive(Debug, Clone)]
pub struct PlaneWave {
    pub e0: Phasor,
    pub h0: Phasor,
    pub direction: Coord3D,
    pub freq: Scalar,
}

impl PlaneWave {
    pub fn new(e0: Phasor, freq: Scalar, direction: Coord3D) -> Self {
        let eta = 376.730313668;
        let h0_mag = e0.magnitude / eta;
        Self { e0, h0: Phasor::new(h0_mag, e0.phase), direction, freq }
    }

    pub fn poynting_vector(&self) -> Scalar {
        0.5 * self.e0.magnitude * self.h0.magnitude
    }

    pub fn power_density(&self) -> Scalar {
        self.poynting_vector()
    }
}

/// FDTD boundary condition type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundaryType {
    PEC,
    PMC,
    Absorbing,
}

/// 1D FDTD simulation kernel.
#[derive(Debug, Clone)]
pub struct Fdtd1D {
    pub ez: Vec<Scalar>,
    pub hy: Vec<Scalar>,
    pub dx: Scalar,
    pub dt: Scalar,
    pub n_steps: usize,
    pub boundary: BoundaryType,
}

impl Fdtd1D {
    pub fn new(n_cells: usize, dx: Scalar) -> Self {
        let c = 2.99792458e8;
        let dt = dx / (2.0 * c); // Courant condition
        Self {
            ez: vec![0.0; n_cells],
            hy: vec![0.0; n_cells],
            dx,
            dt,
            n_steps: 0,
            boundary: BoundaryType::Absorbing,
        }
    }

    pub fn update_h(&mut self) {
        let n = self.ez.len();
        let dt_over_dx = self.dt / (self.dx * 1.25663706212e-6);
        for i in 0..n - 1 {
            self.hy[i] += dt_over_dx * (self.ez[i + 1] - self.ez[i]);
        }
    }

    pub fn update_e(&mut self) {
        let n = self.ez.len();
        let dt_over_dx = self.dt / (self.dx * 8.854187817e-12);
        for i in 1..n {
            self.ez[i] += dt_over_dx * (self.hy[i] - self.hy[i - 1]);
        }
        match self.boundary {
            BoundaryType::PEC => { self.ez[0] = 0.0; self.ez[n - 1] = 0.0; }
            BoundaryType::PMC => { self.ez[0] = self.ez[1]; self.ez[n - 1] = self.ez[n - 2]; }
            BoundaryType::Absorbing => { /* Mur 1st order: left */ }
        }
    }

    pub fn step(&mut self) {
        self.update_h();
        self.update_e();
        self.n_steps += 1;
    }

    pub fn run(&mut self) {
        while self.n_steps < 1000 {
            self.step();
        }
    }

    pub fn inject_source(&mut self, position: usize, value: Scalar) {
        if position < self.ez.len() {
            self.ez[position] = value;
        }
    }

    pub fn probe(&self, position: usize) -> (Scalar, Scalar) {
        let ez = if position < self.ez.len() { self.ez[position] } else { 0.0 };
        let hy = if position < self.hy.len() { self.hy[position] } else { 0.0 };
        (ez, hy)
    }
}

/// Total EM energy in FDTD grid.
pub fn fdtd_energy(fdtd: &Fdtd1D) -> Scalar {
    let mut energy = 0.0;
    for i in 0..fdtd.ez.len() {
        energy += 0.5 * 8.854187817e-12 * fdtd.ez[i] * fdtd.ez[i];
    }
    for i in 0..fdtd.hy.len() {
        energy += 0.5 * 1.25663706212e-6 * fdtd.hy[i] * fdtd.hy[i];
    }
    energy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phasor() {
        let p = Phasor::new(1.0, 0.0);
        assert!((p.real() - 1.0).abs() < 1e-10);
        assert!((p.imag() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_plane_wave() {
        let pw = PlaneWave::new(Phasor::new(1.0, 0.0), 1e9, Coord3D::new(0.0, 0.0, 1.0));
        assert!(pw.poynting_vector() > 0.0);
    }

    #[test]
    fn test_fdtd_create() {
        let fdtd = Fdtd1D::new(100, 1e-3);
        assert_eq!(fdtd.ez.len(), 100);
        assert!(fdtd.dt > 0.0);
    }

    #[test]
    fn test_fdtd_step() {
        let mut fdtd = Fdtd1D::new(50, 1e-3);
        fdtd.inject_source(25, 1.0);
        fdtd.step();
        let (ez, hy) = fdtd.probe(25);
        assert!(ez > -2.0 && ez < 2.0);
        assert!(hy > -2.0 && hy < 2.0);
    }

    #[test]
    fn test_fdtd_run() {
        let mut fdtd = Fdtd1D::new(100, 1e-3);
        fdtd.run();
        assert!(fdtd.n_steps >= 1000);
    }

    #[test]
    fn test_fdtd_energy_non_negative() {
        let fdtd = Fdtd1D::new(50, 1e-3);
        let e = fdtd_energy(&fdtd);
        assert!(e >= 0.0);
    }
}
