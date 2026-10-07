// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Phase 37 — Mesh, boundary regions and field-data workflow.
//!
//! Connects the existing STEP/STL/generic mesh I/O to a verifiable,
//! domain-independent contract for meshes, regions, boundary conditions and
//! fields (blueprint `blue13.md` §5).
//!
//! # Sub-modules
//!
//! - [`topology`] — nodes, elements, element types, connectivity and adjacency.
//! - [`region`] — named material/boundary regions, entity tags and
//!   boundary-condition binding by stable name.
//! - [`validate`] — index, degeneracy, orientation, connectivity, manifold and
//!   per-element-type quality checks.
//! - [`field`] — node/cell/face scalar, vector and tensor fields, plus the
//!   cross-mesh mapping contract with measured conservation error.
//! - [`adapt`] — error indicators, marking, refinement and conservative field
//!   transfer.
//! - [`convert`] — bridge to the existing `bindings::data_io::mesh_io` format.
//!
//! # Contract in one paragraph
//!
//! A [`topology::MeshTopology`] carries a spatial dimension, a coordinate system,
//! units, stable node/element IDs, typed elements with explicit connectivity and
//! provenance. Named [`region::Region`]s are the only way a boundary condition
//! may refer to geometry; [`region::BoundaryCondition::bind`] refuses missing,
//! empty or wrongly-typed regions *before* a solve starts. [`field::FieldData`]
//! records location, component count, unit, timestamp and a per-sample validity
//! mask; [`field::map_scalar_field`] returns a [`field::MappingReport`] with the
//! declared method, coverage and conservation error. [`adapt::adapt_step`] runs
//! estimate → mark → refine → transfer and reports DOF growth and transfer
//! conservation. [`convert`] preserves region names and unit metadata when
//! moving between this contract and the legacy reader/writer.

pub mod adapt;
pub mod convert;
pub mod field;
pub mod region;
pub mod topology;
pub mod validate;

pub use adapt::{
    AdaptationReport, ErrorIndicator, GradientJumpIndicator, MarkDecision, Marker, Refiner,
    ThresholdMarker, TriangleRefiner, adapt_step, transfer_cell_field,
};
pub use convert::{
    DEFAULT_REGION, mesh_data_from_topology, round_trip_mesh_data, topology_from_mesh_data,
    units_from_marker, units_marker_name,
};
pub use field::{FieldData, FieldLocation, MappingMethod, MappingReport, map_scalar_field};
pub use region::{
    BoundaryCondition, BoundaryKind, Region, RegionKind, RegionRegistry, boundary_region,
};
pub use topology::{
    CoordinateSystem, CoordinateUnits, Element, ElementType, MeshDimension, MeshError,
    MeshErrorKind, MeshLocation, MeshSource, MeshTopology, Node,
};
pub use validate::{
    DEGENERATE_MEASURE, ENGINEERING_ASPECT, PERMISSIVE_ASPECT, QualityFinding, QualityThresholds,
    check_indices, check_isolated_regions, check_manifold, check_quality, check_structure,
    element_aspect_ratio, element_centroid, element_measure, measure_of, validate_mesh,
};

#[cfg(test)]
mod integration_tests {
    //! Cross-module workflow tests that exercise the phases together, mirroring
    //! the acceptance list in blueprint `blue13.md` §5.3.

    use super::*;
    use crate::bindings::data_io::mesh_io::{MeshData, MeshElement};
    use crate::core::coord::Coord3D;
    use crate::core::types::Scalar;

