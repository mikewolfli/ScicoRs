//! Geometric optics: ray tracing, optical elements, imaging systems.
//!
//! Provides Ray struct, OpticalElement trait with mirror/lens/aperture
//! implementations, and ImagingSystem for sequential ray tracing.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// A light ray with position, direction, and optical properties.
#[derive(Debug, Clone)]
pub struct Ray {
    /// Origin point in 3D space.
    pub origin: Coord3D,
    /// Unit direction vector.
    pub direction: Coord3D,
    /// Wavelength (m).
    pub wavelength: Scalar,
    /// Intensity (W/m²).
    pub intensity: Scalar,
    /// Phase (radians).
    pub phase: Scalar,
    /// Accumulated optical path length.
    pub optical_path: Scalar,
}

impl Ray {
    pub fn new(origin: Coord3D, direction: Coord3D, wavelength: Scalar) -> Self {
        let norm = direction.norm();
        let dir = if norm > 0.0 {
            Coord3D::new(direction.x / norm, direction.y / norm, direction.z / norm)
        } else {
            direction
        };
        Self {
            origin,
            direction: dir,
            wavelength,
            intensity: 1.0,
            phase: 0.0,
            optical_path: 0.0,
        }
    }

    /// Advance ray by distance `d` along its direction.
    pub fn advance(&mut self, d: Scalar) {
        self.origin = Coord3D::new(
            self.origin.x + self.direction.x * d,
            self.origin.y + self.direction.y * d,
            self.origin.z + self.direction.z * d,
        );
        self.optical_path += d;
    }
}

/// Record of a ray's intersection with an optical element.
#[derive(Debug, Clone)]
pub struct TracePoint {
    /// Name of the optical element.
    pub element_name: String,
    /// Intersection position.
    pub position: Coord3D,
    /// Cumulative optical path length at this point.
    pub path_length: Scalar,
}

