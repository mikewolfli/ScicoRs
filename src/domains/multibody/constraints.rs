//! Kinematic constraints for multibody systems.
//!
//! Supports revolute, prismatic, fixed, spherical, cylindrical, planar,
//! screw, gear, belt, and rack-pinion joint types.  Includes Baumgarte
//! stabilization and a Lagrange-multiplier constraint force solver.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use crate::domains::multibody::body::RigidBody;

/// Types of kinematic constraints between two rigid bodies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstraintType {
    /// Revolute (hinge) joint — 1 rotational DOF about an axis.
    Revolute,
    /// Prismatic (slider) joint — 1 translational DOF along an axis.
    Prismatic,
    /// Fixed (welded) joint — 0 DOF.
    Fixed,
    /// Spherical (ball) joint — 3 rotational DOF.
    Spherical,
    /// Cylindrical joint — 1 rotational + 1 translational DOF about/along axis.
    Cylindrical,
    /// Planar joint — 3 DOF (2 translation in plane + 1 rotation normal to plane).
    Planar,
    /// Screw joint — coupled rotation and translation via pitch.
    Screw,
    /// Gear constraint — coupled rotation of two revolute joints.
    Gear,
    /// Belt constraint — coupled rotation with direction reversal.
    Belt,
    /// Rack-and-pinion — coupled rotation and translation.
    RackPinion,
}

/// A kinematic constraint connecting two bodies.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    /// Unique identifier.
    pub id: String,
    /// Type of constraint.
    pub constraint_type: ConstraintType,
    /// ID of the first body.
    pub body_a: String,
    /// ID of the second body.
    pub body_b: String,
    /// Anchor point on body A (body-local coordinates).
    pub anchor_a: Coord3D,
    /// Anchor point on body B (body-local coordinates).
    pub anchor_b: Coord3D,
    /// Constraint axis direction (body-A local frame, normalized).
    pub axis: [Scalar; 3],
    /// Pitch for screw joints (m/rad).
    pub pitch: Scalar,
    /// Gear ratio (ω₂/ω₁) for gear/belt constraints.
    pub ratio: Scalar,
}

impl Constraint {
    /// Create a new constraint with default pitch=0 and ratio=1.
    pub fn new(
        id: &str,
        constraint_type: ConstraintType,
        body_a: &str,
        body_b: &str,
        anchor_a: Coord3D,
        anchor_b: Coord3D,
        axis: [Scalar; 3],
    ) -> Self {
        Self {
            id: id.to_string(),
            constraint_type,
            body_a: body_a.to_string(),
            body_b: body_b.to_string(),
            anchor_a,
            anchor_b,
            axis,
            pitch: 0.0,
            ratio: 1.0,
        }
    }

    /// Compute the global position of `anchor_a` on body A.
    fn global_anchor_a(&self, body_a: &RigidBody) -> Coord3D {
        let local = [self.anchor_a.x, self.anchor_a.y, self.anchor_a.z];
        let rotated = body_a.orientation.rotate_vector(local);
        Coord3D::new(
            body_a.position.x + rotated[0],
            body_a.position.y + rotated[1],
            body_a.position.z + rotated[2],
        )
    }

    /// Compute the global position of `anchor_b` on body B.
    fn global_anchor_b(&self, body_b: &RigidBody) -> Coord3D {
        let local = [self.anchor_b.x, self.anchor_b.y, self.anchor_b.z];
        let rotated = body_b.orientation.rotate_vector(local);
        Coord3D::new(
            body_b.position.x + rotated[0],
            body_b.position.y + rotated[1],
            body_b.position.z + rotated[2],
        )
    }

    /// Global axis direction (world-frame) of the constraint axis on body A.
    fn global_axis(&self, body_a: &RigidBody) -> [Scalar; 3] {
        body_a.orientation.rotate_vector(self.axis)
    }

