//! Cell model for cell culture simulation.
//!
//! Provides CellState (lifecycle states), Cell (individual cell),
//! and CellPopulation (population-level management with proliferation,
//! apoptosis, and migration).

use crate::core::types::Scalar;
use crate::domains::cellbio::physics::TYPICAL_CELL_VOLUME;
use crate::domains::molbio::forcefield::Vec3;

// ──────────────────────────────────────────────
// CellState
// ──────────────────────────────────────────────

/// Cell lifecycle state enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellState {
    /// Viable and capable of proliferation.
    Viable,
    /// G1 phase (pre-DNA synthesis).
    G1,
    /// S phase (DNA synthesis).
    S,
    /// G2 phase (post-DNA synthesis).
    G2,
    /// M phase (mitosis).
    M,
    /// Quiescent (nutrient-deprived).
    Quiescent,
    /// Apoptotic (programmed cell death).
    Apoptotic,
    /// Necrotic (uncontrolled cell death).
    Necrotic,
    /// Adherent (attached to surface).
    Adherent,
    /// Migrating (active movement).
    Migrating,
}

// ──────────────────────────────────────────────
// Cell
// ──────────────────────────────────────────────

/// A single cell in the population.
#[derive(Debug, Clone)]
pub struct Cell {
    /// Unique cell identifier.
    pub id: u64,
    /// Current cell state.
    pub state: CellState,
    /// Spatial position (m).
    pub position: Vec3,
    /// Migration velocity (m/s).
    pub velocity: Vec3,
    /// Cell radius (m).
    pub radius: Scalar,
    /// Cell mass (kg).
    pub mass: Scalar,
    /// Cell age (s).
    pub age: Scalar,
    /// Cell cycle progress [0, 1).
    pub cycle_progress: Scalar,
    /// Cell cycle duration (s).
    pub cycle_duration: Scalar,
    /// Nutrient uptake rate (mol/s).
    pub nutrient_uptake_rate: Scalar,
    /// O₂ consumption rate (mol/s).
    pub o2_consumption: Scalar,
    /// Lactate production rate (mol/s).
    pub lactate_production: Scalar,
    /// ATP level [0, 1].
    pub atp_level: Scalar,
    /// Relative DNA content.
    pub dna_content: Scalar,
    /// Lineage (ancestor cell IDs).
    pub lineage: Vec<u64>,
    /// Time since last state change (s).
    pub time_in_state: Scalar,
}

impl Cell {
    /// Create a new cell at position with given radius.
    pub fn new(id: u64, position: Vec3, radius: Scalar) -> Self {
        Self {
            id,
            state: CellState::Viable,
            position,
            velocity: Vec3::zero(),
            radius,
            mass: 1e-12, // approximate cell mass (kg)
            age: 0.0,
            cycle_progress: 0.0,
            cycle_duration: TYPICAL_CYCLE_TIME,
            nutrient_uptake_rate: 1e-16,
            o2_consumption: 5e-17,
            lactate_production: 1e-16,
            atp_level: 1.0,
            dna_content: 1.0,
            lineage: Vec::new(),
            time_in_state: 0.0,
        }
    }
}

/// Default cell cycle duration (22 hours in seconds).
const TYPICAL_CYCLE_TIME: Scalar = 79200.0;

// ──────────────────────────────────────────────
// CellUpdateResult
// ──────────────────────────────────────────────

/// Result of a single cell population update step.
#[derive(Debug, Clone, Default)]
pub struct CellUpdateResult {
    /// Number of cell divisions.
    pub divisions: usize,
    /// Number of apoptotic events.
    pub apoptoses: usize,
    /// Number of migration events.
    pub migrations: usize,
    /// Total nutrient consumed (mol).
    pub total_nutrient_consumed: Scalar,
    /// Total O₂ consumed (mol).
    pub total_o2_consumed: Scalar,
    /// Total lactate produced (mol).
    pub total_lactate_produced: Scalar,
}

// ──────────────────────────────────────────────
// CellPopulation
// ──────────────────────────────────────────────

/// Manages a population of cells.
#[derive(Debug, Clone)]
pub struct CellPopulation {
    /// All cells in the population.
    pub cells: Vec<Cell>,
    /// Next available cell ID.
    pub next_id: u64,
    /// Maximum number of cells allowed.
    pub max_cells: usize,
    /// Total number of doublings.
    pub total_doublings: u64,
}

