//! Spacecraft trajectory design: Hohmann transfer, gravity assist, Lambert solver.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use super::orbital::KeplerianElements;

/// Hohmann transfer Δv computation.
/// Returns (Δv₁, Δv₂) for the two burns.
pub fn hohmann_transfer_delta_v(r1: Scalar, r2: Scalar, gm: Scalar) -> (Scalar, Scalar) {
    let v1 = (gm / r1).sqrt();
    let v2 = (gm / r2).sqrt();
    let v_transfer1 = (gm * (2.0 / r1 - 2.0 / (r1 + r2))).sqrt();
    let v_transfer2 = (gm * (2.0 / r2 - 2.0 / (r1 + r2))).sqrt();
    (v_transfer1 - v1, v2 - v_transfer2)
}

/// Gravity assist Δv from a planetary flyby.
pub fn gravity_assist_delta_v(
    v_inf_in: [Scalar; 3],
    planet_velocity: [Scalar; 3],
    turning_angle: Scalar,
) -> [Scalar; 3] {
    let v_rel = [
        v_inf_in[0] - planet_velocity[0],
        v_inf_in[1] - planet_velocity[1],
        v_inf_in[2] - planet_velocity[2],
    ];
    let v_rel_mag = (v_rel[0] * v_rel[0] + v_rel[1] * v_rel[1] + v_rel[2] * v_rel[2]).sqrt();
    let half_turn = turning_angle / 2.0;

    // Rotate the relative velocity vector by the turning angle
    let cos_t = half_turn.cos();
    let sin_t = half_turn.sin();
    let v_out_rel = [
        v_rel[0] * cos_t - v_rel[1] * sin_t,
        v_rel[0] * sin_t + v_rel[1] * cos_t,
        v_rel[2],
    ];
    let scale = v_rel_mag / (v_out_rel[0] * v_out_rel[0] + v_out_rel[1] * v_out_rel[1] + v_out_rel[2] * v_out_rel[2]).sqrt();

    [
        v_out_rel[0] * scale + planet_velocity[0] - v_inf_in[0],
        v_out_rel[1] * scale + planet_velocity[1] - v_inf_in[1],
        v_out_rel[2] * scale + planet_velocity[2] - v_inf_in[2],
    ]
}

/// Lambert problem solver (simplified): find velocity vectors for transfer between two points.
pub fn lambert_solver(
    r1: &Coord3D, r2: &Coord3D,
    dt: Scalar, gm: Scalar, _prograde: bool,
) -> Result<([Scalar; 3], [Scalar; 3]), String> {
    if dt <= 0.0 {
        return Err("Time of flight must be positive".to_string());
    }
    let c = [
        r2.x - r1.x, r2.y - r1.y, r2.z - r1.z,
    ];
    let r1_mag = (r1.x * r1.x + r1.y * r1.y + r1.z * r1.z).sqrt();
    let r2_mag = (r2.x * r2.x + r2.y * r2.y + r2.z * r2.z).sqrt();
    let c_mag = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();

    // Simplified: assume a parabolic transfer (energy = 0) as first guess
    let p = (r1_mag * r2_mag * (1.0 - (c[0] * r1.x + c[1] * r1.y + c[2] * r1.z) / (r1_mag * r2_mag)).cos()) / c_mag;

    let v1 = [
        (gm / p).sqrt() * (-(r1.y * r2.z - r1.z * r2.y).signum() * c[2] / c_mag - (1.0 - r2_mag / p) * r1.x / r1_mag),
        (gm / p).sqrt() * (-(r1.z * r2.x - r1.x * r2.z).signum() * c[0] / c_mag - (1.0 - r2_mag / p) * r1.y / r1_mag),
        (gm / p).sqrt() * (-(r1.x * r2.y - r1.y * r2.x).signum() * c[1] / c_mag - (1.0 - r2_mag / p) * r1.z / r1_mag),
    ];
    let v2 = [
        (gm / p).sqrt() * (-(r1.y * r2.z - r1.z * r2.y).signum() * c[2] / c_mag + (1.0 - r1_mag / p) * r2.x / r2_mag),
        (gm / p).sqrt() * (-(r1.z * r2.x - r1.x * r2.z).signum() * c[0] / c_mag + (1.0 - r1_mag / p) * r2.y / r2_mag),
        (gm / p).sqrt() * (-(r1.x * r2.y - r1.y * r2.x).signum() * c[1] / c_mag + (1.0 - r1_mag / p) * r2.z / r2_mag),
    ];
    Ok((v1, v2))
}

