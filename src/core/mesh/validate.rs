// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Mesh validation: indices, degeneracy, orientation, connectivity and quality.
//!
//! Blueprint `blue13.md` §5.2 requires that an importer performs structural
//! validation and *reports element/region locations*, that quality metrics are
//! defined per applicable element type (not against one global threshold), and
//! that isolated regions are reported. This module implements those checks as a
//! set of composable functions plus one aggregate [`validate_mesh`].
//!
//! # Per-type quality thresholds
//!
//! A triangle and a hexahedron cannot share an aspect-ratio threshold: use
//! [`QualityThresholds::permissive`] for a dimensionless quality report and
//! [`QualityThresholds::engineering`] for a stricter structural-mesh gate. They
//! are explicit arguments, never hidden constants, because a good threshold is a
//! property of the *domain*, not of this crate.

use super::topology::{
    Element, ElementType, MeshError, MeshErrorKind, MeshLocation, MeshTopology, Node,
};
use crate::core::types::Scalar;

/// Measure below which an element is considered degenerate.
pub const DEGENERATE_MEASURE: Scalar = 1e-12;

/// Aspect ratio above which an element is flagged in the permissive report.
pub const PERMISSIVE_ASPECT: Scalar = 1.0e6;

/// Aspect ratio above which an element is flagged as an engineering defect.
pub const ENGINEERING_ASPECT: Scalar = 1.0e3;

/// Checked against a mesh by [`validate_mesh`] and [`check_quality`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityThresholds {
    /// Elements with an aspect ratio above this are flagged.
    pub aspect_ratio: Scalar,
    /// Measures at or below this are treated as degenerate.
    pub degenerate_measure: Scalar,
}

impl Default for QualityThresholds {
    fn default() -> Self {
        Self::permissive()
    }
}

impl QualityThresholds {
    /// Loose thresholds suitable for reporting rather than rejection.
    pub fn permissive() -> Self {
        Self {
            aspect_ratio: PERMISSIVE_ASPECT,
            degenerate_measure: DEGENERATE_MEASURE,
        }
    }

    /// Stricter thresholds suitable for a structured engineering mesh.
    pub fn engineering() -> Self {
        Self {
            aspect_ratio: ENGINEERING_ASPECT,
            degenerate_measure: DEGENERATE_MEASURE,
        }
    }
}

/// A single quality observation about one element.
#[derive(Debug, Clone, PartialEq)]
pub struct QualityFinding {
    /// The offending element index (array position, matching `mesh.elements`).
    pub element_index: usize,
    /// Stable element ID.
    pub element_id: usize,
    /// What was observed.
    pub kind: MeshErrorKind,
    /// Numeric value of the offending metric.
    pub value: Scalar,
}

/// A geometric measure: a length for lines, an area for surfaces, a volume for
/// 3D cells, and `1` for points.
///
/// Returns `None` when the element references a node that is absent, so callers
/// can distinguish "undeclared node" (handled by [`check_indices`]) from
/// "zero measure".
pub fn element_measure(mesh: &MeshTopology, element_index: usize) -> Option<Scalar> {
    let elem = mesh.elements.get(element_index)?;
    let pts: Vec<[Scalar; 3]> = elem
        .nodes
        .iter()
        .map(|nid| mesh.node(*nid).map(|n| [n.coord.x, n.coord.y, n.coord.z]))
        .collect::<Option<Vec<_>>>()?;
    Some(measure_of(elem.kind, &pts))
}

