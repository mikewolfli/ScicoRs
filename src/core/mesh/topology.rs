// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Mesh topology: node IDs, element types, connectivity and adjacency.
//!
//! This module defines the *structural* half of the mesh contract described in
//! blueprint `blue13.md` §5.2:
//!
//! - the spatial dimension and coordinate system of the embedding space,
//! - stable node identifiers (IDs independent of array position),
//! - typed elements with explicit connectivity,
//! - derived, verifiable adjacency (node→element and element→element).
//!
//! # Why stable IDs?
//!
//! Boundary conditions in the new contract reference *named regions*, never raw
//! array positions (see [`crate::core::mesh::region`]). Node and element IDs are
//! surfaced here so that regions, fields and the I/O conversion layer all speak
//! the same language, and so that an element can be found by ID rather than by
//! the accident of its index in a `Vec`.

use crate::core::coord::{Coord2D, Coord3D};
use crate::core::types::Scalar;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

/// Dimensionality of the space a mesh is embedded in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MeshDimension {
    /// 1D (embedded in a line).
    Dim1,
    /// 2D (embedded in a plane).
    Dim2,
    /// 3D (ambient space).
    Dim3,
}

impl MeshDimension {
    /// Numeric dimension, `1`, `2` or `3`.
    pub fn as_usize(self) -> usize {
        match self {
            MeshDimension::Dim1 => 1,
            MeshDimension::Dim2 => 2,
            MeshDimension::Dim3 => 3,
        }
    }

    /// Interpret a 1-based numeric dimension.
    pub fn from_usize(dim: usize) -> Result<Self, MeshError> {
        match dim {
            1 => Ok(MeshDimension::Dim1),
            2 => Ok(MeshDimension::Dim2),
            3 => Ok(MeshDimension::Dim3),
            other => Err(MeshError::new(
                MeshErrorKind::InvalidDimension,
                format!(
                    "spatial dimension {} is not supported (expected 1, 2 or 3)",
                    other
                ),
            )),
        }
    }
}

/// Coordinate system of the embedding space.
///
/// This is the coordinate *system* metadata required by the mesh contract and is
/// kept alongside the dimension so imported STEP/STL geometry keeps its frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateSystem {
    /// Orthonormal Cartesian axes.
    Cartesian,
    /// Cylindrical frame `(r, theta, z)`.
    Cylindrical,
    /// Spherical frame `(r, theta, phi)`.
    Spherical,
}

/// Units attached to the node coordinates.
///
/// The mesh itself is unit-agnostic; this records the unit the *coordinates* are
/// expressed in so that a converted mesh can be checked against the source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordinateUnits {
    /// Symbol of the length unit, e.g. `m` or `mm`.
    pub length: String,
}

impl CoordinateUnits {
    /// Metres.
    pub fn meters() -> Self {
        Self {
            length: "m".to_string(),
        }
    }

    /// Millimetres.
    pub fn millimeters() -> Self {
        Self {
            length: "mm".to_string(),
        }
    }

    /// Construct from an arbitrary unit symbol.
    pub fn from_symbol(symbol: &str) -> Self {
        Self {
            length: symbol.to_string(),
        }
    }
}

impl Default for CoordinateUnits {
    fn default() -> Self {
        Self::meters()
    }
}

/// Kind of topological element, with its reference node count.
///
/// The variants cover the minimum set required by blueprint §5.2 plus the prism
/// already produced by the existing VTK reader, so the new contract is a strict
/// superset of `bindings::data_io::mesh_io::MeshElement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElementType {
    /// Zero-dimensional point element (1 node).
    Point,
    /// Two-node line segment.
    Line,
    /// Three-node triangle.
    Triangle,
    /// Four-node quadrilateral.
    Quad,
    /// Four-node tetrahedron.
    Tet,
    /// Six-node wedge/prism.
    Prism,
    /// Eight-node hexahedron.
    Hex,
}

impl ElementType {
    /// Number of nodes referenced by an element of this type.
    pub fn node_count(self) -> usize {
        match self {
            ElementType::Point => 1,
            ElementType::Line => 2,
            ElementType::Triangle => 3,
            ElementType::Quad => 4,
            ElementType::Tet => 4,
            ElementType::Prism => 6,
            ElementType::Hex => 8,
        }
    }