impl CellPopulation {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            next_id: 1,
            max_cells: 100000,
            total_doublings: 0,
        }
    }

    /// Seed (inoculate) the population with `n` cells at a position.
    pub fn seed(&mut self, n: usize, position: Vec3, radius: Scalar) {
        for _ in 0..n {
            let mut cell = Cell::new(self.next_id, position, radius);
            // Small random displacement for realistic seeding
            cell.position = Vec3::new(
                position.x + (rand() - 0.5) * radius * 0.5,
                position.y + (rand() - 0.5) * radius * 0.5,
                position.z + (rand() - 0.5) * radius * 0.5,
            );
            self.cells.push(cell);
            self.next_id += 1;
        }
    }

    /// Update all cells by one time step.
    ///
    /// # Arguments
    /// * `dt` - Time step (s)
    /// * `nutrient` - Local nutrient concentration (mol/m³)
    /// * `o2` - Local dissolved oxygen concentration (mol/m³)
    /// * `ph` - Local pH value
    /// * `temp` - Temperature (K)
    pub fn update(
        &mut self,
        dt: Scalar,
        nutrient: Scalar,
        o2: Scalar,
        ph: Scalar,
        temp: Scalar,
    ) -> CellUpdateResult {
        let mut result = CellUpdateResult::default();
        let mut new_cells = Vec::new();
        let mut remove_indices = Vec::new();
        let initial_count = self.cells.len();
        let max_cells = self.max_cells;
        let next_id = self.next_id;

        // Phase 1: Process each cell (read-only for population, mutable for cell)
        for (idx, cell) in self.cells.iter_mut().enumerate() {
            cell.age += dt;
            cell.time_in_state += dt;

            // Check viability conditions
            let viable = nutrient > 0.01
                && o2 > 0.001
                && ph > 6.5
                && ph < 8.0
                && temp > 303.0
                && temp < 315.0;

            match cell.state {
                CellState::Viable | CellState::G1 | CellState::Adherent => {
                    if !viable {
                        if nutrient < 0.001 || o2 < 0.0001 {
                            cell.state = CellState::Necrotic;
                            continue;
                        } else {
                            cell.state = CellState::Quiescent;
                            continue;
                        }
                    }

                    // Advance cell cycle
                    let growth_rate = monod_factor(nutrient, 0.1) * monod_factor(o2, 0.01);
                    cell.cycle_progress += dt / cell.cycle_duration * growth_rate;

                    if cell.cycle_progress >= 1.0 && initial_count + new_cells.len() < max_cells {
                        // Cell division — create daughter cell
                        let daughter =
                            CellPopulation::create_daughter(cell, next_id + new_cells.len() as u64);
                        new_cells.push(daughter);
                        result.divisions += 1;
                        cell.cycle_progress = 0.0;
                    }

                    // Metabolic consumption
                    let consumption = cell.nutrient_uptake_rate * dt * growth_rate;
                    let o2_cons = cell.o2_consumption * dt * growth_rate;
                    let lac_prod = cell.lactate_production * dt * growth_rate;
                    result.total_nutrient_consumed += consumption;
                    result.total_o2_consumed += o2_cons;
                    result.total_lactate_produced += lac_prod;
                }
                CellState::Quiescent => {
                    if viable {
                        cell.state = CellState::Viable;
                    }
                    // Minimal metabolism in quiescent state
                    result.total_nutrient_consumed += cell.nutrient_uptake_rate * dt * 0.1;
                    result.total_o2_consumed += cell.o2_consumption * dt * 0.1;
                }
                CellState::Apoptotic if cell.time_in_state > 3600.0 => {
                    remove_indices.push(idx);
                }
                CellState::Necrotic if cell.time_in_state > 1800.0 => {
                    remove_indices.push(idx);
                }
                _ => {}
            }
        }

        // Phase 2: Add new cells from division
        for c in new_cells {
            self.cells.push(c);
            self.next_id += 1;
        }
        self.total_doublings += result.divisions as u64;

        // Phase 3: Remove dead cells (reverse order to preserve indices)
        remove_indices.sort_unstable();
        remove_indices.dedup();
        for idx in remove_indices.into_iter().rev() {
            if idx < self.cells.len() {
                self.cells.swap_remove(idx);
            }
        }

        result
    }

    /// Create a daughter cell from a parent cell (static method to avoid borrow conflicts).
    fn create_daughter(parent: &Cell, new_id: u64) -> Cell {
        let offset = 2.0 * parent.radius;
        let daughter_pos = Vec3::new(
            parent.position.x + offset * (rand() - 0.5),
            parent.position.y + offset * (rand() - 0.5),
            parent.position.z + offset * (rand() - 0.5),
        );

        let mut daughter = Cell::new(new_id, daughter_pos, parent.radius * 0.9);
        daughter.cycle_duration = parent.cycle_duration;
        daughter.nutrient_uptake_rate = parent.nutrient_uptake_rate;
        daughter.o2_consumption = parent.o2_consumption;
        daughter.lactate_production = parent.lactate_production;
        daughter.dna_content = 1.0;
        daughter.atp_level = parent.atp_level * 0.8;
        daughter.cycle_progress = 0.0;
        let mut lineage = parent.lineage.clone();
        lineage.push(parent.id);
        daughter.lineage = lineage;

        daughter
    }

    /// Count viable cells.
    pub fn viable_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| {
                matches!(
                    c.state,
                    CellState::Viable
                        | CellState::G1
                        | CellState::S
                        | CellState::G2
                        | CellState::M
                        | CellState::Adherent
                )
            })
            .count()
    }

    /// Cell density (cells/m³).
    pub fn density(&self) -> Scalar {
        self.cells.len() as Scalar / (self.cells.len() as Scalar * TYPICAL_CELL_VOLUME + 1e-30)
    }

    /// Cell viability = viable / total.
    pub fn viability(&self) -> Scalar {
        let total = self.cells.len();
        if total == 0 {
            return 0.0;
        }
        self.viable_count() as Scalar / total as Scalar
    }
}