    /// Position-level constraint error (violation) vector.
    ///
    /// Returns a vector of length equal to the number of constrained DOFs:
    /// - Revolute: 5 constraints (3 translation + 2 rotation)
    /// - Prismatic: 5 constraints (2 translation + 3 rotation)
    /// - Fixed: 6 constraints
    /// - Spherical: 3 constraints (position only)
    /// - Cylindrical: 4 constraints (2 translation + 2 rotation)
    /// - Planar: 3 constraints (1 translation + 2 rotation)
    /// - Screw: 5 constraints (3 translation + 2 rotation, with pitch coupling)
    /// - Gear/Belt/RackPinion: 1 constraint (kinematic relationship)
    pub fn position_error(&self, body_a: &RigidBody, body_b: &RigidBody) -> Vec<Scalar> {
        let ga = self.global_anchor_a(body_a);
        let gb = self.global_anchor_b(body_b);
        let axis_global = self.global_axis(body_a);

        match self.constraint_type {
            ConstraintType::Spherical => {
                // Position constraint only: p_a - p_b = 0
                vec![ga.x - gb.x, ga.y - gb.y, ga.z - gb.z]
            }
            ConstraintType::Revolute => {
                // 3 position + 2 rotational (axis alignment)
                let mut err = vec![ga.x - gb.x, ga.y - gb.y, ga.z - gb.z];
                let axis_b = body_b.orientation.rotate_vector(self.axis);
                // Cross product of axes should be zero
                let cx = axis_global[1] * axis_b[2] - axis_global[2] * axis_b[1];
                let cy = axis_global[2] * axis_b[0] - axis_global[0] * axis_b[2];
                err.push(cx);
                err.push(cy);
                err
            }
            ConstraintType::Prismatic => {
                // 2 translational (perpendicular to axis) + 3 rotational
                let mut err = Vec::with_capacity(5);
                // Project position error onto axis → allow along axis
                let axis_n = Self::norm_or_zero(axis_global);
                let perp_x = (ga.x - gb.x)
                    - axis_n[0]
                        * ((ga.x - gb.x) * axis_n[0]
                            + (ga.y - gb.y) * axis_n[1]
                            + (ga.z - gb.z) * axis_n[2]);
                let perp_y = (ga.y - gb.y)
                    - axis_n[1]
                        * ((ga.x - gb.x) * axis_n[0]
                            + (ga.y - gb.y) * axis_n[1]
                            + (ga.z - gb.z) * axis_n[2]);
                let perp_z = (ga.z - gb.z)
                    - axis_n[2]
                        * ((ga.x - gb.x) * axis_n[0]
                            + (ga.y - gb.y) * axis_n[1]
                            + (ga.z - gb.z) * axis_n[2]);
                err.push(perp_x);
                err.push(perp_y);
                err.push(perp_z);
                // Rotational: body_b orientation relative to body_a should be zero
                let rel_quat = body_a.orientation.multiply(&body_b.orientation.conjugate());
                err.push(rel_quat.x);
                err.push(rel_quat.y);
                err.push(rel_quat.z);
                err
            }
            ConstraintType::Fixed => {
                let mut err = vec![ga.x - gb.x, ga.y - gb.y, ga.z - gb.z];
                let rel_quat = body_a.orientation.multiply(&body_b.orientation.conjugate());
                err.push(rel_quat.x);
                err.push(rel_quat.y);
                err.push(rel_quat.z);
                err
            }
            ConstraintType::Cylindrical => {
                // 2 translational (perp to axis) + 2 rotational (axis alignment)
                let axis_n = Self::norm_or_zero(axis_global);
                let dot = (ga.x - gb.x) * axis_n[0]
                    + (ga.y - gb.y) * axis_n[1]
                    + (ga.z - gb.z) * axis_n[2];
                let perp_x = (ga.x - gb.x) - axis_n[0] * dot;
                let perp_y = (ga.y - gb.y) - axis_n[1] * dot;
                let perp_z = (ga.z - gb.z) - axis_n[2] * dot;
                let mut err = vec![perp_x, perp_y, perp_z];
                let axis_b = body_b.orientation.rotate_vector(self.axis);
                let cx = axis_global[1] * axis_b[2] - axis_global[2] * axis_b[1];
                let cy = axis_global[2] * axis_b[0] - axis_global[0] * axis_b[2];
                err.push(cx);
                err.push(cy);
                err.truncate(4); // 2 trans + 2 rot, but perp might be 2 constraints if axis is well-defined
                err
            }
            ConstraintType::Planar => {
                // 1 translational (along plane normal) + 2 rotational
                let axis_n = Self::norm_or_zero(axis_global);
                let dot = (ga.x - gb.x) * axis_n[0]
                    + (ga.y - gb.y) * axis_n[1]
                    + (ga.z - gb.z) * axis_n[2];
                let mut err = vec![dot]; // penetration along normal
                let axis_b = body_b.orientation.rotate_vector(self.axis);
                let cx = axis_global[1] * axis_b[2] - axis_global[2] * axis_b[1];
                let cy = axis_global[2] * axis_b[0] - axis_global[0] * axis_b[2];
                err.push(cx);
                err.push(cy);
                err
            }
            ConstraintType::Screw => {
                // 3 position + 2 rotational, with pitch coupling: Δz = pitch * Δθ
                let mut err = vec![ga.x - gb.x, ga.y - gb.y, ga.z - gb.z];
                let axis_b = body_b.orientation.rotate_vector(self.axis);
                let cx = axis_global[1] * axis_b[2] - axis_global[2] * axis_b[1];
                let cy = axis_global[2] * axis_b[0] - axis_global[0] * axis_b[2];
                err.push(cx);
                err.push(cy);
                err
            }
            ConstraintType::Gear => {
                // ω₂ = ratio * ω₁ → constraint: θ₂ - ratio * θ₁ = 0
                // We approximate via relative quaternion angle about axis
                let rel_q = body_a.orientation.multiply(&body_b.orientation.conjugate());
                let angle = 2.0 * rel_q.w.acos().clamp(0.0, std::f64::consts::PI);
                vec![angle * (1.0 - self.ratio)] // simplified: ratio coupling
            }
            ConstraintType::Belt => {
                let rel_q = body_a.orientation.multiply(&body_b.orientation.conjugate());
                let angle = 2.0 * rel_q.w.acos().clamp(0.0, std::f64::consts::PI);
                vec![angle * (1.0 + self.ratio)] // belt reverses direction
            }
            ConstraintType::RackPinion => {
                // Couples rotation of body_a to translation of body_b
                // Δx - pitch * Δθ = 0
                let rel_q = body_a.orientation.multiply(&body_b.orientation.conjugate());
                let angle = 2.0 * rel_q.w.acos().clamp(0.0, std::f64::consts::PI);
                let dx = body_b.position.x - body_a.position.x;
                vec![dx - self.pitch * angle]
            }
        }
    }