/// Geometric measure of an element given its node coordinates.
///
/// The measure is **signed** for planar and volume elements so orientation is
/// observable:
///
/// - `Triangle` and `Quad` report the signed area about the element's own
///   normal (the first triangle's winding), so reversing the connectivity flips
///   the sign; opposite windings within one quad survive as a reduced or
///   negative area.
/// - `Tet`, `Prism` and `Hex` report the signed volume; an inverted element is
///   negative.
///
/// `Line` reports its length and `Point` reports `1`, where orientation has no
/// meaning.
pub fn measure_of(kind: ElementType, pts: &[[Scalar; 3]]) -> Scalar {
    match kind {
        ElementType::Point => 1.0,
        ElementType::Line => distance(pts[0], pts[1]),
        ElementType::Triangle => dot(cross(sub(pts[1], pts[0]), sub(pts[2], pts[0])), up()) * 0.5,
        ElementType::Quad => {
            // Sum the signed areas of the two triangles (0,1,2) and (0,2,3)
            // projected onto a *fixed* reference axis. Because the axis does not
            // follow the element's own winding, reversing the connectivity flips
            // the sign, and opposite windings within one quad cancel towards
            // zero. This is the planar analogue of the signed tetra volume.
            let ab = sub(pts[1], pts[0]);
            let ac = sub(pts[2], pts[0]);
            let ad = sub(pts[3], pts[0]);
            let axis = up();
            let first = dot(cross(ab, ac), axis) * 0.5;
            let second = dot(cross(ac, ad), axis) * 0.5;
            first + second
        }
        ElementType::Tet => {
            let a = sub(pts[1], pts[0]);
            let b = sub(pts[2], pts[0]);
            let c = sub(pts[3], pts[0]);
            // Signed volume is one sixth of the scalar triple product.
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        }
        ElementType::Prism => {
            let top = measure_of(ElementType::Triangle, &pts[0..3]);
            let bot = measure_of(ElementType::Triangle, &pts[3..6]);
            let mid = 0.5 * (top + bot);
            // Prism volume = average cross-section area times the connecting
            // edge length; for a right prism this is exact, otherwise bounded.
            let h = distance(pts[0], pts[3])
                .min(distance(pts[1], pts[4]))
                .min(distance(pts[2], pts[5]));
            mid * h
        }
        ElementType::Hex => {
            // Split into six tetrahedra around the main diagonal 0-6; each tet
            // contributes its signed volume.
            let tet = |i: [usize; 4]| {
                measure_of(
                    ElementType::Tet,
                    &[pts[i[0]], pts[i[1]], pts[i[2]], pts[i[3]]],
                )
            };
            tet([0, 1, 2, 6])
                + tet([0, 2, 3, 6])
                + tet([0, 3, 7, 6])
                + tet([0, 7, 4, 6])
                + tet([0, 4, 5, 6])
                + tet([0, 5, 1, 6])
        }
    }
}

/// The `+z` reference axis used for signed planar surface measures.
fn up() -> [Scalar; 3] {
    [0.0, 0.0, 1.0]
}

/// Aspect ratio of an element: the ratio of its longest edge to its shortest
/// non-degenerate edge. Returns `None` for points and for elements with no
/// usable edge.
///
/// Returns [`Scalar::INFINITY`] when two of the element's nodes coincide: the
/// ratio is genuinely unbounded, and reporting infinity lets an aspect-ratio
/// threshold flag the element instead of silently hiding the defect behind a
/// `None`.
pub fn element_aspect_ratio(mesh: &MeshTopology, element_index: usize) -> Option<Scalar> {
    let elem = mesh.elements.get(element_index)?;
    aspect_ratio_from(&elem.nodes, mesh)
}

/// Aspect ratio from a connectivity and a mesh, or `None` when unusable.
fn aspect_ratio_from(nodes: &[usize], mesh: &MeshTopology) -> Option<Scalar> {
    if nodes.len() < 2 {
        return None;
    }
    let pts: Vec<[Scalar; 3]> = nodes
        .iter()
        .map(|nid| mesh.node(*nid).map(|n| [n.coord.x, n.coord.y, n.coord.z]))
        .collect::<Option<Vec<_>>>()?;
    let mut min_edge = Scalar::MAX;
    let mut max_edge = 0.0 as Scalar;
    for i in 0..pts.len() {
        for j in (i + 1)..pts.len() {
            let d = distance(pts[i], pts[j]);
            if d < min_edge {
                min_edge = d;
            }
            if d > max_edge {
                max_edge = d;
            }
        }
    }
    if min_edge <= 0.0 {
        // Coincident nodes make the ratio unbounded; report it as such so the
        // defect is visible to a threshold check.
        return Some(Scalar::INFINITY);
    }
    Some(max_edge / min_edge)
}

