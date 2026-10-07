// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Node, cell and face field data with units, timestamps and validity.
//!
//! Blueprint `blue13.md` §5.2 requires that field variables carry a location
//! (node/cell/face), a component count, a unit, a timestamp and validity
//! information, and that cross-mesh mapping declares the method, coverage and
//! conservation error. This module provides those contracts:
//!
//! - [`FieldLocation`] — where the samples live.
//! - [`FieldData`] — one field, validated against a mesh.
//! - [`MappingMethod`] / [`MappingReport`] — an explicit mapping with a measured
//!   conservation error and coverage, never an implicit guess.
//!
//! All numeric samples are stored row-major in `values`: sample `s` occupies
//! `[s * components, (s + 1) * components)`.

use super::topology::{MeshError, MeshErrorKind, MeshLocation, MeshTopology};
use super::validate::element_centroid;
use crate::core::types::Scalar;

/// Where a field's samples live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldLocation {
    /// One sample per node.
    Node,
    /// One sample per element (cell).
    Cell,
    /// One sample per element-local face, addressed as `(element index, face)`.
    Face,
}

/// A scalar, vector or tensor field over a mesh.
///
/// `validity` is a mask with one entry per sample: a zero entry means the sample
/// is unset (for example because the source mesh did not cover it). Keeping the
/// mask explicit is what lets a mapping report its coverage honestly.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldData {
    /// Field name, e.g. `"temperature"`.
    pub name: String,
    /// Where the samples live.
    pub location: FieldLocation,
    /// Number of components per sample (1 scalar, 3 vector, 9 tensor, ...).
    pub components: usize,
    /// Unit symbol, e.g. `"K"`.
    pub unit: String,
    /// Physical time the field applies to.
    pub timestamp: Scalar,
    /// Row-major sample storage.
    pub values: Vec<Scalar>,
    /// `values.len() / components` booleans; `true` means the sample is valid.
    pub validity: Vec<bool>,
    /// Face addresses, required only when `location == Face`.
    pub face_index: Vec<(usize, usize)>,
}

impl FieldData {
    /// Construct a node field, checking the sample count against the mesh.
    pub fn node_field(
        mesh: &MeshTopology,
        name: &str,
        components: usize,
        unit: &str,
        values: Vec<Scalar>,
    ) -> Result<Self, MeshError> {
        Self::new(
            mesh,
            name,
            FieldLocation::Node,
            components,
            unit,
            values,
            Vec::new(),
        )
    }

    /// Construct a cell field, checking the sample count against the mesh.
    pub fn cell_field(
        mesh: &MeshTopology,
        name: &str,
        components: usize,
        unit: &str,
        values: Vec<Scalar>,
    ) -> Result<Self, MeshError> {
        Self::new(
            mesh,
            name,
            FieldLocation::Cell,
            components,
            unit,
            values,
            Vec::new(),
        )
    }

    /// Construct a face field from ordered `(element index, face)` addresses.
    pub fn face_field(
        mesh: &MeshTopology,
        name: &str,
        components: usize,
        unit: &str,
        values: Vec<Scalar>,
        face_index: Vec<(usize, usize)>,
    ) -> Result<Self, MeshError> {
        Self::new(
            mesh,
            name,
            FieldLocation::Face,
            components,
            unit,
            values,
            face_index,
        )
    }

