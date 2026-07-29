//! Mechanical system motion and trajectory analysis.
//!
//! Provides utility functions for computing center of mass, total
//! momentum, trajectory length, and linkage ratios from rigid-body
//! state data.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use crate::domains::multibody::body::RigidBody;

/// Compute the center of mass of a set of rigid bodies.
///
/// `COM = Σ (m_i · r_i) / Σ m_i`
pub fn center_of_mass(bodies: &[RigidBody]) -> Coord3D {
    let mut total_mass = 0.0;
    let mut com_x = 0.0;
    let mut com_y = 0.0;
    let mut com_z = 0.0;

    for body in bodies {
        total_mass += body.mass;
        com_x += body.mass * body.position.x;
        com_y += body.mass * body.position.y;
        com_z += body.mass * body.position.z;
    }

    if total_mass > 0.0 {
        Coord3D::new(
            com_x / total_mass,
            com_y / total_mass,
            com_z / total_mass,
        )
    } else {
        Coord3D::new(0.0, 0.0, 0.0)
    }
}

/// Compute the total linear momentum of a system of bodies.
///
/// `P_total = Σ m_i · v_i`
pub fn total_momentum(bodies: &[RigidBody]) -> [Scalar; 3] {
    let mut px = 0.0;
    let mut py = 0.0;
    let mut pz = 0.0;

    for body in bodies {
        let p = body.linear_momentum();
        px += p[0];
        py += p[1];
        pz += p[2];
    }

    [px, py, pz]
}

/// Compute the total angular momentum about the origin.
///
/// `L_total = Σ (r_i × p_i + I_i · ω_i)`
pub fn total_angular_momentum(bodies: &[RigidBody]) -> [Scalar; 3] {
    let mut lx = 0.0;
    let mut ly = 0.0;
    let mut lz = 0.0;

    for body in bodies {
        // Orbital angular momentum: r × p
        let p = body.linear_momentum();
        let r = [body.position.x, body.position.y, body.position.z];
        let orbital = [
            r[1] * p[2] - r[2] * p[1],
            r[2] * p[0] - r[0] * p[2],
            r[0] * p[1] - r[1] * p[0],
        ];
        // Spin angular momentum: I·ω (body-frame → world-frame)
        let spin = body.angular_momentum();

        lx += orbital[0] + spin[0];
        ly += orbital[1] + spin[1];
        lz += orbital[2] + spin[2];
    }

    [lx, ly, lz]
}

/// Compute the total kinetic energy of all bodies.
pub fn total_kinetic_energy(bodies: &[RigidBody]) -> Scalar {
    bodies.iter().map(|b| b.kinetic_energy()).sum()
}

/// Compute the total mass of all bodies.
pub fn total_mass(bodies: &[RigidBody]) -> Scalar {
    bodies.iter().map(|b| b.mass).sum()
}

/// Compute the length of a trajectory from a sequence of positions.
///
/// Sums the Euclidean distance between consecutive points.
pub fn trajectory_length(positions: &[Coord3D]) -> Scalar {
    if positions.len() < 2 {
        return 0.0;
    }
    let mut length = 0.0;
    for i in 1..positions.len() {
        length += positions[i - 1].distance(&positions[i]);
    }
    length
}

/// Compute the linkage transmission ratio between input and output angles.
///
/// `ratio = output_angle / input_angle`
///
/// Returns 0 if input_angle is zero.
pub fn linkage_ratio(input_angle: Scalar, output_angle: Scalar) -> Scalar {
    if input_angle.abs() < 1e-30 {
        0.0
    } else {
        output_angle / input_angle
    }
}

/// Compute the RMS (root-mean-square) velocity of a system.
pub fn rms_velocity(bodies: &[RigidBody]) -> Scalar {
    let n = bodies.len();
    if n == 0 {
        return 0.0;
    }
    let mut sum_sq = 0.0;
    for body in bodies {
        let v = body.linear_velocity;
        sum_sq += v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    }
    (sum_sq / n as Scalar).sqrt()
}

