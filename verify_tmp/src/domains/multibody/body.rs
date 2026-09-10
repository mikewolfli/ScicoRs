//! Rigid body dynamics and quaternion rotation.
//!
//! Provides `RigidBody` (6-DOF rigid body with mass, inertia, position,
//! orientation, linear/angular velocity) and `Quaternion` for singularity-free
//! 3D rotation representation.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// A unit quaternion for singularity-free 3D rotation representation.
///
/// Quaternion: `q = w + x·i + y·j + z·k` with `w² + x² + y² + z² = 1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    /// Scalar (real) part.
    pub w: Scalar,
    /// Vector (imaginary) components.
    pub x: Scalar,
    pub y: Scalar,
    pub z: Scalar,
}

impl Quaternion {
    /// Identity quaternion (no rotation).
    pub fn identity() -> Self {
        Self {
            w: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Create a quaternion from an axis-angle representation.
    ///
    /// # Parameters
    /// - `axis`: unit vector axis of rotation (will be normalized).
    /// - `angle`: rotation angle in radians.
    pub fn from_axis_angle(axis: [Scalar; 3], angle: Scalar) -> Self {
        let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if n < 1e-30 {
            return Self::identity();
        }
        let half = angle * 0.5;
        let s = half.sin() / n;
        Self {
            w: half.cos(),
            x: axis[0] * s,
            y: axis[1] * s,
            z: axis[2] * s,
        }
    }

    /// Multiply two quaternions: `self * other` (Hamilton product).
    pub fn multiply(&self, other: &Quaternion) -> Self {
        Self {
            w: self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
            x: self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            y: self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            z: self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
        }
    }

    /// Rotate a 3D vector by this quaternion.
    ///
    /// Uses `v' = q * v * q_conj` where `v` is treated as a pure quaternion.
    pub fn rotate_vector(&self, v: [Scalar; 3]) -> [Scalar; 3] {
        // p = q * (0, v) * q_conj
        // Direct formula:
        // v' = v + 2 * cross(q_vec, cross(q_vec, v) + w * v)
        let qv = [self.x, self.y, self.z];
        let cross1 = [
            qv[1] * v[2] - qv[2] * v[1],
            qv[2] * v[0] - qv[0] * v[2],
            qv[0] * v[1] - qv[1] * v[0],
        ];
        let cross2 = [
            qv[1] * cross1[2] - qv[2] * cross1[1],
            qv[2] * cross1[0] - qv[0] * cross1[2],
            qv[0] * cross1[1] - qv[1] * cross1[0],
        ];
        [
            v[0] + 2.0 * (cross2[0] + self.w * cross1[0]),
            v[1] + 2.0 * (cross2[1] + self.w * cross1[1]),
            v[2] + 2.0 * (cross2[2] + self.w * cross1[2]),
        ]
    }

    /// Conjugate quaternion: `q* = (w, -x, -y, -z)`.
    pub fn conjugate(&self) -> Self {
        Self {
            w: self.w,
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }

    /// Normalize this quaternion to unit length.
    pub fn normalize(&self) -> Self {
        let n = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if n < 1e-30 {
            return Self::identity();
        }
        Self {
            w: self.w / n,
            x: self.x / n,
            y: self.y / n,
            z: self.z / n,
        }
    }

    /// Convert to a 3×3 rotation matrix (row-major).
    pub fn to_rotation_matrix(&self) -> [[Scalar; 3]; 3] {
        let q = self.normalize();
        let (w, x, y, z) = (q.w, q.x, q.y, q.z);
        let xx = x * x;
        let yy = y * y;
        let zz = z * z;
        let xy = x * y;
        let xz = x * z;
        let yz = y * z;
        let wx = w * x;
        let wy = w * y;
        let wz = w * z;
        [
            [1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy)],
            [2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx)],
            [2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy)],
        ]
    }
}

