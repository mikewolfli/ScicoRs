//! Unified coupling bus for multi-physics field data exchange.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use std::collections::HashMap;

/// Physics field type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicsField {
    Structural,
    Electromagnetic,
    Thermal,
    Fluid,
    Acoustic,
    Optical,
    Chemical,
    Biological,
    Quantum,
    Gravitational,
    Custom(&'static str),
}

/// Quantity type carried by a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityType {
    Scalar,
    Vector3,
    Tensor6,
    Tensor3x3,
}

/// Time synchronization mode for coupled fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSyncMode {
    LockStep,
    SubCycling,
    Interpolated,
    EventDriven,
}

/// Field mapping method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMappingMethod {
    NearestNeighbor,
    LinearInterp,
    RadialBasis,
    InverseDistance,
    FiniteElementInterp,
    Conservative,
}

/// A coupling interface between two physics fields.
#[derive(Debug, Clone)]
pub struct CouplingInterface {
    pub source_field: PhysicsField,
    pub target_field: PhysicsField,
    pub quantity_type: QuantityType,
    pub mapping: FieldMappingMethod,
    pub time_sync: TimeSyncMode,
}

impl CouplingInterface {
    pub fn new(src: PhysicsField, tgt: PhysicsField, qty: QuantityType) -> Self {
        Self {
            source_field: src,
            target_field: tgt,
            quantity_type: qty,
            mapping: FieldMappingMethod::NearestNeighbor,
            time_sync: TimeSyncMode::LockStep,
        }
    }
    pub fn with_mapping(mut self, m: FieldMappingMethod) -> Self {
        self.mapping = m;
        self
    }
    pub fn with_time_sync(mut self, ts: TimeSyncMode) -> Self {
        self.time_sync = ts;
        self
    }
}

/// Field data container.
#[derive(Debug, Clone)]
pub struct FieldData {
    pub field_type: PhysicsField,
    pub quantity: QuantityType,
    pub points: Vec<Coord3D>,
    pub values: Vec<Scalar>,
    pub time: Scalar,
    pub metadata: HashMap<String, Scalar>,
}

impl FieldData {
    pub fn new(
        field_type: PhysicsField,
        quantity: QuantityType,
        points: Vec<Coord3D>,
        values: Vec<Scalar>,
        time: Scalar,
    ) -> Self {
        Self {
            field_type,
            quantity,
            points,
            values,
            time,
            metadata: HashMap::new(),
        }
    }
    pub fn with_metadata(mut self, key: &str, val: Scalar) -> Self {
        self.metadata.insert(key.to_string(), val);
        self
    }
    pub fn num_points(&self) -> usize {
        self.points.len()
    }
}

/// Unified coupling bus.
pub struct CouplingBus {
    pub interfaces: Vec<CouplingInterface>,
}

impl CouplingBus {
    pub fn new() -> Self {
        Self {
            interfaces: Vec::new(),
        }
    }

    pub fn register_interface(&mut self, interface: CouplingInterface) {
        self.interfaces.push(interface);
    }

    pub fn find_interface(
        &self,
        source: PhysicsField,
        target: PhysicsField,
    ) -> Option<&CouplingInterface> {
        self.interfaces
            .iter()
            .find(|i| i.source_field == source && i.target_field == target)
    }

    pub fn exchange(
        &self,
        source_data: &FieldData,
        interface: &CouplingInterface,
    ) -> Result<FieldData, String> {
        let mapper = super::field_mapping::FieldMapper::new(interface.mapping);
        let target_vals = mapper.map(
            &source_data.points,
            &source_data.values,
            &source_data.points,
        )?;
        Ok(FieldData::new(
            interface.target_field,
            interface.quantity_type,
            source_data.points.clone(),
            target_vals,
            source_data.time,
        ))
    }
}

impl Default for CouplingBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_physics_field_equality() {
        assert_eq!(PhysicsField::Thermal, PhysicsField::Thermal);
        assert_ne!(PhysicsField::Fluid, PhysicsField::Structural);
    }
    #[test]
    fn test_coupling_interface_creation() {
        let ci = CouplingInterface::new(
            PhysicsField::Thermal,
            PhysicsField::Structural,
            QuantityType::Scalar,
        );
        assert_eq!(ci.source_field, PhysicsField::Thermal);
    }
    #[test]
    fn test_field_data_creation() {
        let pts = vec![Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0)];
        let vals = vec![300.0, 310.0];
        let fd = FieldData::new(PhysicsField::Thermal, QuantityType::Scalar, pts, vals, 0.0);
        assert_eq!(fd.num_points(), 2);
    }
    #[test]
    fn test_coupling_bus_register() {
        let mut bus = CouplingBus::new();
        bus.register_interface(CouplingInterface::new(
            PhysicsField::Thermal,
            PhysicsField::Structural,
            QuantityType::Scalar,
        ));
        assert!(
            bus.find_interface(PhysicsField::Thermal, PhysicsField::Structural)
                .is_some()
        );
        assert!(
            bus.find_interface(PhysicsField::Fluid, PhysicsField::Structural)
                .is_none()
        );
    }
    #[test]
    fn test_field_data_metadata() {
        let fd = FieldData::new(
            PhysicsField::Thermal,
            QuantityType::Scalar,
            vec![],
            vec![],
            0.0,
        )
        .with_metadata("mesh_id", 1.0);
        assert_eq!(fd.metadata.get("mesh_id"), Some(&1.0));
    }
}
