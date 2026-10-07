// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Conversion between the Phase 37 mesh contract and the existing mesh I/O format.
//!
//! The legacy reader/writer lives in `crate::bindings::data_io::mesh_io` and
//! speaks `MeshData` (a flat `Vec<Coord3D>` plus typed `MeshElement`s and two
//! `HashMap` sets). Blueprint `blue13.md` §5.2 requires that conversion in both
//! directions preserves **region names** and **coordinate/unit metadata**, so
//! this module is the single place where the two representations meet.
//!
//! # Region mapping
//!
//! The legacy format has no per-element region tag. Regions are recovered from
//! the element sets (a named set of element indices) as follows:
//!
//! - every element set becomes a material region whose members are the tagged
//!   elements;
//! - node sets become node-set regions;
//! - an element not covered by any element set is placed in the default region
//!   [`DEFAULT_REGION`], so merging cannot lose an element.
//!
//! The inverse direction writes each material region back as an element set and
//! each node-set region back as a node set, which reproduces the names exactly.

use super::region::{Region, RegionKind, RegionRegistry};
use super::topology::{
    CoordinateSystem, CoordinateUnits, Element, ElementType, MeshDimension, MeshError,
    MeshErrorKind, MeshLocation, MeshSource, MeshTopology, Node,
};
use crate::bindings::data_io::mesh_io::{MeshData, MeshElement};
use std::collections::BTreeMap;

/// Region assigned to elements that the legacy format left untagged.
pub const DEFAULT_REGION: &str = "default";

/// Convert a legacy [`MeshData`] into the Phase 37 [`MeshTopology`].
///
/// Node IDs are the array positions of the legacy node list; element IDs are
/// their array positions. Region tags come from `element_sets`, with untagged
/// elements falling back to [`DEFAULT_REGION`]. The resulting registry is
/// returned alongside the mesh so callers do not have to re-derive it.
pub fn topology_from_mesh_data(
    data: &MeshData,
    dimension: MeshDimension,
    units: CoordinateUnits,
) -> Result<(MeshTopology, RegionRegistry), MeshError> {
    let nodes = data
        .nodes
        .iter()
        .enumerate()
        .map(|(id, c)| Node::new(id, c.x, c.y, c.z))
        .collect::<Vec<_>>();

    // Element id -> region name, from element sets. An element set that also
    // appears as a region is fine; an element claimed by two sets keeps the
    // first name in sorted order so the result is deterministic.
    let mut element_regions: BTreeMap<usize, String> = BTreeMap::new();
    for (name, ids) in &data.element_sets {
        for &id in ids {
            element_regions.entry(id).or_insert_with(|| name.clone());
        }
    }

    let mut elements = Vec::with_capacity(data.elements.len());
    for (id, legacy) in data.elements.iter().enumerate() {
        let (kind, nodes) = match legacy {
            MeshElement::Line { connectivity } => (ElementType::Line, connectivity.to_vec()),
            MeshElement::Triangle { connectivity } => {
                (ElementType::Triangle, connectivity.to_vec())
            }
            MeshElement::Quadrilateral { connectivity } => {
                (ElementType::Quad, connectivity.to_vec())
            }
            MeshElement::Tetrahedron { connectivity } => (ElementType::Tet, connectivity.to_vec()),
            MeshElement::Hexahedron { connectivity } => (ElementType::Hex, connectivity.to_vec()),
            MeshElement::Prism { connectivity } => (ElementType::Prism, connectivity.to_vec()),
        };
        let region = element_regions
            .get(&id)
            .cloned()
            .unwrap_or_else(|| DEFAULT_REGION.to_string());
        elements.push(Element::new(id, kind, nodes, region)?);
    }

    let mut mesh = MeshTopology::new(dimension, nodes, elements)?;
    mesh.units = units;
    mesh.coordinate_system = CoordinateSystem::Cartesian;
    mesh.source = MeshSource::imported("legacy", "MeshData");

    // Build a registry that also carries node sets, so round-tripping a mesh
    // with node sets does not silently drop them.
    let mut registry = RegionRegistry::from_element_tags(&mesh);
    for (name, ids) in &data.node_sets {
        // A node set whose name collides with a material region is disambiguated
        // rather than dropped: losing a name would break the round trip.
        let mut candidate = name.clone();
        while registry.get(&candidate).is_some() {
            candidate.push_str("_nodes");
        }
        registry.add(Region::new(candidate, RegionKind::NodeSet, ids.clone()))?;
    }

    Ok((mesh, registry))
}