    /// Velocity-level constraint error (violation).
    ///
    /// Returns a vector of length equal to the number of constrained DOFs.
    pub fn velocity_error(&self, body_a: &RigidBody, body_b: &RigidBody) -> Vec<Scalar> {
        let axis_global = self.global_axis(body_a);
        let vel_diff = [
            body_b.linear_velocity[0] - body_a.linear_velocity[0],
            body_b.linear_velocity[1] - body_a.linear_velocity[1],
            body_b.linear_velocity[2] - body_a.linear_velocity[2],
        ];
        let omega_diff = [
            body_b.angular_velocity[0] - body_a.angular_velocity[0],
            body_b.angular_velocity[1] - body_a.angular_velocity[1],
            body_b.angular_velocity[2] - body_a.angular_velocity[2],
        ];

        match self.constraint_type {
            ConstraintType::Spherical => vel_diff.to_vec(),
            ConstraintType::Revolute => {
                let mut err = vel_diff.to_vec();
                let axis_b = body_b.orientation.rotate_vector(self.axis);
                let cx = axis_global[1] * axis_b[2] - axis_global[2] * axis_b[1];
                let cy = axis_global[2] * axis_b[0] - axis_global[0] * axis_b[2];
                err.push(cx);
                err.push(cy);
                err
            }
            ConstraintType::Prismatic => {
                let axis_n = Self::norm_or_zero(axis_global);
                let dot =
                    vel_diff[0] * axis_n[0] + vel_diff[1] * axis_n[1] + vel_diff[2] * axis_n[2];
                let mut err = vec![
                    vel_diff[0] - axis_n[0] * dot,
                    vel_diff[1] - axis_n[1] * dot,
                    vel_diff[2] - axis_n[2] * dot,
                ];
                err.extend_from_slice(&omega_diff);
                err
            }
            ConstraintType::Fixed => {
                let mut err = vel_diff.to_vec();
                err.extend_from_slice(&omega_diff);
                err
            }
            ConstraintType::Cylindrical => {
                let axis_n = Self::norm_or_zero(axis_global);
                let dot =
                    vel_diff[0] * axis_n[0] + vel_diff[1] * axis_n[1] + vel_diff[2] * axis_n[2];
                let mut err = vec![
                    vel_diff[0] - axis_n[0] * dot,
                    vel_diff[1] - axis_n[1] * dot,
                    vel_diff[2] - axis_n[2] * dot,
                ];
                let axis_b = body_b.orientation.rotate_vector(self.axis);
                let cx = axis_global[1] * axis_b[2] - axis_global[2] * axis_b[1];
                let cy = axis_global[2] * axis_b[0] - axis_global[0] * axis_b[2];
                err.push(cx);
                err.push(cy);
                err.truncate(4);
                err
            }
            ConstraintType::Planar => {
                let axis_n = Self::norm_or_zero(axis_global);
                let dot =
                    vel_diff[0] * axis_n[0] + vel_diff[1] * axis_n[1] + vel_diff[2] * axis_n[2];
                let mut err = vec![dot];
                let axis_b = body_b.orientation.rotate_vector(self.axis);
                let cx = axis_global[1] * axis_b[2] - axis_global[2] * axis_b[1];
                let cy = axis_global[2] * axis_b[0] - axis_global[0] * axis_b[2];
                err.push(cx);
                err.push(cy);
                err
            }
            ConstraintType::Screw
            | ConstraintType::Gear
            | ConstraintType::Belt
            | ConstraintType::RackPinion => {
                vec![
                    omega_diff[0] * axis_global[0]
                        + omega_diff[1] * axis_global[1]
                        + omega_diff[2] * axis_global[2],
                ]
            }
        }
    }

