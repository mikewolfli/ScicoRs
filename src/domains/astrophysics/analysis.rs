//! Orbital analysis tools: energy, angular momentum, collision probability, visibility.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use super::orbital::KeplerianElements;

/// Specific orbital energy: ε = v²/2 - GM/r.
pub fn orbital_energy(position: &Coord3D, velocity: &[Scalar; 3], gm: Scalar) -> Scalar {
    let r = (position.x * position.x + position.y * position.y + position.z * position.z).sqrt();
    let v2 = velocity[0] * velocity[0] + velocity[1] * velocity[1] + velocity[2] * velocity[2];
    v2 / 2.0 - gm / r
}

/// Specific orbital angular momentum: h = r × v.
pub fn orbital_angular_momentum(position: &Coord3D, velocity: &[Scalar; 3]) -> [Scalar; 3] {
    [
        position.y * velocity[2] - position.z * velocity[1],
        position.z * velocity[0] - position.x * velocity[2],
        position.x * velocity[1] - position.y * velocity[0],
    ]
}

/// Eccentricity vector: e = (v × h)/GM - r̂.
pub fn eccentricity_vector(position: &Coord3D, velocity: &[Scalar; 3], gm: Scalar) -> [Scalar; 3] {
    let h = orbital_angular_momentum(position, velocity);
    let r = (position.x * position.x + position.y * position.y + position.z * position.z).sqrt();
    if r < 1e-15 || gm.abs() < 1e-30 { return [0.0; 3]; }

    let v_cross_h = [
        velocity[1] * h[2] - velocity[2] * h[1],
        velocity[2] * h[0] - velocity[0] * h[2],
        velocity[0] * h[1] - velocity[1] * h[0],
    ];

    [
        v_cross_h[0] / gm - position.x / r,
        v_cross_h[1] / gm - position.y / r,
        v_cross_h[2] / gm - position.z / r,
    ]
}

/// Collision probability based on closest approach distance.
pub fn collision_probability(
    body1_pos: &Coord3D, body1_vel: &[Scalar; 3],
    body2_pos: &Coord3D, body2_vel: &[Scalar; 3],
    body1_radius: Scalar, body2_radius: Scalar,
) -> Scalar {
    let dr = [
        body2_pos.x - body1_pos.x,
        body2_pos.y - body1_pos.y,
        body2_pos.z - body1_pos.z,
    ];
    let dv = [
        body2_vel[0] - body1_vel[0],
        body2_vel[1] - body1_vel[1],
        body2_vel[2] - body1_vel[2],
    ];

    let v_rel_sq = dv[0] * dv[0] + dv[1] * dv[1] + dv[2] * dv[2];
    if v_rel_sq < 1e-30 { return 1.0; }

    let dr_dot_dv = dr[0] * dv[0] + dr[1] * dv[1] + dr[2] * dv[2];
    let t_ca = -dr_dot_dv / v_rel_sq;

    if t_ca < 0.0 { return 0.0; }

    let closest = [
        dr[0] + dv[0] * t_ca,
        dr[1] + dv[1] * t_ca,
        dr[2] + dv[2] * t_ca,
    ];
    let dist = (closest[0] * closest[0] + closest[1] * closest[1] + closest[2] * closest[2]).sqrt();
    let collision_distance = body1_radius + body2_radius;

    if dist <= collision_distance { 1.0 } else { (collision_distance / dist).exp() * 0.5 }
}

/// Orbital lifetime estimation using simplified drag model.
pub fn orbital_lifetime(semi_major: Scalar, eccentricity: Scalar, area_mass_ratio: Scalar, _solar_activity: Scalar) -> Scalar {
    let r_earth = 6371000.0;
    let altitude = semi_major * (1.0 - eccentricity) - r_earth;
    if altitude > 2000000.0 { return 1e9; } // Above significant drag

    // Simple exponential model
    let base_lifetime = 100.0 * (altitude / 100000.0).powi(3);
    base_lifetime / (area_mass_ratio * 100.0).max(0.01)
}

