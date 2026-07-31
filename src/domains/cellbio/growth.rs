//! Tissue growth model using a 3D lattice reaction-diffusion system.
//!
//! Provides GridModel for spatial cell culture simulation with
//! nutrient diffusion, metabolic reactions, and cell growth on a
//! 3D Cartesian grid.

use crate::core::types::Scalar;
use crate::domains::cellbio::cell_model::CellPopulation;
use crate::domains::cellbio::media::CultureMedia;
use crate::domains::molbio::forcefield::Vec3;
use std::collections::HashMap;

// ──────────────────────────────────────────────
// GridCell
// ──────────────────────────────────────────────

/// A single cell in the 3D grid model.
#[derive(Debug, Clone)]
pub struct GridCell {
    /// Cell occupant ID (if any).
    pub cell_occupant: Option<u64>,
    /// Nutrient concentrations by component name.
    pub nutrients: HashMap<String, Scalar>,
    /// Dissolved oxygen concentration (mol/m³).
    pub o2_conc: Scalar,
    /// pH value.
    pub ph: Scalar,
    /// Extracellular matrix density.
    pub ecm_density: Scalar,
    /// Growth factor concentration.
    pub growth_factor: Scalar,
}

impl GridCell {
    fn new() -> Self {
        let mut nutrients = HashMap::new();
        nutrients.insert("Glucose".to_string(), 25.0);
        Self {
            cell_occupant: None,
            nutrients,
            o2_conc: 0.2,
            ph: 7.4,
            ecm_density: 1.0,
            growth_factor: 0.0,
        }
    }
}

// ──────────────────────────────────────────────
// GridModel
// ──────────────────────────────────────────────

/// 3D lattice model for reaction-diffusion tissue growth.
#[derive(Debug, Clone)]
pub struct GridModel {
    /// 3D grid of cells.
    pub grid: Vec<Vec<Vec<GridCell>>>,
    /// Grid spacing (m).
    pub dx: Scalar,
    /// Grid dimensions.
    pub dimensions: (usize, usize, usize),
    /// Culture medium reference.
    pub media: CultureMedia,
}

impl GridModel {
    pub fn new(nx: usize, ny: usize, nz: usize, dx: Scalar, media: CultureMedia) -> Self {
        let grid = (0..nx)
            .map(|_| {
                (0..ny)
                    .map(|_| (0..nz).map(|_| GridCell::new()).collect())
                    .collect()
            })
            .collect();

        Self {
            grid,
            dx,
            dimensions: (nx, ny, nz),
            media,
        }
    }

    /// Seed cells at a specific grid position.
    pub fn seed_cells(
        &mut self,
        population: &mut CellPopulation,
        x: usize,
        y: usize,
        z: usize,
        n: usize,
    ) {
        let (nx, ny, nz) = self.dimensions;
        if x >= nx || y >= ny || z >= nz {
            return;
        }

        let pos = Vec3::new(
            x as Scalar * self.dx,
            y as Scalar * self.dx,
            z as Scalar * self.dx,
        );
        let first_idx = population.cells.len();
        population.seed(n, pos, 7.5e-6);

        // Record the actual first seeded cell ID in the grid cell, enabling
        // occupancy-based checks (e.g. contact inhibition) to use real data.
        self.grid[x][y][z].cell_occupant = population.cells.get(first_idx).map(|c| c.id);
    }

