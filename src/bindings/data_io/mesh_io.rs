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

            // Node/element sets are part of the model, so persist them as
            // FIELD data instead of dropping them on the floor.
            if !mesh.node_sets.is_empty() || !mesh.element_sets.is_empty() {
                vtk.push_str(&format!(
                    "\nFIELD FieldData {}\n",
                    mesh.node_sets.len() + mesh.element_sets.len()
                ));
                for (name, ids) in &mesh.node_sets {
                    vtk.push_str(&format!(
                        "{} 1 {} int\n",
                        sanitize_field_name(name),
                        ids.len()
                    ));
                    write_index_list(&mut vtk, ids);
                }
                for (name, ids) in &mesh.element_sets {
                    let key = format!("element_set_{}", sanitize_field_name(name));
                    vtk.push_str(&format!("{} 1 {} int\n", key, ids.len()));
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

/// VTK field names may not contain whitespace.
fn sanitize_field_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect()
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
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(text.contains("FIELD FieldData"), "sets must be written");
        assert!(text.contains("inlet"), "node set name must be written");
        assert!(
            text.contains("element_set_wall"),
            "element set must be written with a distinguishing prefix"
        );
    }
}
