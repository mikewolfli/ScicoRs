// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! STL (stereolithography) file import/export.
//!
//! Supports binary STL format for triangle mesh data.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// A single STL triangle.
#[derive(Debug, Clone)]
pub struct StlTriangle {
    pub normal: [Scalar; 3],
    pub v1: Coord3D,
    pub v2: Coord3D,
    pub v3: Coord3D,
}

/// Size of the binary STL header (80-byte comment + `u32` triangle count).
pub const BINARY_HEADER_LEN: usize = 84;

/// Size of one binary STL triangle record (12-byte normal, 3 vertices, 2-byte
/// attribute word).
pub const TRIANGLE_RECORD_LEN: usize = 50;

/// STL mesh data.
#[derive(Debug, Clone)]
pub struct StlMesh {
    pub triangles: Vec<StlTriangle>,
    pub unit: String,
}

impl StlMesh {
    pub fn new() -> Self {
        Self {
            triangles: Vec::new(),
            unit: "mm".to_string(),
        }
    }
}

impl Default for StlMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Import a binary STL file.
///
/// Returns an error when the declared triangle count does not match the file
/// size, instead of silently returning a truncated mesh (the previous behaviour
/// stopped at the first short record and reported success).
pub fn import_stl(filepath: &str) -> Result<StlMesh, String> {
    let data = std::fs::read(filepath).map_err(|e| format!("STL read error: {}", e))?;
    if data.len() < BINARY_HEADER_LEN {
        return Err(format!(
            "Invalid STL file: {} bytes is shorter than the {}-byte binary header",
            data.len(),
            BINARY_HEADER_LEN
        ));
    }
    let num_triangles = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;

    // Every triangle is exactly 50 bytes. A mismatch means a truncated or
    // corrupt file; surfacing it prevents downstream geometry from silently
    // missing faces.
    let expected = BINARY_HEADER_LEN + num_triangles * TRIANGLE_RECORD_LEN;
    if data.len() != expected {
        return Err(format!(
            "Truncated STL file: header declares {} triangles ({} bytes expected) \
             but the file is {} bytes",
            num_triangles,
            expected,
            data.len()
        ));
    }

    let mut mesh = StlMesh::new();
    // Recover the length unit from the header comment written by `export_stl`.
    // Binary STL has no dedicated unit field, so the header is the only carrier;
    // ignoring it made `unit` write-only.
    if let Ok(header) = std::str::from_utf8(&data[..80])
        && let Some(pos) = header.find("unit=")
    {
        let unit: String = header[pos + "unit=".len()..]
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '\0')
            .collect();
        if !unit.is_empty() {
            mesh.unit = unit;
        }
    }
    mesh.triangles.reserve(num_triangles);
    for i in 0..num_triangles {
        let offset = BINARY_HEADER_LEN + i * TRIANGLE_RECORD_LEN;
        let n = read_stl_vec3(&data, offset);
        let p1 = read_stl_vec3(&data, offset + 12);
        let p2 = read_stl_vec3(&data, offset + 24);
        let p3 = read_stl_vec3(&data, offset + 36);
        mesh.triangles.push(StlTriangle {
            normal: n,
            v1: Coord3D::new(p1[0], p1[1], p1[2]),
            v2: Coord3D::new(p2[0], p2[1], p2[2]),
            v3: Coord3D::new(p3[0], p3[1], p3[2]),
        });
    }
    Ok(mesh)
}

fn read_stl_vec3(data: &[u8], offset: usize) -> [Scalar; 3] {
    let x = f32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as Scalar;
    let y = f32::from_le_bytes([
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ]) as Scalar;
    let z = f32::from_le_bytes([
        data[offset + 8],
        data[offset + 9],
        data[offset + 10],
        data[offset + 11],
    ]) as Scalar;
    [x, y, z]
}