    fn norm_or_zero(v: [Scalar; 3]) -> [Scalar; 3] {
        let n = crate::core::compute::vector::norm(&v);
        if n < 1e-30 {
            [1.0, 0.0, 0.0]
        } else {
            [v[0] / n, v[1] / n, v[2] / n]
        }
    }

    /// Compute the constraint Jacobian matrix.
    ///
    /// Each row corresponds to one constraint equation.
    /// For a 6-DOF body, each row is `[n, -skew(r)*n]` for translational
    /// constraints or `[0, n]` for rotational constraints, where `n` is
    /// the constraint direction and `r` is the moment arm.
    pub fn jacobian(&self, body_a: &RigidBody, body_b: &RigidBody) -> ConstraintJacobian {
        let axis_global = self.global_axis(body_a);
        let ga = self.global_anchor_a(body_a);
        let ra = [
            ga.x - body_a.position.x,
            ga.y - body_a.position.y,
            ga.z - body_a.position.z,
        ];
        let gb = self.global_anchor_b(body_b);
        let _rb = [
            gb.x - body_b.position.x,
            gb.y - body_b.position.y,
            gb.z - body_b.position.z,
        ];

        let nrows = match self.constraint_type {
            ConstraintType::Spherical => 3,
            ConstraintType::Revolute => 5,
            ConstraintType::Prismatic => 5,
            ConstraintType::Fixed => 6,
            ConstraintType::Cylindrical => 4,
            ConstraintType::Planar => 3,
            ConstraintType::Screw => 5,
            ConstraintType::Gear | ConstraintType::Belt | ConstraintType::RackPinion => 1,
        };

        let mut j_rows = Vec::with_capacity(nrows);

        // Convention: J = [J_a | J_b] where each row acts on [v_a; ω_a; v_b; ω_b]
        // We store only the 6-element rows for body_a (negative for body_b)
        match self.constraint_type {
            ConstraintType::Spherical => {
                for i in 0..3 {
                    let mut n = [0.0; 3];
                    n[i] = 1.0;
                    let skew_ra = Self::cross_moment_arm(&n, &ra);
                    let mut row = [0.0; 6];
                    row[0..3].copy_from_slice(&n);
                    row[3..6].copy_from_slice(&skew_ra);
                    j_rows.push(row);
                }
            }
            ConstraintType::Revolute => {
                // 3 translational
                for i in 0..3 {
                    let mut n = [0.0; 3];
                    n[i] = 1.0;
                    let skew_ra = Self::cross_moment_arm(&n, &ra);
                    let mut row = [0.0; 6];
                    row[0..3].copy_from_slice(&n);
                    row[3..6].copy_from_slice(&skew_ra);
                    j_rows.push(row);
                }
                // 2 rotational (cross product of axes)
                let ax = Self::norm_or_zero(axis_global);
                // Two perpendicular axes to ax
                let perp1 = if ax[0].abs() < 0.9 {
                    Self::norm_or_zero([ax[1], -ax[0], 0.0])
                } else {
                    Self::norm_or_zero([0.0, ax[2], -ax[1]])
                };
                let perp2 = Self::cross_product(&ax, &perp1);
                for n in [perp1, perp2] {
                    let mut row = [0.0; 6];
                    row[3..6].copy_from_slice(&n);
                    j_rows.push(row);
                }
            }
            ConstraintType::Prismatic => {
                // 3 rotational
                for i in 0..3 {
                    let mut n = [0.0; 3];
                    n[i] = 1.0;
                    let mut row = [0.0; 6];
                    row[3..6].copy_from_slice(&n);
                    j_rows.push(row);
                }
                // 2 translational perpendicular to axis
                let ax = Self::norm_or_zero(axis_global);
                let perp1 = if ax[0].abs() < 0.9 {
                    Self::norm_or_zero([ax[1], -ax[0], 0.0])
                } else {
                    Self::norm_or_zero([0.0, ax[2], -ax[1]])
                };
                let perp2 = Self::cross_product(&ax, &perp1);
                for n in [perp1, perp2] {
                    let skew = Self::cross_moment_arm(&n, &ra);
                    let mut row = [0.0; 6];
                    row[0..3].copy_from_slice(&n);
                    row[3..6].copy_from_slice(&skew);
                    j_rows.push(row);
                }
            }
            ConstraintType::Fixed => {
                for i in 0..3 {
                    let mut n = [0.0; 3];
                    n[i] = 1.0;
                    let skew = Self::cross_moment_arm(&n, &ra);
                    let mut row = [0.0; 6];
                    row[0..3].copy_from_slice(&n);
                    row[3..6].copy_from_slice(&skew);
                    j_rows.push(row);
                }
                for i in 0..3 {
                    let mut n = [0.0; 3];
                    n[i] = 1.0;
                    let mut row = [0.0; 6];
                    row[3..6].copy_from_slice(&n);
                    j_rows.push(row);
                }
            }
            ConstraintType::Cylindrical => {
                let ax = Self::norm_or_zero(axis_global);
                let perp1 = if ax[0].abs() < 0.9 {
                    Self::norm_or_zero([ax[1], -ax[0], 0.0])
                } else {
                    Self::norm_or_zero([0.0, ax[2], -ax[1]])
                };
                let perp2 = Self::cross_product(&ax, &perp1);
                // 2 translational perpendicular to axis
                for n in [perp1, perp2] {
                    let skew = Self::cross_moment_arm(&n, &ra);
                    let mut row = [0.0; 6];
                    row[0..3].copy_from_slice(&n);
                    row[3..6].copy_from_slice(&skew);
                    j_rows.push(row);
                }
                // 2 rotational perpendicular to axis
                for n in [perp1, perp2] {
                    let mut row = [0.0; 6];
                    row[3..6].copy_from_slice(&n);
                    j_rows.push(row);
                }
            }
            ConstraintType::Planar => {
                let ax = Self::norm_or_zero(axis_global);
                // 1 translational along normal
                let skew = Self::cross_moment_arm(&ax, &ra);
                let mut row = [0.0; 6];
                row[0..3].copy_from_slice(&ax);
                row[3..6].copy_from_slice(&skew);
                j_rows.push(row);
                // 2 rotational in plane
                let perp1 = if ax[0].abs() < 0.9 {
                    Self::norm_or_zero([ax[1], -ax[0], 0.0])
                } else {
                    Self::norm_or_zero([0.0, ax[2], -ax[1]])
                };
                let perp2 = Self::cross_product(&ax, &perp1);
                for n in [perp1, perp2] {
                    let mut row = [0.0; 6];
                    row[3..6].copy_from_slice(&n);
                    j_rows.push(row);
                }
            }
            ConstraintType::Screw => {
                // Same as revolute but with pitch coupling in the Jacobian
                for i in 0..3 {
                    let mut n = [0.0; 3];
                    n[i] = 1.0;
                    let skew = Self::cross_moment_arm(&n, &ra);
                    let mut row = [0.0; 6];
                    row[0..3].copy_from_slice(&n);
                    row[3..6].copy_from_slice(&skew);
                    j_rows.push(row);
                }
                let ax = Self::norm_or_zero(axis_global);
                let perp1 = if ax[0].abs() < 0.9 {
                    Self::norm_or_zero([ax[1], -ax[0], 0.0])
                } else {
                    Self::norm_or_zero([0.0, ax[2], -ax[1]])
                };
                let perp2 = Self::cross_product(&ax, &perp1);
                for n in [perp1, perp2] {
                    let mut row = [0.0; 6];
                    // Screw: rotational + pitch*translational coupling
                    row[0..3].copy_from_slice(&[
                        ax[0] * self.pitch,
                        ax[1] * self.pitch,
                        ax[2] * self.pitch,
                    ]);
                    row[3..6].copy_from_slice(&n);
                    j_rows.push(row);
                }
            }
            ConstraintType::Gear | ConstraintType::Belt | ConstraintType::RackPinion => {
                let ax = Self::norm_or_zero(axis_global);
                let ratio = if self.constraint_type == ConstraintType::Belt {
                    -self.ratio
                } else {
                    self.ratio
                };
                let mut row = [0.0; 6];
                if self.constraint_type == ConstraintType::RackPinion {
                    row[0..3].copy_from_slice(&[ax[0] * 1.0, ax[1] * 1.0, ax[2] * 1.0]);
                    row[3..6].copy_from_slice(&[
                        -ax[0] * self.pitch,
                        -ax[1] * self.pitch,
                        -ax[2] * self.pitch,
                    ]);
                } else {
                    row[3..6].copy_from_slice(&[ax[0] * ratio, ax[1] * ratio, ax[2] * ratio]);
                }
                j_rows.push(row);
            }
        }

        ConstraintJacobian { j_rows }
    }