    /// Topological dimension of the element itself (`0` for points, `1` for
    /// lines, `2` for surface elements, `3` for volume elements).
    pub fn topological_dimension(self) -> usize {
        match self {
            ElementType::Point => 0,
            ElementType::Line => 1,
            ElementType::Triangle | ElementType::Quad => 2,
            ElementType::Tet | ElementType::Prism | ElementType::Hex => 3,
        }
    }

    /// All element types, in a stable order — handy for tests and exporters.
    pub fn all() -> &'static [ElementType] {
        &[
            ElementType::Point,
            ElementType::Line,
            ElementType::Triangle,
            ElementType::Quad,
            ElementType::Tet,
            ElementType::Prism,
            ElementType::Hex,
        ]
    }
}

impl fmt::Display for ElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ElementType::Point => "Point",
            ElementType::Line => "Line",
            ElementType::Triangle => "Triangle",
            ElementType::Quad => "Quad",
            ElementType::Tet => "Tet",
            ElementType::Prism => "Prism",
            ElementType::Hex => "Hex",
        };
        f.write_str(name)
    }
}

/// What a mesh error refers to, so a caller can locate the offending entity.
///
/// Every [`MeshError`] carries one of these: validation must never merely say
/// "invalid mesh", it must say *where* (blueprint §5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshLocation {
    /// The mesh as a whole (e.g. an empty or inconsistent global setting).
    Mesh,
    /// A specific node, by ID.
    Node(usize),
    /// A specific element, by ID.
    Element(usize),
    /// A specific face of a specific element.
    Face {
        /// Element the face belongs to.
        element: usize,
        /// Local face index within the element.
        face: usize,
    },
    /// A named region.
    Region(String),
}

impl fmt::Display for MeshLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeshLocation::Mesh => f.write_str("mesh"),
            MeshLocation::Node(id) => write!(f, "node {}", id),
            MeshLocation::Element(id) => write!(f, "element {}", id),
            MeshLocation::Face { element, face } => {
                write!(f, "face {} of element {}", face, element)
            }
            MeshLocation::Region(name) => write!(f, "region '{}'", name),
        }
    }
}

/// Classification of a mesh integrity failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshErrorKind {
    /// Connectivity references a node ID that does not exist.
    OutOfRangeIndex,
    /// Element declares the wrong number of nodes for its type.
    WrongNodeCount,
    /// Two nodes or two elements share an ID.
    DuplicateId,
    /// Spatial dimension is zero or unsupported.
    InvalidDimension,
    /// The mesh has no nodes and/or no elements.
    EmptyMesh,
    /// An element has zero (or near-zero) measure — a degenerate element.
    DegenerateElement,
    /// An element has a negative measure, i.e. inverted orientation.
    NegativeVolume,
    /// An element's aspect ratio exceeds every plausible threshold.
    ExtremeAspectRatio,
    /// A region exists but holds no entities.
    EmptyRegion,
    /// Two regions share a name.
    DuplicateRegion,
    /// A referenced region name is not defined.
    MissingRegion,
    /// An element's connectivity repeats a node.
    RepeatedNode,
    /// A face is shared by more than two elements (non-manifold).
    NonManifold,
    /// A field or mapping request is inconsistent with the mesh.
    FieldMismatch,
    /// A conversion to or from the legacy I/O format is inconsistent.
    ConversionFailure,
}

impl fmt::Display for MeshErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            MeshErrorKind::OutOfRangeIndex => "out-of-range index",
            MeshErrorKind::WrongNodeCount => "wrong node count",
            MeshErrorKind::DuplicateId => "duplicate id",
            MeshErrorKind::InvalidDimension => "invalid dimension",
            MeshErrorKind::EmptyMesh => "empty mesh",
            MeshErrorKind::DegenerateElement => "degenerate element",
            MeshErrorKind::NegativeVolume => "negative measure",
            MeshErrorKind::ExtremeAspectRatio => "extreme aspect ratio",
            MeshErrorKind::EmptyRegion => "empty region",
            MeshErrorKind::DuplicateRegion => "duplicate region",
            MeshErrorKind::MissingRegion => "missing region",
            MeshErrorKind::RepeatedNode => "repeated node in connectivity",
            MeshErrorKind::NonManifold => "non-manifold connectivity",
            MeshErrorKind::FieldMismatch => "field/mesh mismatch",
            MeshErrorKind::ConversionFailure => "format conversion failure",
        };
        f.write_str(text)
    }
}