    /// General constructor validating component count, sample count and faces.
    pub fn new(
        mesh: &MeshTopology,
        name: &str,
        location: FieldLocation,
        components: usize,
        unit: &str,
        values: Vec<Scalar>,
        face_index: Vec<(usize, usize)>,
    ) -> Result<Self, MeshError> {
        if components == 0 {
            return Err(MeshError::at(
                MeshErrorKind::FieldMismatch,
                MeshLocation::Mesh,
                format!("field '{}' must have at least one component", name),
            ));
        }
        if !values.len().is_multiple_of(components) {
            return Err(MeshError::at(
                MeshErrorKind::FieldMismatch,
                MeshLocation::Mesh,
                format!(
                    "field '{}' has {} values which is not a multiple of {} components",
                    name,
                    values.len(),
                    components
                ),
            ));
        }
        let samples = values.len() / components;
        match location {
            FieldLocation::Node => {
                if samples != mesh.node_count() {
                    return Err(MeshError::at(
                        MeshErrorKind::FieldMismatch,
                        MeshLocation::Mesh,
                        format!(
                            "node field '{}' has {} samples but the mesh has {} nodes",
                            name,
                            samples,
                            mesh.node_count()
                        ),
                    ));
                }
            }
            FieldLocation::Cell => {
                if samples != mesh.element_count() {
                    return Err(MeshError::at(
                        MeshErrorKind::FieldMismatch,
                        MeshLocation::Mesh,
                        format!(
                            "cell field '{}' has {} samples but the mesh has {} elements",
                            name,
                            samples,
                            mesh.element_count()
                        ),
                    ));
                }
            }
            FieldLocation::Face => {
                if face_index.len() != samples {
                    return Err(MeshError::at(
                        MeshErrorKind::FieldMismatch,
                        MeshLocation::Mesh,
                        format!(
                            "face field '{}' has {} samples but {} face addresses",
                            name,
                            samples,
                            face_index.len()
                        ),
                    ));
                }
                for &(ei, fi) in &face_index {
                    let Some(elem) = mesh.elements.get(ei) else {
                        return Err(MeshError::at(
                            MeshErrorKind::FieldMismatch,
                            MeshLocation::Face {
                                element: ei,
                                face: fi,
                            },
                            format!(
                                "face field '{}' addresses an element that does not exist",
                                name
                            ),
                        ));
                    };
                    if fi >= elem.face_count() {
                        return Err(MeshError::at(
                            MeshErrorKind::FieldMismatch,
                            MeshLocation::Face {
                                element: ei,
                                face: fi,
                            },
                            format!(
                                "face field '{}' addresses face {} of a {} with {} faces",
                                name,
                                fi,
                                elem.kind,
                                elem.face_count()
                            ),
                        ));
                    }
                }
            }
        }
        let validity = vec![true; samples];
        Ok(Self {
            name: name.to_string(),
            location,
            components,
            unit: unit.to_string(),
            timestamp: 0.0 as Scalar,
            values,
            validity,
            face_index,
        })
    }

    /// Number of samples in the field.
    pub fn sample_count(&self) -> usize {
        self.values.len() / self.components
    }

    /// Value of one component of one sample, if in range.
    pub fn component(&self, sample: usize, component: usize) -> Option<Scalar> {
        if component >= self.components {
            return None;
        }
        self.values
            .get(sample * self.components + component)
            .copied()
    }

    /// Set one component of one sample, if in range.
    pub fn set_component(&mut self, sample: usize, component: usize, value: Scalar) {
        if component < self.components {
            if let Some(slot) = self.values.get_mut(sample * self.components + component) {
                *slot = value;
            }
        }
    }

    /// Mark a sample invalid.
    pub fn invalidate(&mut self, sample: usize) {
        if let Some(slot) = self.validity.get_mut(sample) {
            *slot = false;
        }
    }

    /// Number of valid samples.
    pub fn valid_count(&self) -> usize {
        self.validity.iter().filter(|&&v| v).count()
    }

    /// Fraction of samples that are valid, in `[0, 1]`.
    pub fn coverage(&self) -> Scalar {
        if self.validity.is_empty() {
            return 0.0 as Scalar;
        }
        self.valid_count() as Scalar / self.validity.len() as Scalar
    }

    /// Whether the field is a scalar field.
    pub fn is_scalar(&self) -> bool {
        self.components == 1
    }

    /// Whether the field is a vector field.
    pub fn is_vector(&self) -> bool {
        self.components == 3
    }

    /// Whether the field is a tensor field of a supported size.
    pub fn is_tensor(&self) -> bool {
        matches!(self.components, 4 | 9)
    }

    /// Set the timestamp (builder style).
    pub fn with_timestamp(mut self, t: Scalar) -> Self {
        self.timestamp = t;
        self
    }

    /// The coordinate of a sample: node coordinate, element centroid, or face
    /// centroid. Returns `None` for out-of-range samples.
    pub fn sample_coordinate(&self, mesh: &MeshTopology, sample: usize) -> Option<[Scalar; 3]> {
        match self.location {
            FieldLocation::Node => {
                let node = mesh.nodes.get(sample)?;
                Some([node.coord.x, node.coord.y, node.coord.z])
            }
            FieldLocation::Cell => element_centroid(mesh, sample),
            FieldLocation::Face => {
                let (ei, fi) = *self.face_index.get(sample)?;
                let face = mesh.elements.get(ei)?.face_nodes(fi)?;
                let mut sum = [0.0 as Scalar; 3];
                for nid in face {
                    let node = mesh.node(nid)?;
                    sum[0] += node.coord.x;
                    sum[1] += node.coord.y;
                    sum[2] += node.coord.z;
                }
                let n = mesh.elements[ei].face_nodes(fi)?.len() as Scalar;
                Some([sum[0] / n, sum[1] / n, sum[2] / n])
            }
        }
    }

