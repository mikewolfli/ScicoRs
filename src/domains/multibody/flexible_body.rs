//! Flexible body dynamics using FFR formulation.
use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
#[derive(Debug, Clone)]
pub struct FlexibleBody {
    pub modal_coords: Vec<Scalar>, pub mode_shapes: Vec<Vec<Scalar>>, pub natural_frequencies: Vec<Scalar>, pub n_modes: usize,
}
impl FlexibleBody {
    pub fn new(modes: Vec<Vec<Scalar>>, freqs: Vec<Scalar>) -> Self { let n = modes.len(); Self { modal_coords: vec![0.0; n], mode_shapes: modes, natural_frequencies: freqs, n_modes: n } }
    pub fn deflected_position(&self, local_pos: &Coord3D) -> Coord3D {
        let mut dx = 0.0; let mut dy = 0.0; let mut dz = 0.0;
        for m in 0..self.n_modes {
            let q = self.modal_coords.get(m).copied().unwrap_or(0.0);
            if let Some(shape) = self.mode_shapes.get(m) { if shape.len() >= 3 { dx += q * shape[0]; dy += q * shape[1]; dz += q * shape[2]; } }
        }
        Coord3D::new(local_pos.x + dx, local_pos.y + dy, local_pos.z + dz)
    }
    pub fn strain_energy(&self) -> Scalar {
        self.modal_coords.iter().zip(self.natural_frequencies.iter()).map(|(&q, &w)| 0.5 * q * q * w * w).sum()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_flexible_new() { let fb = FlexibleBody::new(vec![vec![1.0,0.0,0.0],vec![0.0,1.0,0.0]], vec![100.0,200.0]); assert_eq!(fb.n_modes, 2); }
    #[test] fn test_deflected() { let mut fb = FlexibleBody::new(vec![vec![0.1,0.0,0.0]], vec![100.0]); fb.modal_coords[0] = 2.0; let p = fb.deflected_position(&Coord3D::new(1.0,1.0,1.0)); assert!((p.x - 1.2).abs() < 1e-10); }
}