    /// Diffusion update using explicit Euler + central difference.
    pub fn diffuse(&mut self, dt: Scalar) {
        let (nx, ny, nz) = self.dimensions;
        let dx2 = self.dx * self.dx;

        // Diffuse each nutrient component
        let component_names: Vec<String> = {
            let mut names = Vec::new();
            if !self.grid.is_empty() && !self.grid[0].is_empty() && !self.grid[0][0].is_empty() {
                names = self.grid[0][0][0].nutrients.keys().cloned().collect();
                names.push("O2".to_string());
            }
            names
        };

        for name in &component_names {
            // Create a copy of current concentrations
            let mut curr = vec![vec![vec![0.0; nz]; ny]; nx];
            for (i, row) in curr.iter_mut().enumerate().take(nx) {
                for (j, col) in row.iter_mut().enumerate().take(ny) {
                    for (k, val) in col.iter_mut().enumerate().take(nz) {
                        *val = if name == "O2" {
                            self.grid[i][j][k].o2_conc
                        } else {
                            *self.grid[i][j][k].nutrients.get(name).unwrap_or(&0.0)
                        };
                    }
                }
            }

            // Apply diffusion: ∂c/∂t = D * ∇²c
            let d_coeff = if name == "O2" {
                2.1e-9
            } else {
                6.7e-10
            };

            let diff_factor = d_coeff * dt / dx2;
            if diff_factor > 0.25 {
                // Stability limit for explicit scheme
                continue;
            }

            #[allow(clippy::needless_range_loop)]
            for i in 1..nx.saturating_sub(1) {
                #[allow(clippy::needless_range_loop)]
                for j in 1..ny.saturating_sub(1) {
                    #[allow(clippy::needless_range_loop)]
                    for k in 1..nz.saturating_sub(1) {
                        let laplacian = curr[i + 1][j][k] + curr[i - 1][j][k]
                            + curr[i][j + 1][k]
                            + curr[i][j - 1][k]
                            + curr[i][j][k + 1]
                            + curr[i][j][k - 1]
                            - 6.0 * curr[i][j][k];
                        let new_val = curr[i][j][k] + diff_factor * laplacian;

                        if name == "O2" {
                            self.grid[i][j][k].o2_conc = new_val.max(0.0);
                        } else if let Some(v) = self.grid[i][j][k].nutrients.get_mut(name.as_str()) {
                            *v = new_val.max(0.0);
                        }
                    }
                }
            }
        }
    }

    /// Reaction update: cell consumption/production of nutrients.
    pub fn react(&mut self, population: &CellPopulation, dt: Scalar) {
        let (nx, ny, nz) = self.dimensions;

        for cell in &population.cells {
            // Map cell position to grid index
            let gx = ((cell.position.x / self.dx) as isize).clamp(0, nx as isize - 1) as usize;
            let gy = ((cell.position.y / self.dx) as isize).clamp(0, ny as isize - 1) as usize;
            let gz = ((cell.position.z / self.dx) as isize).clamp(0, nz as isize - 1) as usize;

            // Consume glucose
            if let Some(glucose) = self.grid[gx][gy][gz].nutrients.get_mut("Glucose") {
                *glucose -= cell.nutrient_uptake_rate * dt * 1000.0; // scale factor
                *glucose = glucose.max(0.0);
            }

            // Consume oxygen
            self.grid[gx][gy][gz].o2_conc -= cell.o2_consumption * dt * 1000.0;
            self.grid[gx][gy][gz].o2_conc = self.grid[gx][gy][gz].o2_conc.max(0.0);

            // Produce lactate
            if let Some(lactate) = self.grid[gx][gy][gz].nutrients.get_mut("Lactate") {
                *lactate += cell.lactate_production * dt * 1000.0;
            } else {
                self.grid[gx][gy][gz]
                    .nutrients
                    .insert("Lactate".to_string(), cell.lactate_production * dt * 1000.0);
            }
        }
    }

    /// Complete growth step: diffuse → react → update cells.
    pub fn step(
        &mut self,
        population: &mut CellPopulation,
        dt: Scalar,
    ) -> Result<(), String> {
        // Diffusion
        self.diffuse(dt);

        // Reaction
        self.react(population, dt);

        // Update cells — compute average conditions then update population once
        let (nx, ny, nz) = self.dimensions;
        let mut avg_nutrient = 0.0;
        let mut avg_o2 = 0.0;
        let mut count = 0;
        for cell in &population.cells {
            let gx = ((cell.position.x / self.dx) as isize).clamp(0, nx as isize - 1) as usize;
            let gy = ((cell.position.y / self.dx) as isize).clamp(0, ny as isize - 1) as usize;
            let gz = ((cell.position.z / self.dx) as isize).clamp(0, nz as isize - 1) as usize;
            avg_nutrient += self.grid[gx][gy][gz]
                .nutrients
                .get("Glucose")
                .copied()
                .unwrap_or(0.0);
            avg_o2 += self.grid[gx][gy][gz].o2_conc;
            count += 1;
        }
        if count > 0 {
            avg_nutrient /= count as Scalar;
            avg_o2 /= count as Scalar;
        }
        let _ = population.update(dt, avg_nutrient, avg_o2, self.grid[0][0][0].ph, self.media.temperature);

        Ok(())
    }

