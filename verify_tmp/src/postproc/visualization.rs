//! Chart, contour, and vector field visualization.

use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

/// Data curve for plotting.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CurveData {
    pub x_values: Vec<Scalar>,
    pub y_values: Vec<Scalar>,
    pub label: String,
    pub color: Option<String>,
}

impl CurveData {
    pub fn new(x: Vec<Scalar>, y: Vec<Scalar>, label: &str) -> Self {
        Self {
            x_values: x,
            y_values: y,
            label: label.to_string(),
            color: None,
        }
    }
    pub fn with_color(mut self, c: &str) -> Self {
        self.color = Some(c.to_string());
        self
    }
}

/// Supported chart types.
pub enum ChartType {
    Line,
    Scatter,
    Bar,
    Histogram,
    Contour,
    Surface3D,
    VectorField,
}

/// Chart generator producing SVG/JSON output.
pub struct ChartGenerator {
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub curves: Vec<CurveData>,
    pub chart_type: ChartType,
}

impl ChartGenerator {
    pub fn new(title: &str, x_label: &str, y_label: &str) -> Self {
        Self {
            title: title.to_string(),
            x_label: x_label.to_string(),
            y_label: y_label.to_string(),
            curves: Vec::new(),
            chart_type: ChartType::Line,
        }
    }
    pub fn add_curve(&mut self, curve: CurveData) {
        self.curves.push(curve);
    }

    pub fn to_svg(&self) -> Result<String, String> {
        if self.curves.is_empty() {
            return Err("No curves to plot".to_string());
        }
        let mut svg = format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 800 500'>\n");
        svg.push_str(&format!(
            "<text x='400' y='30' text-anchor='middle' font-size='16'>{}</text>\n",
            self.title
        ));
        svg.push_str(&format!(
            "<text x='400' y='480' text-anchor='middle'>{}</text>\n",
            self.x_label
        ));
        svg.push_str(&format!(
            "<text x='20' y='250' text-anchor='middle' transform='rotate(-90,20,250)'>{}</text>\n",
            self.y_label
        ));

        let colors = ["#e41a1c", "#377eb8", "#4daf4a", "#984ea3", "#ff7f00"];
        let mut x_min = Scalar::MAX;
        let mut x_max = Scalar::MIN;
        let mut y_min = Scalar::MAX;
        let mut y_max = Scalar::MIN;
        for c in &self.curves {
            for &x in &c.x_values {
                x_min = x_min.min(x);
                x_max = x_max.max(x);
            }
            for &y in &c.y_values {
                y_min = y_min.min(y);
                y_max = y_max.max(y);
            }
        }
        let margin = 60.0;
        let pw = 700.0;
        let ph = 400.0;
        let x_range = (x_max - x_min).max(1e-10);
        let y_range = (y_max - y_min).max(1e-10);

        for (i, curve) in self.curves.iter().enumerate() {
            let color = colors.get(i).unwrap_or(&"#000000");
            if curve.x_values.len() < 2 {
                continue;
            }
            svg.push_str(&format!(
                "<polyline fill='none' stroke='{}' stroke-width='2' points='",
                color
            ));
            for j in 0..curve.x_values.len() {
                let px = margin + (curve.x_values[j] - x_min) / x_range * pw;
                let py = 60.0 + ph - (curve.y_values[j] - y_min) / y_range * ph;
                svg.push_str(&format!("{:.1},{:.1} ", px, py));
            }
            svg.push_str("'/>\n");
            svg.push_str(&format!(
                "<text x='{}' y='{}' fill='{}'>{}</text>\n",
                margin,
                55.0 + i as Scalar * 20.0,
                color,
                curve.label
            ));
        }
        svg.push_str("</svg>");
        Ok(svg)
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.curves).map_err(|e| format!("JSON error: {}", e))
    }
}

/// Contour generator for 2D field data.
pub struct ContourGenerator {
    pub x_grid: Vec<Scalar>,
    pub y_grid: Vec<Scalar>,
    pub z_values: Vec<Vec<Scalar>>,
}

impl ContourGenerator {
    pub fn new(x: Vec<Scalar>, y: Vec<Scalar>, z: Vec<Vec<Scalar>>) -> Self {
        Self {
            x_grid: x,
            y_grid: y,
            z_values: z,
        }
    }