/// A 6-degree-of-freedom rigid body for multibody dynamics.
///
/// Tracks position, orientation (as a quaternion), linear/angular velocity,
/// mass, and inertia tensor.
#[derive(Debug, Clone, PartialEq)]
pub struct RigidBody {
    /// Unique identifier for this body.
    pub id: String,
    /// Mass in kg.
    pub mass: Scalar,
    /// Inertia tensor about the body's COM in the body-local frame (kg·m²).
    pub inertia: [[Scalar; 3]; 3],
    /// World-frame position of the body's COM (m).
    pub position: Coord3D,
    /// Orientation as a unit quaternion (world → body).
    pub orientation: Quaternion,
    /// World-frame linear velocity (m/s).
    pub linear_velocity: [Scalar; 3],
    /// Body-frame angular velocity (rad/s).
    pub angular_velocity: [Scalar; 3],
}

impl RigidBody {
    /// Create a new rigid body.
    ///
    /// Initial orientation is identity (aligned with world frame).  Initial
    /// velocities are zero.
    pub fn new(
        id: &str,
        mass: Scalar,
        inertia: [[Scalar; 3]; 3],
        position: Coord3D,
    ) -> Self {
        Self {
            id: id.to_string(),
            mass,
            inertia,
            position,
            orientation: Quaternion::identity(),
            linear_velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
        }
    }

    /// Kinetic energy: T = ½·m·v·v + ½·ω·I·ω
    ///
    /// The rotational term uses the body-frame angular velocity and the
    /// body-frame inertia tensor.
    pub fn kinetic_energy(&self) -> Scalar {
        let trans = 0.5
            * self.mass
            * (self.linear_velocity[0] * self.linear_velocity[0]
                + self.linear_velocity[1] * self.linear_velocity[1]
                + self.linear_velocity[2] * self.linear_velocity[2]);
        let wx = self.angular_velocity[0];
        let wy = self.angular_velocity[1];
        let wz = self.angular_velocity[2];
        let i = &self.inertia;
        let rot = 0.5
            * (wx * (i[0][0] * wx + i[0][1] * wy + i[0][2] * wz)
                + wy * (i[1][0] * wx + i[1][1] * wy + i[1][2] * wz)
                + wz * (i[2][0] * wx + i[2][1] * wy + i[2][2] * wz));
        trans + rot
    }

    /// Linear momentum: p = m·v
    pub fn linear_momentum(&self) -> [Scalar; 3] {
        [
            self.mass * self.linear_velocity[0],
            self.mass * self.linear_velocity[1],
            self.mass * self.linear_velocity[2],
        ]
    }

    /// Angular momentum (body-frame): L = I·ω
    pub fn angular_momentum(&self) -> [Scalar; 3] {
        let i = &self.inertia;
        let w = &self.angular_velocity;
        [
            i[0][0] * w[0] + i[0][1] * w[1] + i[0][2] * w[2],
            i[1][0] * w[0] + i[1][1] * w[1] + i[1][2] * w[2],
            i[2][0] * w[0] + i[2][1] * w[1] + i[2][2] * w[2],
        ]
    }