    /// Get nutrient concentration at a grid position.
    pub fn nutrient_at(&self, x: usize, y: usize, z: usize, name: &str) -> Option<Scalar> {
        let (nx, ny, nz) = self.dimensions;
        if x >= nx || y >= ny || z >= nz {
            return None;
        }
        if name == "O2" {
            Some(self.grid[x][y][z].o2_conc)
        } else {
            self.grid[x][y][z].nutrients.get(name).copied()
        }
    }

    /// Check contact inhibition at a grid position.
    pub fn contact_inhibition(&self, x: usize, y: usize, z: usize) -> bool {
        let (nx, ny, nz) = self.dimensions;
        if x >= nx || y >= ny || z >= nz {
            return true;
        }

        // Check if all 6 neighbors are occupied
        let mut occupied = 0;
        let mut total = 0;

        let dirs = [
            (-1, 0, 0),
            (1, 0, 0),
            (0, -1, 0),
            (0, 1, 0),
            (0, 0, -1),
            (0, 0, 1),
        ];
        for &(dx, dy, dz) in &dirs {
            let nx2 = x as isize + dx;
            let ny2 = y as isize + dy;
            let nz2 = z as isize + dz;
            if nx2 >= 0 && nx2 < nx as isize && ny2 >= 0 && ny2 < ny as isize && nz2 >= 0 && nz2 < nz as isize
            {
                total += 1;
                if self.grid[nx2 as usize][ny2 as usize][nz2 as usize]
                    .cell_occupant
                    .is_some()
                {
                    occupied += 1;
                }
            }
        }

        occupied >= total * 3 / 4 // 75% threshold
    }
}

// ──────────────────────────────────────────────
// Tissue Morphology Analysis
// ──────────────────────────────────────────────

/// Tissue morphology parameters.
#[derive(Debug, Clone)]
pub struct TissueMorphology {
    /// Total tissue volume (m³).
    pub total_volume: Scalar,
    /// Total cell count.
    pub cell_count: usize,
    /// Viable cell count.
    pub viable_count: usize,
    /// Average cell radius (m).
    pub avg_radius: Scalar,
    /// Necrotic core radius (m), if any.
    pub necrotic_core_radius: Option<Scalar>,
    /// Tissue surface area (m²).
    pub surface_area: Scalar,
    /// Compactness (dimensionless).
    pub compactness: Scalar,
}

/// Analyze tissue morphology from population and grid.
pub fn analyze_tissue_morphology(
    population: &CellPopulation,
    _grid: &GridModel,
) -> TissueMorphology {
    let cell_count = population.cells.len();
    let viable_count = population.viable_count();

    let avg_radius = if cell_count > 0 {
        population.cells.iter().map(|c| c.radius).sum::<Scalar>() / cell_count as Scalar
    } else {
        0.0
    };

    // Estimate volume from cell positions (bounding sphere)
    let total_volume = if cell_count > 0 {
        let com = population
            .cells
            .iter()
            .fold(Vec3::zero(), |acc, c| acc.add(&c.position))
            .scale(1.0 / cell_count as Scalar);
        let max_dist: Scalar = population
            .cells
            .iter()
            .map(|c| c.position.distance(&com))
            .fold(0.0, |a: Scalar, b| a.max(b));
        (4.0 / 3.0) * std::f64::consts::PI * max_dist.powi(3)
    } else {
        0.0
    };

    let surface_area = 4.0 * std::f64::consts::PI * (total_volume / (4.0 / 3.0 * std::f64::consts::PI)).powf(2.0 / 3.0);
    let compactness = if surface_area > 0.0 {
        (36.0 * std::f64::consts::PI * total_volume * total_volume).powf(1.0 / 3.0) / surface_area
    } else {
        0.0
    };

    TissueMorphology {
        total_volume,
        cell_count,
        viable_count,
        avg_radius,
        necrotic_core_radius: None,
        surface_area,
        compactness,
    }
}

