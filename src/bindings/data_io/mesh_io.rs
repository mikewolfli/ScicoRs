//! Multi-format mesh I/O: VTK, Gmsh, Abaqus, Ansys.
//!
//! Provides a unified `MeshData` structure and format-specific
//! import/export functions.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;
use std::collections::HashMap;

/// Supported mesh formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshFormat {
    Vtk,
    Vtu,
    Gmsh,
    Abaqus,
    Ansys,
}

/// Mesh element types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshElement {
    Line { connectivity: [usize; 2] },
    Triangle { connectivity: [usize; 3] },
    Quadrilateral { connectivity: [usize; 4] },
    Tetrahedron { connectivity: [usize; 4] },
    Hexahedron { connectivity: [usize; 8] },
    Prism { connectivity: [usize; 6] },
}

/// Mesh data structure.
#[derive(Debug, Clone)]
pub struct MeshData {
    pub nodes: Vec<Coord3D>,
    pub elements: Vec<MeshElement>,
    pub node_sets: HashMap<String, Vec<usize>>,
    pub element_sets: HashMap<String, Vec<usize>>,
}

impl MeshData {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            elements: Vec::new(),
            node_sets: HashMap::new(),
            element_sets: HashMap::new(),
        }
    }
}

impl Default for MeshData {
    fn default() -> Self {
        Self::new()
    }
}

/// Import mesh from file.
///
/// Performs a real ASCII VTK (`DATASET UNSTRUCTURED_GRID`) parse. Gmsh,
/// Abaqus, Ansys and VTU readers are not implemented yet and are rejected
/// explicitly rather than silently returning an empty mesh.
pub fn import_mesh(filepath: &str, format: MeshFormat) -> Result<MeshData, String> {
    match format {
        MeshFormat::Vtk => import_vtk(filepath),
        MeshFormat::Gmsh | MeshFormat::Abaqus | MeshFormat::Ansys | MeshFormat::Vtu => {
            Err(format!(
                "{} import is not supported; write the mesh as ASCII legacy VTK instead",
                format_name(&format)
            ))
        }
    }
}

/// Human-readable name of a mesh format.
fn format_name(format: &MeshFormat) -> &'static str {
    match format {
        MeshFormat::Vtk => "VTK",
        MeshFormat::Vtu => "VTU",
        MeshFormat::Gmsh => "Gmsh",
        MeshFormat::Abaqus => "Abaqus",
        MeshFormat::Ansys => "Ansys",
    }
}

/// VTK cell type id for a [`MeshElement`].
fn vtk_cell_type(elem: &MeshElement) -> usize {
    match elem {
        MeshElement::Line { .. } => 3,          // VTK_LINE
        MeshElement::Triangle { .. } => 5,      // VTK_TRIANGLE
        MeshElement::Quadrilateral { .. } => 9, // VTK_QUAD
        MeshElement::Tetrahedron { .. } => 10,  // VTK_TETRA
        MeshElement::Hexahedron { .. } => 12,   // VTK_HEXAHEDRON
        MeshElement::Prism { .. } => 13,        // VTK_WEDGE
    }
}

/// Node indices carried by a [`MeshElement`].
fn connectivity(elem: &MeshElement) -> &[usize] {
    match elem {
        MeshElement::Line { connectivity } => connectivity,
        MeshElement::Triangle { connectivity } => connectivity,
        MeshElement::Quadrilateral { connectivity } => connectivity,
        MeshElement::Tetrahedron { connectivity } => connectivity,
        MeshElement::Hexahedron { connectivity } => connectivity,
        MeshElement::Prism { connectivity } => connectivity,
    }
}