    /// Apply a force and torque for a time step `dt` (semi-implicit Euler).
    ///
    /// Updates linear velocity from force and angular velocity from torque:
    /// - `v += F / m · dt`
    /// - `ω += I⁻¹ · τ · dt`
    ///
    /// The orientation is *not* updated here (that is done by the integrator).
    pub fn apply_force_and_torque(
        &mut self,
        force: [Scalar; 3],
        torque: [Scalar; 3],
        dt: Scalar,
    ) {
        let inv_mass = if self.mass > 0.0 {
            1.0 / self.mass
        } else {
            0.0
        };
        self.linear_velocity[0] += force[0] * inv_mass * dt;
        self.linear_velocity[1] += force[1] * inv_mass * dt;
        self.linear_velocity[2] += force[2] * inv_mass * dt;

        // Invert the 3x3 inertia tensor
        let i = &self.inertia;
        let det = i[0][0] * (i[1][1] * i[2][2] - i[1][2] * i[2][1])
            - i[0][1] * (i[1][0] * i[2][2] - i[1][2] * i[2][0])
            + i[0][2] * (i[1][0] * i[2][1] - i[1][1] * i[2][0]);

        if det.abs() > 1e-30 {
            let inv_det = 1.0 / det;
            let inv_i = [
                [
                    (i[1][1] * i[2][2] - i[1][2] * i[2][1]) * inv_det,
                    (i[0][2] * i[2][1] - i[0][1] * i[2][2]) * inv_det,
                    (i[0][1] * i[1][2] - i[0][2] * i[1][1]) * inv_det,
                ],
                [
                    (i[1][2] * i[2][0] - i[1][0] * i[2][2]) * inv_det,
                    (i[0][0] * i[2][2] - i[0][2] * i[2][0]) * inv_det,
                    (i[0][2] * i[1][0] - i[0][0] * i[1][2]) * inv_det,
                ],
                [
                    (i[1][0] * i[2][1] - i[1][1] * i[2][0]) * inv_det,
                    (i[0][1] * i[2][0] - i[0][0] * i[2][1]) * inv_det,
                    (i[0][0] * i[1][1] - i[0][1] * i[1][0]) * inv_det,
                ],
            ];
            self.angular_velocity[0] +=
                (inv_i[0][0] * torque[0] + inv_i[0][1] * torque[1] + inv_i[0][2] * torque[2]) * dt;
            self.angular_velocity[1] +=
                (inv_i[1][0] * torque[0] + inv_i[1][1] * torque[1] + inv_i[1][2] * torque[2]) * dt;
            self.angular_velocity[2] +=
                (inv_i[2][0] * torque[0] + inv_i[2][1] * torque[1] + inv_i[2][2] * torque[2]) * dt;
        }
    }

    /// Compute the inverse of the inertia tensor (3×3) as a flat array.
    pub fn inverse_inertia(&self) -> [[Scalar; 3]; 3] {
        let i = &self.inertia;
        let det = i[0][0] * (i[1][1] * i[2][2] - i[1][2] * i[2][1])
            - i[0][1] * (i[1][0] * i[2][2] - i[1][2] * i[2][0])
            + i[0][2] * (i[1][0] * i[2][1] - i[1][1] * i[2][0]);

        if det.abs() < 1e-30 {
            return [[0.0; 3]; 3];
        }
        let inv_det = 1.0 / det;
        [
            [
                (i[1][1] * i[2][2] - i[1][2] * i[2][1]) * inv_det,
                (i[0][2] * i[2][1] - i[0][1] * i[2][2]) * inv_det,
                (i[0][1] * i[1][2] - i[0][2] * i[1][1]) * inv_det,
            ],
            [
                (i[1][2] * i[2][0] - i[1][0] * i[2][2]) * inv_det,
                (i[0][0] * i[2][2] - i[0][2] * i[2][0]) * inv_det,
                (i[0][2] * i[1][0] - i[0][0] * i[1][2]) * inv_det,
            ],
            [
                (i[1][0] * i[2][1] - i[1][1] * i[2][0]) * inv_det,
                (i[0][1] * i[2][0] - i[0][0] * i[2][1]) * inv_det,
                (i[0][0] * i[1][1] - i[0][1] * i[1][0]) * inv_det,
            ],
        ]
    }
}

/// A flexible body with modal reduction (simplified).
///
/// Stores a reference rigid-body state plus a set of modal coordinates
/// for small elastic deformation.
#[derive(Debug, Clone, PartialEq)]
pub struct FlexibleBody {
    /// Underlying rigid-body state.
    pub rigid: RigidBody,
    /// Modal coordinates: `q_i` for each retained mode.
    pub modal_coords: Vec<Scalar>,
    /// Modal velocities: `dq_i/dt`.
    pub modal_velocities: Vec<Scalar>,
    /// Modal masses (generalized mass for each mode).
    pub modal_masses: Vec<Scalar>,
    /// Modal stiffnesses (generalized stiffness for each mode).
    pub modal_stiffnesses: Vec<Scalar>,
}

