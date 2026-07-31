//! Non-sequential ray tracing.
use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use crate::domains::optical::ray::Ray;
pub struct Intersection {
    pub point: Coord3D,
    pub normal: Coord3D,
    pub distance: Scalar,
}
pub trait OpticalObject: Send + Sync {
    fn intersect(&self, ray: &Ray) -> Option<Intersection>;
    fn scatter(&self, ray: &Ray, hit: &Intersection) -> Vec<Ray>;
}
pub struct NonSequentialRayTracer {
    pub objects: Vec<Box<dyn OpticalObject>>,
    pub rays: Vec<Ray>,
    pub max_bounces: usize,
}
impl NonSequentialRayTracer {
    pub fn new(max_bounces: usize) -> Self {
        Self {
            objects: Vec::new(),
            rays: Vec::new(),
            max_bounces,
        }
    }
    pub fn add_object(&mut self, obj: Box<dyn OpticalObject>) {
        self.objects.push(obj);
    }
    pub fn add_ray(&mut self, ray: Ray) {
        self.rays.push(ray);
    }
    pub fn trace(&mut self) -> Result<(), String> {
        for _ in 0..self.max_bounces {
            let current = std::mem::take(&mut self.rays);
            for ray in current {
                let mut hit = false;
                for obj in &self.objects {
                    if let Some(h) = obj.intersect(&ray) {
                        self.rays.extend(obj.scatter(&ray, &h));
                        hit = true;
                        break;
                    }
                }
                if !hit {
                    self.rays.push(ray);
                }
            }
        }
        Ok(())
    }
    /// Bin the final ray hit positions (ray origins after tracing) onto a
    /// `bins × bins` irradiance map in the XY plane. Each bin counts the
    /// number of rays that terminated there, normalized to the max bin so the
    /// peak is 1.0. Returns all zeros if there are no traced rays.
    pub fn irradiance_map(&self, bins: usize) -> Vec<Vec<Scalar>> {
        let bins = bins.max(1);
        let mut map = vec![vec![0.0; bins]; bins];
        if self.rays.is_empty() {
            return map;
        }
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for r in &self.rays {
            min_x = min_x.min(r.origin.x);
            max_x = max_x.max(r.origin.x);
            min_y = min_y.min(r.origin.y);
            max_y = max_y.max(r.origin.y);
        }
        let range_x = (max_x - min_x).max(1e-12);
        let range_y = (max_y - min_y).max(1e-12);
        for r in &self.rays {
            let bx = (((r.origin.x - min_x) / range_x) * bins as Scalar).floor() as usize;
            let by = (((r.origin.y - min_y) / range_y) * bins as Scalar).floor() as usize;
            map[by.min(bins - 1)][bx.min(bins - 1)] += 1.0;
        }
        let max = map.iter().flatten().copied().fold(0.0_f64, f64::max);
        if max > 0.0 {
            for row in &mut map {
                for v in row.iter_mut() {
                    *v /= max;
                }
            }
        }
        map
    }
}
pub struct FlatMirrorObj {
    pub centre: Coord3D,
    pub normal: Coord3D,
}
impl OpticalObject for FlatMirrorObj {
    fn intersect(&self, ray: &Ray) -> Option<Intersection> {
        let denom = self.normal.x * ray.direction.x
            + self.normal.y * ray.direction.y
            + self.normal.z * ray.direction.z;
        if denom.abs() < 1e-30 {
            return None;
        }
        let t = -(self.normal.x * (ray.origin.x - self.centre.x)
            + self.normal.y * (ray.origin.y - self.centre.y)
            + self.normal.z * (ray.origin.z - self.centre.z))
            / denom;
        if t < 0.0 {
            return None;
        }
        Some(Intersection {
            point: Coord3D::new(
                ray.origin.x + t * ray.direction.x,
                ray.origin.y + t * ray.direction.y,
                ray.origin.z + t * ray.direction.z,
            ),
            normal: self.normal,
            distance: t,
        })
    }
    fn scatter(&self, ray: &Ray, hit: &Intersection) -> Vec<Ray> {
        let cos_i = ray.direction.x * hit.normal.x
            + ray.direction.y * hit.normal.y
            + ray.direction.z * hit.normal.z;
        let rd = Coord3D::new(
            ray.direction.x - 2.0 * cos_i * hit.normal.x,
            ray.direction.y - 2.0 * cos_i * hit.normal.y,
            ray.direction.z - 2.0 * cos_i * hit.normal.z,
        );
        vec![Ray::new(hit.point, rd, ray.wavelength)]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::optical::ray::Ray;
    #[test]
    fn test_tracer_new() {
        let _t = NonSequentialRayTracer::new(10);
    }
    #[test]
    fn test_mirror_intersect() {
        let m = FlatMirrorObj {
            centre: Coord3D::new(0.0, 0.0, 0.0),
            normal: Coord3D::new(0.0, 0.0, 1.0),
        };
        let r = Ray::new(
            Coord3D::new(0.0, 0.0, -1.0),
            Coord3D::new(0.0, 0.0, 1.0),
            500e-9,
        );
        assert!(m.intersect(&r).is_some());
    }
    #[test]
    fn test_mirror_scatter() {
        let m = FlatMirrorObj {
            centre: Coord3D::new(0.0, 0.0, 0.0),
            normal: Coord3D::new(0.0, 0.0, 1.0),
        };
        let r = Ray::new(
            Coord3D::new(0.0, 0.0, -1.0),
            Coord3D::new(0.0, 0.0, 1.0),
            500e-9,
        );
        let h = m.intersect(&r).unwrap();
        let s = m.scatter(&r, &h);
        assert_eq!(s.len(), 1);
        assert!((s[0].direction.z + 1.0).abs() < 1e-10);
    }
}