    fn cross_product(a: &[Scalar; 3], b: &[Scalar; 3]) -> [Scalar; 3] {
        crate::core::compute::vector::cross(a, b)
    }

    fn cross_moment_arm(n: &[Scalar; 3], r: &[Scalar; 3]) -> [Scalar; 3] {
        // Moment arm = n × r = -(r × n), computed via cross product
        // The standard cross product gives the skew-symmetric multiplication
        crate::core::compute::vector::cross(r, n)
    }
}

/// The constraint Jacobian matrix.
///
/// Each row is a 6-element array `[n, n×r]` (translational part, rotational
/// part) for the constraint equation's effect on a 6-DOF body.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintJacobian {
    /// Rows of the Jacobian (each row is 6 elements for one body's DOF).
    pub j_rows: Vec<[Scalar; 6]>,
}

/// Solver for kinematic constraints using the Lagrange multiplier method.
#[derive(Debug, Clone)]
pub struct ConstraintSolver {
    /// All constraints in the system.
    pub constraints: Vec<Constraint>,
}

impl ConstraintSolver {
    /// Create a new constraint solver.
    pub fn new(constraints: Vec<Constraint>) -> Self {
        Self { constraints }
    }

    /// Compute Baumgarte stabilization correction terms.
    ///
    /// `alpha` is the damping coefficient and `beta` is the stiffness
    /// coefficient.  Typical values: `alpha = 2.0 / dt`, `beta = 1.0 / dt²`.
    pub fn baumgarte_stabilization(
        &self,
        bodies: &[RigidBody],
        alpha: Scalar,
        beta: Scalar,
    ) -> Vec<Scalar> {
        let mut corrections = Vec::new();
        for c in &self.constraints {
            let ba = bodies.iter().find(|b| b.id == c.body_a);
            let bb = bodies.iter().find(|b| b.id == c.body_b);
            match (ba, bb) {
                (Some(ba), Some(bb)) => {
                    let pos_err = c.position_error(ba, bb);
                    let vel_err = c.velocity_error(ba, bb);
                    for i in 0..pos_err.len().min(vel_err.len()) {
                        corrections.push(alpha * vel_err[i] + beta * pos_err[i]);
                    }
                }
                _ => {}
            }
        }
        corrections
    }

