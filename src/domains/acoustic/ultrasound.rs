//! Ultrasound phased-array beamforming.
//!
//! Computes time delays for focusing and steering, and beam patterns.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Phased-array ultrasound transducer.
#[derive(Debug, Clone)]
pub struct PhasedArray {
    pub elements: Vec<Coord3D>,
    pub delays: Vec<Scalar>,
    pub frequency: Scalar,
    pub amplitude: Vec<Scalar>,
    pub speed_of_sound: Scalar,
}

impl PhasedArray {
    pub fn new(elements: Vec<Coord3D>, frequency: Scalar, c: Scalar) -> Self {
        let n = elements.len();
        Self { elements, delays: vec![0.0; n], frequency, amplitude: vec![1.0; n], speed_of_sound: c }
    }

    pub fn focus_at(&mut self, target: &Coord3D) -> Vec<Scalar> {
        let max_dist = self.elements.iter().map(|e| {
            let dx = e.x - target.x; let dy = e.y - target.y; let dz = e.z - target.z;
            (dx*dx + dy*dy + dz*dz).sqrt()
        }).fold(0.0_f64, f64::max);
        for (i, e) in self.elements.iter().enumerate() {
            let dx = e.x - target.x; let dy = e.y - target.y; let dz = e.z - target.z;
            let dist = (dx*dx + dy*dy + dz*dz).sqrt();
            self.delays[i] = (max_dist - dist) / self.speed_of_sound;
        }
        self.delays.clone()
    }

    pub fn beam_pattern(&self, theta_range: (Scalar, Scalar, usize)) -> Vec<Scalar> {
        let (t_min, t_max, n) = theta_range;
        let k = 2.0 * std::f64::consts::PI * self.frequency / self.speed_of_sound;
        let mut pattern = Vec::with_capacity(n);
        for i in 0..n {
            let theta = t_min + (t_max - t_min) * i as Scalar / (n - 1).max(1) as Scalar;
            let mut sum_re = 0.0; let mut sum_im = 0.0;
            for (j, e) in self.elements.iter().enumerate() {
                let phase = k * (e.x * theta.sin() + e.y * theta.cos()) + 2.0 * std::f64::consts::PI * self.frequency * self.delays[j];
                let (c, s) = phase.sin_cos();
                sum_re += self.amplitude[j] * c;
                sum_im += self.amplitude[j] * s;
            }
            pattern.push((sum_re * sum_re + sum_im * sum_im).sqrt());
        }
        pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_phased_array_new() {
        let elems = vec![Coord3D::new(0.0,0.0,0.0), Coord3D::new(0.01,0.0,0.0)];
        let pa = PhasedArray::new(elems, 5e6, 1540.0);
        assert_eq!(pa.elements.len(), 2);
    }
    #[test]
    fn test_focus_at() {
        let elems = vec![Coord3D::new(0.0,0.0,0.0), Coord3D::new(0.01,0.0,0.0)];
        let mut pa = PhasedArray::new(elems, 5e6, 1540.0);
        let delays = pa.focus_at(&Coord3D::new(0.0, 0.05, 0.0));
        assert_eq!(delays.len(), 2);
    }
    #[test]
    fn test_beam_pattern() {
        let elems = vec![Coord3D::new(-0.005,0.0,0.0), Coord3D::new(0.005,0.0,0.0)];
        let mut pa = PhasedArray::new(elems, 5e6, 1540.0);
        pa.focus_at(&Coord3D::new(0.0, 0.05, 0.0));
        let pattern = pa.beam_pattern((-1.0, 1.0, 5));
        assert_eq!(pattern.len(), 5);
        for &v in &pattern { assert!(v.is_finite()); }
    }
}
