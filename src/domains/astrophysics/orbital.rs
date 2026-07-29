//! Orbital mechanics: Keplerian elements, two-body/N-body propagators.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use super::celestial_body::CelestialBody;
use super::physics::GRAVITATIONAL;

/// Keplerian orbital elements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeplerianElements {
    pub semi_major_axis: Scalar,
    pub eccentricity: Scalar,
    pub inclination: Scalar,
    pub raan: Scalar,
    pub argument_of_periapsis: Scalar,
    pub true_anomaly: Scalar,
}

impl KeplerianElements {
    pub fn new(a: Scalar, e: Scalar, i: Scalar, raan: Scalar, arg_peri: Scalar, nu: Scalar) -> Self {
        Self {
            semi_major_axis: a, eccentricity: e, inclination: i,
            raan, argument_of_periapsis: arg_peri, true_anomaly: nu,
        }
    }

    pub fn specific_energy(&self, gm: Scalar) -> Scalar {
        -gm / (2.0 * self.semi_major_axis)
    }

    pub fn specific_angular_momentum(&self, gm: Scalar) -> Scalar {
        (gm * self.semi_major_axis * (1.0 - self.eccentricity * self.eccentricity)).sqrt()
    }

    pub fn period(&self, gm: Scalar) -> Scalar {
        2.0 * std::f64::consts::PI * (self.semi_major_axis.powi(3) / gm).sqrt()
    }

    pub fn periapsis_distance(&self) -> Scalar {
        self.semi_major_axis * (1.0 - self.eccentricity)
    }

    pub fn apoapsis_distance(&self) -> Scalar {
        self.semi_major_axis * (1.0 + self.eccentricity)
    }

    pub fn to_cartesian(&self, gm: Scalar) -> (Coord3D, [Scalar; 3]) {
        let e = self.eccentricity;
        let a = self.semi_major_axis;
        let nu = self.true_anomaly;
        let i = self.inclination;
        let raan = self.raan;
        let w = self.argument_of_periapsis;

        let h = self.specific_angular_momentum(gm);
        let r = a * (1.0 - e * e) / (1.0 + e * nu.cos());

        // Position in orbital plane
        let x_orb = r * nu.cos();
        let y_orb = r * nu.sin();

        // Rotate to inertial frame
        let cos_raan = raan.cos();
        let sin_raan = raan.sin();
        let cos_i = i.cos();
        let sin_i = i.sin();
        let cos_w = w.cos();
        let sin_w = w.sin();

        let x = (cos_raan * cos_w - sin_raan * sin_w * cos_i) * x_orb
            + (-cos_raan * sin_w - sin_raan * cos_w * cos_i) * y_orb;
        let y = (sin_raan * cos_w + cos_raan * sin_w * cos_i) * x_orb
            + (-sin_raan * sin_w + cos_raan * cos_w * cos_i) * y_orb;
        let z = (sin_w * sin_i) * x_orb + (cos_w * sin_i) * y_orb;

        // Velocity in orbital frame
        let p = a * (1.0 - e * e);
        let vx_orb = -(h / p) * nu.sin();
        let vy_orb = (h / p) * (e + nu.cos());

        let vx = (cos_raan * cos_w - sin_raan * sin_w * cos_i) * vx_orb
            + (-cos_raan * sin_w - sin_raan * cos_w * cos_i) * vy_orb;
        let vy = (sin_raan * cos_w + cos_raan * sin_w * cos_i) * vx_orb
            + (-sin_raan * sin_w + cos_raan * cos_w * cos_i) * vy_orb;
        let vz = (sin_w * sin_i) * vx_orb + (cos_w * sin_i) * vy_orb;

        (Coord3D::new(x, y, z), [vx, vy, vz])
    }