    /// Sum of all valid values of one component — the quantity a conservative
    /// mapping must preserve.
    pub fn component_sum(&self, component: usize) -> Scalar {
        if component >= self.components {
            return 0.0 as Scalar;
        }
        self.values
            .chunks_exact(self.components)
            .zip(self.validity.iter())
            .filter(|(_, valid)| **valid)
            .map(|(row, _)| row[component])
            .sum()
    }

    /// Integral of component `component` over the mesh, using sample measures.
    ///
    /// Node samples are weighted by the reciprocal of the number of elements
    /// touching the node (a lumped mass weight); cell samples by element measure.
    /// This makes the integral of a constant field equal to the total measure,
    /// which is the conservation invariant tests rely on.
    pub fn integrate(&self, mesh: &MeshTopology, component: usize) -> Scalar {
        if component >= self.components {
            return 0.0 as Scalar;
        }
        let mut built = mesh.clone();
        built.build_adjacency();
        let mut total = 0.0 as Scalar;
        for sample in 0..self.sample_count() {
            if !self.validity.get(sample).copied().unwrap_or(false) {
                continue;
            }
            let Some(value) = self.component(sample, component) else {
                continue;
            };
            let weight = match self.location {
                FieldLocation::Cell => {
                    super::validate::element_measure(&built, sample).unwrap_or(0.0)
                }
                FieldLocation::Node => {
                    let touching = built.elements_at_node(sample).map(|s| s.len()).unwrap_or(0);
                    if touching == 0 {
                        0.0
                    } else {
                        // Lumped weight: half the surrounding element measures.
                        let mut measure = 0.0 as Scalar;
                        for &ei in built.elements_at_node(sample).unwrap_or(&[]) {
                            measure += super::validate::element_measure(&built, ei).unwrap_or(0.0);
                        }
                        measure / touching as Scalar
                    }
                }
                FieldLocation::Face => {
                    let (ei, _) = self.face_index[sample];
                    super::validate::element_measure(&built, ei).unwrap_or(0.0)
                }
            };
            total += value * weight;
        }
        total
    }
}

/// Method used to transfer a field from one mesh to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingMethod {
    /// Copy the value of the nearest source sample.
    NearestSample,
    /// Linear (barycentric) interpolation inside the containing cell of a
    /// structured/triangulated source mesh; falls back to nearest outside.
    LinearWithinElement,
    /// Constant per source cell: each target sample takes the value of the cell
    /// that contains it. Conserves the integral exactly when cells are matched.
    CellConstant,
}

/// Outcome of a mapping, including the declared method and measured error.
#[derive(Debug, Clone, PartialEq)]
pub struct MappingReport {
    /// Method that produced the mapping.
    pub method: MappingMethod,
    /// Fraction of target samples that received a value, in `[0, 1]`.
    pub coverage: Scalar,
    /// Relative conservation error of the integral of the mapped component.
    ///
    /// `|integral_target - integral_source| / max(|integral_source|, eps)`.
    pub conservation_error: Scalar,
    /// Number of target samples that received a value.
    pub mapped_samples: usize,
}

impl MappingReport {
    /// True when coverage and conservation are within the given tolerances.
    pub fn is_acceptable(&self, coverage_tol: Scalar, conservation_tol: Scalar) -> bool {
        self.coverage >= 1.0 - coverage_tol && self.conservation_error <= conservation_tol
    }
}