/// Rebuild a [`MeshElement`] from a VTK cell type id and its node list.
fn element_from_vtk(cell_type: usize, ids: &[usize]) -> Result<MeshElement, String> {
    let need = |n: usize| -> Result<(), String> {
        if ids.len() == n {
            Ok(())
        } else {
            Err(format!(
                "VTK cell type {} expects {} node indices, got {}",
                cell_type,
                n,
                ids.len()
            ))
        }
    };
    let arr = |n: usize| -> Result<[usize; 8], String> {
        let mut out = [0usize; 8];
        out[..n].copy_from_slice(&ids[..n]);
        Ok(out)
    };
    Ok(match cell_type {
        3 => {
            need(2)?;
            let a = arr(2)?;
            MeshElement::Line {
                connectivity: [a[0], a[1]],
            }
        }
        5 => {
            need(3)?;
            let a = arr(3)?;
            MeshElement::Triangle {
                connectivity: [a[0], a[1], a[2]],
            }
        }
        9 => {
            need(4)?;
            let a = arr(4)?;
            MeshElement::Quadrilateral {
                connectivity: [a[0], a[1], a[2], a[3]],
            }
        }
        10 => {
            need(4)?;
            let a = arr(4)?;
            MeshElement::Tetrahedron {
                connectivity: [a[0], a[1], a[2], a[3]],
            }
        }
        12 => {
            need(8)?;
            let a = arr(8)?;
            MeshElement::Hexahedron { connectivity: a }
        }
        13 => {
            need(6)?;
            let a = arr(6)?;
            MeshElement::Prism {
                connectivity: [a[0], a[1], a[2], a[3], a[4], a[5]],
            }
        }
        other => {
            return Err(format!(
                "unsupported VTK cell type {} (expected 3, 5, 9, 10, 12 or 13)",
                other
            ));
        }
    })
}