    pub fn from_cartesian(pos: &Coord3D, vel: &[Scalar; 3], gm: Scalar) -> Self {
        let r = (pos.x * pos.x + pos.y * pos.y + pos.z * pos.z).sqrt();
        let v2 = vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2];
        let h_vec = [
            pos.y * vel[2] - pos.z * vel[1],
            pos.z * vel[0] - pos.x * vel[2],
            pos.x * vel[1] - pos.y * vel[0],
        ];
        let h = (h_vec[0] * h_vec[0] + h_vec[1] * h_vec[1] + h_vec[2] * h_vec[2]).sqrt();

        let a = 1.0 / (2.0 / r - v2 / gm);
        let e_vec = [
            ((v2 / gm - 1.0 / r) * pos.x - (pos.x * vel[0] + pos.y * vel[1] + pos.z * vel[2]) * vel[0] / gm),
            ((v2 / gm - 1.0 / r) * pos.y - (pos.x * vel[0] + pos.y * vel[1] + pos.z * vel[2]) * vel[1] / gm),
            ((v2 / gm - 1.0 / r) * pos.z - (pos.x * vel[0] + pos.y * vel[1] + pos.z * vel[2]) * vel[2] / gm),
        ];
        let e = (e_vec[0] * e_vec[0] + e_vec[1] * e_vec[1] + e_vec[2] * e_vec[2]).sqrt();
        let i = (h_vec[2] / h).acos();
        let n_vec = [-h_vec[1], h_vec[0], 0.0];
        let n = (n_vec[0] * n_vec[0] + n_vec[1] * n_vec[1]).sqrt();
        let raan = if n > 0.0 { n_vec[0].atan2(n_vec[1]) } else { 0.0 };
        let w = if n > 0.0 {
            let dot = (n_vec[0] * e_vec[0] + n_vec[1] * e_vec[1]) / (n * e);
            dot.acos().copysign(e_vec[2])
        } else {
            e_vec[0].atan2(e_vec[1])
        };

        let r_dot_v = pos.x * vel[0] + pos.y * vel[1] + pos.z * vel[2];
        let nu = if r_dot_v >= 0.0 {
            ((e_vec[0] * pos.x + e_vec[1] * pos.y + e_vec[2] * pos.z) / (e * r)).acos()
        } else {
            2.0 * std::f64::consts::PI - ((e_vec[0] * pos.x + e_vec[1] * pos.y + e_vec[2] * pos.z) / (e * r)).acos()
        };

        Self { semi_major_axis: a, eccentricity: e, inclination: i, raan, argument_of_periapsis: w, true_anomaly: nu }
    }

    pub fn solve_kepler(&self, mean_anomaly: Scalar) -> Scalar {
        let mut e = mean_anomaly;
        for _ in 0..100 {
            let delta = (e - self.eccentricity * e.sin() - mean_anomaly) / (1.0 - self.eccentricity * e.cos());
            e -= delta;
            if delta.abs() < 1e-12 { break; }
        }
        // True anomaly from eccentric anomaly: ν = 2·atan(√((1+e)/(1-e))·tan(E/2))
        2.0 * (((1.0 + self.eccentricity) / (1.0 - self.eccentricity)).sqrt() * (e / 2.0).tan()).atan()
    }
}

/// Two-body orbital propagator.
pub struct TwoBodyPropagator {
    pub gm: Scalar,
}

impl TwoBodyPropagator {
    pub fn new(gm: Scalar) -> Self { Self { gm } }

    pub fn propagate(&self, elements: &KeplerianElements, dt: Scalar) -> KeplerianElements {
        let mean_motion = (self.gm / elements.semi_major_axis.powi(3)).sqrt();
        let mean_anomaly = elements.true_anomaly + mean_motion * dt;
        let e_anomaly = elements.solve_kepler(mean_anomaly);
        let nu = 2.0 * (((1.0 + elements.eccentricity) / (1.0 - elements.eccentricity)).sqrt() * (e_anomaly / 2.0).tan()).atan();
        KeplerianElements {
            true_anomaly: nu % (2.0 * std::f64::consts::PI),
            ..*elements
        }
    }

