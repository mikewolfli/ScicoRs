//! Coordinate Systems (Phase 10).
//!
//! Provides 1D, 2D, and 3D coordinate types in Cartesian, polar,
//! cylindrical, and spherical systems, along with transformations
//! (translation, rotation, scaling) and reference frame abstractions.
//!
//! # Coordinate Types
//!
//! - **`Coord1D`** — single scalar coordinate (time, distance along axis)
//! - **`Coord2D`** — planar coordinates (Cartesian, polar)
//! - **`Coord3D`** — spatial coordinates (Cartesian, cylindrical, spherical)
//! - **`CoordSystem`** — enum tagging the system type
//!
//! # Transformations
//!
//! - Translation, rotation (2D/3D), scaling, homogeneous transforms
//! - System conversions: polar ↔ cartesian, cylindrical ↔ cartesian, etc.

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// 1. Coordinate System Enum
// ──────────────────────────────────────────────

/// Enumeration of supported coordinate system types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordSystem {
    /// 1D linear coordinate (e.g. time, position on a line).
    Linear1D,
    /// 2D Cartesian (x, y).
    Cartesian2D,
    /// 2D Polar (r, θ) with θ in radians.
    Polar2D,
    /// 3D Cartesian (x, y, z).
    Cartesian3D,
    /// 3D Cylindrical (r, θ, z) with θ in radians.
    Cylindrical3D,
    /// 3D Spherical (r, θ, φ) with θ inclination, φ azimuth in radians.
    Spherical3D,
}

// ──────────────────────────────────────────────
// 2. Coordinate Structs
// ──────────────────────────────────────────────

/// A 1D coordinate (scalar position on a line or time axis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord1D(pub Scalar);

impl Coord1D {
    pub fn new(x: Scalar) -> Self {
        Self(x)
    }

    pub fn x(&self) -> Scalar {
        self.0
    }

    pub fn translate(&self, dx: Scalar) -> Self {
        Self(self.0 + dx)
    }

    pub fn scale(&self, factor: Scalar) -> Self {
        Self(self.0 * factor)
    }

    pub fn distance(&self, other: &Coord1D) -> Scalar {
        (self.0 - other.0).abs()
    }
}

/// A 2D coordinate in either Cartesian (x, y) or Polar (r, θ) form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2D {
    pub x: Scalar,
    pub y: Scalar,
}

impl Coord2D {
    /// Create a new Cartesian 2D coordinate.
    pub fn new(x: Scalar, y: Scalar) -> Self {
        Self { x, y }
    }

    /// Create from polar coordinates: (r, θ) in radians.
    pub fn from_polar(r: Scalar, theta: Scalar) -> Self {
        Self {
            x: r * theta.cos(),
            y: r * theta.sin(),
        }
    }

    /// Convert to polar: returns (r, θ) with θ in [-π, π].
    pub fn to_polar(&self) -> (Scalar, Scalar) {
        let r = (self.x * self.x + self.y * self.y).sqrt();
        let theta = self.y.atan2(self.x);
        (r, theta)
    }

    pub fn translate(&self, dx: Scalar, dy: Scalar) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    pub fn scale(&self, factor: Scalar) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }

    /// Rotate by `angle` radians counter-clockwise about the origin.
    pub fn rotate(&self, angle: Scalar) -> Self {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        Self::new(
            self.x * cos_a - self.y * sin_a,
            self.x * sin_a + self.y * cos_a,
        )
    }

    /// Euclidean distance to another 2D point.
    pub fn distance(&self, other: &Coord2D) -> Scalar {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// A 3D coordinate supporting Cartesian, cylindrical, and spherical.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: Scalar,
    pub y: Scalar,
    pub z: Scalar,
}

impl Coord3D {
    /// Create a new Cartesian 3D coordinate.
    pub fn new(x: Scalar, y: Scalar, z: Scalar) -> Self {
        Self { x, y, z }
    }

    /// Create from cylindrical: (r, θ, z), θ in radians.
    pub fn from_cylindrical(r: Scalar, theta: Scalar, z: Scalar) -> Self {
        Self {
            x: r * theta.cos(),
            y: r * theta.sin(),
            z,
        }
    }

    /// Convert to cylindrical: returns (r, θ, z).
    pub fn to_cylindrical(&self) -> (Scalar, Scalar, Scalar) {
        let r = (self.x * self.x + self.y * self.y).sqrt();
        let theta = self.y.atan2(self.x);
        (r, theta, self.z)
    }

    /// Create from spherical: (r, θ, φ), θ=inclination [0,π], φ=azimuth [0,2π).
    pub fn from_spherical(r: Scalar, theta: Scalar, phi: Scalar) -> Self {
        Self {
            x: r * theta.sin() * phi.cos(),
            y: r * theta.sin() * phi.sin(),
            z: r * theta.cos(),
        }
    }