/// A mesh error carrying the kind of failure and the entity it concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshError {
    /// What went wrong.
    pub kind: MeshErrorKind,
    /// Where it went wrong.
    pub location: MeshLocation,
    /// Human-readable detail.
    pub detail: String,
}

impl MeshError {
    /// Construct an error at a location.
    pub fn new(kind: MeshErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            location: MeshLocation::Mesh,
            detail: detail.into(),
        }
    }

    /// Construct an error tied to a specific location.
    pub fn at(kind: MeshErrorKind, location: MeshLocation, detail: impl Into<String>) -> Self {
        Self {
            kind,
            location,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}: {}", self.kind, self.location, self.detail)
    }
}

impl Error for MeshError {}

/// Provenance of a mesh: where it came from and how it was framed.
///
/// Part of the common mesh contract (blueprint §5.2, "source info"). Kept
/// deliberately small so it can round-trip through the legacy I/O layer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MeshSource {
    /// Human-readable origin, e.g. a file path or `"generated"`.
    pub origin: String,
    /// Format tag, e.g. `"vtk"` or `"stl"`.
    pub format: String,
    /// Dimension of the source model, if it differed from the mesh.
    pub dimension: Option<usize>,
    /// Free-form note (e.g. units the source file declared).
    pub note: Option<String>,
}

impl MeshSource {
    /// A programmatically generated mesh.
    pub fn generated() -> Self {
        Self {
            origin: "generated".to_string(),
            format: "internal".to_string(),
            dimension: None,
            note: None,
        }
    }

    /// A mesh imported from a file.
    pub fn imported(format: &str, origin: &str) -> Self {
        Self {
            origin: origin.to_string(),
            format: format.to_string(),
            dimension: None,
            note: None,
        }
    }
}

/// A node: a stable ID plus its coordinate.
///
/// Coordinates are stored in full 3D even for 1D/2D meshes; the dimension field
/// of the mesh says which components are meaningful.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    /// Stable identifier of this node.
    pub id: usize,
    /// Coordinate of the node in the mesh frame.
    pub coord: Coord3D,
}

impl Node {
    /// Construct a node.
    pub fn new(id: usize, x: Scalar, y: Scalar, z: Scalar) -> Self {
        Self {
            id,
            coord: Coord3D::new(x, y, z),
        }
    }

    /// 2D coordinate, discarding `z`.
    pub fn xy(&self) -> Coord2D {
        Coord2D::new(self.coord.x, self.coord.y)
    }
}

/// A single element: stable ID, type, region tag and node connectivity.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Stable identifier of this element.
    pub id: usize,
    /// Topological type, which fixes the required number of nodes.
    pub kind: ElementType,
    /// Node IDs referenced by this element, in canonical order.
    pub nodes: Vec<usize>,
    /// Name of the material region this element belongs to.
    pub region: String,
}

impl Element {
    /// Construct an element, returning an error if the node count is wrong.
    pub fn new(
        id: usize,
        kind: ElementType,
        nodes: Vec<usize>,
        region: impl Into<String>,
    ) -> Result<Self, MeshError> {
        if nodes.len() != kind.node_count() {
            return Err(MeshError::at(
                MeshErrorKind::WrongNodeCount,
                MeshLocation::Element(id),
                format!(
                    "{} needs {} nodes but {} were given",
                    kind,
                    kind.node_count(),
                    nodes.len()
                ),
            ));
        }
        Ok(Self {
            id,
            kind,
            nodes,
            region: region.into(),
        })
    }

    /// Number of nodes referenced.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of local faces.
    ///
    /// `Point` has none, `Line` two end points treated as two faces (the two
    /// neighbours in a 1D chain), a triangle three edges, etc. This is the count
    /// used by the face-adjacency machinery.
    pub fn face_count(&self) -> usize {
        match self.kind {
            ElementType::Point => 0,
            ElementType::Line => 2,
            ElementType::Triangle => 3,
            ElementType::Quad => 4,
            ElementType::Tet => 4,
            ElementType::Prism => 5,
            ElementType::Hex => 6,
        }
    }