    pub fn propagate_with_perturbation(&self, elements: &KeplerianElements, dt: Scalar, j2: Scalar) -> KeplerianElements {
        let n = elements.period(self.gm).recip();
        let raan_dot = -1.5 * n * j2 * (super::physics::EARTH_RADIUS / elements.semi_major_axis).powi(2)
            * elements.inclination.cos() / (1.0 - elements.eccentricity * elements.eccentricity).powi(2);
        let arg_peri_dot = 0.75 * n * j2 * (super::physics::EARTH_RADIUS / elements.semi_major_axis).powi(2)
            * (4.0 - 5.0 * elements.inclination.sin().powi(2)) / (1.0 - elements.eccentricity * elements.eccentricity).powi(2);

        let mut result = self.propagate(elements, dt);
        result.raan += raan_dot * dt;
        result.argument_of_periapsis += arg_peri_dot * dt;
        result
    }
}

/// N-body gravitational solver.
pub struct NBodySolver {
    pub bodies: Vec<CelestialBody>,
    pub softening: Scalar,
}

impl NBodySolver {
    pub fn new(bodies: Vec<CelestialBody>, softening: Scalar) -> Self {
        Self { bodies, softening }
    }

    pub fn accelerations(&self) -> Vec<[Scalar; 3]> {
        let n = self.bodies.len();
        let bodies = &self.bodies;
        let softening = self.softening;
        use rayon::prelude::*;
        (0..n).into_par_iter().map(|i| {
            let mut ax = 0.0; let mut ay = 0.0; let mut az = 0.0;
            for j in 0..n {
                if i == j { continue; }
                let dx = bodies[j].position.x - bodies[i].position.x;
                let dy = bodies[j].position.y - bodies[i].position.y;
                let dz = bodies[j].position.z - bodies[i].position.z;
                let r2 = dx * dx + dy * dy + dz * dz + softening * softening;
                let inv_r3 = 1.0 / (r2 * r2.sqrt());
                ax += GRAVITATIONAL * bodies[j].mass * dx * inv_r3;
                ay += GRAVITATIONAL * bodies[j].mass * dy * inv_r3;
                az += GRAVITATIONAL * bodies[j].mass * dz * inv_r3;
            }
            [ax, ay, az]
        }).collect()
    }

    pub fn leapfrog_step(&mut self, dt: Scalar) {
        let accs = self.accelerations();
        for (i, body) in self.bodies.iter_mut().enumerate() {
            body.velocity[0] += 0.5 * accs[i][0] * dt;
            body.velocity[1] += 0.5 * accs[i][1] * dt;
            body.velocity[2] += 0.5 * accs[i][2] * dt;
            body.position = Coord3D::new(
                body.position.x + body.velocity[0] * dt,
                body.position.y + body.velocity[1] * dt,
                body.position.z + body.velocity[2] * dt,
            );
        }
        let accs_new = self.accelerations();
        for (i, body) in self.bodies.iter_mut().enumerate() {
            body.velocity[0] += 0.5 * accs_new[i][0] * dt;
            body.velocity[1] += 0.5 * accs_new[i][1] * dt;
            body.velocity[2] += 0.5 * accs_new[i][2] * dt;
        }
    }

    pub fn total_energy(&self) -> Scalar {
        let mut ke = 0.0;
        let mut pe = 0.0;
        for body in &self.bodies {
            let v2 = body.velocity[0] * body.velocity[0] + body.velocity[1] * body.velocity[1] + body.velocity[2] * body.velocity[2];
            ke += 0.5 * body.mass * v2;
        }
        for i in 0..self.bodies.len() {
            for j in i + 1..self.bodies.len() {
                let dx = self.bodies[j].position.x - self.bodies[i].position.x;
                let dy = self.bodies[j].position.y - self.bodies[i].position.y;
                let dz = self.bodies[j].position.z - self.bodies[i].position.z;
                let r = (dx * dx + dy * dy + dz * dz).sqrt();
                if r > 1e-15 {
                    pe -= GRAVITATIONAL * self.bodies[i].mass * self.bodies[j].mass / r;
                }
            }
        }
        ke + pe
    }