    pub fn contours(&self, num_levels: usize) -> Vec<(Scalar, Vec<[Scalar; 2]>)> {
        let mut levels = Vec::new();
        if self.z_values.is_empty() {
            return levels;
        }
        let mut z_min = Scalar::MAX;
        let mut z_max = Scalar::MIN;
        for row in &self.z_values {
            for &z in row {
                z_min = z_min.min(z);
                z_max = z_max.max(z);
            }
        }
        let step = (z_max - z_min) / (num_levels.max(2) - 1) as Scalar;
        for l in 0..num_levels {
            let level_val = z_min + l as Scalar * step;
            // Simplified contour: just return grid points near this level
            let mut pts = Vec::new();
            for (i, row) in self.z_values.iter().enumerate() {
                for (j, &z) in row.iter().enumerate() {
                    if (z - level_val).abs() < step * 0.5 {
                        if i < self.x_grid.len() && j < self.y_grid.len() {
                            pts.push([self.x_grid[i], self.y_grid[j]]);
                        }
                    }
                }
            }
            levels.push((level_val, pts));
        }
        levels
    }

    pub fn color_map(&self) -> Vec<Vec<(Scalar, Scalar, Scalar)>> {
        self.z_values
            .iter()
            .map(|row| {
                let row_max = row.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let row_min = row.iter().cloned().fold(f64::INFINITY, f64::min);
                let range = (row_max - row_min).max(1e-10);
                row.iter()
                    .map(|&z| {
                        let t = (z - row_min) / range;
                        (t, 0.0, 1.0 - t) // Blue → Red
                    })
                    .collect()
            })
            .collect()
    }
}

/// 3D vector field visualization.
pub struct VectorFieldVisualization {
    pub positions: Vec<Coord3D>,
    pub vectors: Vec<[Scalar; 3]>,
    pub scale: Scalar,
}

impl VectorFieldVisualization {
    pub fn new(positions: Vec<Coord3D>, vectors: Vec<[Scalar; 3]>) -> Self {
        Self {
            positions,
            vectors,
            scale: 1.0,
        }
    }

    pub fn arrow_data(&self) -> Vec<([Scalar; 3], [Scalar; 3])> {
        self.positions
            .iter()
            .zip(self.vectors.iter())
            .map(|(p, v)| {
                (
                    [p.x, p.y, p.z],
                    [v[0] * self.scale, v[1] * self.scale, v[2] * self.scale],
                )
            })
            .collect()
    }
}

// ── 3D Volume & Slice Extensions ──────────────────────────────────────────

/// A 2D slice extracted from a 3D scalar field.
#[derive(Debug, Clone)]
pub struct VolumeSlice3D {
    pub slice_axis: char, // 'x', 'y', or 'z'
    pub slice_index: usize,
    pub grid: Vec<Vec<Scalar>>, // 2D grid of values
    pub x_coords: Vec<Scalar>,
    pub y_coords: Vec<Scalar>,
    pub min_val: Scalar,
    pub max_val: Scalar,
}

impl VolumeSlice3D {
    /// Extract a 2D slice from a 3D scalar field stored as
    /// `data[k][j][i]` where indexing is `[z][y][x]`.
    pub fn from_3d_field(
        data: &[Vec<Vec<Scalar>>],
        axis: char,
        index: usize,
        dx: Scalar,
        dy: Scalar,
        dz: Scalar,
    ) -> Result<Self, String> {
        if data.is_empty() || data[0].is_empty() || data[0][0].is_empty() {
            return Err("Empty 3D field".to_string());
        }
        let nz = data.len();
        let ny = data[0].len();
        let nx = data[0][0].len();

        let (grid, x_coords, y_coords) = match axis {
            'z' => {
                // Slice at constant z (k fixed)
                if index >= nz {
                    return Err(format!("z index {} out of range [0, {})", index, nz));
                }
                let slice = data[index].clone();
                let xs: Vec<Scalar> = (0..nx).map(|i| i as Scalar * dx).collect();
                let ys: Vec<Scalar> = (0..ny).map(|j| j as Scalar * dy).collect();
                (slice, xs, ys)
            }
            'y' => {
                // Slice at constant y (j fixed)
                if index >= ny {
                    return Err(format!("y index {} out of range [0, {})", index, ny));
                }
                let mut slice = vec![vec![0.0; nx]; nz];
                for k in 0..nz {
                    for i in 0..nx {
                        slice[k][i] = data[k][index][i];
                    }
                }
                let xs: Vec<Scalar> = (0..nx).map(|i| i as Scalar * dx).collect();
                let ys: Vec<Scalar> = (0..nz).map(|k| k as Scalar * dz).collect();
                (slice, xs, ys)
            }
            'x' => {
                // Slice at constant x (i fixed)
                if index >= nx {
                    return Err(format!("x index {} out of range [0, {})", index, nx));
                }
                let mut slice = vec![vec![0.0; ny]; nz];
                for k in 0..nz {
                    for j in 0..ny {
                        slice[k][j] = data[k][j][index];
                    }
                }
                let xs: Vec<Scalar> = (0..ny).map(|j| j as Scalar * dy).collect();
                let ys: Vec<Scalar> = (0..nz).map(|k| k as Scalar * dz).collect();
                (slice, xs, ys)
            }
            _ => return Err(format!("Unknown axis '{}' (must be x, y, or z)", axis)),
        };

        let mut min_val = Scalar::MAX;
        let mut max_val = Scalar::MIN;
        for row in &grid {
            for &v in row {
                min_val = min_val.min(v);
                max_val = max_val.max(v);
            }
        }

        Ok(Self {
            slice_axis: axis,
            slice_index: index,
            grid,
            x_coords,
            y_coords,
            min_val,
            max_val,
        })
    }

