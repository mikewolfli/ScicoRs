//! Celestial body models with realistic preset bodies.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use super::physics::*;

/// Classification tag for a celestial body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CelestialBodyType {
    Star, Planet, Moon, DwarfPlanet, Asteroid, Comet, BlackHole, NeutronStar, Galaxy,
}

/// A celestial body with physical properties.
#[derive(Debug, Clone, PartialEq)]
pub struct CelestialBody {
    pub id: String,
    pub name: String,
    pub body_type: CelestialBodyType,
    pub mass: Scalar,
    pub radius: Scalar,
    pub position: Coord3D,
    pub velocity: [Scalar; 3],
    pub rotation_rate: Scalar,
    pub axial_tilt: Scalar,
    pub gravitational_parameter: Scalar,
}

impl CelestialBody {
    pub fn new(id: &str, name: &str, mass: Scalar, radius: Scalar) -> Self {
        Self {
            id: id.to_string(), name: name.to_string(),
            body_type: CelestialBodyType::Planet, mass, radius,
            position: Coord3D::new(0.0, 0.0, 0.0),
            velocity: [0.0, 0.0, 0.0],
            rotation_rate: 0.0, axial_tilt: 0.0,
            gravitational_parameter: GRAVITATIONAL * mass,
        }
    }

    pub fn with_type(mut self, t: CelestialBodyType) -> Self { self.body_type = t; self }
    pub fn with_position(mut self, p: Coord3D) -> Self { self.position = p; self }
    pub fn with_velocity(mut self, v: [Scalar; 3]) -> Self { self.velocity = v; self }
    pub fn with_rotation(mut self, rate: Scalar, tilt: Scalar) -> Self { self.rotation_rate = rate; self.axial_tilt = tilt; self }

    pub fn surface_gravity(&self) -> Scalar { self.gravitational_parameter / (self.radius * self.radius) }

    pub fn escape_velocity(&self, altitude: Scalar) -> Scalar {
        (2.0 * self.gravitational_parameter / (self.radius + altitude)).sqrt()
    }

    pub fn schwarzschild_radius(&self) -> Scalar { 2.0 * self.gravitational_parameter / (C * C) }

    pub fn orbital_period(&self, semi_major_axis: Scalar, parent_mass: Scalar) -> Scalar {
        let gm = GRAVITATIONAL * parent_mass;
        2.0 * std::f64::consts::PI * (semi_major_axis.powi(3) / gm).sqrt()
    }
}

// Presets
pub fn sun() -> CelestialBody {
    CelestialBody::new("sun", "Sun", SOLAR_MASS, SOLAR_RADIUS).with_type(CelestialBodyType::Star).with_rotation(2.865e-6, 7.25_f64.to_radians())
}
pub fn mercury() -> CelestialBody {
    CelestialBody::new("mercury", "Mercury", 3.3011e23, 2.4397e6).with_type(CelestialBodyType::Planet).with_rotation(1.24e-6, 0.034_f64.to_radians())
}
pub fn venus() -> CelestialBody {
    CelestialBody::new("venus", "Venus", 4.8675e24, 6.0518e6).with_type(CelestialBodyType::Planet).with_rotation(-2.99e-7, 177.4_f64.to_radians())
}
pub fn earth() -> CelestialBody {
    CelestialBody::new("earth", "Earth", EARTH_MASS, EARTH_RADIUS).with_type(CelestialBodyType::Planet).with_rotation(7.2921159e-5, 23.44_f64.to_radians())
}
pub fn mars() -> CelestialBody {
    CelestialBody::new("mars", "Mars", 6.4171e23, 3.3895e6).with_type(CelestialBodyType::Planet).with_rotation(7.088e-5, 25.19_f64.to_radians())
}
pub fn jupiter() -> CelestialBody {
    CelestialBody::new("jupiter", "Jupiter", 1.8982e27, 6.9911e7).with_type(CelestialBodyType::Planet).with_rotation(1.758e-4, 3.13_f64.to_radians())
}
pub fn saturn() -> CelestialBody {
    CelestialBody::new("saturn", "Saturn", 5.6834e26, 5.8232e7).with_type(CelestialBodyType::Planet).with_rotation(1.637e-4, 26.73_f64.to_radians())
}
pub fn uranus() -> CelestialBody {
    CelestialBody::new("uranus", "Uranus", 8.6810e25, 2.5362e7).with_type(CelestialBodyType::Planet).with_rotation(-1.091e-4, 97.77_f64.to_radians())
}
pub fn neptune() -> CelestialBody {
    CelestialBody::new("neptune", "Neptune", 1.02413e26, 2.4622e7).with_type(CelestialBodyType::Planet).with_rotation(1.085e-4, 28.32_f64.to_radians())
}
pub fn moon() -> CelestialBody {
    CelestialBody::new("moon", "Moon", 7.342e22, 1.7374e6).with_type(CelestialBodyType::Moon).with_rotation(2.6617e-6, 1.542_f64.to_radians())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_earth_surface_gravity() {
        let e = earth();
        assert!((e.surface_gravity() - 9.81).abs() < 0.05);
    }

    #[test]
    fn test_earth_escape_velocity() {
        let e = earth();
        assert!((e.escape_velocity(0.0) - 11186.0).abs() < 50.0);
    }

    #[test]
    fn test_sun_schwarzschild_radius() {
        let s = sun();
        assert!((s.schwarzschild_radius() - 2950.0).abs() < 10.0);
    }

    #[test]
    fn test_earth_orbital_period() {
        let e = earth();
        let days = e.orbital_period(AU, SOLAR_MASS) / 86400.0;
        assert!((days - 365.25).abs() < 1.0);
    }

    #[test]
    fn test_mars_smaller_than_earth() {
        assert!(mars().mass < earth().mass);
        assert!(mars().radius < earth().radius);
    }

    #[test]
    fn test_jupiter_largest() {
        assert!(jupiter().mass > earth().mass * 100.0);
    }

    #[test]
    fn test_moon_type() {
        assert_eq!(moon().body_type, CelestialBodyType::Moon);
    }
}