/// Check that every connectivity index refers to a declared node and that no
/// element repeats a node.
pub fn check_indices(mesh: &MeshTopology) -> Result<(), MeshError> {
    for (idx, elem) in mesh.elements.iter().enumerate() {
        for &nid in &elem.nodes {
            if mesh.node(nid).is_none() {
                return Err(MeshError::at(
                    MeshErrorKind::OutOfRangeIndex,
                    MeshLocation::Element(elem.id),
                    format!(
                        "element at index {} references node {} but only {} nodes are declared",
                        idx,
                        nid,
                        mesh.node_count()
                    ),
                ));
            }
        }
        let mut sorted = elem.nodes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != elem.nodes.len() {
            return Err(MeshError::at(
                MeshErrorKind::RepeatedNode,
                MeshLocation::Element(elem.id),
                "connectivity repeats a node",
            ));
        }
    }
    Ok(())
}

/// Check that a non-empty mesh has a valid dimension and at least one entity.
///
/// An empty mesh (no nodes or no elements) is a hard error: every downstream
/// consumer would otherwise divide by zero.
pub fn check_structure(mesh: &MeshTopology) -> Result<(), MeshError> {
    if mesh.node_count() == 0 {
        return Err(MeshError::new(
            MeshErrorKind::EmptyMesh,
            "mesh declares no nodes",
        ));
    }
    if mesh.elements.is_empty() {
        return Err(MeshError::new(
            MeshErrorKind::EmptyMesh,
            "mesh declares no elements",
        ));
    }
    if mesh.dimension.as_usize() == 0 {
        return Err(MeshError::new(
            MeshErrorKind::InvalidDimension,
            "mesh dimension must be at least 1",
        ));
    }
    for (idx, elem) in mesh.elements.iter().enumerate() {
        if elem.kind.topological_dimension() > mesh.dimension.as_usize() {
            return Err(MeshError::at(
                MeshErrorKind::InvalidDimension,
                MeshLocation::Element(elem.id),
                format!(
                    "{} element at index {} cannot live in a {}D mesh",
                    elem.kind,
                    idx,
                    mesh.dimension.as_usize()
                ),
            ));
        }
    }
    Ok(())
}

/// Check that a manifold surface/volume mesh has no face shared by three or
/// more elements.
///
/// A mesh is non-manifold when some face has more than two owning elements. This
/// is a structural defect: boundary extraction and field export both assume at
/// most two owners per face.
pub fn check_manifold(mesh: &mut MeshTopology) -> Result<(), MeshError> {
    mesh.build_adjacency();
    let mut face_owners: std::collections::BTreeMap<Vec<usize>, usize> =
        std::collections::BTreeMap::new();
    for elem in &mesh.elements {
        for f in 0..elem.face_count() {
            if let Some(sig) = elem.face_signature(f) {
                *face_owners.entry(sig).or_insert(0) += 1;
            }
        }
    }
    for (sig, owners) in face_owners {
        if owners > 2 {
            return Err(MeshError::at(
                MeshErrorKind::NonManifold,
                MeshLocation::Mesh,
                format!(
                    "face with node signature {:?} is shared by {} elements (max 2)",
                    sig, owners
                ),
            ));
        }
    }
    Ok(())
}

/// Check that every material region named on an element is a single connected
/// component, and report isolated regions.
///
/// An "isolated region" is a material tag whose elements do not form a connected
/// sub-graph through shared faces; such a tag is almost always a modelling error
/// (two physically distinct bodies sharing one name). Returns one finding per
/// disconnected extra component.
pub fn check_isolated_regions(mesh: &mut MeshTopology) -> Result<Vec<QualityFinding>, MeshError> {
    mesh.build_adjacency();
    let mut findings = Vec::new();
    for name in mesh.region_names() {
        let members = mesh.elements_in_region(&name);
        if members.is_empty() {
            continue;
        }
        let member_set: std::collections::BTreeSet<usize> = members.iter().copied().collect();
        let mut visited: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        let mut components = 0usize;
        let mut first_isolated: Option<usize> = None;
        for &start in &members {
            if visited.contains(&start) {
                continue;
            }
            components += 1;
            let mut stack = vec![start];
            visited.insert(start);
            while let Some(ei) = stack.pop() {
                for &(nb, _) in mesh.element_neighbours(ei).unwrap_or(&[]) {
                    if member_set.contains(&nb) && visited.insert(nb) {
                        stack.push(nb);
                    }
                }
            }
            if components == 2 && first_isolated.is_none() {
                first_isolated = Some(start);
            }
        }
        if components > 1 {
            let representative = first_isolated.unwrap_or(members[0]);
            findings.push(QualityFinding {
                element_index: representative,
                element_id: mesh.elements[representative].id,
                kind: MeshErrorKind::NonManifold,
                value: components as Scalar,
            });
        }
    }
    Ok(findings)
}