    /// Generate an SVG heatmap representation of the slice.
    pub fn to_svg_heatmap(&self, width: usize, height: usize) -> String {
        let nx = self.x_coords.len();
        let ny = self.y_coords.len();
        if nx == 0 || ny == 0 {
            return String::new();
        }
        let range = (self.max_val - self.min_val).max(1e-30);

        let cell_w = width as Scalar / nx as Scalar;
        let cell_h = height as Scalar / ny as Scalar;

        let mut svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {} {}'>\n",
            width, height
        );
        svg.push_str(&format!(
            "<text x='{}' y='20' text-anchor='middle' font-size='14'>Slice {}={}</text>\n",
            width / 2,
            self.slice_axis,
            self.slice_index
        ));

        for j in 0..ny {
            for i in 0..nx {
                let t = (self.grid[j][i] - self.min_val) / range;
                let r = (t * 255.0) as u8;
                let b = ((1.0 - t) * 255.0) as u8;
                let g = 128u8;
                svg.push_str(&format!(
                    "<rect x='{:.1}' y='{:.1}' width='{:.1}' height='{:.1}' fill='rgb({},{},{})'/>\n",
                    i as Scalar * cell_w,
                    j as Scalar * cell_h,
                    cell_w,
                    cell_h,
                    r,
                    g,
                    b
                ));
            }
        }

        svg.push_str("</svg>");
        svg
    }
}

/// Iso-surface extraction from a 3D scalar field (marching-cubes like).
#[derive(Debug, Clone)]
pub struct IsoSurface3D {
    pub vertices: Vec<Coord3D>,
    pub normals: Vec<[Scalar; 3]>,
    pub iso_value: Scalar,
}

impl IsoSurface3D {
    /// Extract an iso-surface at the given value using a simple grid-scan.
    /// Returns vertices where the field crosses the iso-value.
    pub fn extract(
        field: &[Vec<Vec<Scalar>>],
        iso_value: Scalar,
        dx: Scalar,
        dy: Scalar,
        dz: Scalar,
    ) -> Self {
        let mut vertices = Vec::new();
        let mut normals = Vec::new();

        if field.is_empty() || field[0].is_empty() || field[0][0].is_empty() {
            return Self {
                vertices,
                normals,
                iso_value,
            };
        }

        let nz = field.len();
        let ny = field[0].len();
        let nx = field[0][0].len();

        // Scan grid cells and find edges crossing the iso-value
        for k in 0..nz.saturating_sub(1) {
            for j in 0..ny.saturating_sub(1) {
                for i in 0..nx.saturating_sub(1) {
                    let v000 = field[k][j][i];
                    let v100 = field[k][j][i + 1];
                    let v010 = field[k][j + 1][i];
                    let v001 = field[k + 1][j][i];

                    // Check x-edge crossings
                    if (v000 - iso_value) * (v100 - iso_value) < 0.0 {
                        let t = (iso_value - v000) / (v100 - v000).max(1e-30);
                        let x = (i as Scalar + t) * dx;
                        let y = j as Scalar * dy;
                        let z = k as Scalar * dz;
                        vertices.push(Coord3D::new(x, y, z));
                        normals.push([1.0, 0.0, 0.0]);
                    }
                    // Check y-edge crossings
                    if (v000 - iso_value) * (v010 - iso_value) < 0.0 {
                        let t = (iso_value - v000) / (v010 - v000).max(1e-30);
                        let x = i as Scalar * dx;
                        let y = (j as Scalar + t) * dy;
                        let z = k as Scalar * dz;
                        vertices.push(Coord3D::new(x, y, z));
                        normals.push([0.0, 1.0, 0.0]);
                    }
                    // Check z-edge crossings
                    if (v000 - iso_value) * (v001 - iso_value) < 0.0 {
                        let t = (iso_value - v000) / (v001 - v000).max(1e-30);
                        let x = i as Scalar * dx;
                        let y = j as Scalar * dy;
                        let z = (k as Scalar + t) * dz;
                        vertices.push(Coord3D::new(x, y, z));
                        normals.push([0.0, 0.0, 1.0]);
                    }
                }
            }
        }

        Self {
            vertices,
            normals,
            iso_value,
        }
    }