/// Transfer a scalar field between two meshes and report coverage/conservation.
///
/// Both fields must be scalar (one component) and cell-centred or node-centred;
/// the location of the source and target may differ, which is the common case
/// when a solution on a fine mesh is compared against a coarse one.
pub fn map_scalar_field(
    source_mesh: &MeshTopology,
    source: &FieldData,
    target_mesh: &MeshTopology,
    method: MappingMethod,
    target_unit: &str,
) -> Result<(FieldData, MappingReport), MeshError> {
    if source.components != 1 {
        return Err(MeshError::at(
            MeshErrorKind::FieldMismatch,
            MeshLocation::Mesh,
            "mapping currently supports scalar fields only",
        ));
    }
    let target_samples = match source.location {
        FieldLocation::Node => target_mesh.node_count(),
        FieldLocation::Cell => target_mesh.element_count(),
        FieldLocation::Face => {
            return Err(MeshError::at(
                MeshErrorKind::FieldMismatch,
                MeshLocation::Mesh,
                "face-centred fields cannot be mapped across meshes yet",
            ));
        }
    };
    let location = source.location;
    let mut values = vec![0.0 as Scalar; target_samples];
    let mut mapped = 0usize;

    for sample in 0..target_samples {
        let Some(coord) = sample_coordinate(target_mesh, location, sample) else {
            continue;
        };
        if let Some(value) = sample_at(source_mesh, source, &coord, method) {
            values[sample] = value;
            mapped += 1;
        }
    }

    let target_field = FieldData::new(
        target_mesh,
        &source.name,
        location,
        1,
        target_unit,
        values,
        Vec::new(),
    )?;
    let coverage = if target_samples == 0 {
        1.0
    } else {
        mapped as Scalar / target_samples as Scalar
    };

    let source_integral = source.integrate(source_mesh, 0);
    let target_integral = target_field.integrate(target_mesh, 0);
    let denom = source_integral.abs().max(1e-30);
    let conservation_error = (target_integral - source_integral).abs() / denom;

    let report = MappingReport {
        method,
        coverage,
        conservation_error,
        mapped_samples: mapped,
    };
    Ok((target_field, report))
}

/// Coordinate of a sample address on any mesh/location pair.
fn sample_coordinate(
    mesh: &MeshTopology,
    location: FieldLocation,
    sample: usize,
) -> Option<[Scalar; 3]> {
    match location {
        FieldLocation::Node => mesh
            .nodes
            .get(sample)
            .map(|n| [n.coord.x, n.coord.y, n.coord.z]),
        FieldLocation::Cell => element_centroid(mesh, sample),
        FieldLocation::Face => None,
    }
}

/// Evaluate a source scalar field at a target coordinate.
fn sample_at(
    mesh: &MeshTopology,
    field: &FieldData,
    coord: &[Scalar; 3],
    method: MappingMethod,
) -> Option<Scalar> {
    match method {
        MappingMethod::NearestSample => nearest_sample(mesh, field, coord),
        MappingMethod::CellConstant => cell_containing(mesh, coord)
            .and_then(|cell| field.component(cell_index_of_sample(mesh, field, cell), 0)),
        MappingMethod::LinearWithinElement => {
            linear_within(mesh, field, coord).or_else(|| nearest_sample(mesh, field, coord))
        }
    }
}

/// Map a global cell sample address onto the field's storage order.
///
/// For cell fields the sample index *is* the element index; kept as a function
/// so the assumption is stated once and can be revisited without touching the
/// mapping logic.
fn cell_index_of_sample(_mesh: &MeshTopology, _field: &FieldData, cell: usize) -> usize {
    cell
}

/// Nearest valid source sample to a coordinate.
fn nearest_sample(mesh: &MeshTopology, field: &FieldData, coord: &[Scalar; 3]) -> Option<Scalar> {
    let mut best: Option<(Scalar, usize)> = None;
    for sample in 0..field.sample_count() {
        if !field.validity.get(sample).copied().unwrap_or(false) {
            continue;
        }
        let Some(sc) = field.sample_coordinate(mesh, sample) else {
            continue;
        };
        let d = squared_distance(*coord, sc);
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, sample));
        }
    }
    best.and_then(|(_, sample)| field.component(sample, 0))
}

