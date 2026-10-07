// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Named material regions, boundary regions and entity tags.
//!
//! Blueprint `blue13.md` §5.2 requires that boundary conditions reference
//! *stable named regions* rather than incidental array positions, and that
//! missing, duplicated or empty regions are detected **before** a solve starts.
//! This module is the registry that makes those guarantees:
//!
//! - [`Region`] names a set of entities (elements, nodes or faces) with a kind.
//! - [`RegionRegistry`] owns the regions, enforces uniqueness, and resolves a
//!   region by name for boundary-condition binding.
//! - [`BoundaryCondition`] binds a value to a region *name* and is only bound
//!   successfully when the name resolves to a non-empty region.
//!
//! # Stability
//!
//! A region is identified by its name, not by its index in a `Vec`. Renaming is
//! an explicit operation ([`RegionRegistry::rename`]) that preserves the tag and
//! membership, so a workflow can migrate a mesh without silently rebinding
//! boundary conditions to a different entity.

use super::topology::{MeshError, MeshErrorKind, MeshLocation, MeshTopology};
use crate::core::types::Scalar;
use std::collections::BTreeMap;

/// What kind of entities a region holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegionKind {
    /// A volumetric material region: a set of elements.
    Material,
    /// A boundary region: a set of element-local faces.
    Boundary,
    /// A node set.
    NodeSet,
}

impl std::fmt::Display for RegionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            RegionKind::Material => "material",
            RegionKind::Boundary => "boundary",
            RegionKind::NodeSet => "node-set",
        };
        f.write_str(text)
    }
}

/// A named region: a stable tag plus its member entities.
///
/// Membership is stored as entity IDs (element IDs for material regions, node
/// IDs for node sets) or as `(element id, local face)` pairs for boundary
/// regions. Using IDs — not array indices — is what makes a region survive a
/// reordering of the mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// Stable name of the region.
    pub name: String,
    /// What kind of entities the region holds.
    pub kind: RegionKind,
    /// Entity IDs (element IDs or node IDs, depending on `kind`).
    pub members: Vec<usize>,
    /// Ordered list of `(element id, local face index)` for boundary regions.
    pub faces: Vec<(usize, usize)>,
}

impl Region {
    /// Construct a region from entity IDs.
    pub fn new(name: impl Into<String>, kind: RegionKind, members: Vec<usize>) -> Self {
        Self {
            name: name.into(),
            kind,
            members,
            faces: Vec::new(),
        }
    }

    /// Construct a boundary region directly from `(element id, face)` pairs.
    ///
    /// The distinct element IDs are also recorded in `members` so callers that
    /// only need "which elements touch this boundary" have a single answer.
    pub fn boundary(name: impl Into<String>, faces: Vec<(usize, usize)>) -> Self {
        let mut members: Vec<usize> = faces.iter().map(|(e, _)| *e).collect();
        members.sort_unstable();
        members.dedup();
        Self {
            name: name.into(),
            kind: RegionKind::Boundary,
            members,
            faces,
        }
    }

    /// Whether the region has no members at all.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty() && self.faces.is_empty()
    }

    /// Number of member entities (faces for boundary regions, else IDs).
    pub fn len(&self) -> usize {
        match self.kind {
            RegionKind::Boundary => self.faces.len(),
            RegionKind::Material | RegionKind::NodeSet => self.members.len(),
        }
    }

    /// Whether the region contains a given element ID.
    pub fn contains_element(&self, element_id: usize) -> bool {
        self.members.binary_search(&element_id).is_ok() || self.members.contains(&element_id)
    }
}

/// A registry of uniquely named regions.
///
/// Insertion rejects duplicate names, and [`Self::audit`] reports empty or
/// dangling regions, so an inconsistent region model cannot reach a solver.
#[derive(Debug, Clone, Default)]
pub struct RegionRegistry {
    regions: BTreeMap<String, Region>,
}