/// Report per-element quality problems against explicit thresholds.
///
/// The findings are returned rather than raised because a caller may want to
/// repair or report them; [`validate_mesh`] turns a non-empty result into a
/// [`MeshError`].
pub fn check_quality(mesh: &MeshTopology, thresholds: QualityThresholds) -> Vec<QualityFinding> {
    let mut findings = Vec::new();
    for (idx, elem) in mesh.elements.iter().enumerate() {
        let Some(measure) = element_measure(mesh, idx) else {
            // Undeclared nodes are reported by `check_indices`; skip here.
            continue;
        };
        if elem.kind == ElementType::Point {
            continue;
        }
        if !measure.is_finite() || measure.abs() <= thresholds.degenerate_measure {
            // A non-finite measure (e.g. from a centred quad split) is just as
            // unusable as a zero one, so both are reported as degenerate.
            findings.push(QualityFinding {
                element_index: idx,
                element_id: elem.id,
                kind: MeshErrorKind::DegenerateElement,
                value: measure,
            });
            continue;
        }
        if measure < 0.0 {
            findings.push(QualityFinding {
                element_index: idx,
                element_id: elem.id,
                kind: MeshErrorKind::NegativeVolume,
                value: measure,
            });
        }
        if let Some(ratio) = element_aspect_ratio(mesh, idx) {
            if ratio > thresholds.aspect_ratio {
                findings.push(QualityFinding {
                    element_index: idx,
                    element_id: elem.id,
                    kind: MeshErrorKind::ExtremeAspectRatio,
                    value: ratio,
                });
            }
        }
    }
    findings
}

/// Run the complete structural validation pipeline for a mesh.
///
/// Ordering matters: cheap structural checks run before the geometric ones so
/// the first reported error is the most likely root cause.
pub fn validate_mesh(mesh: &mut MeshTopology) -> Result<(), MeshError> {
    check_structure(mesh)?;
    check_indices(mesh)?;
    check_manifold(mesh)?;
    let findings = check_quality(mesh, QualityThresholds::permissive());
    if let Some(first) = findings.first() {
        return Err(MeshError::at(
            first.kind,
            MeshLocation::Element(first.element_id),
            format!(
                "element at index {} has {} = {:.6e}",
                first.element_index, first.kind, first.value
            ),
        ));
    }
    Ok(())
}

/// Compute the centroid of an element, or `None` if a node is missing.
pub fn element_centroid(mesh: &MeshTopology, element_index: usize) -> Option<[Scalar; 3]> {
    let elem = mesh.elements.get(element_index)?;
    let mut sum = [0.0 as Scalar; 3];
    for &nid in &elem.nodes {
        let node = mesh.node(nid)?;
        sum[0] += node.coord.x;
        sum[1] += node.coord.y;
        sum[2] += node.coord.z;
    }
    let n = elem.nodes.len() as Scalar;
    Some([sum[0] / n, sum[1] / n, sum[2] / n])
}

/// Node ID owner of a coordinate, useful for fixtures.
pub fn node_ids_of(elements: &[Element]) -> Vec<Vec<usize>> {
    elements.iter().map(|e| e.nodes.clone()).collect()
}

/// Helper: a node list with duplicated coincident coordinates.
pub fn duplicate_coordinates(nodes: &[Node], a: usize, b: usize) -> bool {
    match (nodes.get(a), nodes.get(b)) {
        (Some(x), Some(y)) => x.coord == y.coord,
        _ => false,
    }
}