/// Parse an ASCII legacy VTK unstructured-grid file.
fn import_vtk(filepath: &str) -> Result<MeshData, String> {
    let content =
        std::fs::read_to_string(filepath).map_err(|e| format!("VTK read error: {}", e))?;

    let mut mesh = MeshData::new();
    let mut tokens = content.split_whitespace().peekable();
    // Raw node lists per cell, resolved into `MeshElement`s once CELL_TYPES is
    // known (VTK stores connectivity and cell type in separate sections).
    let mut raw_cells: Vec<Vec<usize>> = Vec::new();
    let mut cell_types: Option<Vec<usize>> = None;
    let mut saw_dataset = false;

    while let Some(tok) = tokens.next() {
        match tok {
            // `DATASET UNSTRUCTURED_GRID` — any other dataset kind is rejected.
            "DATASET" => {
                let kind = tokens.next().unwrap_or_default();
                if !kind.eq_ignore_ascii_case("UNSTRUCTURED_GRID") {
                    return Err(format!(
                        "unsupported VTK DATASET '{}': only UNSTRUCTURED_GRID is supported",
                        kind
                    ));
                }
                saw_dataset = true;
            }
            "POINTS" => {
                let count = parse_count(tokens.next(), "POINTS")?;
                // Data type token (`float`, `double`, ...) is not needed: every
                // coordinate is parsed as f64 and narrowed to `Scalar`.
                let _data_type = tokens.next();
                mesh.nodes.reserve(count);
                for _ in 0..count {
                    let x = parse_real(tokens.next(), "POINTS x")?;
                    let y = parse_real(tokens.next(), "POINTS y")?;
                    let z = parse_real(tokens.next(), "POINTS z")?;
                    mesh.nodes.push(Coord3D::new(x, y, z));
                }
            }
            "CELLS" => {
                let count = parse_count(tokens.next(), "CELLS")?;
                // The second header number is the total list length, which must
                // agree with the per-cell `n` prefixes that follow.
                let declared_list_len = parse_count(tokens.next(), "CELLS list length")?;
                raw_cells.reserve(count);
                let mut consumed = 0usize;
                for cell in 0..count {
                    let n = parse_count(tokens.next(), "CELLS node count")?;
                    if n == 0 {
                        return Err(format!("VTK cell {} declares zero nodes", cell));
                    }
                    consumed += 1 + n;
                    let mut ids = Vec::with_capacity(n);
                    for _ in 0..n {
                        ids.push(parse_count(tokens.next(), "CELLS node id")?);
                    }
                    raw_cells.push(ids);
                }
                if consumed != declared_list_len {
                    return Err(format!(
                        "VTK CELLS declares a list length of {} but the cells consumed {}",
                        declared_list_len, consumed
                    ));
                }
            }
            "CELL_TYPES" => {
                let count = parse_count(tokens.next(), "CELL_TYPES")?;
                let mut types = Vec::with_capacity(count);
                for _ in 0..count {
                    types.push(parse_count(tokens.next(), "CELL_TYPES entry")?);
                }
                cell_types = Some(types);
            }
            "FIELD" => {
                // `FIELD FieldData <n>` followed by, per array, a name, a
                // component count, a tuple count, a type and the values.
                //
                // Names written by `export_mesh` carry an unambiguous encoding
                // that records both the original name and whether the set holds
                // node or element indices. A FIELD array from a foreign tool
                // lacks that prefix and is read as a node set under its literal
                // name, so importing a third-party file still works.
                let _field_name = tokens.next();
                let n_arrays = parse_count(tokens.next(), "FIELD array count")?;
                for _ in 0..n_arrays {
                    let raw_name = match tokens.next() {
                        Some(n) => n.to_string(),
                        None => break,
                    };
                    let _components = parse_count(tokens.next(), "FIELD components")?;
                    let tuples = parse_count(tokens.next(), "FIELD tuples")?;
                    let _value_type = tokens.next();
                    let mut ids = Vec::with_capacity(tuples);
                    for _ in 0..tuples {
                        ids.push(parse_count(tokens.next(), "FIELD value")?);
                    }

                    match decode_set_name(&raw_name) {
                        Some((MeshSetKind::Element, name)) => {
                            mesh.element_sets.insert(name, ids);
                        }
                        Some((MeshSetKind::Node, name)) => {
                            mesh.node_sets.insert(name, ids);
                        }
                        // A foreign array: keep it as-is rather than renaming it.
                        None => {
                            mesh.node_sets.insert(raw_name, ids);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if !saw_dataset {
        return Err("not a VTK unstructured-grid file (missing DATASET)".to_string());
    }
    let cell_types =
        cell_types.ok_or("VTK file has CELLS but no CELL_TYPES section".to_string())?;
    if cell_types.len() != raw_cells.len() {
        return Err(format!(
            "VTK CELL_TYPES declares {} cells but CELLS declared {}",
            cell_types.len(),
            raw_cells.len()
        ));
    }

    for (ids, ct) in raw_cells.iter().zip(cell_types.iter()) {
        mesh.elements.push(element_from_vtk(*ct, ids)?);
    }

    // Validate node indices so a corrupt file cannot produce out-of-range
    // connectivity that panics later in the solver.
    for elem in &mesh.elements {
        for &id in connectivity(elem) {
            if id >= mesh.nodes.len() {
                return Err(format!(
                    "VTK connectivity references node {} but only {} nodes were declared",
                    id,
                    mesh.nodes.len()
                ));
            }
        }
    }
    Ok(mesh)
}

fn parse_count(tok: Option<&str>, what: &str) -> Result<usize, String> {
    tok.ok_or_else(|| format!("VTK: missing {}", what))?
        .parse::<usize>()
        .map_err(|e| format!("VTK: invalid {}: {}", what, e))
}

fn parse_real(tok: Option<&str>, what: &str) -> Result<Scalar, String> {
    tok.ok_or_else(|| format!("VTK: missing {}", what))?
        .parse::<Scalar>()
        .map_err(|e| format!("VTK: invalid {}: {}", what, e))
}

/// Which of the two mesh set collections a VTK FIELD array belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeshSetKind {
    Node,
    Element,
}

/// Prefix byte that marks an encoded set name, so a set whose own name happens
/// to look encoded is unambiguous on the way back in.
const SET_NAME_ESCAPE: char = '~';

/// Encode a set name for the VTK name space, reversibly.
///
/// Two problems have to be solved at once:
///
/// 1. **Reserved words.** A set named `CELLS`/`POINTS`/`FIELD` would be re-read as
///    a VTK *section header* and make the file unparseable. VTK FIELD names also
///    may not contain whitespace.
/// 2. **Losslessness.** A prefix scheme such as `element_set_<name>` is ambiguous:
///    a *node* set genuinely named `element_set_wall` collides with the element
///    set `wall`, and one of the two is silently lost.
///
/// This encoding escapes every character that is unsafe in a VTK name using
/// `~` followed by the character's hex code point, and marks the kind with a
/// leading `n`/`e`. `decode_set_name` inverts it exactly, so the original name
/// (including spaces and reserved words) is restored.
fn encode_set_name(kind: MeshSetKind, name: &str) -> Result<String, String> {
    let mut out = String::with_capacity(name.len() + 2);
    out.push(SET_NAME_ESCAPE);
    out.push(match kind {
        MeshSetKind::Node => 'n',
        MeshSetKind::Element => 'e',
    });
    for c in name.chars() {
        // Only characters that are safe as a single VTK token are kept literal.
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            out.push(c);
        } else {
            out.push(SET_NAME_ESCAPE);
            out.push_str(&format!("{:x}", c as u32));
            out.push(';');
        }
    }
    Ok(out)
}

/// Decode a name produced by [`encode_set_name`].
///
/// Returns `None` for a name that does not carry the encoding prefix, which is
/// how a foreign VTK file's own FIELD arrays (created by other tools) are left
/// untouched: they are read as node sets under their literal name.
fn decode_set_name(encoded: &str) -> Option<(MeshSetKind, String)> {
    let mut chars = encoded.chars();
    if chars.next()? != SET_NAME_ESCAPE {
        return None;
    }
    let kind = match chars.next()? {
        'n' => MeshSetKind::Node,
        'e' => MeshSetKind::Element,
        _ => return None,
    };

    let mut out = String::new();
    let mut rest: Vec<char> = chars.collect();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == SET_NAME_ESCAPE {
            // `~<hex>;` denotes an escaped character.
            let start = i + 1;
            let end_offset = rest[start..].iter().position(|c| *c == ';')?;
            let hex: String = rest[start..start + end_offset].iter().collect();
            let code = u32::from_str_radix(&hex, 16).ok()?;
            out.push(char::from_u32(code)?);
            i = start + end_offset + 1;
        } else {
            out.push(rest[i]);
            i += 1;
        }
    }
    rest.clear();
    Some((kind, out))
}

/// Export mesh to file.
///
/// Writes ASCII legacy VTK for every [`MeshElement`] variant, with a correct
/// `CELLS` list length and VTK cell type ids.
pub fn export_mesh(mesh: &MeshData, format: MeshFormat, filepath: &str) -> Result<(), String> {
    match format {
        MeshFormat::Vtk => {
            // Validate connectivity before writing: an out-of-range index would
            // otherwise produce a file that no reader can load.
            for elem in &mesh.elements {
                for &id in connectivity(elem) {
                    if id >= mesh.nodes.len() {
                        return Err(format!(
                            "mesh connectivity references node {} but only {} nodes exist",
                            id,
                            mesh.nodes.len()
                        ));
                    }
                }
            }

            let mut vtk = String::from(
                "# vtk DataFile Version 3.0\nSCIcoRS export\nASCII\nDATASET UNSTRUCTURED_GRID\n",
            );
            vtk.push_str(&format!("POINTS {} double\n", mesh.nodes.len()));
            for n in &mesh.nodes {
                vtk.push_str(&format!("{} {} {}\n", n.x, n.y, n.z));
            }

            // Each cell contributes `1 + n_nodes` integers to the list.
            let list_len: usize = mesh
                .elements
                .iter()
                .map(|e| 1 + connectivity(e).len())
                .sum();
            vtk.push_str(&format!("CELLS {} {}\n", mesh.elements.len(), list_len));
            for elem in &mesh.elements {
                let ids = connectivity(elem);
                vtk.push_str(&format!("{}", ids.len()));
                for id in ids {
                    vtk.push_str(&format!(" {}", id));
                }
                vtk.push('\n');
            }

            vtk.push_str(&format!("CELL_TYPES {}\n", mesh.elements.len()));
            for elem in &mesh.elements {
                vtk.push_str(&format!("{}\n", vtk_cell_type(elem)));
            }

            // Node/element sets are part of the model, so persist them as FIELD
            // data instead of dropping them on the floor. Both kinds share the
            // same VTK name space, so each is given an unambiguous prefix and the
            // *exact* original name is restored on load (see `encode_set_name`).
            if !mesh.node_sets.is_empty() || !mesh.element_sets.is_empty() {
                vtk.push_str(&format!(
                    "\nFIELD FieldData {}\n",
                    mesh.node_sets.len() + mesh.element_sets.len()
                ));
                for (name, ids) in &mesh.node_sets {
                    vtk.push_str(&format!(
                        "{} 1 {} int\n",
                        encode_set_name(MeshSetKind::Node, name)?,
                        ids.len()
                    ));
                    write_index_list(&mut vtk, ids);
                }
                for (name, ids) in &mesh.element_sets {
                    vtk.push_str(&format!(
                        "{} 1 {} int\n",
                        encode_set_name(MeshSetKind::Element, name)?,
                        ids.len()
                    ));
                    write_index_list(&mut vtk, ids);
                }
            }

            std::fs::write(filepath, &vtk).map_err(|e| format!("VTK write error: {}", e))
        }
        other => Err(format!(
            "{} export is not supported; use VTK",
            format_name(&other)
        )),
    }
}

fn write_index_list(out: &mut String, ids: &[usize]) {
    for id in ids {
        out.push_str(&format!(" {}", id));
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("scico_mesh_{}_{}.vtk", name, std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn test_mesh_data_new() {
        let m = MeshData::new();
        assert!(m.nodes.is_empty());
        assert!(m.elements.is_empty());
    }

    /// The historical exporter wrote `len*4` for every `CELLS` header and gave
    /// non-triangle/tetra cells a fabricated single-node cell. Both made the
    /// file unreadable. This asserts a real round trip over *all* element kinds.
    #[test]
    fn test_vtk_roundtrip_preserves_all_element_kinds() {
        let mut mesh = MeshData::new();
        for p in [
            (0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.0, 0.0, 1.0),
            (1.0, 1.0, 0.0),
            (1.0, 1.0, 1.0),
            (0.5, 0.5, 0.5),
            (2.0, 0.0, 0.0),
        ] {
            mesh.nodes.push(Coord3D::new(p.0, p.1, p.2));
        }
        mesh.elements.push(MeshElement::Line {
            connectivity: [0, 1],
        });
        mesh.elements.push(MeshElement::Triangle {
            connectivity: [0, 1, 2],
        });
        mesh.elements.push(MeshElement::Quadrilateral {
            connectivity: [0, 1, 4, 2],
        });
        mesh.elements.push(MeshElement::Tetrahedron {
            connectivity: [0, 1, 2, 3],
        });
        mesh.elements.push(MeshElement::Hexahedron {
            connectivity: [0, 1, 4, 2, 3, 7, 5, 6],
        });
        mesh.elements.push(MeshElement::Prism {
            connectivity: [0, 1, 2, 3, 7, 0],
        });

        let path = scratch("allkinds");
        export_mesh(&mesh, MeshFormat::Vtk, &path).expect("export must succeed");
        let back = import_mesh(&path, MeshFormat::Vtk).expect("import must succeed");
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            back.nodes.len(),
            mesh.nodes.len(),
            "node count must survive"
        );
        assert_eq!(
            back.elements.len(),
            mesh.elements.len(),
            "every element must survive the round trip"
        );
        // Compare each element by (vtk type, connectivity) so a reordering that
        // silently changes element semantics is still caught.
        let got: Vec<(usize, Vec<usize>)> = back
            .elements
            .iter()
            .map(|e| (vtk_cell_type(e), connectivity(e).to_vec()))
            .collect();
        let want: Vec<(usize, Vec<usize>)> = mesh
            .elements
            .iter()
            .map(|e| (vtk_cell_type(e), connectivity(e).to_vec()))
            .collect();
        assert_eq!(got, want, "element kinds and connectivity must survive");

        // Coordinates must round-trip exactly (exported as `double`).
        for (a, b) in back.nodes.iter().zip(mesh.nodes.iter()) {
            assert_eq!((a.x, a.y, a.z), (b.x, b.y, b.z));
        }
    }

    #[test]
    fn test_export_mesh_vtk() {
        let mut mesh = MeshData::new();
        mesh.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        mesh.nodes.push(Coord3D::new(1.0, 0.0, 0.0));
        mesh.nodes.push(Coord3D::new(0.0, 1.0, 0.0));
        mesh.elements.push(MeshElement::Triangle {
            connectivity: [0, 1, 2],
        });
        let path = scratch("triangle");
        assert!(export_mesh(&mesh, MeshFormat::Vtk, &path).is_ok());
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        // One triangle consumes 1 + 3 = 4 list entries, not `elements*4 == 4`
        // by coincidence: assert the actual contract.
        assert!(text.contains("CELLS 1 4"), "bad CELLS header:\n{text}");
        assert!(text.contains("CELL_TYPES 1"), "bad CELL_TYPES header");
        assert!(text.contains("DATASET UNSTRUCTURED_GRID"));
    }

    /// `import_mesh` used to ignore the file entirely and return an empty mesh
    /// while reporting success. That silent data loss is now an explicit error
    /// for formats without a reader, and a real parse for VTK.
    #[test]
    fn test_import_rejects_unsupported_formats_loudly() {
        for fmt in [
            MeshFormat::Gmsh,
            MeshFormat::Abaqus,
            MeshFormat::Ansys,
            MeshFormat::Vtu,
        ] {
            let err = import_mesh("anything.msh", fmt).unwrap_err();
            assert!(
                err.contains("not supported"),
                "expected an explicit unsupported-format error, got: {err}"
            );
        }
    }

    #[test]
    fn test_import_missing_file_errors() {
        let err = import_mesh("/tmp/scico_does_not_exist_9d2f.vtk", MeshFormat::Vtk).unwrap_err();
        assert!(err.contains("read error"), "got: {err}");
    }

    #[test]
    fn test_import_vtk_rejects_wrong_dataset() {
        let path = scratch("structured");
        std::fs::write(
            &path,
            "# vtk DataFile Version 3.0\nx\nASCII\nDATASET STRUCTURED_POINTS\n",
        )
        .unwrap();
        let err = import_mesh(&path, MeshFormat::Vtk).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.contains("unsupported VTK DATASET"), "got: {err}");
    }

    #[test]
    fn test_import_vtk_rejects_out_of_range_connectivity() {
        let path = scratch("oob");
        std::fs::write(
            &path,
            "# vtk DataFile Version 3.0\nx\nASCII\nDATASET UNSTRUCTURED_GRID\n\
             POINTS 3 double\n0 0 0\n1 0 0\n0 1 0\n\
             CELLS 1 4\n3 0 1 7\n\
             CELL_TYPES 1\n5\n",
        )
        .unwrap();
        let err = import_mesh(&path, MeshFormat::Vtk).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(
            err.contains("references node 7"),
            "expected an out-of-range node error, got: {err}"
        );
    }

    #[test]
    fn test_export_rejects_out_of_range_connectivity() {
        let mut mesh = MeshData::new();
        mesh.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        mesh.elements.push(MeshElement::Triangle {
            connectivity: [0, 1, 2],
        });
        let path = scratch("bad_export");
        let err = export_mesh(&mesh, MeshFormat::Vtk, &path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.contains("only 1 nodes exist"), "got: {err}");
    }

    #[test]
    fn test_vtk_roundtrip_preserves_node_and_element_sets() {
        let mut mesh = MeshData::new();
        mesh.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        mesh.nodes.push(Coord3D::new(1.0, 0.0, 0.0));
        mesh.nodes.push(Coord3D::new(0.0, 1.0, 0.0));
        mesh.elements.push(MeshElement::Triangle {
            connectivity: [0, 1, 2],
        });
        mesh.node_sets.insert("inlet".to_string(), vec![0, 1]);
        mesh.element_sets.insert("wall".to_string(), vec![0]);

        let path = scratch("sets");
        export_mesh(&mesh, MeshFormat::Vtk, &path).unwrap();
        // The real test: read the sets back. Asserting only on the written text
        // would not catch the importer ignoring the FIELD section.
        let back =
            import_mesh(&path, MeshFormat::Vtk).expect("the exported VTK must be importable");
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            back.node_sets.get("inlet"),
            Some(&vec![0, 1]),
            "node sets must survive the round trip"
        );
        assert_eq!(
            back.element_sets.get("wall"),
            Some(&vec![0]),
            "element sets must survive the round trip"
        );
    }

    /// A set whose name collides with a VTK section keyword must not be able to
    /// corrupt the file *and* must come back with its original name.
    ///
    /// The previous scheme prefixed such names with `set_`, which was a silent
    /// rename: the caller asked for `CELLS` and got `set_CELLS`.
    #[test]
    fn test_set_named_like_a_vtk_keyword_round_trips_with_its_exact_name() {
        let mut mesh = MeshData::new();
        mesh.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
        mesh.nodes.push(Coord3D::new(1.0, 0.0, 0.0));
        mesh.nodes.push(Coord3D::new(0.0, 1.0, 0.0));
        mesh.elements.push(MeshElement::Triangle {
            connectivity: [0, 1, 2],
        });
        mesh.node_sets.insert("CELLS".to_string(), vec![0]);
        mesh.node_sets.insert("POINTS".to_string(), vec![1]);
        mesh.node_sets.insert("FIELD".to_string(), vec![2]);

        let path = scratch("keyword_sets");
        export_mesh(&mesh, MeshFormat::Vtk, &path).unwrap();
        let back = import_mesh(&path, MeshFormat::Vtk)
            .expect("a reserved set name must not make the file unreadable");
        let _ = std::fs::remove_file(&path);

        assert_eq!(back.nodes.len(), 3, "geometry must still parse");
        assert_eq!(back.elements.len(), 1, "the element must still parse");
        assert_eq!(
            back.node_sets.get("CELLS"),
            Some(&vec![0]),
            "the original name must be preserved exactly"
        );
        assert_eq!(back.node_sets.get("POINTS"), Some(&vec![1]));
        assert_eq!(back.node_sets.get("FIELD"), Some(&vec![2]));
    }

    /// Set names are preserved character for character, including names that the
    /// old prefix scheme silently mangled or that collided with it.
    #[test]
    fn test_arbitrary_set_names_round_trip_losslessly() {
        // Names chosen to break a naive prefix scheme.
        let node_name_cases = [
            "element_set_wall", // collides with the old element prefix
            "with space",
            "tab\there",
            "",          // empty name
            "set_CELLS", // the old escaped form
            "unicode-\u{6d4b}\u{8bd5}",
            "many~~~tildes~",
            "~n~65;", // looks pre-encoded
            "comma,separated",
            "quote\"inside",
        ];

        for name in node_name_cases {
            let mut mesh = MeshData::new();
            mesh.nodes.push(Coord3D::new(0.0, 0.0, 0.0));
            mesh.nodes.push(Coord3D::new(1.0, 0.0, 0.0));
            mesh.nodes.push(Coord3D::new(0.0, 1.0, 0.0));
            mesh.elements.push(MeshElement::Triangle {
                connectivity: [0, 1, 2],
            });
            mesh.node_sets.insert(name.to_string(), vec![0, 1]);
            // An element set of the same name must stay distinct too.
            mesh.element_sets.insert(name.to_string(), vec![0]);

            let path = scratch("names");
            export_mesh(&mesh, MeshFormat::Vtk, &path)
                .unwrap_or_else(|e| panic!("name {name:?} failed to export: {e}"));
            let back = import_mesh(&path, MeshFormat::Vtk)
                .unwrap_or_else(|e| panic!("name {name:?} failed to import: {e}"));
            let _ = std::fs::remove_file(&path);

            assert_eq!(
                back.node_sets.get(name),
                Some(&vec![0, 1]),
                "node set {name:?} must round-trip with its exact name"
            );
            assert_eq!(
                back.element_sets.get(name),
                Some(&vec![0]),
                "element set {name:?} must stay distinct from the node set"
            );
            assert_eq!(back.node_sets.len(), 1, "no extra node sets");
            assert_eq!(back.element_sets.len(), 1, "no extra element sets");
        }
    }

    /// A FIELD array written by another tool (no encoding prefix) must be read
    /// back under its literal name rather than being rejected or renamed.
    #[test]
    fn test_foreign_field_arrays_are_read_under_their_literal_name() {
        let path = scratch("foreign");
        std::fs::write(
            &path,
            "# vtk DataFile Version 3.0\nx\nASCII\nDATASET UNSTRUCTURED_GRID\n\
             POINTS 3 double\n0 0 0\n1 0 0\n0 1 0\n\
             CELLS 1 4\n3 0 1 2\n\
             CELL_TYPES 1\n5\n\
             FIELD FieldData 1\n\
             external_group 1 2 int\n 0 2\n",
        )
        .unwrap();
        let mesh = import_mesh(&path, MeshFormat::Vtk).expect("a foreign file must load");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            mesh.node_sets.get("external_group"),
            Some(&vec![0, 2]),
            "a foreign FIELD array keeps its literal name"
        );
        assert!(mesh.element_sets.is_empty());
    }

    /// The encode/decode pair must be an exact inverse for every input.
    #[test]
    fn test_set_name_encoding_is_a_lossless_bijection() {
        let cases = [
            "",
            "plain",
            "with space",
            "~",
            "~~",
            "~n~65;",
            "CELLS",
            "element_set_x",
            "a~b~c",
            "\u{6d4b}\u{8bd5}",
            "tab\there\nnewline",
            "semi;colon",
        ];
        for name in cases {
            for kind in [MeshSetKind::Node, MeshSetKind::Element] {
                let encoded = encode_set_name(kind, name).unwrap();
                // VTK FIELD names must be a single whitespace-free token.
                assert!(
                    !encoded.chars().any(char::is_whitespace),
                    "encoded {encoded:?} must not contain whitespace"
                );
                let (decoded_kind, decoded) = decode_set_name(&encoded)
                    .unwrap_or_else(|| panic!("failed to decode {encoded:?} for {name:?}"));
                assert_eq!(decoded_kind, kind, "kind must survive for {name:?}");
                assert_eq!(decoded, name, "name must survive exactly for {name:?}");
            }
        }
    }

    /// A name that does not carry the encoding prefix is not decoded, so a
    /// foreign array can never be mistaken for one of ours.
    #[test]
    fn test_unescaped_names_are_not_decoded() {
        for name in ["plain", "element_set_wall", "", "n_test", "~x"] {
            assert!(
                decode_set_name(name).is_none(),
                "{name:?} must not be treated as an encoded set name"
            );
        }
    }
}