impl FlexibleBody {
    /// Create a new flexible body from a rigid body and modal parameters.
    pub fn new(
        rigid: RigidBody,
        n_modes: usize,
        modal_masses: Vec<Scalar>,
        modal_stiffnesses: Vec<Scalar>,
    ) -> Self {
        assert_eq!(
            modal_masses.len(),
            n_modes,
            "modal_masses length must equal n_modes"
        );
        assert_eq!(
            modal_stiffnesses.len(),
            n_modes,
            "modal_stiffnesses length must equal n_modes"
        );
        Self {
            rigid,
            modal_coords: vec![0.0; n_modes],
            modal_velocities: vec![0.0; n_modes],
            modal_masses,
            modal_stiffnesses,
        }
    }

    /// Elastic potential energy: V = ½ Σ k_i · q_i²
    pub fn elastic_energy(&self) -> Scalar {
        let mut energy = 0.0;
        for i in 0..self.modal_coords.len() {
            energy += 0.5 * self.modal_stiffnesses[i] * self.modal_coords[i].powi(2);
        }
        energy
    }

    /// Update modal coordinates with a time step (decoupled spring-mass).
    pub fn integrate_modes(&mut self, dt: Scalar) {
        for i in 0..self.modal_coords.len() {
            let m = self.modal_masses[i];
            let k = self.modal_stiffnesses[i];
            if m > 0.0 {
                let omega2 = k / m;
                // Simple Verlet-like step
                let acc = -omega2 * self.modal_coords[i];
                self.modal_velocities[i] += acc * dt;
                self.modal_coords[i] += self.modal_velocities[i] * dt;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quaternion_identity() {
        let q = Quaternion::identity();
        assert!((q.w - 1.0).abs() < 1e-12);
        assert!((q.x).abs() < 1e-12);
        assert!((q.y).abs() < 1e-12);
        assert!((q.z).abs() < 1e-12);
    }

    #[test]
    fn test_quaternion_axis_angle() {
        let q = Quaternion::from_axis_angle([0.0, 0.0, 1.0], std::f64::consts::PI);
        // 180° about Z: q = (0, 0, 0, 1)
        assert!((q.w).abs() < 1e-12);
        assert!((q.x).abs() < 1e-12);
        assert!((q.y).abs() < 1e-12);
        assert!((q.z - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_quaternion_rotate_vector() {
        // 90° about Z: (1,0,0) → (0,1,0)
        let q = Quaternion::from_axis_angle([0.0, 0.0, 1.0], std::f64::consts::PI / 2.0);
        let v = q.rotate_vector([1.0, 0.0, 0.0]);
        assert!((v[0]).abs() < 1e-10);
        assert!((v[1] - 1.0).abs() < 1e-10);
        assert!((v[2]).abs() < 1e-10);
    }

    #[test]
    fn test_quaternion_conjugate() {
        let q = Quaternion::from_axis_angle([1.0, 2.0, 3.0], 0.5);
        let c = q.conjugate();
        let prod = q.multiply(&c);
        // q * q_conj should give identity
        assert!((prod.w - 1.0).abs() < 1e-10);
        assert!((prod.x).abs() < 1e-10);
        assert!((prod.y).abs() < 1e-10);
        assert!((prod.z).abs() < 1e-10);
    }

    #[test]
    fn test_quaternion_normalize() {
        let q = Quaternion {
            w: 2.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let n = q.normalize();
        assert!((n.w - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_rigid_body_new() {
        let b = RigidBody::new(
            "body1",
            1.0,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Coord3D::new(0.0, 0.0, 0.0),
        );
        assert_eq!(b.id, "body1");
        assert!((b.mass - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_kinetic_energy() {
        let mut b = RigidBody::new(
            "b",
            2.0,
            [[0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 0.5]],
            Coord3D::new(0.0, 0.0, 0.0),
        );
        b.linear_velocity = [3.0, 0.0, 0.0]; // ½*2*9 = 9
        b.angular_velocity = [2.0, 0.0, 0.0]; // ½*0.5*4 = 1
        let ke = b.kinetic_energy();
        assert!((ke - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_linear_momentum() {
        let mut b = RigidBody::new(
            "b",
            3.0,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Coord3D::new(0.0, 0.0, 0.0),
        );
        b.linear_velocity = [2.0, -1.0, 4.0];
        let p = b.linear_momentum();
        assert!((p[0] - 6.0).abs() < 1e-12);
        assert!((p[1] + 3.0).abs() < 1e-12);
        assert!((p[2] - 12.0).abs() < 1e-12);
    }

    #[test]
    fn test_angular_momentum() {
        let mut b = RigidBody::new(
            "b",
            1.0,
            [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]],
            Coord3D::new(0.0, 0.0, 0.0),
        );
        b.angular_velocity = [1.0, 2.0, 3.0];
        let l = b.angular_momentum();
        assert!((l[0] - 2.0).abs() < 1e-12);
        assert!((l[1] - 6.0).abs() < 1e-12);
        assert!((l[2] - 12.0).abs() < 1e-12);
    }

    #[test]
    fn test_apply_force_and_torque() {
        let mut b = RigidBody::new(
            "b",
            2.0,
            [[4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 4.0]],
            Coord3D::new(0.0, 0.0, 0.0),
        );
        b.apply_force_and_torque([10.0, 0.0, 0.0], [0.0, 8.0, 0.0], 0.1);
        // v = 10/2 * 0.1 = 0.5
        assert!((b.linear_velocity[0] - 0.5).abs() < 1e-12);
        // ωy = 8/4 * 0.1 = 0.2
        assert!((b.angular_velocity[1] - 0.2).abs() < 1e-12);
    }

    #[test]
    fn test_quaternion_to_rotation_matrix() {
        let q = Quaternion::identity();
        let r = q.to_rotation_matrix();
        assert!((r[0][0] - 1.0).abs() < 1e-12);
        assert!((r[1][1] - 1.0).abs() < 1e-12);
        assert!((r[2][2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_flexible_body_elastic_energy() {
        let rigid = RigidBody::new(
            "flex",
            1.0,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Coord3D::new(0.0, 0.0, 0.0),
        );
        let mut flex = FlexibleBody::new(rigid, 2, vec![1.0, 1.0], vec![100.0, 200.0]);
        flex.modal_coords = vec![0.1, 0.05];
        let e = flex.elastic_energy();
        // ½*100*0.01 + ½*200*0.0025 = 0.5 + 0.25 = 0.75
        assert!((e - 0.75).abs() < 1e-12);
    }

    #[test]
    fn test_flexible_body_integrate_modes() {
        let rigid = RigidBody::new(
            "flex",
            1.0,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Coord3D::new(0.0, 0.0, 0.0),
        );
        let mut flex = FlexibleBody::new(rigid, 1, vec![1.0], vec![0.0]); // zero stiffness => free
        flex.modal_coords = vec![0.0];
        flex.modal_velocities = vec![1.0];
        flex.integrate_modes(0.5);
        assert!((flex.modal_coords[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_quaternion_rotate_vector_no_change() {
        // Identity quaternion should not change the vector
        let q = Quaternion::identity();
        let v = q.rotate_vector([3.0, -2.0, 5.0]);
        assert!((v[0] - 3.0).abs() < 1e-12);
        assert!((v[1] + 2.0).abs() < 1e-12);
        assert!((v[2] - 5.0).abs() < 1e-12);
    }
}
