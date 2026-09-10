//! Electrostatic and magnetostatic field calculations.

use crate::core::types::Scalar;

/// Point charge electric field: E = Q/(4πεr²).
pub fn point_charge_field(q: Scalar, r: Scalar) -> Scalar {
    if r <= 0.0 { return 0.0; }
    q / (4.0 * std::f64::consts::PI * 8.854187817e-12 * r * r)
}

/// Parallel plate capacitance: C = ε·A/d.
pub fn parallel_plate_capacitance(area: Scalar, distance: Scalar, epsilon: Scalar) -> Scalar {
    if distance <= 0.0 { return 0.0; }
    epsilon * area / distance
}

/// Infinite wire magnetic field: B = μ₀·I/(2πr).
pub fn wire_magnetic_field(current: Scalar, r: Scalar) -> Scalar {
    if r <= 0.0 { return 0.0; }
    1.25663706212e-6 * current / (2.0 * std::f64::consts::PI * r)
}

/// Solenoid magnetic field: B = μ₀·n·I.
pub fn solenoid_field(turns_per_meter: Scalar, current: Scalar) -> Scalar {
    1.25663706212e-6 * turns_per_meter * current
}

/// 1D electrostatic solver using Gauss-Seidel iteration.
#[derive(Debug, Clone)]
pub struct ElectrostaticSolver1D {
    pub n_points: usize,
    pub boundary_conditions: Vec<(usize, Scalar)>,
}

impl ElectrostaticSolver1D {
    pub fn new(n_points: usize) -> Self {
        Self { n_points, boundary_conditions: Vec::new() }
    }

    pub fn solve(&self) -> Vec<Scalar> {
        let mut v = vec![0.0; self.n_points];
        // Apply boundary conditions
        for &(idx, val) in &self.boundary_conditions {
            if idx < self.n_points { v[idx] = val; }
        }
        // Gauss-Seidel iteration
        for _iter in 0..10000 {
            let mut max_diff = 0.0;
            for i in 1..self.n_points - 1 {
                if self.boundary_conditions.iter().any(|(idx, _)| *idx == i) { continue; }
                let new_v = 0.5 * (v[i - 1] + v[i + 1]);
                let diff = f64::abs(new_v - v[i]);
                if diff > max_diff { max_diff = diff; }
                v[i] = new_v;
            }
            if max_diff < 1e-10 { break; }
        }
        v
    }

    pub fn electric_field(&self, potential: &[Scalar]) -> Vec<Scalar> {
        let mut e = vec![0.0; potential.len()];
        for i in 1..potential.len() - 1 {
            e[i] = -(potential[i + 1] - potential[i - 1]) / 2.0;
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_charge_field() {
        let e = point_charge_field(1e-6, 0.1);
        let expected = 1e-6 / (4.0 * std::f64::consts::PI * 8.854187817e-12 * 0.01);
        assert!((e - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_parallel_plate_capacitance() {
        let c = parallel_plate_capacitance(1e-4, 1e-3, 8.854187817e-12);
        assert!((c - 8.854187817e-13).abs() < 1e-20);
    }

    #[test]
    fn test_wire_magnetic_field() {
        let b = wire_magnetic_field(1.0, 0.01);
        assert!(b > 0.0);
    }

    #[test]
    fn test_solenoid_field() {
        let b = solenoid_field(1000.0, 1.0);
        assert!((b - 1.25663706212e-3).abs() < 1e-10);
    }

    #[test]
    fn test_es_solver_converges() {
        let mut solver = ElectrostaticSolver1D::new(10);
        solver.boundary_conditions.push((0, 0.0));
        solver.boundary_conditions.push((9, 1.0));
        let v = solver.solve();
        assert!((v[0] - 0.0).abs() < 1e-10);
        assert!(v[4] > 0.3 && v[4] < 0.6);
    }

    #[test]
    fn test_electric_field() {
        let solver = ElectrostaticSolver1D::new(5);
        let v = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let e = solver.electric_field(&v);
        assert!((e[2] + 1.0).abs() < 1e-10); // -dV/dx = -(3-1)/2 = -1
    }
}