    /// Convert to spherical: returns (r, θ, φ).
    pub fn to_spherical(&self) -> (Scalar, Scalar, Scalar) {
        let r = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if r < 1e-30 {
            return (0.0, 0.0, 0.0);
        }
        let theta = (self.z / r).acos();
        let phi = self.y.atan2(self.x);
        (r, theta, phi)
    }

    pub fn translate(&self, dx: Scalar, dy: Scalar, dz: Scalar) -> Self {
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }

    pub fn scale(&self, factor: Scalar) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }

    /// Rotate about the X axis by `angle` radians.
    pub fn rotate_x(&self, angle: Scalar) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self::new(self.x, self.y * c - self.z * s, self.y * s + self.z * c)
    }

    /// Rotate about the Y axis by `angle` radians.
    pub fn rotate_y(&self, angle: Scalar) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self::new(self.x * c + self.z * s, self.y, -self.x * s + self.z * c)
    }

    /// Rotate about the Z axis by `angle` radians.
    pub fn rotate_z(&self, angle: Scalar) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self::new(self.x * c - self.y * s, self.x * s + self.y * c, self.z)
    }

    /// Euclidean distance to another 3D point.
    pub fn distance(&self, other: &Coord3D) -> Scalar {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Cross product with another 3D vector.
    pub fn cross(&self, other: &Coord3D) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// Dot product with another 3D vector.
    pub fn dot(&self, other: &Coord3D) -> Scalar {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Magnitude (norm) of this vector.
    pub fn norm(&self) -> Scalar {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Return a unit vector in the same direction.
    pub fn normalize(&self) -> Option<Self> {
        let n = self.norm();
        if n < 1e-30 {
            None
        } else {
            Some(Self::new(self.x / n, self.y / n, self.z / n))
        }
    }
}

// ──────────────────────────────────────────────
// 3. Homogeneous Transformations (4x4)
// ──────────────────────────────────────────────

/// A 4×4 homogeneous transformation matrix for 3D coordinate transforms.
///
/// Stored in row-major order as a flat 16-element array.
/// Represents: `[R | t; 0 0 0 1]` where R is 3×3 rotation and t is translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform4x4 {
    /// Flat 16-element row-major matrix data [r00, r01, r02, tx, ...].
    pub data: [Scalar; 16],
}

impl Transform4x4 {
    /// Identity transformation.
    pub fn identity() -> Self {
        Self {
            data: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Translation-only transform.
    pub fn translation(tx: Scalar, ty: Scalar, tz: Scalar) -> Self {
        Self {
            data: [
                1.0, 0.0, 0.0, tx, 0.0, 1.0, 0.0, ty, 0.0, 0.0, 1.0, tz, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Uniform scaling transform.
    pub fn scale(s: Scalar) -> Self {
        Self {
            data: [
                s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Rotate about X axis by `angle` radians.
    pub fn rotation_x(angle: Scalar) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self {
            data: [
                1.0, 0.0, 0.0, 0.0, 0.0, c, -s, 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Rotate about Y axis by `angle` radians.
    pub fn rotation_y(angle: Scalar) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self {
            data: [
                c, 0.0, s, 0.0, 0.0, 1.0, 0.0, 0.0, -s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Rotate about Z axis by `angle` radians.
    pub fn rotation_z(angle: Scalar) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self {
            data: [
                c, -s, 0.0, 0.0, s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Apply this transform to a 3D point (treats point as [x, y, z, 1]).
    pub fn apply(&self, point: &Coord3D) -> Coord3D {
        let x =
            self.data[0] * point.x + self.data[1] * point.y + self.data[2] * point.z + self.data[3];
        let y =
            self.data[4] * point.x + self.data[5] * point.y + self.data[6] * point.z + self.data[7];
        let z = self.data[8] * point.x
            + self.data[9] * point.y
            + self.data[10] * point.z
            + self.data[11];
        Coord3D::new(x, y, z)
    }

    /// Compose this transform with another: `self * other`.
    pub fn compose(&self, other: &Transform4x4) -> Self {
        let mut result = [0.0; 16];
        for row in 0..4 {
            for col in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += self.data[row * 4 + k] * other.data[k * 4 + col];
                }
                result[row * 4 + col] = sum;
            }
        }
        Self { data: result }
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::EPSILON;

    #[test]
    fn test_coord2d_cartesian() {
        let p = Coord2D::new(3.0, 4.0);
        assert!((p.x - 3.0).abs() < EPSILON);
        assert!((p.y - 4.0).abs() < EPSILON);
    }

    #[test]
    fn test_coord2d_polar_conversion() {
        let p = Coord2D::new(1.0, 1.0);
        let (r, theta) = p.to_polar();
        assert!((r - 2.0_f64.sqrt()).abs() < 1e-12);
        assert!((theta - std::f64::consts::FRAC_PI_4).abs() < 1e-12);

        let back = Coord2D::from_polar(r, theta);
        assert!((back.x - p.x).abs() < 1e-12);
        assert!((back.y - p.y).abs() < 1e-12);
    }

    #[test]
    fn test_coord2d_rotate() {
        let p = Coord2D::new(1.0, 0.0);
        let rotated = p.rotate(std::f64::consts::FRAC_PI_2);
        assert!((rotated.x - 0.0).abs() < 1e-12);
        assert!((rotated.y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_coord2d_distance() {
        let a = Coord2D::new(0.0, 0.0);
        let b = Coord2D::new(3.0, 4.0);
        assert!((a.distance(&b) - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_coord3d_cartesian() {
        let p = Coord3D::new(1.0, 2.0, 3.0);
        assert!((p.x - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_coord3d_cylindrical_conversion() {
        let p = Coord3D::new(1.0, 1.0, 2.0);
        let (r, theta, z) = p.to_cylindrical();
        assert!((r - 2.0_f64.sqrt()).abs() < 1e-12);
        assert!((z - 2.0).abs() < EPSILON);

        let back = Coord3D::from_cylindrical(r, theta, z);
        assert!((back.x - p.x).abs() < 1e-12);
        assert!((back.y - p.y).abs() < 1e-12);
    }

    #[test]
    fn test_coord3d_spherical_conversion() {
        let p = Coord3D::new(0.0, 0.0, 1.0);
        let (r, theta, phi) = p.to_spherical();
        assert!((r - 1.0).abs() < EPSILON);
        assert!((theta - 0.0).abs() < 1e-12);

        let back = Coord3D::from_spherical(r, theta, phi);
        assert!((back.x - p.x).abs() < 1e-12);
        assert!((back.z - p.z).abs() < 1e-12);
    }

    #[test]
    fn test_coord3d_rotate_x() {
        let p = Coord3D::new(0.0, 1.0, 0.0);
        let r = p.rotate_x(std::f64::consts::FRAC_PI_2);
        assert!((r.y - 0.0).abs() < 1e-12);
        assert!((r.z - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_coord3d_cross_dot() {
        let a = Coord3D::new(1.0, 0.0, 0.0);
        let b = Coord3D::new(0.0, 1.0, 0.0);
        let cross = a.cross(&b);
        assert!((cross.z - 1.0).abs() < EPSILON);
        assert!((a.dot(&b) - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_coord3d_normalize() {
        let p = Coord3D::new(3.0, 0.0, 0.0);
        let n = p.normalize().unwrap();
        assert!((n.x - 1.0).abs() < EPSILON);

        let zero = Coord3D::new(0.0, 0.0, 0.0);
        assert!(zero.normalize().is_none());
    }

    #[test]
    fn test_transform_identity() {
        let t = Transform4x4::identity();
        let p = Coord3D::new(1.0, 2.0, 3.0);
        let result = t.apply(&p);
        assert!((result.x - 1.0).abs() < EPSILON);
        assert!((result.y - 2.0).abs() < EPSILON);
        assert!((result.z - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_transform_translation() {
        let t = Transform4x4::translation(1.0, 2.0, 3.0);
        let p = Coord3D::new(0.0, 0.0, 0.0);
        let result = t.apply(&p);
        assert!((result.x - 1.0).abs() < EPSILON);
        assert!((result.y - 2.0).abs() < EPSILON);
        assert!((result.z - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_transform_compose() {
        let t1 = Transform4x4::translation(1.0, 0.0, 0.0);
        let t2 = Transform4x4::translation(0.0, 2.0, 0.0);
        let composed = t1.compose(&t2);
        let p = Coord3D::new(0.0, 0.0, 0.0);
        let result = composed.apply(&p);
        assert!((result.x - 1.0).abs() < EPSILON);
        assert!((result.y - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_coord1d() {
        let a = Coord1D::new(5.0);
        let b = Coord1D::new(3.0);
        assert!((a.distance(&b) - 2.0).abs() < EPSILON);
        assert!((a.translate(-2.0).x() - 3.0).abs() < EPSILON);
        assert!((a.scale(2.0).x() - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_coord_system_enum() {
        assert_eq!(format!("{:?}", CoordSystem::Cartesian3D), "Cartesian3D");
        assert_eq!(format!("{:?}", CoordSystem::Spherical3D), "Spherical3D");
    }
}