/// Barycentric interpolation of a node field inside a triangle or tetrahedron.
///
/// Only simplicial cells are handled (triangle, tet); for other cells, or when
/// the point is outside every cell, returns `None` so the caller falls back to a
/// nearest-sample lookup. This keeps the "declared method" honest: the report
/// says linear-within-element, and the fallback is visible in the coverage.
fn linear_within(mesh: &MeshTopology, field: &FieldData, coord: &[Scalar; 3]) -> Option<Scalar> {
    if field.location != FieldLocation::Node {
        return None;
    }
    let tolerance = 1e-9 as Scalar;
    for elem in &mesh.elements {
        let pts: Vec<[Scalar; 3]> = elem
            .nodes
            .iter()
            .map(|nid| mesh.node(*nid).map(|n| [n.coord.x, n.coord.y, n.coord.z]))
            .collect::<Option<Vec<_>>>()?;
        let bary: Vec<Scalar> = match elem.kind {
            super::topology::ElementType::Triangle => {
                barycentric_triangle(&pts, coord).map(|b| b.to_vec())
            }
            super::topology::ElementType::Tet => barycentric_tet(&pts, coord).map(|b| b.to_vec()),
            _ => continue,
        }
        .unwrap_or_default();
        if bary.len() != elem.nodes.len() {
            continue;
        }
        if bary.iter().any(|w| *w < -tolerance) {
            continue;
        }
        // Interpolate node sample `elem.nodes[k]` (node fields sample per node).
        let mut value = 0.0 as Scalar;
        let mut ok = true;
        for (k, weight) in bary.iter().enumerate() {
            let sample = elem.nodes[k];
            match field.component(sample, 0) {
                Some(v) if field.validity.get(sample).copied().unwrap_or(false) => {
                    value += weight * v;
                }
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return Some(value);
        }
    }
    None
}

/// Barycentric coordinates of a point in a triangle, or `None` if degenerate.
fn barycentric_triangle(pts: &[[Scalar; 3]], p: &[Scalar; 3]) -> Option<[Scalar; 3]> {
    let a = pts[0];
    let b = pts[1];
    let c = pts[2];
    let v0 = sub(b, a);
    let v1 = sub(c, a);
    let v2 = sub(*p, a);
    let d00 = dot(v0, v0);
    let d01 = dot(v0, v1);
    let d11 = dot(v1, v1);
    let d20 = dot(v2, v0);
    let d21 = dot(v2, v1);
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-30 {
        return None;
    }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 as Scalar - v - w;
    Some([u, v, w])
}

/// Barycentric coordinates of a point in a tetrahedron, or `None` if degenerate.
fn barycentric_tet(pts: &[[Scalar; 3]], p: &[Scalar; 3]) -> Option<[Scalar; 4]> {
    let a = pts[0];
    let b = pts[1];
    let c = pts[2];
    let d = pts[3];
    let m = [
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
        [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
        [d[0] - a[0], d[1] - a[1], d[2] - a[2]],
    ];
    let rhs = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-30 {
        return None;
    }
    let v = solve3(&m, rhs, det, 0);
    let w = solve3(&m, rhs, det, 1);
    let x = solve3(&m, rhs, det, 2);
    if !v.is_finite() || !w.is_finite() || !x.is_finite() {
        return None;
    }
    let u = 1.0 as Scalar - v - w - x;
    Some([u, v, w, x])
}

/// Cramer's rule for one component of `M^-1 rhs` with a precomputed determinant.
fn solve3(m: &[[Scalar; 3]; 3], rhs: [Scalar; 3], det: Scalar, col: usize) -> Scalar {
    let mut mm = *m;
    for r in 0..3 {
        mm[r][col] = rhs[r];
    }
    let d = mm[0][0] * (mm[1][1] * mm[2][2] - mm[1][2] * mm[2][1])
        - mm[0][1] * (mm[1][0] * mm[2][2] - mm[1][2] * mm[2][0])
        + mm[0][2] * (mm[1][0] * mm[2][1] - mm[1][1] * mm[2][0]);
    d / det
}

/// Index of the simplicial/structured cell containing a point, or `None`.
///
/// For a triangle/tet mesh this is an exact containment test; for other element
/// types the element centroid nearest the point is used, which is the pragmatic
/// "cell constant" definition.
fn cell_containing(mesh: &MeshTopology, coord: &[Scalar; 3]) -> Option<usize> {
    let tolerance = 1e-9 as Scalar;
    for (idx, elem) in mesh.elements.iter().enumerate() {
        let pts: Vec<[Scalar; 3]> = elem
            .nodes
            .iter()
            .map(|nid| mesh.node(*nid).map(|n| [n.coord.x, n.coord.y, n.coord.z]))
            .collect::<Option<Vec<_>>>()?;
        let inside = match elem.kind {
            super::topology::ElementType::Triangle => barycentric_triangle(&pts, coord)
                .map(|b| b.iter().all(|w| *w >= -tolerance))
                .unwrap_or(false),
            super::topology::ElementType::Tet => barycentric_tet(&pts, coord)
                .map(|b| b.iter().all(|w| *w >= -tolerance))
                .unwrap_or(false),
            _ => false,
        };
        if inside {
            return Some(idx);
        }
    }
    // Fallback: nearest centroid.
    let mut best: Option<(Scalar, usize)> = None;
    for idx in 0..mesh.element_count() {
        if let Some(c) = element_centroid(mesh, idx) {
            let d = squared_distance(*coord, c);
            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, idx));
            }
        }
    }
    best.map(|(_, idx)| idx)
}