/// Export as a binary STL file.
///
/// Writes `mesh.unit` into the 80-byte header (binary STL has no dedicated unit
/// field, so the header comment is the conventional place to record it).
/// Coordinates are written as `f32` because that is what the format mandates.
pub fn export_stl(mesh: &StlMesh, filepath: &str) -> Result<(), String> {
    let count = u32::try_from(mesh.triangles.len()).map_err(|_| {
        format!(
            "too many triangles for binary STL: {} exceeds the u32 count field",
            mesh.triangles.len()
        )
    })?;

    // Reject non-finite coordinates: they would be written as `NaN`/`inf`
    // bit patterns that every consumer silently mis-handles.
    for (i, tri) in mesh.triangles.iter().enumerate() {
        for (name, v) in [
            ("normal", tri.normal),
            ("v1", [tri.v1.x, tri.v1.y, tri.v1.z]),
            ("v2", [tri.v2.x, tri.v2.y, tri.v2.z]),
            ("v3", [tri.v3.x, tri.v3.y, tri.v3.z]),
        ] {
            if !v.iter().all(|c| c.is_finite()) {
                return Err(format!(
                    "triangle {} has a non-finite {} component ({:?}); refusing to write an \
                     unreadable STL file",
                    i, name, v
                ));
            }
        }
    }

    let mut data: Vec<u8> =
        Vec::with_capacity(BINARY_HEADER_LEN + mesh.triangles.len() * TRIANGLE_RECORD_LEN);
    // 80-byte header: unit marker first, zero-padded to 80 bytes.
    let mut header = [0u8; 80];
    let unit = format!("scico-rs unit={}", mesh.unit);
    let bytes = unit.as_bytes();
    let n = bytes.len().min(80);
    header[..n].copy_from_slice(&bytes[..n]);
    data.extend_from_slice(&header);
    // Number of triangles
    data.extend_from_slice(&count.to_le_bytes());
    for tri in &mesh.triangles {
        for v in [
            tri.normal,
            [tri.v1.x, tri.v1.y, tri.v1.z],
            [tri.v2.x, tri.v2.y, tri.v2.z],
            [tri.v3.x, tri.v3.y, tri.v3.z],
        ] {
            for coord in v {
                data.extend_from_slice(&(coord as f32).to_le_bytes());
            }
        }
        data.extend_from_slice(&[0u8; 2]); // attribute byte count
    }
    std::fs::write(filepath, &data).map_err(|e| format!("STL write error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("scico_stl_{}_{}.stl", name, std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    fn tri(normal: [Scalar; 3]) -> StlTriangle {
        StlTriangle {
            normal,
            v1: Coord3D::new(0.0, 0.0, 0.0),
            v2: Coord3D::new(1.0, 0.0, 0.0),
            v3: Coord3D::new(0.0, 1.0, 0.0),
        }
    }

    #[test]
    fn test_stl_mesh_creation() {
        let m = StlMesh::new();
        assert!(m.triangles.is_empty());
        assert_eq!(m.unit, "mm");
    }

    #[test]
    fn test_export_stl_triangle() {
        let mut mesh = StlMesh::new();
        mesh.triangles.push(tri([0.0, 0.0, 1.0]));
        let path = scratch("triangle");
        assert!(export_stl(&mesh, &path).is_ok());
        let imported = import_stl(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(imported.triangles.len(), 1);
        assert_eq!(imported.triangles[0].normal, [0.0, 0.0, 1.0]);
        assert_eq!(imported.triangles[0].v2.x, 1.0);
        assert_eq!(imported.triangles[0].v3.y, 1.0);
    }

    /// The STL unit used to be written into the header but never read back, so
    /// the round trip silently returned the default. This asserts the *imported*
    /// unit, not just the written bytes.
    #[test]
    fn test_stl_roundtrip_preserves_unit_through_import() {
        let mut mesh = StlMesh::new();
        mesh.unit = "inch".to_string();
        mesh.triangles.push(tri([1.0, 0.0, 0.0]));
        let path = scratch("unit");
        export_stl(&mesh, &path).unwrap();
        let imported = import_stl(&path).expect("the exported STL must be importable");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            imported.unit, "inch",
            "import_stl must read the unit.from the header, not default it"
        );
    }

    /// A header without a `unit=` marker must keep the documented default rather
    /// than producing an empty unit string.
    #[test]
    fn test_stl_import_defaults_unit_when_header_has_no_marker() {
        let path = scratch("nounit");
        let mut data = vec![0u8; 84];
        data[80..84].copy_from_slice(&0u32.to_le_bytes());
        std::fs::write(&path, &data).unwrap();
        let imported = import_stl(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(imported.unit, "mm", "the default unit must survive");
    }

    #[test]
    fn test_stl_roundtrip_multiple_triangles_preserves_count() {
        let mut mesh = StlMesh::new();
        for i in 0..7 {
            mesh.triangles.push(tri([i as Scalar, 0.0, 0.0]));
        }
        let path = scratch("many");
        export_stl(&mesh, &path).unwrap();
        let imported = import_stl(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(imported.triangles.len(), 7, "all triangles must survive");
        for (i, t) in imported.triangles.iter().enumerate() {
            assert_eq!(t.normal[0], i as Scalar);
        }
    }

    /// The importer used to `break` out of its loop on a short record, silently
    /// returning a partial mesh while reporting success. It must now refuse.
    #[test]
    fn test_import_truncated_stl_is_rejected_not_silently_partial() {
        let mut mesh = StlMesh::new();
        for i in 0..4 {
            mesh.triangles.push(tri([i as Scalar, 0.0, 0.0]));
        }
        let path = scratch("truncated");
        export_stl(&mesh, &path).unwrap();

        // Chop off the last two triangle records, leaving the header intact.
        let full = std::fs::read(&path).unwrap();
        let truncated = &full[..BINARY_HEADER_LEN + 2 * TRIANGLE_RECORD_LEN];
        std::fs::write(&path, truncated).unwrap();

        let err = import_stl(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(
            err.contains("Truncated STL"),
            "expected an explicit truncation error, got: {err}"
        );
    }

    #[test]
    fn test_import_stl_shorter_than_header_is_rejected() {
        let path = scratch("tiny");
        std::fs::write(&path, [0u8; 10]).unwrap();
        let err = import_stl(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.contains("shorter than"), "got: {err}");
    }

    #[test]
    fn test_export_stl_rejects_non_finite_coordinates() {
        let mut mesh = StlMesh::new();
        mesh.triangles.push(StlTriangle {
            normal: [0.0, 0.0, 1.0],
            v1: Coord3D::new(Scalar::NAN, 0.0, 0.0),
            v2: Coord3D::new(1.0, 0.0, 0.0),
            v3: Coord3D::new(0.0, 1.0, 0.0),
        });
        let path = scratch("nan");
        let err = export_stl(&mesh, &path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(
            err.contains("non-finite"),
            "expected a non-finite rejection, got: {err}"
        );
    }

    #[test]
    fn test_import_stl_invalid() {
        assert!(import_stl("/tmp/nonexistent.stl").is_err());
    }
}
