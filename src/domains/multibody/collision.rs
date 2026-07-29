//! Collision detection and contact mechanics.
//!
//! Provides bounding‑volume (AABB) tests, sphere–sphere collision,
//! spring‑damper contact force, Coulomb friction, and collision impulse
//! computations for multibody simulation.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Result of a collision query between two bodies.
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionResult {
    /// ID of the first body.
    pub body_a: String,
    /// ID of the second body.
    pub body_b: String,
    /// Contact point on body A in world coordinates.
    pub contact_point_a: Coord3D,
    /// Contact point on body B in world coordinates.
    pub contact_point_b: Coord3D,
    /// Contact normal pointing from B toward A (unit vector).
    pub contact_normal: [Scalar; 3],
    /// Penetration depth (positive if bodies overlap).
    pub penetration_depth: Scalar,
    /// Whether a collision was detected.
    pub has_collision: bool,
}

/// Axis-aligned bounding box for broad-phase collision culling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Minimum corner (x_min, y_min, z_min).
    pub min: Coord3D,
    /// Maximum corner (x_max, y_max, z_max).
    pub max: Coord3D,
}

impl Aabb {
    /// Create an AABB from a set of points.
    pub fn from_points(points: &[Coord3D]) -> Self {
        if points.is_empty() {
            return Self {
                min: Coord3D::new(0.0, 0.0, 0.0),
                max: Coord3D::new(0.0, 0.0, 0.0),
            };
        }
        let mut min_x = points[0].x;
        let mut min_y = points[0].y;
        let mut min_z = points[0].z;
        let mut max_x = min_x;
        let mut max_y = min_y;
        let mut max_z = min_z;
        for p in points {
            if p.x < min_x {
                min_x = p.x;
            }
            if p.y < min_y {
                min_y = p.y;
            }
            if p.z < min_z {
                min_z = p.z;
            }
            if p.x > max_x {
                max_x = p.x;
            }
            if p.y > max_y {
                max_y = p.y;
            }
            if p.z > max_z {
                max_z = p.z;
            }
        }
        Self {
            min: Coord3D::new(min_x, min_y, min_z),
            max: Coord3D::new(max_x, max_y, max_z),
        }
    }