    /// A 2×1 rectangle split into two triangles, tagged `steel` and `air`.
    fn two_region_triangles() -> MeshTopology {
        MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
                (2.0, 0.0, 0.0),
            ],
            vec![
                (ElementType::Triangle, vec![0, 1, 2], "steel"),
                (ElementType::Triangle, vec![0, 2, 3], "steel"),
                (ElementType::Triangle, vec![1, 4, 2], "air"),
            ],
        )
        .unwrap()
    }

    #[test]
    fn end_to_end_workflow_validates_regions_maps_and_refines() {
        let mut mesh = two_region_triangles();
        validate_mesh(&mut mesh).expect("fixture must be a valid mesh");

        // Regions derived from tags must audit clean.
        let registry = RegionRegistry::from_element_tags(&mesh);
        registry
            .audit(&mesh)
            .expect("derived registry must be consistent");
        assert_eq!(
            registry.names(),
            vec!["air".to_string(), "steel".to_string()]
        );

        // A boundary condition bound by name resolves to real entities.
        let mut bc_registry = RegionRegistry::new();
        bc_registry
            .add(Region::boundary("left", vec![(1, 2)]))
            .unwrap();
        let bc =
            BoundaryCondition::new("left", BoundaryKind::Dirichlet, 300.0 as Scalar).with_unit("K");
        let bound = bc.bind(&bc_registry).unwrap();
        assert_eq!(bound.len(), 1);

        // A constant cell field maps with zero conservation error.
        let field = FieldData::cell_field(&mesh, "p", 1, "Pa", vec![50.0, 50.0, 50.0]).unwrap();
        let (mapped, report) =
            map_scalar_field(&mesh, &field, &mesh, MappingMethod::CellConstant, "Pa").unwrap();
        assert!(report.is_acceptable(1e-9, 1e-9));
        assert!(report.coverage >= 1.0 - 1e-12);
        assert_eq!(mapped.sample_count(), mesh.element_count());

        // Refine every element and confirm tags survive.
        let decision = MarkDecision {
            refine: vec![0, 1, 2],
            coarsen: vec![],
        };
        let refined = TriangleRefiner::new().refine(&mesh, &decision).unwrap();
        assert_eq!(refined.element_count(), 6);
        assert_eq!(refined.elements_in_region("steel").len(), 4);
        assert_eq!(refined.elements_in_region("air").len(), 2);
    }

    #[test]
    fn legacy_mesh_data_converts_both_ways_preserving_regions() {
        let mut data = MeshData::new();
        data.nodes = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(1.0, 1.0, 0.0),
            Coord3D::new(0.0, 1.0, 0.0),
        ];
        data.elements = vec![
            MeshElement::Triangle {
                connectivity: [0, 1, 2],
            },
            MeshElement::Triangle {
                connectivity: [0, 2, 3],
            },
        ];
        data.element_sets.insert("skin".to_string(), vec![0]);
        data.node_sets
            .insert("corners".to_string(), vec![0, 1, 2, 3]);

        let (mesh, registry) =
            topology_from_mesh_data(&data, MeshDimension::Dim2, CoordinateUnits::millimeters())
                .unwrap();
        assert_eq!(mesh.units.length, "mm");
        assert_eq!(mesh.element_count(), 2);
        // The first element is tagged by the element set, the second defaults.
        assert_eq!(mesh.elements[0].region, "skin");
        assert_eq!(mesh.elements[1].region, DEFAULT_REGION);
        assert_eq!(
            registry.names(),
            vec![
                "corners".to_string(),
                DEFAULT_REGION.to_string(),
                "skin".to_string()
            ]
        );

        // Round trip back to the legacy representation.
        let (back, units) = mesh_data_from_topology(&mesh, &registry).unwrap();
        assert_eq!(units.length, "mm");
        assert_eq!(back.nodes.len(), 4);
        assert_eq!(back.elements.len(), 2);
        assert_eq!(
            back.element_sets.get("skin").map(|v| v.as_slice()),
            Some(&[0usize][..])
        );
        assert!(back.node_sets.contains_key("corners"));
        assert!(back.node_sets.contains_key(&units_marker_name(&units)));
    }

    #[test]
    fn unit_marker_round_trips() {
        let units = CoordinateUnits::from_symbol("ft");
        let marker = units_marker_name(&units);
        assert_eq!(marker, "units=ft");
        assert_eq!(units_from_marker(&marker).unwrap().length, "ft");
        assert!(units_from_marker("not_a_marker").is_none());
    }

    #[test]
    fn every_element_type_has_a_small_fixture() {
        // Line (1D)
        let line = MeshTopology::new_sequential(
            MeshDimension::Dim1,
            vec![(0.0, 0.0, 0.0), (2.0, 0.0, 0.0)],
            vec![(ElementType::Line, vec![0, 1], "wire")],
        )
        .unwrap();
        assert_eq!(line.element_count(), 1);

        // Point (0D)
        let point = MeshTopology::new_sequential(
            MeshDimension::Dim1,
            vec![(0.0, 0.0, 0.0)],
            vec![(ElementType::Point, vec![0], "marker")],
        )
        .unwrap();
        assert_eq!(point.element_count(), 1);

        // Quad + Triangle (2D)
        let quad = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ],
            vec![(ElementType::Quad, vec![0, 1, 2, 3], "plate")],
        )
        .unwrap();
        assert_eq!(quad.element_count(), 1);

        // Tet + Hex (3D)
        let tet = MeshTopology::new_sequential(
            MeshDimension::Dim3,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (0.0, 1.0, 0.0),
                (0.0, 0.0, 1.0),
            ],
            vec![(ElementType::Tet, vec![0, 1, 2, 3], "solid")],
        )
        .unwrap();
        assert_eq!(tet.element_count(), 1);

        let hex = MeshTopology::new_sequential(
            MeshDimension::Dim3,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
                (0.0, 0.0, 1.0),
                (1.0, 0.0, 1.0),
                (1.0, 1.0, 1.0),
                (0.0, 1.0, 1.0),
            ],
            vec![(ElementType::Hex, vec![0, 1, 2, 3, 4, 5, 6, 7], "block")],
        )
        .unwrap();
        assert_eq!(hex.element_count(), 1);

        // Prism (3D)
        let prism = MeshTopology::new_sequential(
            MeshDimension::Dim3,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (0.0, 1.0, 0.0),
                (0.0, 0.0, 1.0),
                (1.0, 0.0, 1.0),
                (0.0, 1.0, 1.0),
            ],
            vec![(ElementType::Prism, vec![0, 1, 2, 3, 4, 5], "wedge")],
        )
        .unwrap();
        assert_eq!(prism.element_count(), 1);
    }

    #[test]
    fn corrupt_mesh_is_rejected_with_location() {
        let mut mesh = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)],
            vec![(ElementType::Triangle, vec![0, 1, 2], "r")],
        )
        .unwrap();
        mesh.elements[0].nodes[1] = 77;
        let err = validate_mesh(&mut mesh).unwrap_err();
        assert_eq!(err.location, MeshLocation::Element(0));
        assert!(err.to_string().contains("element 0"));
    }

    #[test]
    fn missing_boundary_region_is_caught_before_solving() {
        let mesh = two_region_triangles();
        let registry = RegionRegistry::from_element_tags(&mesh);
        let bc = BoundaryCondition::new("outlet", BoundaryKind::Neumann, 0.0 as Scalar);
        // The tag-based registry has no boundary region called "outlet".
        let err = bc.bind(&registry).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::MissingRegion);
        assert_eq!(err.location, MeshLocation::Region("outlet".to_string()));
    }

    #[test]
    fn non_sequential_ids_survive_refinement() {
        let nodes = vec![
            Node::new(100, 0.0, 0.0, 0.0),
            Node::new(200, 1.0, 0.0, 0.0),
            Node::new(300, 0.0, 1.0, 0.0),
        ];
        let elements =
            vec![Element::new(500, ElementType::Triangle, vec![100, 200, 300], "tag").unwrap()];
        let mesh = MeshTopology::new(MeshDimension::Dim2, nodes, elements).unwrap();
        let decision = MarkDecision {
            refine: vec![0],
            coarsen: vec![],
        };
        let refined = TriangleRefiner::new().refine(&mesh, &decision).unwrap();
        // Original IDs preserved, new midpoint gets a fresh ID above the max.
        assert!(refined.node(100).is_some());
        assert!(refined.node(200).is_some());
        assert!(refined.node(300).is_some());
        let max_original = 300;
        assert!(refined.nodes.iter().any(|n| n.id == max_original + 1));
        assert!(
            refined.element(500).is_none(),
            "parent element was replaced"
        );
    }

    #[test]
    fn vector_and_tensor_fields_are_supported() {
        let mesh = two_region_triangles();
        let n = mesh.node_count();
        let vector = FieldData::node_field(&mesh, "u", 3, "m/s", vec![0.0; n * 3]).unwrap();
        assert!(vector.is_vector());
        assert_eq!(vector.sample_count(), n);
        let tensor =
            FieldData::cell_field(&mesh, "sigma", 9, "Pa", vec![0.0; mesh.element_count() * 9])
                .unwrap();
        assert!(tensor.is_tensor());
        assert_eq!(tensor.sample_count(), mesh.element_count());
    }
}
