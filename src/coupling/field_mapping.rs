//! Field mapping and interpolation between meshes.

use super::bus::FieldMappingMethod;
use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Radial basis function types.
pub enum RbfType {
    Gaussian(Scalar),
    Multiquadric(Scalar),
    ThinPlateSpline,
}

/// Field mapper for transferring data between meshes.
pub struct FieldMapper {
    pub method: FieldMappingMethod,
}

impl FieldMapper {
    pub fn new(method: FieldMappingMethod) -> Self {
        Self { method }
    }

    pub fn map(
        &self,
        source_points: &[Coord3D],
        source_values: &[Scalar],
        target_points: &[Coord3D],
    ) -> Result<Vec<Scalar>, String> {
        if source_points.is_empty() || source_values.is_empty() {
            return Err("Empty source data".to_string());
        }
        match self.method {
            FieldMappingMethod::NearestNeighbor => Ok(Self::nearest_neighbor(
                source_points,
                source_values,
                target_points,
            )),
            FieldMappingMethod::InverseDistance => Ok(Self::inverse_distance_weighted(
                source_points,
                source_values,
                target_points,
                2.0,
            )),
            FieldMappingMethod::RadialBasis => Self::radial_basis_interpolation(
                source_points,
                source_values,
                target_points,
                RbfType::Gaussian(1.0),
            ),
            _ => Ok(Self::nearest_neighbor(
                source_points,
                source_values,
                target_points,
            )),
        }
    }

    pub fn nearest_neighbor(src: &[Coord3D], vals: &[Scalar], tgt: &[Coord3D]) -> Vec<Scalar> {
        tgt.iter()
            .map(|tp| {
                let mut best_dist = Scalar::MAX;
                let mut best_val = 0.0;
                for (sp, &v) in src.iter().zip(vals.iter()) {
                    let d = tp.distance(sp);
                    if d < best_dist {
                        best_dist = d;
                        best_val = v;
                    }
                }
                best_val
            })
            .collect()
    }

    pub fn inverse_distance_weighted(
        src: &[Coord3D],
        vals: &[Scalar],
        tgt: &[Coord3D],
        power: Scalar,
    ) -> Vec<Scalar> {
        tgt.iter()
            .map(|tp| {
                let mut num = 0.0;
                let mut den = 0.0;
                for (sp, &v) in src.iter().zip(vals.iter()) {
                    let d = tp.distance(sp).max(1e-15);
                    let w = 1.0 / d.powf(power);
                    num += w * v;
                    den += w;
                }
                if den > 0.0 { num / den } else { 0.0 }
            })
            .collect()
    }

    pub fn radial_basis_interpolation(
        src: &[Coord3D],
        vals: &[Scalar],
        tgt: &[Coord3D],
        rbf: RbfType,
    ) -> Result<Vec<Scalar>, String> {
        let n = src.len();
        if n == 0 {
            return Err("Empty source".to_string());
        }
        // Build RBF interpolation matrix
        let mut a = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                let r = src[i].distance(&src[j]);
                a[i][j] = rbf_eval(r, &rbf);
            }
        }
        // Solve A·w = vals using Gaussian elimination
        let w = crate::core::compute::solve_linear(&a, vals)
            .map_err(|e| format!("RBF solve failed: {}", e))?;

        // Evaluate at target points
        let result: Vec<Scalar> = tgt
            .iter()
            .map(|tp| {
                let mut s = 0.0;
                for (sp, &wi) in src.iter().zip(w.iter()) {
                    let r = tp.distance(sp);
                    s += wi * rbf_eval(r, &rbf);
                }
                s
            })
            .collect();
        Ok(result)
    }
}

fn rbf_eval(r: Scalar, rbf: &RbfType) -> Scalar {
    match rbf {
        RbfType::Gaussian(eps) => (-(eps * r).powi(2)).exp(),
        RbfType::Multiquadric(eps) => (1.0 + (eps * r).powi(2)).sqrt(),
        RbfType::ThinPlateSpline => {
            if r > 0.0 {
                r * r * r.ln()
            } else {
                0.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_nearest_neighbor() {
        let src = vec![Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0)];
        let vals = vec![10.0, 20.0];
        let tgt = vec![Coord3D::new(0.0, 0.1, 0.0), Coord3D::new(0.9, 0.0, 0.0)];
        let result = FieldMapper::nearest_neighbor(&src, &vals, &tgt);
        assert!((result[0] - 10.0).abs() < 1e-10);
        assert!((result[1] - 20.0).abs() < 1e-10);
    }
    #[test]
    fn test_inverse_distance_weighted() {
        let src = vec![Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0)];
        let vals = vec![0.0, 10.0];
        let tgt = vec![Coord3D::new(0.5, 0.0, 0.0)];
        let result = FieldMapper::inverse_distance_weighted(&src, &vals, &tgt, 2.0);
        assert!((result[0] - 5.0).abs() < 0.1);
    }
    #[test]
    fn test_map_nearest() {
        let src = vec![Coord3D::new(0.0, 0.0, 0.0)];
        let mapper = FieldMapper::new(FieldMappingMethod::NearestNeighbor);
        let r = mapper
            .map(&src, &[42.0], &[Coord3D::new(0.1, 0.0, 0.0)])
            .unwrap();
        assert!((r[0] - 42.0).abs() < 1e-10);
    }
    #[test]
    fn test_empty_source() {
        let mapper = FieldMapper::new(FieldMappingMethod::NearestNeighbor);
        assert!(mapper.map(&[], &[], &[]).is_err());
    }
    #[test]
    fn test_rbf_gaussian() {
        let v = rbf_eval(0.0, &RbfType::Gaussian(1.0));
        assert!((v - 1.0).abs() < 1e-10);
    }
    #[test]
    fn test_radial_basis_single_point() {
        let src = vec![Coord3D::new(0.0, 0.0, 0.0)];
        let result = FieldMapper::radial_basis_interpolation(
            &src,
            &[5.0],
            &[Coord3D::new(0.5, 0.0, 0.0)],
            RbfType::Gaussian(1.0),
        )
        .unwrap();
        assert!((result[0] - 5.0).abs() < 10.0);
    }
}