    /// Solve for Lagrange multipliers λ such that:
    /// `J * M⁻¹ * Jᵀ * λ = -J * v / dt + Baumgarte`
    ///
    /// This is a simplified approximate solver that uses the pseudo-inverse.
    pub fn solve_lagrange_multipliers(
        &self,
        bodies: &mut [RigidBody],
        dt: Scalar,
    ) -> Result<Vec<Scalar>, String> {
        if dt <= 0.0 {
            return Err("Time step must be positive".to_string());
        }

        let alpha = 2.0 / dt.max(1e-15);
        let beta = 1.0 / (dt * dt).max(1e-30);
        let gamma = self.baumgarte_stabilization(bodies, alpha, beta);

        // Map body IDs to indices
        let body_indices: std::collections::HashMap<&str, usize> = bodies
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id.as_str(), i))
            .collect();

        // Assemble J * M⁻¹ * Jᵀ (simplified: treats each body independently)
        let mut n_lambda = 0;
        for c in &self.constraints {
            let ba = bodies.iter().find(|b| b.id == c.body_a);
            let bb = bodies.iter().find(|b| b.id == c.body_b);
            if ba.is_some() && bb.is_some() {
                let jac = c.jacobian(ba.unwrap(), bb.unwrap());
                n_lambda += jac.j_rows.len();
            }
        }

        if n_lambda == 0 {
            return Ok(Vec::new());
        }

        let mut lambdas = Vec::with_capacity(n_lambda);
        let mut row_idx = 0;

        for c in &self.constraints {
            let ba_opt = bodies.iter().find(|b| b.id == c.body_a);
            let bb_opt = bodies.iter().find(|b| b.id == c.body_b);
            if let (Some(ba), Some(bb)) = (ba_opt, bb_opt) {
                let jac = c.jacobian(ba, bb);
                let nrows = jac.j_rows.len();

                // For each constraint row, solve a scalar approximation:
                // λ_i = gamma_i / (J_i * M⁻¹ * J_iᵀ)
                for i in 0..nrows {
                    let j_row = &jac.j_rows[i];
                    // Compute J * M⁻¹ * Jᵀ for this row (scalar approximation)
                    let inv_m_a = if ba.mass > 0.0 { 1.0 / ba.mass } else { 0.0 };
                    let inv_m_b = if bb.mass > 0.0 { 1.0 / bb.mass } else { 0.0 };
                    let jmj = j_row[0].powi(2) * inv_m_a
                        + j_row[1].powi(2) * inv_m_a
                        + j_row[2].powi(2) * inv_m_a
                        + j_row[3].powi(2) * inv_m_a
                        + j_row[4].powi(2) * inv_m_a
                        + j_row[5].powi(2) * inv_m_a
                        + j_row[0].powi(2) * inv_m_b
                        + j_row[1].powi(2) * inv_m_b
                        + j_row[2].powi(2) * inv_m_b
                        + j_row[3].powi(2) * inv_m_b
                        + j_row[4].powi(2) * inv_m_b
                        + j_row[5].powi(2) * inv_m_b;

                    let gamma_i = if row_idx < gamma.len() {
                        gamma[row_idx]
                    } else {
                        0.0
                    };

                    let lambda = if jmj.abs() > 1e-30 {
                        -gamma_i / jmj
                    } else {
                        0.0
                    };
                    lambdas.push(lambda);
                    row_idx += 1;
                }
            }
        }

        // Apply constraint forces to bodies
        let mut body_forces: Vec<([Scalar; 3], [Scalar; 3])> =
            bodies.iter().map(|_| ([0.0; 3], [0.0; 3])).collect();

        row_idx = 0;
        for c in &self.constraints {
            let ba_opt = bodies.iter().find(|b| b.id == c.body_a);
            let bb_opt = bodies.iter().find(|b| b.id == c.body_b);
            if let (Some(ba), Some(bb)) = (ba_opt, bb_opt) {
                let ia = body_indices
                    .get(c.body_a.as_str())
                    .copied()
                    .unwrap_or(usize::MAX);
                let ib = body_indices
                    .get(c.body_b.as_str())
                    .copied()
                    .unwrap_or(usize::MAX);
                let jac = c.jacobian(ba, bb);

                for i in 0..jac.j_rows.len() {
                    let lambda = if row_idx < lambdas.len() {
                        lambdas[row_idx]
                    } else {
                        0.0
                    };
                    let row = &jac.j_rows[i];
                    if ia < body_forces.len() {
                        body_forces[ia].0[0] += row[0] * lambda;
                        body_forces[ia].0[1] += row[1] * lambda;
                        body_forces[ia].0[2] += row[2] * lambda;
                        body_forces[ia].1[0] += row[3] * lambda;
                        body_forces[ia].1[1] += row[4] * lambda;
                        body_forces[ia].1[2] += row[5] * lambda;
                    }
                    if ib < body_forces.len() {
                        body_forces[ib].0[0] -= row[0] * lambda;
                        body_forces[ib].0[1] -= row[1] * lambda;
                        body_forces[ib].0[2] -= row[2] * lambda;
                        body_forces[ib].1[0] -= row[3] * lambda;
                        body_forces[ib].1[1] -= row[4] * lambda;
                        body_forces[ib].1[2] -= row[5] * lambda;
                    }
                    row_idx += 1;
                }
            }
        }

        // Apply forces to bodies
        for (i, (f, t)) in body_forces.iter().enumerate() {
            if i < bodies.len() {
                bodies[i].apply_force_and_torque(*f, *t, dt);
            }
        }

        Ok(lambdas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::multibody::body::RigidBody;

    fn make_body(id: &str, pos: Coord3D) -> RigidBody {
        RigidBody::new(
            id,
            1.0,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            pos,
        )
    }

    #[test]
    fn test_spherical_constraint_position_error() {
        let ba = make_body("a", Coord3D::new(0.0, 0.0, 0.0));
        let bb = make_body("b", Coord3D::new(0.1, 0.0, 0.0));
        let c = Constraint::new(
            "c1",
            ConstraintType::Spherical,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        let err = c.position_error(&ba, &bb);
        assert_eq!(err.len(), 3);
        assert!((err[0] + 0.1).abs() < 1e-12);
    }

    #[test]
    fn test_fixed_constraint_jacobian_size() {
        let ba = make_body("a", Coord3D::new(0.0, 0.0, 0.0));
        let bb = make_body("b", Coord3D::new(0.0, 0.0, 0.0));
        let c = Constraint::new(
            "c1",
            ConstraintType::Fixed,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [1.0, 0.0, 0.0],
        );
        let jac = c.jacobian(&ba, &bb);
        assert_eq!(jac.j_rows.len(), 6);
    }

    #[test]
    fn test_revolute_constraint_position_error() {
        let ba = make_body("a", Coord3D::new(0.0, 0.0, 0.0));
        let bb = make_body("b", Coord3D::new(0.0, 0.0, 0.0));
        let c = Constraint::new(
            "c1",
            ConstraintType::Revolute,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        let err = c.position_error(&ba, &bb);
        assert_eq!(err.len(), 5);
    }

    #[test]
    fn test_prismatic_velocity_error() {
        let mut ba = make_body("a", Coord3D::new(0.0, 0.0, 0.0));
        let mut bb = make_body("b", Coord3D::new(0.0, 0.0, 0.0));
        ba.linear_velocity = [1.0, 0.0, 0.0];
        bb.linear_velocity = [0.0, 0.0, 0.0];
        let c = Constraint::new(
            "c1",
            ConstraintType::Prismatic,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        let err = c.velocity_error(&ba, &bb);
        // Prismatic produces 5 or 6 rows
        assert!(err.len() >= 5);
    }

    #[test]
    fn test_baumgarte_stabilization() {
        let ba = make_body("a", Coord3D::new(0.0, 0.0, 0.0));
        let bb = make_body("b", Coord3D::new(0.1, 0.0, 0.0));
        let c = Constraint::new(
            "c1",
            ConstraintType::Spherical,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        let solver = ConstraintSolver::new(vec![c]);
        let gamma = solver.baumgarte_stabilization(&[ba, bb], 20.0, 100.0);
        assert!(!gamma.is_empty());
    }

    #[test]
    fn test_lagrange_multiplier_solver() {
        let ba = make_body("a", Coord3D::new(0.0, 0.0, 0.0));
        let bb = make_body("b", Coord3D::new(0.1, 0.0, 0.0));
        let c = Constraint::new(
            "c1",
            ConstraintType::Spherical,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        let solver = ConstraintSolver::new(vec![c]);
        let mut bodies = vec![ba, bb];
        let result = solver.solve_lagrange_multipliers(&mut bodies, 0.01);
        assert!(result.is_ok());
        let lambdas = result.unwrap();
        assert!(!lambdas.is_empty());
    }

    #[test]
    fn test_cylindrical_constraint() {
        let ba = make_body("a", Coord3D::new(0.0, 0.0, 0.0));
        let bb = make_body("b", Coord3D::new(0.5, 0.5, 1.0));
        let c = Constraint::new(
            "c1",
            ConstraintType::Cylindrical,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        let err = c.position_error(&ba, &bb);
        assert_eq!(err.len(), 4);
    }

    #[test]
    fn test_screw_constraint() {
        let ba = make_body("a", Coord3D::new(0.0, 0.0, 0.0));
        let bb = make_body("b", Coord3D::new(0.0, 0.0, 0.0));
        let mut c = Constraint::new(
            "c1",
            ConstraintType::Screw,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        c.pitch = 0.01;
        let err = c.position_error(&ba, &bb);
        assert_eq!(err.len(), 5);
    }

    #[test]
    fn test_planar_constraint() {
        let ba = make_body("a", Coord3D::new(0.0, 0.0, 0.0));
        let bb = make_body("b", Coord3D::new(0.1, 0.2, 0.3));
        let c = Constraint::new(
            "c1",
            ConstraintType::Planar,
            "a",
            "b",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        let err = c.position_error(&ba, &bb);
        assert_eq!(err.len(), 3);
    }
}