    /// Test whether this AABB overlaps with another.
    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// Return the center of the AABB.
    pub fn center(&self) -> Coord3D {
        Coord3D::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    /// Return the half-extents (half-widths) along each axis.
    pub fn half_extents(&self) -> Coord3D {
        Coord3D::new(
            (self.max.x - self.min.x) * 0.5,
            (self.max.y - self.min.y) * 0.5,
            (self.max.z - self.min.z) * 0.5,
        )
    }
}

/// Primitive collision shape for narrow-phase detection.
#[derive(Debug, Clone, PartialEq)]
pub enum CollisionShape {
    /// Sphere with given radius.
    Sphere {
        /// Radius in meters.
        radius: Scalar,
    },
    /// Axis-aligned box with half-extents.
    Box {
        /// Half-extents along local x, y, z.
        half_extents: Coord3D,
    },
    /// Infinite plane defined by `n·x + d = 0`.
    Plane {
        /// Unit normal vector.
        normal: [Scalar; 3],
        /// Distance from origin along normal.
        d: Scalar,
    },
    /// Triangle mesh.
    Mesh {
        /// Vertex positions in local coordinates.
        vertices: Vec<Coord3D>,
        /// Triangle index tuples.
        triangles: Vec<(usize, usize, usize)>,
    },
}

/// Sphere–sphere collision detection.
///
/// Returns a `CollisionResult` with contact information if the two spheres
/// overlap (distance between centers < sum of radii).
pub fn sphere_sphere_collision(
    pos_a: Coord3D,
    radius_a: Scalar,
    pos_b: Coord3D,
    radius_b: Scalar,
) -> CollisionResult {
    let dx = pos_b.x - pos_a.x;
    let dy = pos_b.y - pos_a.y;
    let dz = pos_b.z - pos_a.z;
    let dist_sq = dx * dx + dy * dy + dz * dz;
    let sum_r = radius_a + radius_b;

    let result = if dist_sq < sum_r * sum_r && dist_sq > 1e-30 {
        let dist = dist_sq.sqrt();
        let penetration = sum_r - dist;
        let nx = dx / dist;
        let ny = dy / dist;
        let nz = dz / dist;
        CollisionResult {
            body_a: String::new(),
            body_b: String::new(),
            contact_point_a: Coord3D::new(
                pos_a.x + nx * radius_a,
                pos_a.y + ny * radius_a,
                pos_a.z + nz * radius_a,
            ),
            contact_point_b: Coord3D::new(
                pos_b.x - nx * radius_b,
                pos_b.y - ny * radius_b,
                pos_b.z - nz * radius_b,
            ),
            contact_normal: [nx, ny, nz],
            penetration_depth: penetration,
            has_collision: true,
        }
    } else {
        CollisionResult {
            body_a: String::new(),
            body_b: String::new(),
            contact_point_a: pos_a,
            contact_point_b: pos_b,
            contact_normal: [0.0; 3],
            penetration_depth: 0.0,
            has_collision: false,
        }
    };
    result
}

/// Contact force magnitude from a linear spring-damper model.
///
/// `f = stiffness * penetration + damping * penetration_velocity`
///
/// Only produces positive (repulsive) force when penetration > 0.
pub fn contact_force_spring_damper(
    penetration: Scalar,
    penetration_velocity: Scalar,
    stiffness: Scalar,
    damping: Scalar,
) -> Scalar {
    if penetration <= 0.0 {
        return 0.0;
    }
    let force = stiffness * penetration + damping * penetration_velocity;
    force.max(0.0) // no adhesive force
}

/// Coulomb friction force magnitude.
///
/// If `relative_velocity` is near zero, returns static friction;
/// otherwise returns kinetic friction.
pub fn friction_force(
    normal_force: Scalar,
    mu_static: Scalar,
    mu_kinetic: Scalar,
    relative_velocity: Scalar,
) -> Scalar {
    if normal_force <= 0.0 {
        return 0.0;
    }
    let v_abs = relative_velocity.abs();
    let mu = if v_abs < 1e-8 {
        mu_static
    } else {
        mu_kinetic
    };
    mu * normal_force * relative_velocity.signum()
}

/// Collision impulse magnitude (nonlinear coefficient of restitution model).
///
/// `j = -(1 + e) * (v_rel · n) / (1/m_a + 1/m_b)`
///
/// The result is the impulse magnitude applied along the contact normal.
pub fn collision_impulse(
    relative_velocity: [Scalar; 3],
    normal: [Scalar; 3],
    restitution: Scalar,
    mass_a: Scalar,
    mass_b: Scalar,
) -> Scalar {
    let vn = relative_velocity[0] * normal[0]
        + relative_velocity[1] * normal[1]
        + relative_velocity[2] * normal[2];

    if vn >= 0.0 {
        // Bodies are separating; no impulse needed
        return 0.0;
    }

    let inv_m_a = if mass_a > 0.0 { 1.0 / mass_a } else { 0.0 };
    let inv_m_b = if mass_b > 0.0 { 1.0 / mass_b } else { 0.0 };
    let effective_mass = inv_m_a + inv_m_b;

    if effective_mass < 1e-30 {
        return 0.0;
    }

    -(1.0 + restitution) * vn / effective_mass
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_from_points() {
        let pts = vec![
            Coord3D::new(1.0, 2.0, 3.0),
            Coord3D::new(4.0, 5.0, 6.0),
            Coord3D::new(-1.0, 0.0, 2.0),
        ];
        let aabb = Aabb::from_points(&pts);
        assert!((aabb.min.x + 1.0).abs() < 1e-12);
        assert!((aabb.max.x - 4.0).abs() < 1e-12);
        assert!((aabb.min.y).abs() < 1e-12);
        assert!((aabb.max.y - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_aabb_overlaps() {
        let a = Aabb {
            min: Coord3D::new(0.0, 0.0, 0.0),
            max: Coord3D::new(1.0, 1.0, 1.0),
        };
        let b = Aabb {
            min: Coord3D::new(0.5, 0.5, 0.5),
            max: Coord3D::new(1.5, 1.5, 1.5),
        };
        assert!(a.overlaps(&b));
    }

    #[test]
    fn test_aabb_no_overlap() {
        let a = Aabb {
            min: Coord3D::new(0.0, 0.0, 0.0),
            max: Coord3D::new(1.0, 1.0, 1.0),
        };
        let b = Aabb {
            min: Coord3D::new(2.0, 2.0, 2.0),
            max: Coord3D::new(3.0, 3.0, 3.0),
        };
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn test_aabb_center() {
        let a = Aabb {
            min: Coord3D::new(0.0, 0.0, 0.0),
            max: Coord3D::new(2.0, 4.0, 6.0),
        };
        let c = a.center();
        assert!((c.x - 1.0).abs() < 1e-12);
        assert!((c.y - 2.0).abs() < 1e-12);
        assert!((c.z - 3.0).abs() < 1e-12);
    }

    #[test]
    fn test_sphere_sphere_collision_hit() {
        let result = sphere_sphere_collision(
            Coord3D::new(0.0, 0.0, 0.0),
            1.0,
            Coord3D::new(1.5, 0.0, 0.0),
            1.0,
        );
        assert!(result.has_collision);
        // penetration = 2.0 - 1.5 = 0.5
        assert!((result.penetration_depth - 0.5).abs() < 1e-12);
        assert!((result.contact_normal[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_sphere_sphere_collision_miss() {
        let result = sphere_sphere_collision(
            Coord3D::new(0.0, 0.0, 0.0),
            1.0,
            Coord3D::new(3.0, 0.0, 0.0),
            1.0,
        );
        assert!(!result.has_collision);
    }

    #[test]
    fn test_sphere_sphere_touching() {
        let result = sphere_sphere_collision(
            Coord3D::new(0.0, 0.0, 0.0),
            1.0,
            Coord3D::new(2.0, 0.0, 0.0),
            1.0,
        );
        // Exactly touching: dist = 2.0, sum_r = 2.0 → dist_sq = 4, sum_r² = 4 → 4 < 4 is false
        assert!(!result.has_collision);
    }

    #[test]
    fn test_contact_force_spring_damper() {
        let f = contact_force_spring_damper(0.1, -0.5, 1000.0, 50.0);
        // 1000 * 0.1 + 50 * (-0.5) = 100 - 25 = 75
        assert!((f - 75.0).abs() < 1e-10);
    }

    #[test]
    fn test_contact_force_no_penetration() {
        let f = contact_force_spring_damper(-0.1, 0.0, 1000.0, 50.0);
        assert!((f).abs() < 1e-12);
    }

    #[test]
    fn test_friction_force_static() {
        let f = friction_force(100.0, 0.5, 0.3, 0.0);
        // static friction at velocity = 0 → 0.5 * 100 = 50
        assert!((f - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_friction_force_kinetic() {
        let f = friction_force(100.0, 0.5, 0.3, 2.0);
        // kinetic at v=2 → 0.3 * 100 = 30, sign positive
        assert!((f - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_collision_impulse_separating() {
        let j = collision_impulse([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.5, 1.0, 1.0);
        // vn = 1 >= 0, separating → no impulse
        assert!((j).abs() < 1e-12);
    }

    #[test]
    fn test_collision_impulse_approaching() {
        let j = collision_impulse([-2.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.5, 1.0, 1.0);
        // vn = -2; j = -(1+0.5)*(-2) / (1+1) = 3/2 = 1.5
        assert!((j - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_collision_impulse_infinite_mass() {
        // One body with infinite mass (mass=0)
        let j = collision_impulse([-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.0, 0.0, 1.0);
        // inv_m_a = 0, inv_m_b = 1, effective = 1
        // j = -(1+0)*(-1)/(1) = 1
        assert!((j - 1.0).abs() < 1e-10);
    }
}