    /// Node IDs of local face `face`, or `None` if out of range.
    pub fn face_nodes(&self, face: usize) -> Option<Vec<usize>> {
        let local: &[&[usize]] = match self.kind {
            ElementType::Point => &[],
            ElementType::Line => &[&[0], &[1]],
            ElementType::Triangle => &[&[0, 1], &[1, 2], &[2, 0]],
            ElementType::Quad => &[&[0, 1], &[1, 2], &[2, 3], &[3, 0]],
            ElementType::Tet => &[&[0, 2, 1], &[0, 1, 3], &[1, 2, 3], &[2, 0, 3]],
            ElementType::Prism => &[
                &[0, 2, 1],
                &[3, 4, 5],
                &[0, 1, 4, 3],
                &[1, 2, 5, 4],
                &[2, 0, 3, 5],
            ],
            ElementType::Hex => &[
                &[0, 3, 2, 1],
                &[4, 5, 6, 7],
                &[0, 1, 5, 4],
                &[1, 2, 6, 5],
                &[2, 3, 7, 6],
                &[3, 0, 4, 7],
            ],
        };
        local
            .get(face)
            .map(|idx| idx.iter().map(|&i| self.nodes[i]).collect())
    }

    /// Face signature: node IDs sorted ascending.
    ///
    /// Two elements share a face when their sorted signatures are equal,
    /// regardless of the winding each uses. This is exactly what adjacency needs.
    pub fn face_signature(&self, face: usize) -> Option<Vec<usize>> {
        let mut sig = self.face_nodes(face)?;
        sig.sort_unstable();
        Some(sig)
    }
}

/// A finite-element mesh: nodes, elements and their derived adjacency.
///
/// Construction is deliberately cheap; call
/// [`crate::core::mesh::validate::validate_mesh`] to run the full structural
/// checks and [`MeshTopology::build_adjacency`] to populate adjacency.
#[derive(Debug, Clone)]
pub struct MeshTopology {
    /// Spatial dimension of the embedding space.
    pub dimension: MeshDimension,
    /// Coordinate system of the node coordinates.
    pub coordinate_system: CoordinateSystem,
    /// Units of the node coordinates.
    pub units: CoordinateUnits,
    /// Nodes in array order; node IDs need not equal their index.
    pub nodes: Vec<Node>,
    /// Elements in array order; element IDs need not equal their index.
    pub elements: Vec<Element>,
    /// Where the mesh came from.
    pub source: MeshSource,
    /// Node ID → index into `nodes`.
    node_index: BTreeMap<usize, usize>,
    /// Element ID → index into `elements`.
    element_index: BTreeMap<usize, usize>,
    /// Node ID → element indices touching that node (`None` until built).
    node_to_elements: Option<Vec<Vec<usize>>>,
    /// Element index → (neighbour element index, shared face index).
    element_neighbours: Option<Vec<Vec<(usize, usize)>>>,
}

impl MeshTopology {
    /// Construct a topology from nodes and elements, building the ID index.
    ///
    /// Duplicate IDs are rejected up-front because every later lookup depends on
    /// the index being a bijection.
    pub fn new(
        dimension: MeshDimension,
        nodes: Vec<Node>,
        elements: Vec<Element>,
    ) -> Result<Self, MeshError> {
        let mut node_index = BTreeMap::new();
        for (idx, node) in nodes.iter().enumerate() {
            if node_index.insert(node.id, idx).is_some() {
                return Err(MeshError::at(
                    MeshErrorKind::DuplicateId,
                    MeshLocation::Node(node.id),
                    "duplicate node id",
                ));
            }
        }
        let mut element_index = BTreeMap::new();
        for (idx, elem) in elements.iter().enumerate() {
            if element_index.insert(elem.id, idx).is_some() {
                return Err(MeshError::at(
                    MeshErrorKind::DuplicateId,
                    MeshLocation::Element(elem.id),
                    "duplicate element id",
                ));
            }
        }
        Ok(Self {
            dimension,
            coordinate_system: CoordinateSystem::Cartesian,
            units: CoordinateUnits::default(),
            nodes,
            elements,
            source: MeshSource::generated(),
            node_index,
            element_index,
            node_to_elements: None,
            element_neighbours: None,
        })
    }

    /// Construct a mesh with sequential IDs equal to array positions.
    pub fn new_sequential(
        dimension: MeshDimension,
        coords: Vec<(Scalar, Scalar, Scalar)>,
        elements: Vec<(ElementType, Vec<usize>, &str)>,
    ) -> Result<Self, MeshError> {
        let nodes = coords
            .into_iter()
            .enumerate()
            .map(|(id, (x, y, z))| Node::new(id, x, y, z))
            .collect();
        let elements = elements
            .into_iter()
            .enumerate()
            .map(|(id, (kind, conn, region))| Element::new(id, kind, conn, region))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(dimension, nodes, elements)
    }

