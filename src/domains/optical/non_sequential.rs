//! Non-sequential ray tracing.
use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use crate::domains::optical::ray::Ray;
pub struct Intersection { pub point: Coord3D, pub normal: Coord3D, pub distance: Scalar }
pub trait OpticalObject: Send + Sync {
    fn intersect(&self, ray: &Ray) -> Option<Intersection>;
    fn scatter(&self, ray: &Ray, hit: &Intersection) -> Vec<Ray>;
}
pub struct NonSequentialRayTracer {
    pub objects: Vec<Box<dyn OpticalObject>>, pub rays: Vec<Ray>, pub max_bounces: usize,
}
impl NonSequentialRayTracer {
    pub fn new(max_bounces: usize) -> Self { Self { objects: Vec::new(), rays: Vec::new(), max_bounces } }
    pub fn add_object(&mut self, obj: Box<dyn OpticalObject>) { self.objects.push(obj); }
    pub fn add_ray(&mut self, ray: Ray) { self.rays.push(ray); }
    pub fn trace(&mut self) -> Result<(), String> {
        for _ in 0..self.max_bounces {
            let current = std::mem::take(&mut self.rays);
            for ray in current {
                let mut hit = false;
                for obj in &self.objects {
                    if let Some(h) = obj.intersect(&ray) {
                        self.rays.extend(obj.scatter(&ray, &h));
                        hit = true; break;
                    }
                }
                if !hit { self.rays.push(ray); }
            }
        }
        Ok(())
    }
    pub fn irradiance_map(&self, _bins: usize) -> Vec<Vec<Scalar>> {
        vec![vec![0.0; _bins]; _bins]
    }
}
pub struct FlatMirrorObj { pub centre: Coord3D, pub normal: Coord3D }
impl OpticalObject for FlatMirrorObj {
    fn intersect(&self, ray: &Ray) -> Option<Intersection> {
        let denom = self.normal.x * ray.direction.x + self.normal.y * ray.direction.y + self.normal.z * ray.direction.z;
        if denom.abs() < 1e-30 { return None; }
        let t = -(self.normal.x * (ray.origin.x - self.centre.x) + self.normal.y * (ray.origin.y - self.centre.y) + self.normal.z * (ray.origin.z - self.centre.z)) / denom;
        if t < 0.0 { return None; }
        Some(Intersection { point: Coord3D::new(ray.origin.x + t * ray.direction.x, ray.origin.y + t * ray.direction.y, ray.origin.z + t * ray.direction.z), normal: self.normal, distance: t })
    }
    fn scatter(&self, ray: &Ray, hit: &Intersection) -> Vec<Ray> {
        let cos_i = ray.direction.x * hit.normal.x + ray.direction.y * hit.normal.y + ray.direction.z * hit.normal.z;
        let rd = Coord3D::new(ray.direction.x - 2.0 * cos_i * hit.normal.x, ray.direction.y - 2.0 * cos_i * hit.normal.y, ray.direction.z - 2.0 * cos_i * hit.normal.z);
        vec![Ray::new(hit.point, rd, ray.wavelength)]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::optical::ray::Ray;
    #[test] fn test_tracer_new() { let _t = NonSequentialRayTracer::new(10); }
    #[test] fn test_mirror_intersect() {
        let m = FlatMirrorObj { centre: Coord3D::new(0.0,0.0,0.0), normal: Coord3D::new(0.0,0.0,1.0) };
        let r = Ray::new(Coord3D::new(0.0,0.0,-1.0), Coord3D::new(0.0,0.0,1.0), 500e-9);
        assert!(m.intersect(&r).is_some());
    }
    #[test] fn test_mirror_scatter() {
        let m = FlatMirrorObj { centre: Coord3D::new(0.0,0.0,0.0), normal: Coord3D::new(0.0,0.0,1.0) };
        let r = Ray::new(Coord3D::new(0.0,0.0,-1.0), Coord3D::new(0.0,0.0,1.0), 500e-9);
        let h = m.intersect(&r).unwrap();
        let s = m.scatter(&r, &h);
        assert_eq!(s.len(), 1);
        assert!((s[0].direction.z + 1.0).abs() < 1e-10);
    }
}