/// Trait for optical elements that can interact with rays.
pub trait OpticalElement: Send + Sync {
    /// Display name of this element.
    fn name(&self) -> &str;
    /// Compute intersection point with a ray. Returns None if no intersection.
    fn intersect(&self, ray: &Ray) -> Option<Coord3D>;
    /// Transmit ray through the element at the given hit point.
    fn transmit(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String>;
    /// Reflect ray off the element at the given hit point.
    fn reflect(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String>;
    /// Paraxial ABCD matrix of this element (ray-vector convention
    /// [y, θ]ᵀ → M·[y, θ]ᵀ). Returns `None` if no closed-form matrix exists.
    fn abcd(&self) -> Option<[[Scalar; 2]; 2]> {
        None
    }
}

/// Flat mirror: reflects rays with θ_out = θ_in.
pub struct FlatMirror {
    pub name: String,
    /// Point on the mirror plane.
    pub point: Coord3D,
    /// Unit normal vector of the mirror surface.
    pub normal: Coord3D,
}

impl FlatMirror {
    pub fn new(name: &str, point: Coord3D, normal: Coord3D) -> Self {
        let norm = normal.norm();
        let n = if norm > 0.0 {
            Coord3D::new(normal.x / norm, normal.y / norm, normal.z / norm)
        } else {
            normal
        };
        Self {
            name: name.to_string(),
            point,
            normal: n,
        }
    }
}

impl OpticalElement for FlatMirror {
    fn name(&self) -> &str {
        &self.name
    }

    fn intersect(&self, ray: &Ray) -> Option<Coord3D> {
        // Plane equation: (p - point)·normal = 0
        // Ray: p = origin + t * direction
        let denom = ray.direction.x * self.normal.x
            + ray.direction.y * self.normal.y
            + ray.direction.z * self.normal.z;
        if denom.abs() < 1e-15 {
            return None; // parallel
        }
        let dx = self.point.x - ray.origin.x;
        let dy = self.point.y - ray.origin.y;
        let dz = self.point.z - ray.origin.z;
        let t = (dx * self.normal.x + dy * self.normal.y + dz * self.normal.z) / denom;
        if t < 1e-15 {
            return None; // behind ray origin
        }
        Some(Coord3D::new(
            ray.origin.x + ray.direction.x * t,
            ray.origin.y + ray.direction.y * t,
            ray.origin.z + ray.direction.z * t,
        ))
    }

    fn transmit(&self, _ray: &mut Ray, _hit: &Coord3D) -> Result<(), String> {
        Err("FlatMirror does not transmit".to_string())
    }

    fn reflect(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String> {
        // r = d - 2(d·n)n
        let dot = ray.direction.x * self.normal.x
            + ray.direction.y * self.normal.y
            + ray.direction.z * self.normal.z;
        ray.origin = *hit;
        ray.direction = Coord3D::new(
            ray.direction.x - 2.0 * dot * self.normal.x,
            ray.direction.y - 2.0 * dot * self.normal.y,
            ray.direction.z - 2.0 * dot * self.normal.z,
        );
        Ok(())
    }

    fn abcd(&self) -> Option<[[Scalar; 2]; 2]> {
        // Flat mirror: no optical power in paraxial approximation.
        Some([[1.0, 0.0], [0.0, 1.0]])
    }
}

/// Spherical mirror with radius of curvature R.
/// Positive R = concave (focusing), negative R = convex.
pub struct SphericalMirror {
    pub name: String,
    pub center: Coord3D,
    pub radius: Scalar, // R > 0: concave
}

impl SphericalMirror {
    pub fn new(name: &str, center: Coord3D, radius: Scalar) -> Self {
        Self {
            name: name.to_string(),
            center,
            radius,
        }
    }
}

impl OpticalElement for SphericalMirror {
    fn name(&self) -> &str {
        &self.name
    }

    fn intersect(&self, ray: &Ray) -> Option<Coord3D> {
        // Sphere: |p - center| = |R|
        // Ray: p = origin + t * dir
        let ocx = ray.origin.x - self.center.x;
        let ocy = ray.origin.y - self.center.y;
        let ocz = ray.origin.z - self.center.z;
        let a = ray.direction.x * ray.direction.x
            + ray.direction.y * ray.direction.y
            + ray.direction.z * ray.direction.z;
        let b = 2.0 * (ray.direction.x * ocx + ray.direction.y * ocy + ray.direction.z * ocz);
        let c = ocx * ocx + ocy * ocy + ocz * ocz - self.radius * self.radius;
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            return None;
        }
        let sqrt_disc = disc.sqrt();
        let t1 = (-b - sqrt_disc) / (2.0 * a);
        let t2 = (-b + sqrt_disc) / (2.0 * a);
        let t = if t1 > 1e-15 {
            t1
        } else if t2 > 1e-15 {
            t2
        } else {
            return None;
        };
        Some(Coord3D::new(
            ray.origin.x + ray.direction.x * t,
            ray.origin.y + ray.direction.y * t,
            ray.origin.z + ray.direction.z * t,
        ))
    }

    fn transmit(&self, _ray: &mut Ray, _hit: &Coord3D) -> Result<(), String> {
        Err("SphericalMirror does not transmit".to_string())
    }

    fn reflect(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String> {
        let nx = (hit.x - self.center.x) / self.radius;
        let ny = (hit.y - self.center.y) / self.radius;
        let nz = (hit.z - self.center.z) / self.radius;
        let norm = (nx * nx + ny * ny + nz * nz).sqrt();
        if norm < 1e-15 {
            return Err("zero normal at hit point".to_string());
        }
        let nnx = nx / norm;
        let nny = ny / norm;
        let nnz = nz / norm;
        let dot = ray.direction.x * nnx + ray.direction.y * nny + ray.direction.z * nnz;
        ray.origin = *hit;
        ray.direction = Coord3D::new(
            ray.direction.x - 2.0 * dot * nnx,
            ray.direction.y - 2.0 * dot * nny,
            ray.direction.z - 2.0 * dot * nnz,
        );
        Ok(())
    }

    fn abcd(&self) -> Option<[[Scalar; 2]; 2]> {
        // Curved mirror with radius R: power Φ = 2/R.
        if self.radius.abs() < 1e-30 {
            return Some([[1.0, 0.0], [0.0, 1.0]]);
        }
        Some([[1.0, 0.0], [-2.0 / self.radius, 1.0]])
    }
}

/// Thin lens with focal length f.
/// Positive f = converging, negative f = diverging.
pub struct ThinLens {
    pub name: String,
    pub center: Coord3D,
    /// Optical axis direction (unit vector).
    pub axis: Coord3D,
    pub focal_length: Scalar,
    pub aperture_radius: Scalar,
}

impl ThinLens {
    pub fn new(
        name: &str,
        center: Coord3D,
        axis: Coord3D,
        focal_length: Scalar,
        aperture: Scalar,
    ) -> Self {
        let norm = axis.norm();
        let ax = if norm > 0.0 {
            Coord3D::new(axis.x / norm, axis.y / norm, axis.z / norm)
        } else {
            axis
        };
        Self {
            name: name.to_string(),
            center,
            axis: ax,
            focal_length,
            aperture_radius: aperture,
        }
    }
}

impl OpticalElement for ThinLens {
    fn name(&self) -> &str {
        &self.name
    }

    fn intersect(&self, ray: &Ray) -> Option<Coord3D> {
        // Intersect with plane through center, normal = axis
        let denom = ray.direction.x * self.axis.x
            + ray.direction.y * self.axis.y
            + ray.direction.z * self.axis.z;
        if denom.abs() < 1e-15 {
            return None;
        }
        let dx = self.center.x - ray.origin.x;
        let dy = self.center.y - ray.origin.y;
        let dz = self.center.z - ray.origin.z;
        let t = (dx * self.axis.x + dy * self.axis.y + dz * self.axis.z) / denom;
        if t < 1e-15 {
            return None;
        }
        let hit = Coord3D::new(
            ray.origin.x + ray.direction.x * t,
            ray.origin.y + ray.direction.y * t,
            ray.origin.z + ray.direction.z * t,
        );
        // Check aperture
        let rx = hit.x - self.center.x;
        let ry = hit.y - self.center.y;
        let rz = hit.z - self.center.z;
        let dist_to_axis = (rx * rx + ry * ry + rz * rz).sqrt();
        if dist_to_axis > self.aperture_radius {
            return None;
        }
        Some(hit)
    }

    fn transmit(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String> {
        // Paraxial approximation: thin lens equation
        // Angular deflection proportional to radial distance
        if self.focal_length.abs() < 1e-15 {
            return Err("zero focal length".to_string());
        }
        let rx = hit.x - self.center.x;
        let ry = hit.y - self.center.y;
        let rz = hit.z - self.center.z;
        // Project radial vector onto plane perpendicular to axis
        let dot_axis = rx * self.axis.x + ry * self.axis.y + rz * self.axis.z;
        let perp_x = rx - dot_axis * self.axis.x;
        let perp_y = ry - dot_axis * self.axis.y;
        let perp_z = rz - dot_axis * self.axis.z;
        // Deflection: Δθ = -r/f (paraxial)
        ray.origin = *hit;
        ray.direction = Coord3D::new(
            ray.direction.x - perp_x / self.focal_length,
            ray.direction.y - perp_y / self.focal_length,
            ray.direction.z - perp_z / self.focal_length,
        );
        // Re-normalize
        let norm = ray.direction.norm();
        if norm > 0.0 {
            ray.direction = Coord3D::new(
                ray.direction.x / norm,
                ray.direction.y / norm,
                ray.direction.z / norm,
            );
        }
        Ok(())
    }

    fn reflect(&self, _ray: &mut Ray, _hit: &Coord3D) -> Result<(), String> {
        Err("ThinLens does not reflect (anti-reflective coating assumed)".to_string())
    }

    fn abcd(&self) -> Option<[[Scalar; 2]; 2]> {
        // Thin lens: power Φ = 1/f.
        if self.focal_length.abs() < 1e-30 {
            return None;
        }
        Some([[1.0, 0.0], [-1.0 / self.focal_length, 1.0]])
    }
}

/// Flat dielectric interface: Snell's law refraction.
pub struct FlatInterface {
    pub name: String,
    pub point: Coord3D,
    pub normal: Coord3D,
    pub n1: Scalar, // refractive index on incident side
    pub n2: Scalar, // refractive index on transmission side
}

impl FlatInterface {
    pub fn new(name: &str, point: Coord3D, normal: Coord3D, n1: Scalar, n2: Scalar) -> Self {
        let norm = normal.norm();
        let n = if norm > 0.0 {
            Coord3D::new(normal.x / norm, normal.y / norm, normal.z / norm)
        } else {
            normal
        };
        Self {
            name: name.to_string(),
            point,
            normal: n,
            n1,
            n2,
        }
    }
}

impl OpticalElement for FlatInterface {
    fn name(&self) -> &str {
        &self.name
    }

    fn intersect(&self, ray: &Ray) -> Option<Coord3D> {
        let denom = ray.direction.x * self.normal.x
            + ray.direction.y * self.normal.y
            + ray.direction.z * self.normal.z;
        if denom.abs() < 1e-15 {
            return None;
        }
        let dx = self.point.x - ray.origin.x;
        let dy = self.point.y - ray.origin.y;
        let dz = self.point.z - ray.origin.z;
        let t = (dx * self.normal.x + dy * self.normal.y + dz * self.normal.z) / denom;
        if t < 1e-15 {
            return None;
        }
        Some(Coord3D::new(
            ray.origin.x + ray.direction.x * t,
            ray.origin.y + ray.direction.y * t,
            ray.origin.z + ray.direction.z * t,
        ))
    }

    fn transmit(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String> {
        // Snell's law: n1*sin(θ1) = n2*sin(θ2)
        let cos_i = -(ray.direction.x * self.normal.x
            + ray.direction.y * self.normal.y
            + ray.direction.z * self.normal.z);
        let sin_i_sq = 1.0 - cos_i * cos_i;
        let ratio = self.n1 / self.n2;
        let sin_t_sq = ratio * ratio * sin_i_sq;
        if sin_t_sq > 1.0 {
            return Err("total internal reflection".to_string());
        }
        let cos_t = (1.0 - sin_t_sq).sqrt();
        ray.origin = *hit;
        ray.direction = Coord3D::new(
            ratio * ray.direction.x + (ratio * cos_i - cos_t) * self.normal.x,
            ratio * ray.direction.y + (ratio * cos_i - cos_t) * self.normal.y,
            ratio * ray.direction.z + (ratio * cos_i - cos_t) * self.normal.z,
        );
        Ok(())
    }

    fn reflect(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String> {
        let dot = ray.direction.x * self.normal.x
            + ray.direction.y * self.normal.y
            + ray.direction.z * self.normal.z;
        ray.origin = *hit;
        ray.direction = Coord3D::new(
            ray.direction.x - 2.0 * dot * self.normal.x,
            ray.direction.y - 2.0 * dot * self.normal.y,
            ray.direction.z - 2.0 * dot * self.normal.z,
        );
        Ok(())
    }

    fn abcd(&self) -> Option<[[Scalar; 2]; 2]> {
        // Plane interface: angle scales by n1/n2, position unchanged.
        Some([[1.0, 0.0], [0.0, self.n1 / self.n2]])
    }
}

/// Aperture stop: circular hole that limits ray bundle.
pub struct Aperture {
    pub name: String,
    pub center: Coord3D,
    pub normal: Coord3D,
    pub radius: Scalar,
}

impl Aperture {
    pub fn new(name: &str, center: Coord3D, normal: Coord3D, radius: Scalar) -> Self {
        let norm = normal.norm();
        let n = if norm > 0.0 {
            Coord3D::new(normal.x / norm, normal.y / norm, normal.z / norm)
        } else {
            normal
        };
        Self {
            name: name.to_string(),
            center,
            normal: n,
            radius,
        }
    }
}

impl OpticalElement for Aperture {
    fn name(&self) -> &str {
        &self.name
    }

    fn intersect(&self, ray: &Ray) -> Option<Coord3D> {
        let denom = ray.direction.x * self.normal.x
            + ray.direction.y * self.normal.y
            + ray.direction.z * self.normal.z;
        if denom.abs() < 1e-15 {
            return None;
        }
        let dx = self.center.x - ray.origin.x;
        let dy = self.center.y - ray.origin.y;
        let dz = self.center.z - ray.origin.z;
        let t = (dx * self.normal.x + dy * self.normal.y + dz * self.normal.z) / denom;
        if t < 1e-15 {
            return None;
        }
        let hit = Coord3D::new(
            ray.origin.x + ray.direction.x * t,
            ray.origin.y + ray.direction.y * t,
            ray.origin.z + ray.direction.z * t,
        );
        let rx = hit.x - self.center.x;
        let ry = hit.y - self.center.y;
        let rz = hit.z - self.center.z;
        let dist = (rx * rx + ry * ry + rz * rz).sqrt();
        if dist > self.radius {
            return None;
        }
        Some(hit)
    }

    fn transmit(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String> {
        ray.origin = *hit;
        Ok(())
    }

    fn reflect(&self, _ray: &mut Ray, _hit: &Coord3D) -> Result<(), String> {
        Err("Aperture does not reflect".to_string())
    }

    fn abcd(&self) -> Option<[[Scalar; 2]; 2]> {
        // Aperture: no optical power.
        Some([[1.0, 0.0], [0.0, 1.0]])
    }
}

/// Sequential imaging system: traces rays through ordered elements.
pub struct ImagingSystem {
    pub elements: Vec<Box<dyn OpticalElement>>,
}

impl ImagingSystem {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    pub fn add_element(&mut self, element: Box<dyn OpticalElement>) {
        self.elements.push(element);
    }

    /// Trace a single ray through all elements. Returns list of intersection points.
    pub fn trace_ray(&self, ray: &mut Ray) -> Result<Vec<TracePoint>, String> {
        let mut trace = Vec::new();
        for element in &self.elements {
            let hit = element
                .intersect(ray)
                .ok_or_else(|| format!("{}: missed", element.name()))?;
            let path_sofar = ray.optical_path;
            // Attempt transmission; if fails, try reflection
            if element.transmit(ray, &hit).is_err() {
                element.reflect(ray, &hit)?;
            }
            trace.push(TracePoint {
                element_name: element.name().to_string(),
                position: hit,
                path_length: ray.optical_path,
            });
            // Update optical path with distance from last hit
            let _ = path_sofar;
        }
        Ok(trace)
    }

    /// Trace a fan of rays from a given origin with specified angles (radians).
    pub fn trace_fan(
        &self,
        origin: Coord3D,
        angles: &[Scalar],
        wavelength: Scalar,
    ) -> Vec<Vec<TracePoint>> {
        let mut results = Vec::new();
        for &theta in angles {
            let mut ray = Ray::new(
                origin,
                Coord3D::new(theta.sin(), 0.0, theta.cos()),
                wavelength,
            );
            if let Ok(trace) = self.trace_ray(&mut ray) {
                results.push(trace);
            }
        }
        results
    }

    /// Paraxial ABCD matrix of the system for a given wavelength.
    ///
    /// Composes the per-element ABCD matrices in ray-propagation order
    /// (result = M_n·…·M_2·M_1). Elements without a closed-form matrix are
    /// skipped. Note: the 3D element model does not store inter-element
    /// distances, so free-space propagation between elements is not included;
    /// this is a thin-optics approximation.
    pub fn paraxial_matrix(&self, _wavelength: Scalar) -> [[Scalar; 2]; 2] {
        let mut m: [[Scalar; 2]; 2] = [[1.0, 0.0], [0.0, 1.0]];
        for element in &self.elements {
            if let Some(em) = element.abcd() {
                m = abcd_multiply(m, em);
            }
        }
        m
    }

    /// Estimate defocus spot radius from ray fan.
    pub fn defocus_spot(&self, source: Coord3D, n_rays: usize) -> Scalar {
        if n_rays == 0 {
            return 0.0;
        }
        let mut max_r = 0.0;
        for i in 0..n_rays {
            let theta = (i as Scalar / n_rays as Scalar) * std::f64::consts::PI * 0.5;
            let mut ray = Ray::new(source, Coord3D::new(theta.sin(), 0.0, theta.cos()), 500e-9);
            if let Ok(Some(last)) = self.trace_ray(&mut ray).as_ref().map(|v| v.last()) {
                let r = (last.position.x * last.position.x
                    + last.position.y * last.position.y
                    + last.position.z * last.position.z)
                    .sqrt();
                if r > max_r {
                    max_r = r;
                }
            }
        }
        max_r
    }
}

impl Default for ImagingSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute ABCD matrix for free propagation of distance d.
pub fn abcd_free_space(d: Scalar) -> [[Scalar; 2]; 2] {
    [[1.0, d], [0.0, 1.0]]
}

/// Compute ABCD matrix for thin lens of focal length f.
pub fn abcd_thin_lens(f: Scalar) -> [[Scalar; 2]; 2] {
    [[1.0, 0.0], [-1.0 / f, 1.0]]
}

/// Compute ABCD matrix for spherical refraction at radius R.
///
/// Power Φ = (n2 − n1)/R (positive R = centre on transmission side):
/// `M = [[1, 0], [−(n2−n1)/(n2·R), n1/n2]]`.
pub fn abcd_spherical_refraction(n1: Scalar, n2: Scalar, r: Scalar) -> [[Scalar; 2]; 2] {
    [[1.0, 0.0], [(n1 - n2) / (r * n2), n1 / n2]]
}

/// Multiply two 2x2 ABCD matrices: result = b * a.
pub fn abcd_multiply(a: [[Scalar; 2]; 2], b: [[Scalar; 2]; 2]) -> [[Scalar; 2]; 2] {
    [
        [
            a[0][0] * b[0][0] + a[0][1] * b[1][0],
            a[0][0] * b[0][1] + a[0][1] * b[1][1],
        ],
        [
            a[1][0] * b[0][0] + a[1][1] * b[1][0],
            a[1][0] * b[0][1] + a[1][1] * b[1][1],
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_new_normalizes_direction() {
        let ray = Ray::new(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1.0, 1.0, 0.0),
            500e-9,
        );
        let norm = ray.direction.norm();
        assert!((norm - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_flat_mirror_reflection() {
        let mirror = FlatMirror::new(
            "m1",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 1.0),
        );
        let mut ray = Ray::new(
            Coord3D::new(0.0, 0.0, 1.0),
            Coord3D::new(0.0, 0.0, -1.0),
            500e-9,
        );
        let hit = mirror.intersect(&ray).unwrap();
        mirror.reflect(&mut ray, &hit).unwrap();
        assert!((ray.direction.z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_flat_mirror_reflection_angle() {
        // Incident at 45° should reflect at 45°
        let mirror = FlatMirror::new(
            "m1",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 1.0),
        );
        let dir = 2.0_f64.sqrt() / 2.0;
        // Ray coming from +x,+z heading towards -x,-z (towards mirror at origin)
        let mut ray = Ray::new(
            Coord3D::new(1.0, 0.0, 1.0),
            Coord3D::new(-dir, 0.0, -dir),
            500e-9,
        );
        let hit = mirror.intersect(&ray).unwrap();
        mirror.reflect(&mut ray, &hit).unwrap();
        // After reflection: x component stays -dir (pointing away from axis),
        // z component becomes +dir (bouncing back)
        // r = d - 2(d·n)n = (-dir,0,-dir) - 2(-dir)*n = (-dir,0,-dir)+(0,0,2dir)
        // = (-dir, 0, dir)
        assert!((ray.direction.x + dir).abs() < 1e-10);
        assert!((ray.direction.z - dir).abs() < 1e-10);
    }

    #[test]
    fn test_flat_interface_snell_refraction() {
        let interface = FlatInterface::new(
            "air-glass",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, -1.0), // normal points towards incident ray
            1.0,
            1.5,
        );
        let angle = 30.0_f64.to_radians();
        let mut ray = Ray::new(
            Coord3D::new(angle.sin() * 0.5, 0.0, 0.5),
            Coord3D::new(angle.sin(), 0.0, -angle.cos()),
            500e-9,
        );
        let hit = interface.intersect(&ray).unwrap();
        interface.transmit(&mut ray, &hit).unwrap();
        // Snell's law: n1*sin(θ1) = n2*sin(θ2)
        let sin_t = ray.direction.x; // direction is normalized
        let expected_sin_t = (1.0 / 1.5) * angle.sin();
        assert!((sin_t - expected_sin_t).abs() < 0.01);
    }

    #[test]
    fn test_thin_lens_focal_point() {
        let lens = ThinLens::new(
            "lens",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 1.0),
            0.1,  // f = 10 cm
            0.05, // aperture radius = 5 cm
        );
        // Ray parallel to axis at height 0.01 m should focus at f
        let mut ray = Ray::new(
            Coord3D::new(0.01, 0.0, -0.2),
            Coord3D::new(0.0, 0.0, 1.0),
            500e-9,
        );
        let hit = lens.intersect(&ray).unwrap();
        lens.transmit(&mut ray, &hit).unwrap();
        // After lens, direction should point towards focal point
        // At z=f=0.1, x should be ~0
        let t_to_focus = (0.1 - hit.z) / ray.direction.z;
        let x_at_focus = hit.x + ray.direction.x * t_to_focus;
        assert!(x_at_focus.abs() < 0.005);
    }

    #[test]
    fn test_aperture_blocks_ray() {
        let aperture = Aperture::new(
            "stop",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 1.0),
            0.01, // radius = 1 cm
        );
        // Ray through center should pass
        let ray_center = Ray::new(
            Coord3D::new(0.0, 0.0, -0.1),
            Coord3D::new(0.0, 0.0, 1.0),
            500e-9,
        );
        assert!(aperture.intersect(&ray_center).is_some());
        // Ray far off-axis should be blocked
        let ray_edge = Ray::new(
            Coord3D::new(0.1, 0.0, -0.1),
            Coord3D::new(0.0, 0.0, 1.0),
            500e-9,
        );
        assert!(aperture.intersect(&ray_edge).is_none());
    }

    #[test]
    fn test_spherical_mirror_reflection() {
        // Concave mirror with R=0.2 m, focal length f=0.1 m
        let mirror = SphericalMirror::new("concave", Coord3D::new(0.0, 0.0, 0.0), 0.2);
        // Ray parallel to axis at height 0.01 m
        let mut ray = Ray::new(
            Coord3D::new(0.01, 0.0, -0.15),
            Coord3D::new(0.0, 0.0, 1.0),
            500e-9,
        );
        let hit = mirror.intersect(&ray).expect("should hit");
        mirror.reflect(&mut ray, &hit).unwrap();
        // After reflection, direction should have negative x component (towards axis)
        assert!(ray.direction.x < 0.0);
    }

    #[test]
    fn test_abcd_free_space() {
        let m = abcd_free_space(0.1);
        assert!((m[0][0] - 1.0).abs() < 1e-12);
        assert!((m[0][1] - 0.1).abs() < 1e-12);
        assert!((m[1][0]).abs() < 1e-12);
        assert!((m[1][1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_abcd_thin_lens() {
        let f = 0.1;
        let m = abcd_thin_lens(f);
        assert!((m[0][0] - 1.0).abs() < 1e-12);
        assert!((m[1][0] + 10.0).abs() < 1e-12);
    }

    #[test]
    fn test_abcd_multiply() {
        let a = abcd_free_space(0.05);
        let b = abcd_thin_lens(0.1);
        let m = abcd_multiply(a, b);
        // Propagation then lens
        assert!((m[0][0] - (1.0 - 0.05 / 0.1)).abs() < 1e-12); // = 0.5
        assert!((m[0][1] - 0.05).abs() < 1e-12);
    }

    #[test]
    fn test_imaging_system_trace_ray() {
        let mut sys = ImagingSystem::new();
        let lens = Box::new(ThinLens::new(
            "lens",
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 1.0),
            0.1,
            0.05,
        ));
        sys.add_element(lens);
        let mut ray = Ray::new(
            Coord3D::new(0.005, 0.0, -0.2),
            Coord3D::new(0.0, 0.0, 1.0),
            500e-9,
        );
        let trace = sys.trace_ray(&mut ray).unwrap();
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].element_name, "lens");
    }
}
