//! SHAKE constraint algorithm for rigid bonds in molecular dynamics.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// SHAKE constraint set for bond length constraints.
#[derive(Debug, Clone)]
pub struct ShakeConstraints {
    pub constraints: Vec<(usize, usize, Scalar)>,
}

impl ShakeConstraints {
    pub fn new() -> Self { Self { constraints: Vec::new() } }
    pub fn add_constraint(&mut self, i: usize, j: usize, target: Scalar) { self.constraints.push((i, j, target)); }

    pub fn satisfy(&self, positions: &mut [Coord3D], tolerance: Scalar) -> Result<(), String> {
        let max_iter = 1000;
        for _iter in 0..max_iter {
            let mut max_err: Scalar = 0.0;
            for &(i, j, target) in &self.constraints {
                let dx = positions[i].x - positions[j].x;
                let dy = positions[i].y - positions[j].y;
                let dz = positions[i].z - positions[j].z;
                let r2 = dx*dx + dy*dy + dz*dz;
                let err = r2 - target * target;
                max_err = max_err.max(err.abs());
                if err.abs() < tolerance { continue; }
                let r = r2.sqrt().max(1e-30);
                let corr = err / (4.0 * r);
                positions[i].x -= corr * dx / r;
                positions[i].y -= corr * dy / r;
                positions[i].z -= corr * dz / r;
                positions[j].x += corr * dx / r;
                positions[j].y += corr * dy / r;
                positions[j].z += corr * dz / r;
            }
            if max_err < tolerance { return Ok(()); }
        }
        Err("SHAKE did not converge".to_string())
    }
}

impl Default for ShakeConstraints { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_shake_new() { let s = ShakeConstraints::new(); assert!(s.constraints.is_empty()); }
    #[test]
    fn test_shake_satisfy() {
        let mut positions = vec![Coord3D::new(0.0,0.0,0.0), Coord3D::new(1.1,0.0,0.0)];
        let sc = ShakeConstraints::new();
        let result = sc.satisfy(&mut positions, 1e-6);
        // No constraints → should succeed immediately
        assert!(result.is_ok());
    }
    #[test]
    fn test_shake_with_constraint() {
        let mut positions = vec![Coord3D::new(0.0,0.0,0.0), Coord3D::new(1.1,0.0,0.0)];
        let mut sc = ShakeConstraints::new();
        sc.add_constraint(0, 1, 1.0);
        sc.satisfy(&mut positions, 1e-6).unwrap();
        let dx = positions[1].x - positions[0].x;
        assert!((dx - 1.0).abs() < 1e-4, "bond length should be 1.0, got {}", dx);
    }
}