    /// Index of a node by its ID, if present.
    pub fn node_index(&self, id: usize) -> Option<usize> {
        self.node_index.get(&id).copied()
    }

    /// Index of an element by its ID, if present.
    pub fn element_index(&self, id: usize) -> Option<usize> {
        self.element_index.get(&id).copied()
    }

    /// A node by ID.
    pub fn node(&self, id: usize) -> Option<&Node> {
        self.node_index(id).map(|i| &self.nodes[i])
    }

    /// An element by ID.
    pub fn element(&self, id: usize) -> Option<&Element> {
        self.element_index(id).map(|i| &self.elements[i])
    }

    /// Number of nodes (degrees of freedom for a node-centred field).
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of elements (degrees of freedom for a cell-centred field).
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }

    /// All distinct node IDs, ascending.
    pub fn node_ids(&self) -> Vec<usize> {
        self.node_index.keys().copied().collect()
    }

    /// All distinct element IDs, ascending.
    pub fn element_ids(&self) -> Vec<usize> {
        self.element_index.keys().copied().collect()
    }

    /// Set the source metadata (builder style).
    pub fn with_source(mut self, source: MeshSource) -> Self {
        self.source = source;
        self
    }

    /// Set the coordinate system and units (builder style).
    pub fn with_frame(mut self, system: CoordinateSystem, units: CoordinateUnits) -> Self {
        self.coordinate_system = system;
        self.units = units;
        self
    }

    /// Add a node, returning an error if its ID already exists.
    pub fn add_node(&mut self, node: Node) -> Result<(), MeshError> {
        if self.node_index.contains_key(&node.id) {
            return Err(MeshError::at(
                MeshErrorKind::DuplicateId,
                MeshLocation::Node(node.id),
                "duplicate node id",
            ));
        }
        self.node_index.insert(node.id, self.nodes.len());
        self.nodes.push(node);
        self.invalidate_adjacency();
        Ok(())
    }

    /// Add an element, returning an error if its ID already exists.
    pub fn add_element(&mut self, elem: Element) -> Result<(), MeshError> {
        if self.element_index.contains_key(&elem.id) {
            return Err(MeshError::at(
                MeshErrorKind::DuplicateId,
                MeshLocation::Element(elem.id),
                "duplicate element id",
            ));
        }
        self.element_index.insert(elem.id, self.elements.len());
        self.elements.push(elem);
        self.invalidate_adjacency();
        Ok(())
    }

    /// Drop cached adjacency after a mutation.
    fn invalidate_adjacency(&mut self) {
        self.node_to_elements = None;
        self.element_neighbours = None;
    }

    /// Build (or rebuild) node→element and element→element adjacency.
    ///
    /// Adjacency is derived, never stored in the file format: a shared face is
    /// detected purely from the sorted node signature, so it stays correct even
    /// if the importer reorders elements.
    pub fn build_adjacency(&mut self) {
        let mut node_to_elements: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        for (ei, elem) in self.elements.iter().enumerate() {
            for &nid in &elem.nodes {
                if let Some(&ni) = self.node_index.get(&nid) {
                    node_to_elements[ni].push(ei);
                }
            }
        }
        for bucket in &mut node_to_elements {
            bucket.sort_unstable();
            bucket.dedup();
        }

        // Map sorted face signature → list of (element index, local face).
        let mut faces: BTreeMap<Vec<usize>, Vec<(usize, usize)>> = BTreeMap::new();
        for (ei, elem) in self.elements.iter().enumerate() {
            for f in 0..elem.face_count() {
                if let Some(sig) = elem.face_signature(f) {
                    faces.entry(sig).or_default().push((ei, f));
                }
            }
        }
        let mut element_neighbours: Vec<Vec<(usize, usize)>> =
            vec![Vec::new(); self.elements.len()];
        for owners in faces.values() {
            for &(ei, fi) in owners {
                for &(ej, _) in owners {
                    if ej != ei {
                        element_neighbours[ei].push((ej, fi));
                    }
                }
            }
        }
        for bucket in &mut element_neighbours {
            bucket.sort_unstable();
            bucket.dedup();
        }

        self.node_to_elements = Some(node_to_elements);
        self.element_neighbours = Some(element_neighbours);
    }

    /// Element indices touching a node index (requires [`Self::build_adjacency`]).
    pub fn elements_at_node(&self, node_index: usize) -> Option<&[usize]> {
        self.node_to_elements
            .as_ref()
            .and_then(|adj| adj.get(node_index))
            .map(|v| v.as_slice())
    }

    /// Neighbours of an element index: `(neighbour index, shared local face)`
    /// (requires [`Self::build_adjacency`]).
    pub fn element_neighbours(&self, element_index: usize) -> Option<&[(usize, usize)]> {
        self.element_neighbours
            .as_ref()
            .and_then(|adj| adj.get(element_index))
            .map(|v| v.as_slice())
    }

    /// Whether adjacency has been built.
    pub fn has_adjacency(&self) -> bool {
        self.node_to_elements.is_some()
    }

    /// Node indices reachable from a seed node through element connectivity.
    ///
    /// This is the connected-component primitive used to detect isolated
    /// regions. Returns indices into `nodes`, ascending.
    pub fn component_of(&self, seed_index: usize) -> Vec<usize> {
        let mut seen = BTreeSet::new();
        if seed_index >= self.nodes.len() {
            return Vec::new();
        }
        let mut stack = vec![seed_index];
        seen.insert(seed_index);
        while let Some(ni) = stack.pop() {
            for &ei in self.elements_at_node(ni).unwrap_or(&[]) {
                for &nid in &self.elements[ei].nodes {
                    if let Some(&nj) = self.node_index.get(&nid) {
                        if seen.insert(nj) {
                            stack.push(nj);
                        }
                    }
                }
            }
        }
        seen.into_iter().collect()
    }

    /// Partition node indices into connected components.
    ///
    /// Requires adjacency to have been built. The partitions are sorted by their
    /// smallest member, so the result is deterministic.
    pub fn connected_components(&self) -> Vec<Vec<usize>> {
        let mut visited = vec![false; self.nodes.len()];
        let mut components = Vec::new();
        for start in 0..self.nodes.len() {
            if visited[start] {
                continue;
            }
            let component = self.component_of(start);
            for &ni in &component {
                visited[ni] = true;
            }
            if !component.is_empty() {
                components.push(component);
            }
        }
        components
    }

    /// Count of elements whose region name equals `region`.
    pub fn elements_in_region(&self, region: &str) -> Vec<usize> {
        self.elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.region == region)
            .map(|(i, _)| i)
            .collect()
    }

    /// Distinct region names present in the mesh, ascending.
    pub fn region_names(&self) -> Vec<String> {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for e in &self.elements {
            names.insert(e.region.clone());
        }
        names.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_square_quad() -> MeshTopology {
        MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ],
            vec![(ElementType::Quad, vec![0, 1, 2, 3], "solid")],
        )
        .unwrap()
    }

    #[test]
    fn element_type_node_counts_match_variants() {
        for &kind in ElementType::all() {
            let conn: Vec<usize> = (0..kind.node_count()).collect();
            let elem = Element::new(0, kind, conn.clone(), "r").unwrap();
            assert_eq!(elem.node_count(), kind.node_count());
            assert_eq!(elem.nodes, conn);
        }
    }

    #[test]
    fn element_rejects_wrong_node_count() {
        let err = Element::new(0, ElementType::Triangle, vec![0, 1], "r").unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::WrongNodeCount);
        assert_eq!(err.location, MeshLocation::Element(0));
    }

    #[test]
    fn facet_counts_are_topologically_consistent() {
        let expected = [
            (ElementType::Point, 0),
            (ElementType::Line, 2),
            (ElementType::Triangle, 3),
            (ElementType::Quad, 4),
            (ElementType::Tet, 4),
            (ElementType::Prism, 5),
            (ElementType::Hex, 6),
        ];
        for (kind, count) in expected {
            let conn: Vec<usize> = (0..kind.node_count()).collect();
            let elem = Element::new(0, kind, conn, "r").unwrap();
            assert_eq!(elem.face_count(), count, "{}", kind);
            for f in 0..count {
                assert!(elem.face_nodes(f).is_some());
            }
            assert!(elem.face_nodes(count).is_none());
        }
    }

    #[test]
    fn duplicate_node_id_is_rejected() {
        let nodes = vec![Node::new(0, 0.0, 0.0, 0.0), Node::new(0, 1.0, 0.0, 0.0)];
        let err = MeshTopology::new(MeshDimension::Dim2, nodes, vec![]).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::DuplicateId);
        assert_eq!(err.location, MeshLocation::Node(0));
    }

    #[test]
    fn duplicate_element_id_is_rejected() {
        let nodes = vec![
            Node::new(0, 0.0, 0.0, 0.0),
            Node::new(1, 1.0, 0.0, 0.0),
            Node::new(2, 0.0, 1.0, 0.0),
        ];
        let elements = vec![
            Element::new(7, ElementType::Triangle, vec![0, 1, 2], "a").unwrap(),
            Element::new(7, ElementType::Triangle, vec![0, 1, 2], "b").unwrap(),
        ];
        let err = MeshTopology::new(MeshDimension::Dim2, nodes, elements).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::DuplicateId);
        assert_eq!(err.location, MeshLocation::Element(7));
    }

    #[test]
    fn non_sequential_ids_resolve_by_id_not_position() {
        let nodes = vec![
            Node::new(10, 0.0, 0.0, 0.0),
            Node::new(20, 1.0, 0.0, 0.0),
            Node::new(30, 2.0, 0.0, 0.0),
        ];
        let elements = vec![Element::new(99, ElementType::Line, vec![20, 10], "wire").unwrap()];
        let mesh = MeshTopology::new(MeshDimension::Dim1, nodes, elements).unwrap();
        assert_eq!(mesh.node_index(30), Some(2));
        assert_eq!(mesh.node(20).unwrap().coord.x, 1.0);
        assert_eq!(mesh.element(99).unwrap().nodes, vec![20, 10]);
    }

    #[test]
    fn adjacency_detects_shared_quad_edge() {
        let mut mesh = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
                (2.0, 0.0, 0.0),
                (2.0, 1.0, 0.0),
            ],
            vec![
                (ElementType::Quad, vec![0, 1, 2, 3], "left"),
                (ElementType::Quad, vec![1, 4, 5, 2], "right"),
            ],
        )
        .unwrap();
        mesh.build_adjacency();
        assert!(mesh.has_adjacency());
        // Element 0 shares the edge 1-2 with element 1.
        let neighbours = mesh.element_neighbours(0).unwrap();
        assert_eq!(neighbours.len(), 1);
        assert_eq!(neighbours[0].0, 1);
        // Node 1 touches both elements.
        let at_node = mesh.elements_at_node(1).unwrap();
        assert_eq!(at_node, &[0, 1]);
        // A corner node touches only one element.
        assert_eq!(mesh.elements_at_node(0).unwrap(), &[0]);
    }

    #[test]
    fn adjacency_is_refreshed_after_mutation() {
        let mut mesh = unit_square_quad();
        mesh.build_adjacency();
        assert!(mesh.has_adjacency());
        mesh.add_node(Node::new(4, 2.0, 0.0, 0.0)).unwrap();
        mesh.add_element(Element::new(1, ElementType::Triangle, vec![0, 1, 4], "extra").unwrap())
            .unwrap();
        assert!(!mesh.has_adjacency());
        mesh.build_adjacency();
        // The new triangle shares edge 0-1 with the quad, so both are now
        // neighbours of each other.
        assert_eq!(mesh.element_neighbours(1).unwrap().len(), 1);
        assert_eq!(mesh.element_neighbours(1).unwrap()[0].0, 0);
        // The fresh node 4 is touched by the new element only.
        assert_eq!(mesh.elements_at_node(4).unwrap(), &[1]);
    }

    #[test]
    fn connected_components_split_disjoint_triangles() {
        let mut mesh = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (0.0, 1.0, 0.0),
                (10.0, 0.0, 0.0),
                (11.0, 0.0, 0.0),
                (10.0, 1.0, 0.0),
            ],
            vec![
                (ElementType::Triangle, vec![0, 1, 2], "a"),
                (ElementType::Triangle, vec![3, 4, 5], "b"),
            ],
        )
        .unwrap();
        mesh.build_adjacency();
        let comps = mesh.connected_components();
        assert_eq!(comps.len(), 2);
        assert_eq!(comps[0], vec![0, 1, 2]);
        assert_eq!(comps[1], vec![3, 4, 5]);
    }

    #[test]
    fn face_signature_is_winding_independent() {
        let a = Element::new(0, ElementType::Triangle, vec![0, 1, 2], "a").unwrap();
        let b = Element::new(1, ElementType::Triangle, vec![1, 0, 2], "b").unwrap();
        assert_eq!(a.face_signature(0), b.face_signature(0));
    }

    #[test]
    fn region_queries_are_consistent() {
        let mesh = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (0.0, 1.0, 0.0),
                (2.0, 0.0, 0.0),
                (3.0, 0.0, 0.0),
                (2.0, 1.0, 0.0),
            ],
            vec![
                (ElementType::Triangle, vec![0, 1, 2], "steel"),
                (ElementType::Triangle, vec![3, 4, 5], "steel"),
                (ElementType::Triangle, vec![1, 0, 2], "fluid"),
            ],
        )
        .unwrap();
        assert_eq!(mesh.elements_in_region("steel"), vec![0, 1]);
        assert_eq!(mesh.elements_in_region("fluid"), vec![2]);
        assert!(mesh.elements_in_region("missing").is_empty());
        assert_eq!(
            mesh.region_names(),
            vec!["fluid".to_string(), "steel".to_string()]
        );
    }

    #[test]
    fn dimension_round_trips_and_rejects_invalid() {
        for dim in 1..=3 {
            let d = MeshDimension::from_usize(dim).unwrap();
            assert_eq!(d.as_usize(), dim);
        }
        let err = MeshDimension::from_usize(4).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::InvalidDimension);
        let err0 = MeshDimension::from_usize(0).unwrap_err();
        assert_eq!(err0.kind, MeshErrorKind::InvalidDimension);
    }

    #[test]
    fn error_display_mentions_kind_and_location() {
        let err = MeshError::at(
            MeshErrorKind::OutOfRangeIndex,
            MeshLocation::Element(3),
            "node 42 does not exist",
        );
        let text = err.to_string();
        assert!(text.contains("out-of-range index"));
        assert!(text.contains("element 3"));
        assert!(text.contains("node 42 does not exist"));
    }

    #[test]
    fn frame_metadata_can_be_attached() {
        let mesh = unit_square_quad()
            .with_frame(
                CoordinateSystem::Cylindrical,
                CoordinateUnits::millimeters(),
            )
            .with_source(MeshSource::imported("stl", "part.stl"));
        assert_eq!(mesh.coordinate_system, CoordinateSystem::Cylindrical);
        assert_eq!(mesh.units.length, "mm");
        assert_eq!(mesh.source.format, "stl");
        assert_eq!(mesh.source.origin, "part.stl");
    }

    #[test]
    fn hex_face_nodes_are_six_quads() {
        let elem =
            Element::new(0, ElementType::Hex, vec![0, 1, 2, 3, 4, 5, 6, 7], "block").unwrap();
        assert_eq!(elem.face_count(), 6);
        for f in 0..6 {
            assert_eq!(elem.face_nodes(f).unwrap().len(), 4);
        }
    }

    #[test]
    fn tet_and_prism_face_counts_and_sizes() {
        let tet = Element::new(0, ElementType::Tet, vec![0, 1, 2, 3], "t").unwrap();
        assert_eq!(tet.face_count(), 4);
        for f in 0..4 {
            assert_eq!(tet.face_nodes(f).unwrap().len(), 3);
        }
        let prism = Element::new(1, ElementType::Prism, vec![0, 1, 2, 3, 4, 5], "p").unwrap();
        assert_eq!(prism.face_count(), 5);
        assert_eq!(prism.face_nodes(0).unwrap().len(), 3);
        assert_eq!(prism.face_nodes(1).unwrap().len(), 3);
        assert_eq!(prism.face_nodes(2).unwrap().len(), 4);
    }

    #[test]
    fn id_accessors_report_stored_ids() {
        // Two nodes and one triangle; the ID accessors must return exactly the
        // IDs that were inserted, in ascending order.
        let nodes = vec![
            Node::new(5, 0.0, 0.0, 0.0),
            Node::new(2, 1.0, 0.0, 0.0),
            Node::new(9, 0.0, 1.0, 0.0),
        ];
        let elems = vec![Element::new(7, ElementType::Triangle, vec![5, 2, 9], "mat").unwrap()];
        let mesh = MeshTopology::new(MeshDimension::Dim2, nodes, elems).unwrap();
        assert_eq!(mesh.node_ids(), vec![2, 5, 9]);
        assert_eq!(mesh.element_ids(), vec![7]);
        assert_eq!(mesh.node_count(), 3);
        assert_eq!(mesh.element_count(), 1);
    }
}