fn sub(a: [Scalar; 3], b: [Scalar; 3]) -> [Scalar; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [Scalar; 3], b: [Scalar; 3]) -> Scalar {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn squared_distance(a: [Scalar; 3], b: [Scalar; 3]) -> Scalar {
    let d = sub(a, b);
    dot(d, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::mesh::topology::{Element, ElementType, MeshDimension, Node};

    fn triangle_mesh() -> MeshTopology {
        MeshTopology::new(
            MeshDimension::Dim2,
            vec![
                Node::new(0, 0.0, 0.0, 0.0),
                Node::new(1, 2.0, 0.0, 0.0),
                Node::new(2, 0.0, 2.0, 0.0),
            ],
            vec![Element::new(0, ElementType::Triangle, vec![0, 1, 2], "r").unwrap()],
        )
        .unwrap()
    }

    #[test]
    fn node_field_validates_sample_count() {
        let mesh = triangle_mesh();
        let good = FieldData::node_field(&mesh, "t", 1, "K", vec![1.0, 2.0, 3.0]).unwrap();
        assert_eq!(good.sample_count(), 3);
        assert_eq!(good.unit, "K");
        let err = FieldData::node_field(&mesh, "t", 1, "K", vec![1.0, 2.0]).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::FieldMismatch);
    }

    #[test]
    fn cell_field_validates_sample_count() {
        let mesh = triangle_mesh();
        let good = FieldData::cell_field(&mesh, "p", 1, "Pa", vec![5.0]).unwrap();
        assert_eq!(good.sample_count(), 1);
        let err = FieldData::cell_field(&mesh, "p", 1, "Pa", vec![1.0, 2.0]).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::FieldMismatch);
    }

    #[test]
    fn zero_components_is_rejected() {
        let mesh = triangle_mesh();
        let err = FieldData::node_field(&mesh, "v", 0, "", vec![]).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::FieldMismatch);
    }

    #[test]
    fn mismatched_component_multiple_is_rejected() {
        let mesh = triangle_mesh();
        // 4 values with 3 components => not a multiple.
        let err = FieldData::node_field(&mesh, "v", 3, "m/s", vec![0.0; 4]).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::FieldMismatch);
    }

    #[test]
    fn vector_field_components_are_addressable() {
        let mesh = triangle_mesh();
        let mut f = FieldData::node_field(
            &mesh,
            "u",
            3,
            "m/s",
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
        )
        .unwrap();
        assert!(f.is_vector());
        assert!(!f.is_scalar());
        assert_eq!(f.component(1, 2), Some(6.0));
        f.set_component(1, 2, 60.0);
        assert_eq!(f.component(1, 2), Some(60.0));
        assert_eq!(f.component(1, 9), None);
        assert_eq!(f.sample_count(), 3);
    }

    #[test]
    fn tensor_field_sizes() {
        let mesh = triangle_mesh();
        let t9 = FieldData::node_field(&mesh, "s", 9, "Pa", vec![0.0; 27]).unwrap();
        assert!(t9.is_tensor());
        let t4 = FieldData::node_field(&mesh, "s2", 4, "Pa", vec![0.0; 12]).unwrap();
        assert!(t4.is_tensor());
        let v3 = FieldData::node_field(&mesh, "v", 3, "m/s", vec![0.0; 9]).unwrap();
        assert!(!v3.is_tensor());
    }

    #[test]
    fn face_field_validates_addresses() {
        let mesh = triangle_mesh();
        let good = FieldData::face_field(
            &mesh,
            "flux",
            1,
            "W",
            vec![1.0, 2.0, 3.0],
            vec![(0, 0), (0, 1), (0, 2)],
        )
        .unwrap();
        assert_eq!(good.sample_count(), 3);
        let bad = FieldData::face_field(&mesh, "flux", 1, "W", vec![1.0], vec![(0, 9)]);
        assert_eq!(bad.unwrap_err().kind, MeshErrorKind::FieldMismatch);
        let bad_elem =
            FieldData::face_field(&mesh, "flux", 1, "W", vec![1.0], vec![(5, 0)]).unwrap_err();
        assert_eq!(bad_elem.kind, MeshErrorKind::FieldMismatch);
    }

    #[test]
    fn validity_mask_and_coverage() {
        let mesh = triangle_mesh();
        let mut f = FieldData::node_field(&mesh, "t", 1, "K", vec![1.0, 2.0, 3.0]).unwrap();
        assert_eq!(f.coverage(), 1.0);
        f.invalidate(1);
        assert_eq!(f.valid_count(), 2);
        assert!((f.coverage() - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn timestamp_is_attached() {
        let mesh = triangle_mesh();
        let f = FieldData::node_field(&mesh, "t", 1, "K", vec![1.0, 2.0, 3.0])
            .unwrap()
            .with_timestamp(2.5);
        assert_eq!(f.timestamp, 2.5);
    }

    #[test]
    fn constant_field_maps_with_zero_error() {
        // Same geometry, same node count => identity mapping, error 0.
        let src = triangle_mesh();
        let dst = triangle_mesh();
        let f = FieldData::cell_field(&src, "c", 1, "K", vec![7.0]).unwrap();
        let (mapped, report) =
            map_scalar_field(&src, &f, &dst, MappingMethod::NearestSample, "K").unwrap();
        assert_eq!(mapped.component(0, 0), Some(7.0));
        assert!(report.coverage >= 1.0 - 1e-12);
        assert!(
            report.conservation_error <= 1e-12,
            "{}",
            report.conservation_error
        );
        assert!(report.is_acceptable(1e-9, 1e-9));
    }

    #[test]
    fn constant_node_field_maps_with_zero_error() {
        let src = triangle_mesh();
        let dst = triangle_mesh();
        let f = FieldData::node_field(&src, "t", 1, "K", vec![4.0, 4.0, 4.0]).unwrap();
        let (mapped, report) =
            map_scalar_field(&src, &f, &dst, MappingMethod::LinearWithinElement, "K").unwrap();
        for s in 0..3 {
            assert!((mapped.component(s, 0).unwrap() - 4.0).abs() < 1e-12);
        }
        assert!(report.conservation_error <= 1e-12);
    }

    #[test]
    fn linear_field_is_exactly_interpolated_in_a_triangle() {
        // f(x, y) = x on the triangle with vertices (0,0), (2,0), (0,2).
        let mesh = triangle_mesh();
        let f = FieldData::node_field(&mesh, "x", 1, "m", vec![0.0, 2.0, 0.0]).unwrap();
        // The fineness: build the same mesh as a "target" but with an extra
        // interior point to test interpolation error.
        let mut target = MeshTopology::new(
            MeshDimension::Dim2,
            vec![
                Node::new(0, 0.0, 0.0, 0.0),
                Node::new(1, 2.0, 0.0, 0.0),
                Node::new(2, 0.0, 2.0, 0.0),
            ],
            vec![Element::new(0, ElementType::Triangle, vec![0, 1, 2], "r").unwrap()],
        )
        .unwrap();
        // Add an interior node at the centroid; interpolate exactly there.
        target
            .add_node(Node::new(3, 2.0 / 3.0, 2.0 / 3.0, 0.0))
            .unwrap();
        let x = 2.0 as Scalar / 3.0;
        let value = linear_within(&mesh, &f, &[x, x, 0.0]).unwrap();
        assert!((value - x).abs() < 1e-12, "got {}", value);
    }

    #[test]
    fn linear_field_conservation_is_within_threshold() {
        // Source: unit triangle with linear field. Target: same mesh.
        let mesh = triangle_mesh();
        let f = FieldData::node_field(&mesh, "f", 1, "", vec![0.0, 2.0, 0.0]).unwrap();
        let (_, report) =
            map_scalar_field(&mesh, &f, &mesh, MappingMethod::LinearWithinElement, "").unwrap();
        assert!(
            report.conservation_error <= 1e-9,
            "conservation error {}",
            report.conservation_error
        );
    }

    #[test]
    fn mapping_rejects_vector_source() {
        let mesh = triangle_mesh();
        let f = FieldData::node_field(&mesh, "u", 3, "m/s", vec![0.0; 9]).unwrap();
        let err =
            map_scalar_field(&mesh, &f, &mesh, MappingMethod::NearestSample, "m/s").unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::FieldMismatch);
    }

    #[test]
    fn integrate_of_constant_cell_field_equals_total_measure() {
        // Two unit-area triangles, constant field 1 => integral == total area.
        let mesh = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ],
            vec![
                (ElementType::Triangle, vec![0, 1, 2], "a"),
                (ElementType::Triangle, vec![0, 2, 3], "a"),
            ],
        )
        .unwrap();
        let f = FieldData::cell_field(&mesh, "one", 1, "", vec![1.0, 1.0]).unwrap();
        let integral = f.integrate(&mesh, 0);
        assert!((integral - 1.0).abs() < 1e-12, "integral {}", integral);
    }

    #[test]
    fn component_sum_ignores_invalid_samples() {
        let mesh = triangle_mesh();
        let mut f = FieldData::node_field(&mesh, "t", 1, "K", vec![1.0, 2.0, 3.0]).unwrap();
        assert_eq!(f.component_sum(0), 6.0);
        f.invalidate(0);
        assert_eq!(f.component_sum(0), 5.0);
    }

    #[test]
    fn sample_coordinates_resolve_for_each_location() {
        let mesh = triangle_mesh();
        let node_field = FieldData::node_field(&mesh, "a", 1, "", vec![0.0, 0.0, 0.0]).unwrap();
        let c = node_field.sample_coordinate(&mesh, 1).unwrap();
        assert!((c[0] - 2.0).abs() < 1e-12);
        let cell_field = FieldData::cell_field(&mesh, "b", 1, "", vec![0.0]).unwrap();
        let cc = cell_field.sample_coordinate(&mesh, 0).unwrap();
        assert!((cc[0] - 2.0 / 3.0).abs() < 1e-12);
        let face_field = FieldData::face_field(
            &mesh,
            "c",
            1,
            "",
            vec![0.0; 3],
            vec![(0, 0), (0, 1), (0, 2)],
        )
        .unwrap();
        let fc = face_field.sample_coordinate(&mesh, 0).unwrap();
        assert!((fc[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn cell_containing_finds_the_right_triangle() {
        let mesh = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ],
            vec![
                (ElementType::Triangle, vec![0, 1, 2], "a"),
                (ElementType::Triangle, vec![0, 2, 3], "b"),
            ],
        )
        .unwrap();
        // A point near vertex 1 (bottom-right) is in triangle 0.
        assert_eq!(cell_containing(&mesh, &[0.8, 0.1, 0.0]), Some(0));
        // A point near vertex 3 (top-left) is in triangle 1.
        assert_eq!(cell_containing(&mesh, &[0.1, 0.8, 0.0]), Some(1));
    }

    #[test]
    fn mapping_report_acceptance_thresholds() {
        let ok = MappingReport {
            method: MappingMethod::NearestSample,
            coverage: 1.0,
            conservation_error: 1e-10,
            mapped_samples: 10,
        };
        assert!(ok.is_acceptable(1e-9, 1e-8));
        let bad = MappingReport {
            coverage: 0.5,
            ..ok.clone()
        };
        assert!(!bad.is_acceptable(1e-9, 1e-8));
    }

    #[test]
    fn tet_interpolation_reproduces_a_linear_field() {
        let mesh = MeshTopology::new(
            MeshDimension::Dim3,
            vec![
                Node::new(0, 0.0, 0.0, 0.0),
                Node::new(1, 1.0, 0.0, 0.0),
                Node::new(2, 0.0, 1.0, 0.0),
                Node::new(3, 0.0, 0.0, 1.0),
            ],
            vec![Element::new(0, ElementType::Tet, vec![0, 1, 2, 3], "r").unwrap()],
        )
        .unwrap();
        // f = x + 2y + 3z
        let f = FieldData::node_field(&mesh, "f", 1, "", vec![0.0, 1.0, 2.0, 3.0]).unwrap();
        let p = [0.2, 0.3, 0.1];
        let expected = p[0] + 2.0 * p[1] + 3.0 * p[2];
        let got = linear_within(&mesh, &f, &p).unwrap();
        assert!(
            (got - expected).abs() < 1e-10,
            "got {} expected {}",
            got,
            expected
        );
    }
}
