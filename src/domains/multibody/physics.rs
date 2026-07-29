//! Multibody physics constants and rigid-body material properties.
//!
//! Provides the standard gravitational acceleration vector and a
//! properties struct for rigid-body mass distribution (mass, center
//! of mass, inertia tensor).

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Standard gravitational acceleration on Earth (m/s²) along −Z.
///
/// This is a right-handed Cartesian convention where +Z is upward.
pub const GRAVITY: [Scalar; 3] = [0.0, 0.0, -9.80665];

/// Inertial properties of a rigid body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyProperties {
    /// Mass in kg.
    pub mass: Scalar,
    /// Center of mass location in body-local coordinates (m).
    pub com: Coord3D,
    /// Inertia tensor about the center of mass, in body-local frame (kg·m²).
    pub inertia_tensor: [[Scalar; 3]; 3],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::coord::Coord3D;

    #[test]
    fn test_gravity_vector() {
        assert_eq!(GRAVITY[0], 0.0);
        assert_eq!(GRAVITY[1], 0.0);
        assert!(GRAVITY[2] < 0.0);
    }

    #[test]
    fn test_rigid_body_properties_default() {
        let props = RigidBodyProperties {
            mass: 1.0,
            com: Coord3D::new(0.0, 0.0, 0.0),
            inertia_tensor: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        };
        assert_eq!(props.mass, 1.0);
        assert_eq!(props.inertia_tensor[0][0], 1.0);
    }
}