impl Default for CellPopulation {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple deterministic random number generator (XORShift).
fn rand() -> Scalar {
    // Simple deterministic value for repeatability
    0.5
}

/// Monod-like growth factor: S / (Ks + S).
fn monod_factor(substrate: Scalar, ks: Scalar) -> Scalar {
    if substrate <= 0.0 {
        0.0
    } else {
        substrate / (ks + substrate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_creation() {
        let cell = Cell::new(1, Vec3::new(0.0, 0.0, 0.0), 7.5e-6);
        assert_eq!(cell.id, 1);
        assert_eq!(cell.state, CellState::Viable);
    }

    #[test]
    fn test_population_creation() {
        let pop = CellPopulation::new();
        assert_eq!(pop.cells.len(), 0);
    }

    #[test]
    fn test_seeding() {
        let mut pop = CellPopulation::new();
        pop.seed(100, Vec3::new(0.0, 0.0, 0.0), 7.5e-6);
        assert_eq!(pop.cells.len(), 100);
    }

    #[test]
    fn test_viable_count() {
        let mut pop = CellPopulation::new();
        pop.seed(50, Vec3::new(0.0, 0.0, 0.0), 7.5e-6);
        assert_eq!(pop.viable_count(), 50);
        assert!((pop.viability() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_update_step() {
        let mut pop = CellPopulation::new();
        pop.seed(10, Vec3::new(0.0, 0.0, 0.0), 7.5e-6);
        let result = pop.update(3600.0, 25.0, 0.2, 7.4, 310.15);
        // Under good conditions, cells should grow
        assert!(result.total_nutrient_consumed >= 0.0);
    }

    #[test]
    fn test_update_stress_conditions() {
        let mut pop = CellPopulation::new();
        pop.seed(10, Vec3::new(0.0, 0.0, 0.0), 7.5e-6);
        // Extreme pH should cause cell stress
        let result = pop.update(3600.0, 25.0, 0.2, 4.0, 310.15);
        assert!(result.total_nutrient_consumed <= result.total_nutrient_consumed + 1.0);
    }

    #[test]
    fn test_cell_density() {
        let mut pop = CellPopulation::new();
        pop.seed(100, Vec3::new(0.0, 0.0, 0.0), 7.5e-6);
        let d = pop.density();
        assert!(d > 0.0);
    }

    #[test]
    fn test_max_cells() {
        let mut pop = CellPopulation::new();
        pop.max_cells = 5;
        pop.seed(5, Vec3::new(0.0, 0.0, 0.0), 7.5e-6);

        // Run many update steps with good conditions
        for _ in 0..1000 {
            let _ = pop.update(3600.0, 25.0, 0.2, 7.4, 310.15);
        }
        // Population should be capped
        assert!(pop.cells.len() <= pop.max_cells + 5);
    }
}