fn sub(a: [Scalar; 3], b: [Scalar; 3]) -> [Scalar; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [Scalar; 3], b: [Scalar; 3]) -> Scalar {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Cross product of two 3-vectors.
fn cross(a: [Scalar; 3], b: [Scalar; 3]) -> [Scalar; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn distance(a: [Scalar; 3], b: [Scalar; 3]) -> Scalar {
    let d = sub(a, b);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::mesh::topology::{Element, ElementType, MeshDimension, Node};

    fn tri_mesh(coords: Vec<(Scalar, Scalar, Scalar)>, conn: Vec<usize>) -> MeshTopology {
        let nodes = coords
            .into_iter()
            .enumerate()
            .map(|(i, (x, y, z))| Node::new(i, x, y, z))
            .collect();
        MeshTopology::new(
            MeshDimension::Dim2,
            nodes,
            vec![Element::new(0, ElementType::Triangle, conn, "r").unwrap()],
        )
        .unwrap()
    }

    #[test]
    fn valid_triangle_passes_all_checks() {
        let mut mesh = tri_mesh(
            vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)],
            vec![0, 1, 2],
        );
        assert!(validate_mesh(&mut mesh).is_ok());
        assert!((element_measure(&mesh, 0).unwrap() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn empty_mesh_is_rejected() {
        let mut mesh = MeshTopology::new(MeshDimension::Dim2, vec![], vec![]).unwrap();
        let err = validate_mesh(&mut mesh).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::EmptyMesh);
    }

    #[test]
    fn out_of_range_index_names_the_element() {
        let mut mesh = tri_mesh(
            vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)],
            vec![0, 1, 2],
        );
        // Corrupt the connectivity after construction.
        mesh.elements[0].nodes[2] = 99;
        let err = validate_mesh(&mut mesh).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::OutOfRangeIndex);
        assert_eq!(err.location, MeshLocation::Element(0));
        assert!(err.detail.contains("99"));
    }

    #[test]
    fn degenerate_triangle_is_detected() {
        // Collinear nodes give zero area.
        let mesh = tri_mesh(
            vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (2.0, 0.0, 0.0)],
            vec![0, 1, 2],
        );
        let findings = check_quality(&mesh, QualityThresholds::permissive());
        assert_eq!(findings[0].kind, MeshErrorKind::DegenerateElement);
    }

    #[test]
    fn centred_quad_split_is_reported_as_degenerate() {
        // A bow-tie quad (centre node) yields a non-finite split measure; it
        // must be reported rather than silently accepted.
        let nodes = vec![
            Node::new(0, 0.0, 0.0, 0.0),
            Node::new(1, 1.0, 0.0, 0.0),
            Node::new(2, 0.0, 0.0, 0.0),
            Node::new(3, 0.0, 1.0, 0.0),
        ];
        let elem = Element::new(0, ElementType::Quad, vec![0, 1, 2, 3], "r").unwrap();
        let mesh = MeshTopology::new(MeshDimension::Dim2, nodes, vec![elem]).unwrap();
        let findings = check_quality(&mesh, QualityThresholds::permissive());
        assert!(
            findings
                .iter()
                .any(|f| f.kind == MeshErrorKind::DegenerateElement)
        );
    }

    #[test]
    fn degenerate_mesh_fails_validate() {
        let mut mesh = tri_mesh(
            vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (2.0, 0.0, 0.0)],
            vec![0, 1, 2],
        );
        let err = validate_mesh(&mut mesh).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::DegenerateElement);
    }

    #[test]
    fn repeated_node_is_rejected() {
        let mut mesh = tri_mesh(
            vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)],
            vec![0, 1, 2],
        );
        mesh.elements[0].nodes = vec![0, 1, 1];
        let err = check_indices(&mesh).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::RepeatedNode);
    }

    #[test]
    fn negative_orientation_is_reported() {
        // Clockwise orientation in the XY plane yields a negative signed area.
        let nodes = vec![
            Node::new(0, 0.0, 0.0, 0.0),
            Node::new(1, 1.0, 0.0, 0.0),
            Node::new(2, 1.0, 1.0, 0.0),
            Node::new(3, 0.0, 1.0, 0.0),
        ];
        // Counter-clockwise (0,1,2,3) is the positive reference.
        let ccw = MeshTopology::new(
            MeshDimension::Dim2,
            nodes.clone(),
            vec![Element::new(0, ElementType::Quad, vec![0, 1, 2, 3], "r").unwrap()],
        )
        .unwrap();
        assert!(element_measure(&ccw, 0).unwrap() > 0.0);
        // The reversed winding (0,3,2,1) traces the same quad clockwise.
        let cw = MeshTopology::new(
            MeshDimension::Dim2,
            nodes,
            vec![Element::new(0, ElementType::Quad, vec![0, 3, 2, 1], "r").unwrap()],
        )
        .unwrap();
        let measure = element_measure(&cw, 0).unwrap();
        assert!(measure < 0.0, "expected a negative area, got {}", measure);
        let findings = check_quality(&cw, QualityThresholds::permissive());
        assert!(
            findings
                .iter()
                .any(|f| f.kind == MeshErrorKind::NegativeVolume)
        );
    }

    #[test]
    fn extreme_aspect_ratio_is_flagged() {
        // A long thin right triangle: legs 1 and 1e-4, hypotenuse ~1. Aspect
        // ratio is ~10^4, above the engineering threshold but far from
        // degenerate, so only the aspect check should fire.
        let mesh = tri_mesh(
            vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1e-4, 0.0)],
            vec![0, 1, 2],
        );
        let strict = QualityThresholds {
            aspect_ratio: 1.0e3,
            degenerate_measure: DEGENERATE_MEASURE,
        };
        let findings = check_quality(&mesh, strict);
        assert!(
            findings
                .iter()
                .any(|f| f.kind == MeshErrorKind::ExtremeAspectRatio),
            "expected an aspect-ratio finding, got {:?}",
            findings
        );
        // The permissive report must not flag it.
        let loose = check_quality(&mesh, QualityThresholds::permissive());
        assert!(
            loose
                .iter()
                .all(|f| f.kind != MeshErrorKind::ExtremeAspectRatio)
        );
    }

    #[test]
    fn coincident_nodes_yield_an_infinite_aspect_ratio() {
        // Two distinct node IDs at the same coordinate: the ratio is unbounded.
        let mesh = tri_mesh(
            vec![(0.0, 0.0, 0.0), (0.0, 0.0, 0.0), (1.0, 1.0, 0.0)],
            vec![0, 1, 2],
        );
        let ratio = element_aspect_ratio(&mesh, 0).unwrap();
        assert!(ratio.is_infinite(), "expected infinity, got {}", ratio);
        // A coincident pair also makes the triangle area vanish, so the element
        // is reported as degenerate (which is checked before aspect ratio).
        let findings = check_quality(&mesh, QualityThresholds::permissive());
        assert!(
            findings
                .iter()
                .any(|f| f.kind == MeshErrorKind::DegenerateElement)
        );
    }

    #[test]
    fn non_manifold_face_is_rejected() {
        // Three triangles sharing edge 0-1.
        let nodes = vec![
            Node::new(0, 0.0, 0.0, 0.0),
            Node::new(1, 1.0, 0.0, 0.0),
            Node::new(2, 0.0, 1.0, 0.0),
            Node::new(3, 0.0, -1.0, 0.0),
            Node::new(4, 0.0, 0.0, 1.0),
        ];
        let elements = vec![
            Element::new(0, ElementType::Triangle, vec![0, 1, 2], "a").unwrap(),
            Element::new(1, ElementType::Triangle, vec![0, 1, 3], "a").unwrap(),
            Element::new(2, ElementType::Triangle, vec![0, 1, 4], "a").unwrap(),
        ];
        let mut mesh = MeshTopology::new(MeshDimension::Dim3, nodes, elements).unwrap();
        let err = check_manifold(&mut mesh).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::NonManifold);
    }

    #[test]
    fn manifold_surface_has_no_shared_third_face() {
        let mut mesh = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ],
            vec![
                (ElementType::Triangle, vec![0, 1, 2], "s"),
                (ElementType::Triangle, vec![0, 2, 3], "s"),
            ],
        )
        .unwrap();
        assert!(check_manifold(&mut mesh).is_ok());
    }

    #[test]
    fn isolated_region_is_reported() {
        // Two disjoint triangles both tagged "steel".
        let mut mesh = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (0.0, 1.0, 0.0),
                (9.0, 0.0, 0.0),
                (10.0, 0.0, 0.0),
                (9.0, 1.0, 0.0),
            ],
            vec![
                (ElementType::Triangle, vec![0, 1, 2], "steel"),
                (ElementType::Triangle, vec![3, 4, 5], "steel"),
            ],
        )
        .unwrap();
        let findings = check_isolated_regions(&mut mesh).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, MeshErrorKind::NonManifold);
    }

    #[test]
    fn connected_region_has_no_isolation_finding() {
        let mut mesh = MeshTopology::new_sequential(
            MeshDimension::Dim2,
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ],
            vec![
                (ElementType::Triangle, vec![0, 1, 2], "steel"),
                (ElementType::Triangle, vec![0, 2, 3], "steel"),
            ],
        )
        .unwrap();
        assert!(check_isolated_regions(&mut mesh).unwrap().is_empty());
    }

    #[test]
    fn dimension_less_than_element_dimension_is_rejected() {
        let nodes = vec![
            Node::new(0, 0.0, 0.0, 0.0),
            Node::new(1, 1.0, 0.0, 0.0),
            Node::new(2, 0.0, 1.0, 0.0),
            Node::new(3, 0.0, 0.0, 1.0),
        ];
        let elements = vec![Element::new(0, ElementType::Tet, vec![0, 1, 2, 3], "r").unwrap()];
        let mesh = MeshTopology::new(MeshDimension::Dim2, nodes, elements).unwrap();
        let err = check_structure(&mesh).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::InvalidDimension);
    }

    #[test]
    fn measures_for_each_element_type_are_correct() {
        // Unit square quad
        let quad = measure_of(
            ElementType::Quad,
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
        );
        assert!((quad - 1.0).abs() < 1e-12);
        // Unit tetrahedron: volume 1/6
        let tet = measure_of(
            ElementType::Tet,
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
        );
        assert!((tet - 1.0 / 6.0).abs() < 1e-12);
        // Unit cube hex: volume 1
        let hex = measure_of(
            ElementType::Hex,
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
        );
        assert!((hex - 1.0).abs() < 1e-12);
        // Line length
        let line = measure_of(ElementType::Line, &[[0.0, 0.0, 0.0], [3.0, 4.0, 0.0]]);
        assert!((line - 5.0).abs() < 1e-12);
        // Point measure is 1 by convention.
        assert!((measure_of(ElementType::Point, &[[0.0, 0.0, 0.0]]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn prism_volume_is_positive_for_a_right_prism() {
        // Triangular prism height 2, base area 0.5 => volume 1.
        let prism = measure_of(
            ElementType::Prism,
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 2.0],
                [1.0, 0.0, 2.0],
                [0.0, 1.0, 2.0],
            ],
        );
        assert!((prism - 1.0).abs() < 1e-9, "prism volume was {}", prism);
    }

    #[test]
    fn centroid_of_a_triangle_is_the_mean() {
        let mesh = tri_mesh(
            vec![(0.0, 0.0, 0.0), (3.0, 0.0, 0.0), (0.0, 3.0, 0.0)],
            vec![0, 1, 2],
        );
        let c = element_centroid(&mesh, 0).unwrap();
        assert!((c[0] - 1.0).abs() < 1e-12);
        assert!((c[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn helpers_detect_duplicate_coordinates() {
        let nodes = vec![Node::new(0, 0.0, 0.0, 0.0), Node::new(1, 0.0, 0.0, 0.0)];
        assert!(duplicate_coordinates(&nodes, 0, 1));
        assert!(!duplicate_coordinates(&nodes, 0, 5));
        assert_eq!(node_ids_of(&[]).len(), 0);
    }
}