/// Launch window computation (simplified).
pub fn launch_window(
    target_oe: &KeplerianElements,
    _launch_latitude: Scalar,
    _launch_longitude: Scalar,
    time_range: (Scalar, Scalar),
) -> Vec<Scalar> {
    // Simplified: return time points where the target crosses a reference plane
    let mut windows = Vec::new();
    let period = target_oe.period(super::physics::SOLAR_GM);
    let mut t = time_range.0;
    while t < time_range.1 {
        let phase = (t / period) * 2.0 * std::f64::consts::PI;
        if (phase.sin()).abs() < 0.1 {
            windows.push(t);
            t += period * 0.5;
        }
        t += 1000.0;
    }
    windows
}

/// Station-keeping Δv budget.
pub fn station_keeping_budget(semi_major: Scalar, drag_perturbation: Scalar, duration: Scalar) -> Scalar {
    let v_orb = (super::physics::EARTH_GM / semi_major).sqrt();
    drag_perturbation * v_orb * duration / 2.0
}

/// Rendezvous maneuver planning.
pub fn rendezvous_maneuver(
    chaser_oe: &KeplerianElements,
    target_oe: &KeplerianElements,
    gm: Scalar,
) -> Result<Vec<(Scalar, [Scalar; 3])>, String> {
    let mut maneuvers = Vec::new();
    // Compute phasing orbit
    let period_diff = chaser_oe.period(gm) - target_oe.period(gm);
    let phase_time = if period_diff.abs() < 1.0 {
        return Err("Already in same orbit".to_string());
    } else {
        (target_oe.true_anomaly - chaser_oe.true_anomaly).abs() * target_oe.period(gm) / (2.0 * std::f64::consts::PI)
    };

    let r_chaser = chaser_oe.periapsis_distance();
    let r_target = target_oe.apoapsis_distance();
    let (dv1, dv2) = if r_chaser < r_target {
        hohmann_transfer_delta_v(r_chaser, r_target, gm)
    } else {
        hohmann_transfer_delta_v(r_target, r_chaser, gm)
    };

    maneuvers.push((phase_time, [dv1, 0.0, 0.0]));
    maneuvers.push((phase_time + 3600.0, [dv2, 0.0, 0.0]));
    Ok(maneuvers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::coord::Coord3D;
    use super::super::physics::{EARTH_GM, EARTH_RADIUS};

    #[test]
    fn test_hohmann_transfer() {
        let (dv1, dv2) = hohmann_transfer_delta_v(6771000.0, 42164000.0, EARTH_GM);
        assert!(dv1 > 0.0);
        assert!(dv2 > 0.0);
    }

    #[test]
    fn test_gravity_assist() {
        let dv = gravity_assist_delta_v([10000.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.5);
        assert!(dv[0].abs() > 0.0);
    }

    #[test]
    fn test_lambert_solver() {
        let r1 = Coord3D::new(EARTH_RADIUS + 400e3, 0.0, 0.0);
        let r2 = Coord3D::new(0.0, EARTH_RADIUS + 400e3, 0.0);
        let result = lambert_solver(&r1, &r2, 1800.0, EARTH_GM, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_launch_window() {
        let oe = KeplerianElements::new(42164000.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let windows = launch_window(&oe, 0.0, 0.0, (0.0, 86400.0));
        assert!(!windows.is_empty());
    }

    #[test]
    fn test_station_keeping() {
        let dv = station_keeping_budget(42164000.0, 1e-6, 86400.0 * 30.0);
        assert!(dv > 0.0);
    }

    #[test]
    fn test_rendezvous() {
        let chaser = KeplerianElements::new(6771000.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let target = KeplerianElements::new(7000000.0, 0.0, 0.0, 0.0, 0.0, 0.1);
        let result = rendezvous_maneuver(&chaser, &target, EARTH_GM);
        assert!(result.is_ok());
    }
}