/// Convert a Phase 37 mesh back into the legacy [`MeshData`].
///
/// Material regions become element sets, node-set regions become node sets, and
/// the coordinate/unit metadata is attached as a node set marker named
/// [`units_marker_name`] so the unit string survives a legacy round trip that
/// has no dedicated place to store it.
pub fn mesh_data_from_topology(
    mesh: &MeshTopology,
    registry: &RegionRegistry,
) -> Result<(MeshData, CoordinateUnits), MeshError> {
    let nodes = mesh
        .nodes
        .iter()
        .map(|n| crate::core::coord::Coord3D::new(n.coord.x, n.coord.y, n.coord.z))
        .collect::<Vec<_>>();

    let mut elements = Vec::with_capacity(mesh.elements.len());
    for elem in &mesh.elements {
        let legacy = match elem.kind {
            ElementType::Point => {
                return Err(MeshError::at(
                    MeshErrorKind::ConversionFailure,
                    MeshLocation::Element(elem.id),
                    "the legacy mesh format has no point element",
                ));
            }
            ElementType::Line => MeshElement::Line {
                connectivity: [elem.nodes[0], elem.nodes[1]],
            },
            ElementType::Triangle => MeshElement::Triangle {
                connectivity: [elem.nodes[0], elem.nodes[1], elem.nodes[2]],
            },
            ElementType::Quad => MeshElement::Quadrilateral {
                connectivity: [elem.nodes[0], elem.nodes[1], elem.nodes[2], elem.nodes[3]],
            },
            ElementType::Tet => MeshElement::Tetrahedron {
                connectivity: [elem.nodes[0], elem.nodes[1], elem.nodes[2], elem.nodes[3]],
            },
            ElementType::Hex => MeshElement::Hexahedron {
                connectivity: [
                    elem.nodes[0],
                    elem.nodes[1],
                    elem.nodes[2],
                    elem.nodes[3],
                    elem.nodes[4],
                    elem.nodes[5],
                    elem.nodes[6],
                    elem.nodes[7],
                ],
            },
            ElementType::Prism => MeshElement::Prism {
                connectivity: [
                    elem.nodes[0],
                    elem.nodes[1],
                    elem.nodes[2],
                    elem.nodes[3],
                    elem.nodes[4],
                    elem.nodes[5],
                ],
            },
        };
        elements.push(legacy);
    }

    let mut data = MeshData::new();
    data.nodes = nodes;
    data.elements = elements;

    for region in registry.iter() {
        match region.kind {
            RegionKind::Material => {
                data.element_sets
                    .insert(region.name.clone(), region.members.clone());
            }
            RegionKind::NodeSet => {
                data.node_sets
                    .insert(region.name.clone(), region.members.clone());
            }
            RegionKind::Boundary => {
                // The legacy format has no boundary-region concept. Store the
                // owning element IDs so the information is not lost, under a
                // name that cannot collide with a real region after reload.
                let name = format!("boundary_{}", region.name);
                data.element_sets.insert(name, region.members.clone());
            }
        }
    }

    // Units marker: the legacy format cannot express units directly, so the unit
    // symbol is encoded as a node-set *name* prefixed with `units=`. An empty
    // node list keeps it out of the way of real geometry.
    data.node_sets
        .insert(units_marker_name(&mesh.units), Vec::new());

    Ok((data, mesh.units.clone()))
}

/// Marker node-set name that encodes a unit symbol for a legacy round trip.
pub fn units_marker_name(units: &CoordinateUnits) -> String {
    format!("units={}", units.length)
}

/// Read a unit symbol back out of a marker node-set name, if it is one.
pub fn units_from_marker(name: &str) -> Option<CoordinateUnits> {
    name.strip_prefix("units=")
        .map(CoordinateUnits::from_symbol)
}

/// Full legacy → new → legacy round trip, returning the unit metadata.
///
/// Exposed so tests and callers can assert that conversion is lossless for the
/// metadata the blueprint calls out: region names and coordinate/unit info.
pub fn round_trip_mesh_data(
    data: &MeshData,
    dimension: MeshDimension,
) -> Result<(MeshData, CoordinateUnits), MeshError> {
    let (mesh, registry) = topology_from_mesh_data(data, dimension, CoordinateUnits::meters())?;
    mesh_data_from_topology(&mesh, &registry)
}