    /// Number of extracted vertices.
    pub fn num_vertices(&self) -> usize {
        self.vertices.len()
    }
}

/// 3D vector field slice extraction.
pub fn vector_field_slice(
    positions: &[Coord3D],
    vectors: &[[Scalar; 3]],
    axis: char,
    index: Scalar,
    tolerance: Scalar,
) -> Vec<(Coord3D, [Scalar; 3])> {
    positions
        .iter()
        .zip(vectors.iter())
        .filter(|(p, _)| match axis {
            'x' => (p.x - index).abs() < tolerance,
            'y' => (p.y - index).abs() < tolerance,
            'z' => (p.z - index).abs() < tolerance,
            _ => false,
        })
        .map(|(p, v)| (*p, *v))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_curve_data() {
        let c = CurveData::new(vec![0.0, 1.0], vec![0.0, 1.0], "test");
        assert_eq!(c.label, "test");
    }
    #[test]
    fn test_chart_svg() {
        let mut chart = ChartGenerator::new("Test", "x", "y");
        chart.add_curve(CurveData::new(
            vec![0.0, 1.0, 2.0],
            vec![0.0, 1.0, 0.0],
            "sine",
        ));
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("sine"));
    }
    #[test]
    fn test_chart_empty_error() {
        let chart = ChartGenerator::new("Empty", "x", "y");
        assert!(chart.to_svg().is_err());
    }
    #[test]
    fn test_chart_json() {
        let mut chart = ChartGenerator::new("Test", "x", "y");
        chart.add_curve(CurveData::new(vec![1.0], vec![2.0], "p"));
        assert!(chart.to_json().is_ok());
    }
    #[test]
    fn test_contour_generator() {
        let cg = ContourGenerator::new(
            vec![0.0, 1.0],
            vec![0.0, 1.0],
            vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        );
        let levels = cg.contours(5);
        assert_eq!(levels.len(), 5);
    }
    #[test]
    fn test_color_map() {
        let cg = ContourGenerator::new(
            vec![0.0, 1.0],
            vec![0.0, 1.0],
            vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        );
        let cmap = cg.color_map();
        assert_eq!(cmap.len(), 2);
    }
    #[test]
    fn test_vector_field_arrow_data() {
        let vf =
            VectorFieldVisualization::new(vec![Coord3D::new(0.0, 0.0, 0.0)], vec![[1.0, 0.0, 0.0]]);
        let arrows = vf.arrow_data();
        assert_eq!(arrows.len(), 1);
        assert!((arrows[0].1[0] - 1.0).abs() < 1e-10);
    }
    // ── 3D slice tests ──────────────────────────────────────────────────
    #[test]
    fn test_volume_slice_z() {
        // 3D field: 4×4×4, value = k+j+i
        let data: Vec<Vec<Vec<Scalar>>> = (0..4)
            .map(|k| {
                (0..4)
                    .map(|j| (0..4).map(|i| (k + j + i) as Scalar).collect())
                    .collect()
            })
            .collect();
        let slice = VolumeSlice3D::from_3d_field(&data, 'z', 2, 0.5, 0.5, 0.5).unwrap();
        assert_eq!(slice.slice_axis, 'z');
        assert_eq!(slice.grid.len(), 4); // ny rows
        assert_eq!(slice.grid[0].len(), 4); // nx cols
        // At z=2 (k=2): value = 2 + j + i; centre should be higher
        assert!(slice.grid[2][2] > slice.grid[0][0]);
        // SVG output
        let svg = slice.to_svg_heatmap(200, 200);
        assert!(svg.contains("<svg"));
    }
    #[test]
    fn test_volume_slice_y() {
        let data: Vec<Vec<Vec<Scalar>>> = (0..3)
            .map(|_k| {
                (0..3)
                    .map(|j| (0..3).map(|i| i as Scalar * 10.0 + j as Scalar).collect())
                    .collect()
            })
            .collect();
        let slice = VolumeSlice3D::from_3d_field(&data, 'y', 1, 0.2, 0.2, 0.2).unwrap();
        assert_eq!(slice.slice_axis, 'y');
        assert_eq!(slice.grid.len(), 3);
    }
    #[test]
    fn test_volume_slice_x() {
        let data: Vec<Vec<Vec<Scalar>>> = (0..3)
            .map(|k| {
                (0..3)
                    .map(|j| (0..3).map(|i| (i + j + k) as Scalar).collect())
                    .collect()
            })
            .collect();
        let slice = VolumeSlice3D::from_3d_field(&data, 'x', 1, 1.0, 1.0, 1.0).unwrap();
        assert_eq!(slice.slice_axis, 'x');
        assert!(slice.max_val >= slice.min_val);
    }
    #[test]
    fn test_volume_slice_empty() {
        let data: Vec<Vec<Vec<Scalar>>> = vec![];
        assert!(VolumeSlice3D::from_3d_field(&data, 'z', 0, 1.0, 1.0, 1.0).is_err());
    }
    #[test]
    fn test_volume_slice_bad_axis() {
        let data: Vec<Vec<Vec<Scalar>>> = vec![vec![vec![0.0; 2]; 2]; 2];
        assert!(VolumeSlice3D::from_3d_field(&data, 'w', 0, 1.0, 1.0, 1.0).is_err());
    }
    #[test]
    fn test_volume_slice_out_of_range() {
        let data: Vec<Vec<Vec<Scalar>>> = vec![vec![vec![0.0; 2]; 2]; 2];
        assert!(VolumeSlice3D::from_3d_field(&data, 'z', 5, 1.0, 1.0, 1.0).is_err());
    }
    // ── Iso-surface tests ───────────────────────────────────────────────
    #[test]
    fn test_iso_surface_extract() {
        // A 3D field with a gradient: val = i + j + k (creates a diagonal surface).
        // Values range from 0..9 in a 4×4×4 grid.
        let field: Vec<Vec<Vec<Scalar>>> = (0..4)
            .map(|k| {
                (0..4)
                    .map(|j| (0..4).map(|i| (i + j + k) as Scalar).collect())
                    .collect()
            })
            .collect();
        // Use iso_value = 2.5 to ensure edge crossings (some cells span 2.5)
        // Cell (0,0,0): v000=0, v100=1 — both < 2.5; but cell (1,1,0): v000=2, v100=3 so crossing
        let iso = IsoSurface3D::extract(&field, 2.5, 1.0, 1.0, 1.0);
        // Should find some edge crossings
        assert!(
            iso.num_vertices() > 0,
            "expected >0 vertices, got {}",
            iso.num_vertices()
        );
        assert!((iso.iso_value - 2.5).abs() < 1e-10);
    }
    #[test]
    fn test_iso_surface_empty() {
        let field: Vec<Vec<Vec<Scalar>>> = vec![];
        let iso = IsoSurface3D::extract(&field, 1.0, 1.0, 1.0, 1.0);
        assert_eq!(iso.num_vertices(), 0);
    }
    #[test]
    fn test_iso_surface_uniform() {
        // Uniform field above iso-value → no crossings
        let field: Vec<Vec<Vec<Scalar>>> = vec![vec![vec![10.0; 3]; 3]; 3];
        let iso = IsoSurface3D::extract(&field, 5.0, 1.0, 1.0, 1.0);
        assert_eq!(iso.num_vertices(), 0);
    }
    // ── Vector field slice test ─────────────────────────────────────────
    #[test]
    fn test_vector_field_slice() {
        let positions = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.5, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
        ];
        let vectors = vec![[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
        let slice = vector_field_slice(&positions, &vectors, 'y', 0.0, 0.1);
        assert_eq!(slice.len(), 3); // all have y=0
        let slice2 = vector_field_slice(&positions, &vectors, 'x', 0.5, 0.01);
        assert_eq!(slice2.len(), 1);
    }
}