/// Compute the average speed of all bodies.
pub fn average_speed(bodies: &[RigidBody]) -> Scalar {
    let n = bodies.len();
    if n == 0 {
        return 0.0;
    }
    let mut sum = 0.0;
    for body in bodies {
        let v = body.linear_velocity;
        sum += (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    }
    sum / n as Scalar
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_body(id: &str, pos: Coord3D, vel: [Scalar; 3]) -> RigidBody {
        let mut b = RigidBody::new(
            id,
            2.0,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            pos,
        );
        b.linear_velocity = vel;
        b
    }

    #[test]
    fn test_center_of_mass_single() {
        let bodies = vec![make_body("b1", Coord3D::new(3.0, 4.0, 5.0), [0.0; 3])];
        let com = center_of_mass(&bodies);
        assert!((com.x - 3.0).abs() < 1e-12);
        assert!((com.y - 4.0).abs() < 1e-12);
        assert!((com.z - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_center_of_mass_two() {
        let bodies = vec![
            make_body("b1", Coord3D::new(0.0, 0.0, 0.0), [0.0; 3]),
            make_body("b2", Coord3D::new(2.0, 0.0, 0.0), [0.0; 3]),
        ];
        // Both have mass 2, so COM = (1, 0, 0)
        let com = center_of_mass(&bodies);
        assert!((com.x - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_total_momentum() {
        let bodies = vec![
            make_body("b1", Coord3D::new(0.0, 0.0, 0.0), [1.0, 0.0, 0.0]),
            make_body("b2", Coord3D::new(0.0, 0.0, 0.0), [0.0, 2.0, 0.0]),
        ];
        // mass=2 each: p1 = [2,0,0], p2 = [0,4,0], total = [2,4,0]
        let p = total_momentum(&bodies);
        assert!((p[0] - 2.0).abs() < 1e-12);
        assert!((p[1] - 4.0).abs() < 1e-12);
        assert!((p[2]).abs() < 1e-12);
    }

    #[test]
    fn test_total_angular_momentum() {
        let bodies = vec![make_body(
            "b1",
            Coord3D::new(1.0, 0.0, 0.0),
            [0.0, 1.0, 0.0],
        )];
        // orbital: r × p = (1,0,0) × (0,2,0) = (0,0,2)
        // spin: I·ω = 0 (ω=0)
        let l = total_angular_momentum(&bodies);
        assert!((l[0]).abs() < 1e-12);
        assert!((l[1]).abs() < 1e-12);
        assert!((l[2] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn test_trajectory_length() {
        let positions = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(3.0, 0.0, 0.0),
            Coord3D::new(3.0, 4.0, 0.0),
        ];
        let len = trajectory_length(&positions);
        assert!((len - 7.0).abs() < 1e-12); // 3 + 5 = 8? Wait: first segment=3, second=4, total=7
    }

    #[test]
    fn test_trajectory_length_single() {
        let len = trajectory_length(&[Coord3D::new(1.0, 2.0, 3.0)]);
        assert!((len).abs() < 1e-12);
    }

    #[test]
    fn test_trajectory_length_empty() {
        let len = trajectory_length(&[]);
        assert!((len).abs() < 1e-12);
    }

    #[test]
    fn test_linkage_ratio() {
        let r = linkage_ratio(std::f64::consts::PI / 4.0, std::f64::consts::PI / 2.0);
        assert!((r - 2.0).abs() < 1e-12);
    }

    #[test]
    fn test_linkage_ratio_zero_input() {
        let r = linkage_ratio(0.0, 1.0);
        assert!((r).abs() < 1e-12);
    }

    #[test]
    fn test_total_kinetic_energy() {
        let bodies = vec![make_body("b1", Coord3D::new(0.0, 0.0, 0.0), [2.0, 0.0, 0.0])];
        // KE = 0.5 * 2 * 4 = 4
        let ke = total_kinetic_energy(&bodies);
        assert!((ke - 4.0).abs() < 1e-12);
    }

    #[test]
    fn test_total_mass() {
        let bodies = vec![
            make_body("b1", Coord3D::new(0.0, 0.0, 0.0), [0.0; 3]),
            make_body("b2", Coord3D::new(0.0, 0.0, 0.0), [0.0; 3]),
        ];
        let m = total_mass(&bodies);
        assert!((m - 4.0).abs() < 1e-12);
    }

    #[test]
    fn test_rms_velocity() {
        let bodies = vec![make_body("b1", Coord3D::new(0.0, 0.0, 0.0), [3.0, 4.0, 0.0])];
        let rms = rms_velocity(&bodies);
        assert!((rms - 5.0).abs() < 1e-12); // sqrt(25) = 5
    }

    #[test]
    fn test_average_speed() {
        let bodies = vec![make_body("b1", Coord3D::new(0.0, 0.0, 0.0), [3.0, 4.0, 0.0])];
        let avg = average_speed(&bodies);
        assert!((avg - 5.0).abs() < 1e-12);
    }
}