/// Detect necrotic core (region where O₂ is below threshold).
pub fn detect_necrotic_core(grid: &GridModel, o2_threshold: Scalar) -> Option<Scalar> {
    let (nx, ny, nz) = grid.dimensions;
    let center_x = nx as Scalar / 2.0;
    let center_y = ny as Scalar / 2.0;
    let center_z = nz as Scalar / 2.0;

    let mut max_hypoxic_radius = 0.0;

    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                if grid.grid[i][j][k].o2_conc < o2_threshold {
                    let dist = ((i as Scalar - center_x).powi(2)
                        + (j as Scalar - center_y).powi(2)
                        + (k as Scalar - center_z).powi(2))
                    .sqrt();
                    if dist > max_hypoxic_radius {
                        max_hypoxic_radius = dist;
                    }
                }
            }
        }
    }

    if max_hypoxic_radius > 0.0 {
        Some(max_hypoxic_radius * grid.dx)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_creation() {
        let media = CultureMedia::new();
        let grid = GridModel::new(10, 10, 10, 1e-5, media);
        assert_eq!(grid.dimensions, (10, 10, 10));
    }

    #[test]
    fn test_diffuse_conservation() {
        let media = CultureMedia::new();
        let mut grid = GridModel::new(10, 10, 10, 1e-5, media);

        // Set initial glucose
        for i in 0..10 {
            for j in 0..10 {
                for k in 0..10 {
                    grid.grid[i][j][k]
                        .nutrients
                        .insert("Glucose".to_string(), 25.0);
                }
            }
        }

        let initial_total: Scalar = {
            let mut total = 0.0;
            for i in 0..10 {
                for j in 0..10 {
                    for k in 0..10 {
                        total += grid.grid[i][j][k].nutrients.get("Glucose").copied().unwrap_or(0.0);
                    }
                }
            }
            total
        };

        grid.diffuse(1.0);

        let final_total: Scalar = {
            let mut total = 0.0;
            for i in 0..10 {
                for j in 0..10 {
                    for k in 0..10 {
                        total += grid.grid[i][j][k].nutrients.get("Glucose").copied().unwrap_or(0.0);
                    }
                }
            }
            total
        };

        // Concentration should be approximately conserved
        assert!((final_total - initial_total).abs() < 1.0);
    }

    #[test]
    fn test_nutrient_at() {
        let media = CultureMedia::new();
        let grid = GridModel::new(10, 10, 10, 1e-5, media);
        let conc = grid.nutrient_at(1, 1, 1, "Glucose");
        assert!(conc.is_some());
        assert!(conc.unwrap() > 0.0);
    }

    #[test]
    fn test_contact_inhibition() {
        let media = CultureMedia::new();
        let grid = GridModel::new(10, 10, 10, 1e-5, media);
        let inhibited = grid.contact_inhibition(0, 0, 0);
        // Corner should not be inhibited
        assert!(!inhibited);
    }

    #[test]
    fn test_tissue_morphology() {
        let mut pop = CellPopulation::new();
        pop.seed(50, Vec3::new(0.0, 0.0, 0.0), 7.5e-6);
        let media = CultureMedia::new();
        let grid = GridModel::new(10, 10, 10, 1e-5, media);

        let morph = analyze_tissue_morphology(&pop, &grid);
        assert_eq!(morph.cell_count, 50);
        // Cells are at same position, so bounding sphere radius is small
        assert!(morph.total_volume >= 0.0);
    }

    #[test]
    fn test_detect_necrotic_core() {
        let media = CultureMedia::new();
        let mut grid = GridModel::new(10, 10, 10, 1e-5, media);
        // Set the entire center region to hypoxic
        for i in 4..=6 {
            for j in 4..=6 {
                for k in 4..=6 {
                    grid.grid[i][j][k].o2_conc = 0.0001;
                }
            }
        }
        let core = detect_necrotic_core(&grid, 0.001);
        assert!(core.is_some());
    }
}
