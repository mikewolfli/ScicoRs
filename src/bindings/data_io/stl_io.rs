//! STL (stereolithography) file import/export.
//!
//! Supports binary STL format for triangle mesh data.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// A single STL triangle.
pub struct StlTriangle {
    pub normal: [Scalar; 3],
    pub v1: Coord3D,
    pub v2: Coord3D,
    pub v3: Coord3D,
}

/// STL mesh data.
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

/// Import binary STL file.
pub fn import_stl(filepath: &str) -> Result<StlMesh, String> {
    let data = std::fs::read(filepath).map_err(|e| format!("STL read error: {}", e))?;
    if data.len() < 84 {
        return Err("Invalid STL file".to_string());
    }
    let num_triangles = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
    let mut mesh = StlMesh::new();
    for i in 0..num_triangles {
        let offset = 84 + i * 50;
        if offset + 50 > data.len() {
            break;
        }
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

/// Export as binary STL file.
pub fn export_stl(mesh: &StlMesh, filepath: &str) -> Result<(), String> {
    let mut data: Vec<u8> = Vec::new();
    // 80-byte header
    data.extend_from_slice(&[0u8; 80]);
    // Number of triangles
    let n = mesh.triangles.len() as u32;
    data.extend_from_slice(&n.to_le_bytes());
    for tri in &mesh.triangles {
        for &v in &[
            tri.normal,
            [tri.v1.x, tri.v1.y, tri.v1.z],
            [tri.v2.x, tri.v2.y, tri.v2.z],
            [tri.v3.x, tri.v3.y, tri.v3.z],
        ] {
            for &coord in &v {
                data.extend_from_slice(&(coord as f32).to_le_bytes());
            }
        }
        data.extend_from_slice(&[0u8; 2]); // attribute
    }
    std::fs::write(filepath, &data).map_err(|e| format!("STL write error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stl_mesh_creation() {
        let m = StlMesh::new();
        assert!(m.triangles.is_empty());
    }

    #[test]
    fn test_export_stl_triangle() {
        let mut mesh = StlMesh::new();
        mesh.triangles.push(StlTriangle {
            normal: [0.0, 0.0, 1.0],
            v1: Coord3D::new(0.0, 0.0, 0.0),
            v2: Coord3D::new(1.0, 0.0, 0.0),
            v3: Coord3D::new(0.0, 1.0, 0.0),
        });
        let path = "/tmp/test_export.stl";
        assert!(export_stl(&mesh, path).is_ok());
        let imported = import_stl(path).unwrap();
        assert_eq!(imported.triangles.len(), 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_import_stl_invalid() {
        assert!(import_stl("/tmp/nonexistent.stl").is_err());
    }
}
