//! Gravitational physics: force, acceleration, tidal, N-body, Lagrange points.

use super::physics::GRAVITATIONAL;
use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Gravitational force between two point masses: F = G·m₁·m₂/r².
pub fn gravitational_force(m1: Scalar, m2: Scalar, distance: Scalar) -> Scalar {
    if distance <= 0.0 {
        return 0.0;
    }
    GRAVITATIONAL * m1 * m2 / (distance * distance)
}

/// Gravitational acceleration at a point due to a central mass.
pub fn gravitational_acceleration(gm: Scalar, position: &Coord3D) -> [Scalar; 3] {
    let r2 = position.x * position.x + position.y * position.y + position.z * position.z;
    if r2 < 1e-30 {
        return [0.0; 3];
    }
    let r = r2.sqrt();
    let factor = -gm / (r2 * r);
    [
        factor * position.x,
        factor * position.y,
        factor * position.z,
    ]
}

/// N-body gravitational accelerations (parallelized with rayon).
pub fn nbody_accelerations(
    positions: &[Coord3D],
    masses: &[Scalar],
    softening: Scalar,
) -> Vec<[Scalar; 3]> {
    let n = positions.len().min(masses.len());
    use rayon::prelude::*;
    (0..n)
        .into_par_iter()
        .map(|i| {
            let mut ax = 0.0;
            let mut ay = 0.0;
            let mut az = 0.0;
            for j in 0..n {
                if i == j {
                    continue;
                }
                let dx = positions[j].x - positions[i].x;
                let dy = positions[j].y - positions[i].y;
                let dz = positions[j].z - positions[i].z;
                let r2 = dx * dx + dy * dy + dz * dz + softening * softening;
                let inv_r3 = 1.0 / (r2 * r2.sqrt());
                ax += GRAVITATIONAL * masses[j] * dx * inv_r3;
                ay += GRAVITATIONAL * masses[j] * dy * inv_r3;
                az += GRAVITATIONAL * masses[j] * dz * inv_r3;
            }
            [ax, ay, az]
        })
        .collect()
}

/// Tidal force on secondary body due to primary.
pub fn tidal_force(
    primary_mass: Scalar,
    primary_pos: &Coord3D,
    secondary_pos: &Coord3D,
    secondary_mass: Scalar,
) -> [Scalar; 3] {
    let dx = secondary_pos.x - primary_pos.x;
    let dy = secondary_pos.y - primary_pos.y;
    let dz = secondary_pos.z - primary_pos.z;
    let r = (dx * dx + dy * dy + dz * dz).sqrt();
    if r < 1e-15 {
        return [0.0; 3];
    }
    let factor = GRAVITATIONAL * primary_mass * secondary_mass / (r * r * r);
    [factor * dx, factor * dy, factor * dz]
}

/// Hill sphere radius (stable orbit maximum distance).
pub fn hill_sphere_radius(semi_major: Scalar, mass: Scalar, parent_mass: Scalar) -> Scalar {
    semi_major * (mass / (3.0 * parent_mass)).powf(1.0 / 3.0)
}

/// L1 Lagrange point distance from secondary (along the line connecting primary and secondary).
pub fn lagrange_l1_distance(semi_major: Scalar, mass_ratio: Scalar) -> Scalar {
    semi_major * (1.0 - (mass_ratio / 3.0).powf(1.0 / 3.0))
}

/// Gravitational potential energy of an N-body system.
pub fn gravitational_potential_energy(positions: &[Coord3D], masses: &[Scalar]) -> Scalar {
    let n = positions.len().min(masses.len());
    let mut pe = 0.0;
    for i in 0..n {
        for j in i + 1..n {
            let dx = positions[j].x - positions[i].x;
            let dy = positions[j].y - positions[i].y;
            let dz = positions[j].z - positions[i].z;
            let r = (dx * dx + dy * dy + dz * dz).sqrt();
            if r > 1e-15 {
                pe -= GRAVITATIONAL * masses[i] * masses[j] / r;
            }
        }
    }
    pe
}

#[cfg(test)]
mod tests {
    use super::super::physics::{AU, EARTH_GM, EARTH_MASS, EARTH_RADIUS, SOLAR_MASS};
    use super::*;
    use crate::core::coord::Coord3D;

    #[test]
    fn test_gravitational_force_earth_sun() {
        let f = gravitational_force(SOLAR_MASS, EARTH_MASS, AU);
        assert!(f > 3.5e22);
        assert!(f < 3.6e22);
    }

    #[test]
    fn test_gravitational_acceleration() {
        let pos = Coord3D::new(EARTH_RADIUS, 0.0, 0.0);
        let acc = gravitational_acceleration(EARTH_GM, &pos);
        let g = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt();
        assert!((g - 9.81).abs() < 0.05);
    }

    #[test]
    fn test_hill_sphere() {
        let r_h = hill_sphere_radius(AU, EARTH_MASS, SOLAR_MASS);
        assert!(r_h > 0.0);
        assert!(r_h < AU);
    }

    #[test]
    fn test_potential_energy() {
        let positions = vec![Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(AU, 0.0, 0.0)];
        let masses = vec![SOLAR_MASS, EARTH_MASS];
        let pe = gravitational_potential_energy(&positions, &masses);
        assert!(pe < 0.0);
    }

    #[test]
    fn test_tidal_force() {
        let f = tidal_force(
            SOLAR_MASS,
            &Coord3D::new(0.0, 0.0, 0.0),
            &Coord3D::new(AU, 0.0, 0.0),
            EARTH_MASS,
        );
        assert!(f[0] != 0.0);
    }

    #[test]
    fn test_lagrange_l1() {
        let l1 = lagrange_l1_distance(AU, EARTH_MASS / SOLAR_MASS);
        assert!(l1 > 0.0);
        assert!(l1 < AU);
    }

    #[test]
    fn test_nbody_accelerations_two_body() {
        let positions = vec![Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(AU, 0.0, 0.0)];
        let masses = vec![SOLAR_MASS, EARTH_MASS];
        let accs = nbody_accelerations(&positions, &masses, 1e3);
        assert_eq!(accs.len(), 2);
    }

    #[test]
    fn test_zero_distance() {
        let f = gravitational_force(1.0, 1.0, 0.0);
        assert!(f == 0.0);
    }
}
