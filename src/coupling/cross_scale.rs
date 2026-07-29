//! Cross-scale coupling: nano → micro → meter → cosmic.

use std::collections::HashMap;
use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use super::bus::FieldData;

/// Scale level hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScaleLevel { Nano, Micro, Milli, Meter, Kilo, Mega, Giga, Tera }

/// Configuration for a cross-scale coupling.
pub struct CrossScaleCoupling {
    pub source_scale: ScaleLevel,
    pub target_scale: ScaleLevel,
    pub homogenization: bool,
    pub localization: bool,
}

/// Bridge for transferring data between scales.
pub struct ScaleBridge {
    pub couplings: Vec<CrossScaleCoupling>,
}

impl ScaleBridge {
    pub fn new() -> Self { Self { couplings: Vec::new() } }
    pub fn add_coupling(&mut self, c: CrossScaleCoupling) { self.couplings.push(c); }

    /// Upscale: fine → coarse (homogenization/averaging).
    pub fn upscale(&self, fine_data: &FieldData, target_points: &[Coord3D], _scale_ratio: Scalar) -> Result<FieldData, String> {
        if fine_data.values.is_empty() { return Err("No fine data".to_string()); }
        let avg = fine_data.values.iter().sum::<Scalar>() / fine_data.values.len() as Scalar;
        let coarse_vals: Vec<Scalar> = target_points.iter().map(|_| avg).collect();
        Ok(FieldData::new(fine_data.field_type, fine_data.quantity, target_points.to_vec(), coarse_vals, fine_data.time))
    }

    /// Downscale: coarse → fine (localization/interpolation).
    pub fn downscale(&self, coarse_data: &FieldData, target_points: &[Coord3D], _scale_ratio: Scalar) -> Result<FieldData, String> {
        if coarse_data.points.is_empty() { return Err("No coarse data".to_string()); }
        // Nearest-neighbor interpolation from coarse to fine
        use super::field_mapping::FieldMapper;
        let mapper = FieldMapper::new(super::bus::FieldMappingMethod::NearestNeighbor);
        let fine_vals = mapper.map(&coarse_data.points, &coarse_data.values, target_points)?;
        Ok(FieldData::new(coarse_data.field_type, coarse_data.quantity, target_points.to_vec(), fine_vals, coarse_data.time))
    }
}

impl Default for ScaleBridge { fn default() -> Self { Self::new() } }

/// RVE homogenization (micro → macro).
pub struct RveHomogenization {
    pub rve_size: Scalar,
    pub micro_fields: Vec<FieldData>,
}

impl RveHomogenization {
    pub fn volume_average(&self) -> FieldData {
        if self.micro_fields.is_empty() {
            return FieldData::new(
                crate::coupling::bus::PhysicsField::Structural,
                crate::coupling::bus::QuantityType::Scalar, vec![], vec![], 0.0);
        }
        let ref_field = &self.micro_fields[0];
        let n = ref_field.values.len();
        let avg_vals: Vec<Scalar> = if n > 0 {
            (0..n).map(|i| self.micro_fields.iter().map(|f| f.values[i]).sum::<Scalar>() / self.micro_fields.len() as Scalar).collect()
        } else { vec![] };
        FieldData::new(ref_field.field_type, ref_field.quantity, ref_field.points.clone(), avg_vals, ref_field.time)
    }

    pub fn effective_properties(&self) -> HashMap<String, Scalar> {
        let mut props = HashMap::new();
        let avg = self.volume_average();
        if !avg.values.is_empty() {
            let mean_val = avg.values.iter().sum::<Scalar>() / avg.values.len() as Scalar;
            props.insert("E_effective".to_string(), mean_val);
        }
        props.insert("rve_size".to_string(), self.rve_size);
        props
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::coord::Coord3D;
    use crate::coupling::bus::{FieldData, PhysicsField, QuantityType};

    #[test]
    fn test_scale_level_order() {
        assert!(ScaleLevel::Nano < ScaleLevel::Micro);
        assert!(ScaleLevel::Meter < ScaleLevel::Kilo);
        assert!(ScaleLevel::Tera > ScaleLevel::Giga);
    }
    #[test]
    fn test_upscale() {
        let bridge = ScaleBridge::new();
        let fine = FieldData::new(PhysicsField::Thermal, QuantityType::Scalar,
            vec![Coord3D::new(0.0,0.0,0.0), Coord3D::new(1.0,0.0,0.0)], vec![100.0, 200.0], 0.0);
        let result = bridge.upscale(&fine, &[Coord3D::new(0.5,0.0,0.0)], 10.0).unwrap();
        assert!((result.values[0] - 150.0).abs() < 1e-10);
    }
    #[test]
    fn test_downscale() {
        let bridge = ScaleBridge::new();
        let coarse = FieldData::new(PhysicsField::Thermal, QuantityType::Scalar,
            vec![Coord3D::new(0.0,0.0,0.0)], vec![100.0], 0.0);
        let result = bridge.downscale(&coarse, &[Coord3D::new(0.5,0.0,0.0)], 0.1).unwrap();
        assert!((result.values[0] - 100.0).abs() < 1e-10);
    }
    #[test]
    fn test_rve_volume_average() {
        let f1 = FieldData::new(PhysicsField::Structural, QuantityType::Scalar, vec![Coord3D::new(0.0,0.0,0.0)], vec![10.0], 0.0);
        let f2 = FieldData::new(PhysicsField::Structural, QuantityType::Scalar, vec![Coord3D::new(0.0,0.0,0.0)], vec![20.0], 0.0);
        let rve = RveHomogenization { rve_size: 1e-6, micro_fields: vec![f1, f2] };
        let avg = rve.volume_average();
        assert!((avg.values[0] - 15.0).abs() < 1e-10);
    }
    #[test]
    fn test_effective_properties() {
        let f1 = FieldData::new(PhysicsField::Structural, QuantityType::Tensor6, vec![Coord3D::new(0.0,0.0,0.0)], vec![1e9], 0.0);
        let rve = RveHomogenization { rve_size: 1e-6, micro_fields: vec![f1] };
        let props = rve.effective_properties();
        assert!(props.contains_key("E_effective"));
        assert!(props.contains_key("rve_size"));
    }
    #[test]
    fn test_empty_rve() {
        let rve = RveHomogenization { rve_size: 1.0, micro_fields: vec![] };
        let avg = rve.volume_average();
        assert!(avg.values.is_empty());
    }
}