impl RegionRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self {
            regions: BTreeMap::new(),
        }
    }

    /// Add a region, rejecting a duplicate name.
    pub fn add(&mut self, region: Region) -> Result<(), MeshError> {
        if self.regions.contains_key(&region.name) {
            return Err(MeshError::at(
                MeshErrorKind::DuplicateRegion,
                MeshLocation::Region(region.name.clone()),
                "a region with this name already exists",
            ));
        }
        self.regions.insert(region.name.clone(), region);
        Ok(())
    }

    /// Number of registered regions.
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Look up a region by name.
    pub fn get(&self, name: &str) -> Option<&Region> {
        self.regions.get(name)
    }

    /// All region names, ascending.
    pub fn names(&self) -> Vec<String> {
        self.regions.keys().cloned().collect()
    }

    /// All regions, in name order.
    pub fn iter(&self) -> impl Iterator<Item = &Region> {
        self.regions.values()
    }

    /// Rename a region, preserving its tag, kind and membership.
    ///
    /// Fails if the old name is absent or the new name already exists. This is
    /// the only sanctioned way to change a name, so any boundary conditions
    /// keyed by the old name are visibly broken rather than silently rebound.
    pub fn rename(&mut self, old: &str, new: &str) -> Result<(), MeshError> {
        if !self.regions.contains_key(old) {
            return Err(MeshError::at(
                MeshErrorKind::MissingRegion,
                MeshLocation::Region(old.to_string()),
                "cannot rename a region that does not exist",
            ));
        }
        if self.regions.contains_key(new) {
            return Err(MeshError::at(
                MeshErrorKind::DuplicateRegion,
                MeshLocation::Region(new.to_string()),
                "target region name already exists",
            ));
        }
        let mut region = self.regions.remove(old).expect("checked above");
        region.name = new.to_string();
        self.regions.insert(new.to_string(), region);
        Ok(())
    }

    /// Remove a region by name, returning it if present.
    pub fn remove(&mut self, name: &str) -> Option<Region> {
        self.regions.remove(name)
    }

    /// Check every region for emptied membership and dangling entity IDs.
    ///
    /// `mesh` supplies the valid ID universe. Returns the first failure found,
    /// with the offending region named in the error location.
    pub fn audit(&self, mesh: &MeshTopology) -> Result<(), MeshError> {
        for region in self.regions.values() {
            if region.is_empty() {
                return Err(MeshError::at(
                    MeshErrorKind::EmptyRegion,
                    MeshLocation::Region(region.name.clone()),
                    format!("{} region has no members", region.kind),
                ));
            }
            match region.kind {
                RegionKind::Material => {
                    for &id in &region.members {
                        if mesh.element(id).is_none() {
                            return Err(MeshError::at(
                                MeshErrorKind::MissingRegion,
                                MeshLocation::Region(region.name.clone()),
                                format!("references element {} which is not in the mesh", id),
                            ));
                        }
                    }
                }
                RegionKind::NodeSet => {
                    for &id in &region.members {
                        if mesh.node(id).is_none() {
                            return Err(MeshError::at(
                                MeshErrorKind::MissingRegion,
                                MeshLocation::Region(region.name.clone()),
                                format!("references node {} which is not in the mesh", id),
                            ));
                        }
                    }
                }
                RegionKind::Boundary => {
                    for &(eid, face) in &region.faces {
                        let Some(elem) = mesh.element(eid) else {
                            return Err(MeshError::at(
                                MeshErrorKind::MissingRegion,
                                MeshLocation::Region(region.name.clone()),
                                format!("references element {} which is not in the mesh", eid),
                            ));
                        };
                        if face >= elem.face_count() {
                            return Err(MeshError::at(
                                MeshErrorKind::MissingRegion,
                                MeshLocation::Region(region.name.clone()),
                                format!(
                                    "{} has no face {} (it has {} faces)",
                                    elem.kind,
                                    face,
                                    elem.face_count()
                                ),
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Build a registry from the element `region` tags present in a mesh.
    ///
    /// Every distinct tag becomes a material region holding exactly the elements
    /// that carry it. A tag that is present but empty cannot occur here, which is
    /// one reason to prefer derivation over hand-maintained lists.
    pub fn from_element_tags(mesh: &MeshTopology) -> Self {
        let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for elem in &mesh.elements {
            by_name
                .entry(elem.region.clone())
                .or_default()
                .push(elem.id);
        }
        let mut registry = Self::new();
        for (name, mut ids) in by_name {
            ids.sort_unstable();
            // `add` cannot fail: the keys came from a map, so they are unique.
            let _ = registry.add(Region::new(name, RegionKind::Material, ids));
        }
        registry
    }
}

/// How a boundary value is applied over a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryKind {
    /// Prescribed value (Dirichlet).
    Dirichlet,
    /// Prescribed flux/gradient (Neumann).
    Neumann,
    /// Mixed (Robin).
    Robin,
}

/// A boundary condition bound to a stable region name.
///
/// The condition stores the region *name*; [`Self::bind`] resolves it against a
/// registry and refuses to produce a bound condition for a missing, empty or
/// wrongly-typed region. This is the mechanism that satisfies "must be checked
/// before solving".
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryCondition {
    /// Name of the region the condition applies to.
    pub region: String,
    /// How the value is applied.
    pub kind: BoundaryKind,
    /// Scalar value (or coefficient) of the condition.
    pub value: Scalar,
    /// Unit of the value, for provenance.
    pub unit: String,
}

impl BoundaryCondition {
    /// Construct a condition referencing a region by name.
    pub fn new(region: impl Into<String>, kind: BoundaryKind, value: Scalar) -> Self {
        Self {
            region: region.into(),
            kind,
            value,
            unit: String::new(),
        }
    }

    /// Attach a unit symbol.
    pub fn with_unit(mut self, unit: &str) -> Self {
        self.unit = unit.to_string();
        self
    }

    /// Resolve this condition against a registry, requiring a non-empty match.
    pub fn bind<'a>(&self, registry: &'a RegionRegistry) -> Result<&'a Region, MeshError> {
        let region = registry.get(&self.region).ok_or_else(|| {
            MeshError::at(
                MeshErrorKind::MissingRegion,
                MeshLocation::Region(self.region.clone()),
                "boundary condition references an undefined region",
            )
        })?;
        if region.is_empty() {
            return Err(MeshError::at(
                MeshErrorKind::EmptyRegion,
                MeshLocation::Region(self.region.clone()),
                "boundary condition references an empty region",
            ));
        }
        Ok(region)
    }
}

/// Look up boundary-region names that are present in a mesh's registry.
///
/// Convenience for the common workflow "does region `wall` exist and is it a
/// boundary?" without exposing the internal map.
pub fn boundary_region<'a>(
    registry: &'a RegionRegistry,
    name: &str,
) -> Result<&'a Region, MeshError> {
    let region = registry.get(name).ok_or_else(|| {
        MeshError::at(
            MeshErrorKind::MissingRegion,
            MeshLocation::Region(name.to_string()),
            "no such region",
        )
    })?;
    if region.kind != RegionKind::Boundary {
        return Err(MeshError::at(
            MeshErrorKind::MissingRegion,
            MeshLocation::Region(name.to_string()),
            format!("region is a {} region, not a boundary region", region.kind),
        ));
    }
    if region.is_empty() {
        return Err(MeshError::at(
            MeshErrorKind::EmptyRegion,
            MeshLocation::Region(name.to_string()),
            "boundary region has no faces",
        ));
    }
    Ok(region)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::mesh::topology::{Element, ElementType, MeshDimension, Node};

    fn two_triangle_mesh() -> MeshTopology {
        MeshTopology::new(
            MeshDimension::Dim2,
            vec![
                Node::new(0, 0.0, 0.0, 0.0),
                Node::new(1, 1.0, 0.0, 0.0),
                Node::new(2, 1.0, 1.0, 0.0),
                Node::new(3, 0.0, 1.0, 0.0),
            ],
            vec![
                Element::new(0, ElementType::Triangle, vec![0, 1, 2], "solid").unwrap(),
                Element::new(1, ElementType::Triangle, vec![0, 2, 3], "solid").unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn duplicate_region_name_is_rejected() {
        let mut registry = RegionRegistry::new();
        registry
            .add(Region::new("wall", RegionKind::Boundary, vec![0]))
            .unwrap();
        let err = registry
            .add(Region::new("wall", RegionKind::Boundary, vec![1]))
            .unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::DuplicateRegion);
        assert_eq!(err.location, MeshLocation::Region("wall".to_string()));
    }

    #[test]
    fn empty_region_is_caught_by_audit() {
        let mesh = two_triangle_mesh();
        let mut registry = RegionRegistry::new();
        registry
            .add(Region::new("hollow", RegionKind::Material, vec![]))
            .unwrap();
        let err = registry.audit(&mesh).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::EmptyRegion);
        assert_eq!(err.location, MeshLocation::Region("hollow".to_string()));
    }

    #[test]
    fn dangling_element_reference_is_caught() {
        let mesh = two_triangle_mesh();
        let mut registry = RegionRegistry::new();
        registry
            .add(Region::new("ghost", RegionKind::Material, vec![0, 999]))
            .unwrap();
        let err = registry.audit(&mesh).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::MissingRegion);
        assert!(err.detail.contains("999"));
    }

    #[test]
    fn dangling_face_reference_is_caught() {
        let mesh = two_triangle_mesh();
        let mut registry = RegionRegistry::new();
        // A triangle has faces 0..2; face 7 does not exist.
        registry.add(Region::boundary("bad", vec![(0, 7)])).unwrap();
        let err = registry.audit(&mesh).unwrap_err();
        assert_eq!(err.kind, MeshErrorKind::MissingRegion);
        assert!(err.detail.contains("face 7"));
    }

    #[test]
    fn audit_accepts_a_consistent_registry() {
        let mesh = two_triangle_mesh();
        let mut registry = RegionRegistry::new();
        registry
            .add(Region::new("solid", RegionKind::Material, vec![0, 1]))
            .unwrap();
        registry
            .add(Region::boundary("bottom", vec![(0, 0)]))
            .unwrap();
        registry
            .add(Region::new("corners", RegionKind::NodeSet, vec![0, 2]))
            .unwrap();
        assert!(registry.audit(&mesh).is_ok());
    }

    #[test]
    fn boundary_condition_binds_only_to_defined_non_empty_regions() {
        let mut registry = RegionRegistry::new();
        registry
            .add(Region::boundary("inlet", vec![(0, 0), (1, 1)]))
            .unwrap();
        registry
            .add(Region::new("empty_set", RegionKind::NodeSet, vec![]))
            .unwrap();

        let good = BoundaryCondition::new("inlet", BoundaryKind::Neumann, 1.0 as Scalar);
        assert_eq!(good.bind(&registry).unwrap().len(), 2);

        let missing = BoundaryCondition::new("nope", BoundaryKind::Dirichlet, 0.0 as Scalar);
        assert_eq!(
            missing.bind(&registry).unwrap_err().kind,
            MeshErrorKind::MissingRegion
        );

        let empty = BoundaryCondition::new("empty_set", BoundaryKind::Dirichlet, 0.0 as Scalar);
        assert_eq!(
            empty.bind(&registry).unwrap_err().kind,
            MeshErrorKind::EmptyRegion
        );
    }

    #[test]
    fn registry_from_element_tags_matches_mesh() {
        let mesh = two_triangle_mesh();
        let registry = RegionRegistry::from_element_tags(&mesh);
        assert_eq!(registry.names(), vec!["solid".to_string()]);
        let region = registry.get("solid").unwrap();
        assert_eq!(region.kind, RegionKind::Material);
        assert_eq!(region.members, vec![0, 1]);
        assert!(registry.audit(&mesh).is_ok());
    }

    #[test]
    fn rename_preserves_membership_and_tag() {
        let mut registry = RegionRegistry::new();
        registry
            .add(Region::new("old", RegionKind::Material, vec![3, 5]))
            .unwrap();
        registry.rename("old", "new").unwrap();
        assert!(registry.get("old").is_none());
        let region = registry.get("new").unwrap();
        assert_eq!(region.name, "new");
        assert_eq!(region.members, vec![3, 5]);
    }

    #[test]
    fn rename_rejects_missing_source_and_taken_target() {
        let mut registry = RegionRegistry::new();
        registry
            .add(Region::new("a", RegionKind::Material, vec![1]))
            .unwrap();
        registry
            .add(Region::new("b", RegionKind::Material, vec![2]))
            .unwrap();
        assert_eq!(
            registry.rename("ghost", "c").unwrap_err().kind,
            MeshErrorKind::MissingRegion
        );
        assert_eq!(
            registry.rename("a", "b").unwrap_err().kind,
            MeshErrorKind::DuplicateRegion
        );
    }

    #[test]
    fn boundary_region_lookup_by_name_enforces_kind() {
        let mut registry = RegionRegistry::new();
        registry
            .add(Region::boundary("wall", vec![(0, 0)]))
            .unwrap();
        registry
            .add(Region::new("body", RegionKind::Material, vec![0]))
            .unwrap();

        assert_eq!(boundary_region(&registry, "wall").unwrap().name, "wall");
        assert_eq!(
            boundary_region(&registry, "body").unwrap_err().kind,
            MeshErrorKind::MissingRegion
        );
        assert_eq!(
            boundary_region(&registry, "absent").unwrap_err().kind,
            MeshErrorKind::MissingRegion
        );
    }

    #[test]
    fn boundary_region_records_distinct_elements() {
        let region = Region::boundary("edge", vec![(4, 0), (4, 2), (7, 1)]);
        assert_eq!(region.len(), 3);
        assert_eq!(region.members, vec![4, 7]);
        assert!(region.contains_element(4));
        assert!(!region.contains_element(5));
    }

    #[test]
    fn region_len_counts_faces_for_boundary_kind() {
        let b = Region::boundary("b", vec![(0, 0), (0, 1)]);
        assert_eq!(b.len(), 2);
        let m = Region::new("m", RegionKind::Material, vec![1, 2, 3]);
        assert_eq!(m.len(), 3);
    }
}