/// Visibility window computation (simplified).
pub fn visibility_window(
    observer_pos: &Coord3D, target_oe: &KeplerianElements,
    gm: Scalar, min_elevation: Scalar, time_range: (Scalar, Scalar),
) -> Vec<(Scalar, Scalar)> {
    let mut windows = Vec::new();
    let period = target_oe.period(gm);
    let mut t = time_range.0;
    while t < time_range.1 {
        let (target_pos, _) = target_oe.to_cartesian(gm);

        // Compute elevation
        let los = [
            target_pos.x - observer_pos.x,
            target_pos.y - observer_pos.y,
            target_pos.z - observer_pos.z,
        ];
        let los_mag = (los[0] * los[0] + los[1] * los[1] + los[2] * los[2]).sqrt();
        if los_mag < 1e-15 { t += 60.0; continue; }

        let obs_dist = (observer_pos.x * observer_pos.x + observer_pos.y * observer_pos.y + observer_pos.z * observer_pos.z).sqrt();
        let cos_zenith = (los[0] * observer_pos.x + los[1] * observer_pos.y + los[2] * observer_pos.z) / (los_mag * obs_dist);
        let elevation = std::f64::consts::FRAC_PI_2 - cos_zenith.acos();

        if elevation > min_elevation {
            let window_start = t;
            let mut window_end = t + 600.0; // Assume 10 min visibility window
            // Extend window while visible
            let mut t_ext = t + 600.0;
            while t_ext < time_range.1.min(t + 3600.0) {
                let (check_pos, _) = target_oe.to_cartesian(gm);
                let check_los = [
                    check_pos.x - observer_pos.x,
                    check_pos.y - observer_pos.y,
                    check_pos.z - observer_pos.z,
                ];
                let check_mag = (check_los[0] * check_los[0] + check_los[1] * check_los[1] + check_los[2] * check_los[2]).sqrt();
                let check_cz = (check_los[0] * observer_pos.x + check_los[1] * observer_pos.y + check_los[2] * observer_pos.z) / (check_mag * obs_dist);
                let check_el = std::f64::consts::FRAC_PI_2 - check_cz.acos();
                if check_el > min_elevation {
                    window_end = t_ext;
                    t_ext += 60.0;
                } else {
                    break;
                }
            }
            windows.push((window_start, window_end));
            t = window_end;
        }
        t += period / 100.0;
    }
    windows
}

#[cfg(test)]
mod tests {
    use crate::core::coord::Coord3D;
    use super::super::physics::EARTH_GM;
    use super::*;

    #[test]
    fn test_orbital_energy_negative() {
        let pos = Coord3D::new(7000000.0, 0.0, 0.0);
        let v_circ = (EARTH_GM / 7000000.0).sqrt();
        let vel = [0.0, v_circ, 0.0];
        let e = orbital_energy(&pos, &vel, EARTH_GM);
        assert!(e < 0.0); // Bound orbit
    }

    #[test]
    fn test_orbital_angular_momentum() {
        let pos = Coord3D::new(1.0, 0.0, 0.0);
        let vel = [0.0, 1.0, 0.0];
        let h = orbital_angular_momentum(&pos, &vel);
        assert!((h[2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_eccentricity_vector_circular() {
        let pos = Coord3D::new(7000000.0, 0.0, 0.0);
        let v_circ = (EARTH_GM / 7000000.0).sqrt();
        let vel = [0.0, v_circ, 0.0];
        let e = eccentricity_vector(&pos, &vel, EARTH_GM);
        let e_mag = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
        assert!(e_mag < 0.01);
    }

    #[test]
    fn test_collision_probability_coincident() {
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let vel = [1.0, 0.0, 0.0];
        let prob = collision_probability(&pos, &vel, &pos, &vel, 1.0, 1.0);
        assert!((prob - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_orbital_lifetime() {
        let life = orbital_lifetime(7000000.0, 0.0, 0.01, 1.0);
        assert!(life > 0.0);
    }

    #[test]
    fn test_visibility_window() {
        use super::super::orbital::KeplerianElements;
        let observer = Coord3D::new(6371000.0, 0.0, 0.0);
        let oe = KeplerianElements::new(7000000.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let windows = visibility_window(&observer, &oe, EARTH_GM, 0.1, (0.0, 86400.0));
        assert!(!windows.is_empty());
    }

    #[test]
    fn test_orbital_energy_hyperbolic() {
        let pos = Coord3D::new(7000000.0, 0.0, 0.0);
        let vel = [0.0, 15000.0, 0.0]; // > escape velocity
        let e = orbital_energy(&pos, &vel, EARTH_GM);
        assert!(e > 0.0); // Unbound orbit
    }
}