    pub fn total_angular_momentum(&self) -> [Scalar; 3] {
        let mut l = [0.0; 3];
        for body in &self.bodies {
            l[0] += body.mass * (body.position.y * body.velocity[2] - body.position.z * body.velocity[1]);
            l[1] += body.mass * (body.position.z * body.velocity[0] - body.position.x * body.velocity[2]);
            l[2] += body.mass * (body.position.x * body.velocity[1] - body.position.y * body.velocity[0]);
        }
        l
    }
}

/// J2 precession rate of RAAN (rad/s).
pub fn j2_precession_rate(semi_major: Scalar, eccentricity: Scalar, inclination: Scalar, j2: Scalar, radius: Scalar, gm: Scalar) -> Scalar {
    let n = (gm / semi_major.powi(3)).sqrt();
    -1.5 * n * j2 * (radius / semi_major).powi(2) * inclination.cos()
        / (1.0 - eccentricity * eccentricity).powi(2)
}

pub struct J2PrecessionRate;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::coord::Coord3D;
    use super::super::physics::{SOLAR_GM, AU};
    use super::super::celestial_body::{earth, sun};
    #[test]
    fn test_circular_orbit_energy() {
        let oe = KeplerianElements::new(AU, 0.0, 0.0, 0.0, 0.0, 0.0);
        let e = oe.specific_energy(SOLAR_GM);
        assert!(e < 0.0);
    }

    #[test]
    fn test_period() {
        let oe = KeplerianElements::new(AU, 0.0, 0.0, 0.0, 0.0, 0.0);
        let days = oe.period(SOLAR_GM) / 86400.0;
        assert!((days - 365.25).abs() < 1.0);
    }

    #[test]
    fn test_to_cartesian() {
        let oe = KeplerianElements::new(AU, 0.0, 0.0, 0.0, 0.0, 0.0);
        let (pos, vel) = oe.to_cartesian(SOLAR_GM);
        let r = (pos.x * pos.x + pos.y * pos.y + pos.z * pos.z).sqrt();
        assert!((r - AU).abs() / AU < 0.01);
        let v_mag = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
        assert!(v_mag > 0.0);
    }

    #[test]
    fn test_from_cartesian_roundtrip() {
        let pos = Coord3D::new(AU, 0.0, 0.0);
        let v_circ = (SOLAR_GM / AU).sqrt();
        let vel = [0.0, v_circ, 0.0];
        let oe = KeplerianElements::from_cartesian(&pos, &vel, SOLAR_GM);
        assert!((oe.semi_major_axis - AU).abs() / AU < 0.01);
        assert!(oe.eccentricity < 0.01);
    }

    #[test]
    fn test_nbody_accelerations() {
        let e = earth().with_position(Coord3D::new(AU, 0.0, 0.0));
        let s = sun().with_position(Coord3D::new(0.0, 0.0, 0.0));
        let solver = NBodySolver::new(vec![s, e], 1e3);
        let accs = solver.accelerations();
        assert_eq!(accs.len(), 2);
    }

    #[test]
    fn test_two_body_propagator() {
        let oe = KeplerianElements::new(AU, 0.0, 0.0, 0.0, 0.0, 0.0);
        let prop = TwoBodyPropagator::new(SOLAR_GM);
        let result = prop.propagate(&oe, 86400.0 * 90.0);
        // After 90 days on 365.25-day orbit: 2π * 90/365.25 ≈ 1.548 rad
        assert!((result.true_anomaly - 1.548).abs() < 0.01);
    }

    #[test]
    fn test_solve_kepler() {
        let oe = KeplerianElements::new(AU, 0.1, 0.0, 0.0, 0.0, 0.0);
        let e = oe.solve_kepler(0.5);
        assert!(e.is_finite());
    }
}
