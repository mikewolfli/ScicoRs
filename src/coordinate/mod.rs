//! Unified Coordinate System (All-Scale Universal)
//!
//! Supports multiple coordinate systems (Cartesian, polar, cylindrical,
//! spherical), reference frames, coordinate transformations, and field
//! coordinate binding across scales from nanometer to light-year.

use crate::core::types::Scalar;

/// The number of coordinate dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordDimension {
    /// 0D (point / scalar).
    D0,
    /// 1D (line).
    D1,
    /// 2D (plane).
    D2,
    /// 3D (volume).
    D3,
    /// High-dimensional abstract space.
    HighDim(usize),
}

/// Types of coordinate systems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoordSystem {
    Cartesian,
    Polar,
    Cylindrical,
    Spherical,
    /// Natural / parametric coordinates (e.g., finite element isoparametric).
    Natural,
    /// User-defined custom system.
    Custom(u32),
}

/// A coordinate point in a specified system.
#[derive(Debug, Clone)]
pub struct Coordinate {
    pub system: CoordSystem,
    pub values: Vec<Scalar>,
}

impl Coordinate {
    pub fn new(system: CoordSystem, values: Vec<Scalar>) -> Self {
        Self { system, values }
    }

    pub fn cartesian(x: Scalar, y: Scalar, z: Scalar) -> Self {
        Self { system: CoordSystem::Cartesian, values: vec![x, y, z] }
    }

    pub fn cartesian_2d(x: Scalar, y: Scalar) -> Self {
        Self { system: CoordSystem::Cartesian, values: vec![x, y] }
    }

    pub fn polar(r: Scalar, theta: Scalar) -> Self {
        Self { system: CoordSystem::Polar, values: vec![r, theta] }
    }

    pub fn spherical(r: Scalar, theta: Scalar, phi: Scalar) -> Self {
        Self { system: CoordSystem::Spherical, values: vec![r, theta, phi] }
    }

    pub fn x(&self) -> Scalar { self.values.first().copied().unwrap_or(0.0) }
    pub fn y(&self) -> Scalar { self.values.get(1).copied().unwrap_or(0.0) }
    pub fn z(&self) -> Scalar { self.values.get(2).copied().unwrap_or(0.0) }
}

/// A 3x3 transformation matrix for coordinate operations.
#[derive(Debug, Clone)]
pub struct TransformMatrix {
    pub data: [[Scalar; 4]; 4], // 4x4 homogeneous transformation
}

impl TransformMatrix {
    pub fn identity() -> Self {
        Self {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn translation(dx: Scalar, dy: Scalar, dz: Scalar) -> Self {
        let mut t = Self::identity();
        t.data[0][3] = dx;
        t.data[1][3] = dy;
        t.data[2][3] = dz;
        t
    }

    pub fn rotation_x(angle: Scalar) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, c, -s, 0.0],
                [0.0, s, c, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn rotation_y(angle: Scalar) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            data: [
                [c, 0.0, s, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [-s, 0.0, c, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn rotation_z(angle: Scalar) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            data: [
                [c, -s, 0.0, 0.0],
                [s, c, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn scale(sx: Scalar, sy: Scalar, sz: Scalar) -> Self {
        let mut t = Self::identity();
        t.data[0][0] = sx;
        t.data[1][1] = sy;
        t.data[2][2] = sz;
        t
    }

    /// Apply this transformation to a 3D point.
    pub fn apply(&self, p: &Coordinate) -> Coordinate {
        let x = p.values.first().copied().unwrap_or(0.0);
        let y = p.values.get(1).copied().unwrap_or(0.0);
        let z = p.values.get(2).copied().unwrap_or(0.0);
        let w = 1.0;

        let xf = self.data[0][0] * x + self.data[0][1] * y + self.data[0][2] * z + self.data[0][3] * w;
        let yf = self.data[1][0] * x + self.data[1][1] * y + self.data[1][2] * z + self.data[1][3] * w;
        let zf = self.data[2][0] * x + self.data[2][1] * y + self.data[2][2] * z + self.data[2][3] * w;

        Coordinate::cartesian(xf, yf, zf)
    }

    /// Compose with another transformation: self * other
    pub fn compose(&self, other: &Self) -> Self {
        let mut result = Self::identity();
        for i in 0..4 {
            for j in 0..4 {
                result.data[i][j] = 0.0;
                for k in 0..4 {
                    result.data[i][j] += self.data[i][k] * other.data[k][j];
                }
            }
        }
        result
    }
}

/// Reference frame classification.
#[derive(Debug, Clone, PartialEq)]
pub enum ReferenceFrame {
    /// Inertial (non-accelerating) frame.
    Inertial,
    /// Non-inertial (accelerating) frame.
    NonInertial,
    /// Local part-level frame.
    LocalPart(String),
    /// Local node-level frame.
    LocalNode(String),
}

/// Coordinate transformation utilities.
pub struct CoordinateTransform;

impl CoordinateTransform {
    /// Convert polar to cartesian coordinates.
    pub fn polar_to_cartesian(r: Scalar, theta: Scalar) -> (Scalar, Scalar) {
        (r * theta.cos(), r * theta.sin())
    }

    /// Convert cartesian to polar coordinates.
    pub fn cartesian_to_polar(x: Scalar, y: Scalar) -> (Scalar, Scalar) {
        ( (x * x + y * y).sqrt(), y.atan2(x) )
    }

    /// Convert spherical to cartesian coordinates.
    pub fn spherical_to_cartesian(r: Scalar, theta: Scalar, phi: Scalar) -> (Scalar, Scalar, Scalar) {
        let st = theta.sin();
        (
            r * st * phi.cos(),
            r * st * phi.sin(),
            r * theta.cos(),
        )
    }

    /// Convert cartesian to spherical coordinates.
    pub fn cartesian_to_spherical(x: Scalar, y: Scalar, z: Scalar) -> (Scalar, Scalar, Scalar) {
        let r = (x * x + y * y + z * z).sqrt();
        let theta = if r > 0.0 { (z / r).acos() } else { 0.0 };
        let phi = y.atan2(x);
        (r, theta, phi)
    }
}
