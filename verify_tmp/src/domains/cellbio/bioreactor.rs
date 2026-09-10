//! Bioreactor dynamic model for cell culture simulation.
//!
//! Supports batch, fed-batch, continuous (chemostat), and perfusion
//! operation modes with basic pH and DO control.

use crate::core::types::Scalar;
use crate::domains::cellbio::cell_model::CellPopulation;
use crate::domains::cellbio::media::CultureMedia;
use crate::domains::molbio::forcefield::Vec3;

// ──────────────────────────────────────────────
// BioreactorMode
// ──────────────────────────────────────────────

/// Bioreactor operation mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BioreactorMode {
    /// Batch culture (no addition/removal).
    Batch,
    /// Fed-batch with constant feed rate (m³/s).
    FedBatch { feed_rate: Scalar },
    /// Continuous culture with dilution rate (s⁻¹).
    Continuous { dilution_rate: Scalar },
    /// Perfusion with perfusion rate (m³/s).
    Perfusion { perfusion_rate: Scalar },
}

// ──────────────────────────────────────────────
// Bioreactor
// ──────────────────────────────────────────────

/// Bioreactor model for cell culture simulation.
#[derive(Debug, Clone)]
pub struct Bioreactor {
    /// Operation mode.
    pub mode: BioreactorMode,
    /// Working volume (m³).
    pub working_volume: Scalar,
    /// Culture medium.
    pub media: CultureMedia,
    /// Cell population.
    pub population: CellPopulation,
    /// Agitation speed (rpm).
    pub agitation_speed: Scalar,
    /// Aeration rate (vvm).
    pub aeration_rate: Scalar,
    /// Temperature setpoint (K).
    pub temperature_setpoint: Scalar,
    /// pH setpoint.
    pub ph_setpoint: Scalar,
    /// Dissolved oxygen setpoint (% saturation).
    pub o2_setpoint: Scalar,
    /// Harvest interval (s), None = continuous.
    pub harvest_interval: Option<Scalar>,
    /// Feed nutrient concentration (mol/m³).
    pub feed_concentration: Scalar,
    /// Elapsed time since last harvest (s).
    time_since_harvest: Scalar,
    /// Total biomass produced (kg).
    pub total_biomass: Scalar,
    /// Total nutrient consumed (mol).
    pub total_nutrient_consumed: Scalar,
}

impl Bioreactor {
    pub fn new(mode: BioreactorMode, volume: Scalar) -> Self {
        Self {
            mode,
            working_volume: volume,
            media: CultureMedia::dmem_high_glucose(),
            population: CellPopulation::new(),
            agitation_speed: 100.0,
            aeration_rate: 0.1,
            temperature_setpoint: 310.15,
            ph_setpoint: 7.4,
            o2_setpoint: 50.0,
            harvest_interval: None,
            feed_concentration: 25.0,
            time_since_harvest: 0.0,
            total_biomass: 0.0,
            total_nutrient_consumed: 0.0,
        }
    }

    /// Inoculate the bioreactor with cells at a target density.
    pub fn inoculate(&mut self, cells: &mut CellPopulation, density: Scalar) {
        let n = (density * self.working_volume) as usize;
        cells.seed(n, Vec3::new(0.0, 0.0, 0.0), 7.5e-6);
        self.population = cells.clone();
    }

    /// Execute one time step of the bioreactor simulation.
    pub fn step(&mut self, dt: Scalar) -> Result<(), String> {
        let nutrient = self.media.get_concentration("Glucose").unwrap_or(0.0);
        let o2 = self.media.dissolved_o2();
        let ph = self.media.ph;
        let temp = self.media.temperature;

        // Update cell population
        let result = self.population.update(dt, nutrient, o2, ph, temp);
        self.total_nutrient_consumed += result.total_nutrient_consumed;
        self.total_biomass += self.population.cells.len() as Scalar
            * self.population.cells.first().map(|c| c.mass).unwrap_or(1e-12);

        // Update medium concentrations based on consumption
        let glucose_conc = self.media.get_concentration("Glucose").unwrap_or(0.0);
        let new_glucose = (glucose_conc * self.working_volume
            - result.total_nutrient_consumed)
            / self.working_volume;
        self.media
            .set_concentration("Glucose", new_glucose.max(0.0))
            .ok();

        // pH control
        self.control_ph();

        // DO control
        self.control_o2();

        // Operation mode specific logic
        match self.mode {
            BioreactorMode::FedBatch { feed_rate } => {
                self.feed(dt, feed_rate);
            }
            BioreactorMode::Continuous { dilution_rate } => {
                self.continuous_operation(dt, dilution_rate);
            }
            BioreactorMode::Perfusion { perfusion_rate } => {
                self.perfusion_operation(dt, perfusion_rate);
            }
            BioreactorMode::Batch => {
                // No addition or removal
            }
        }

        // Check harvest interval
        if let Some(interval) = self.harvest_interval {
            self.time_since_harvest += dt;
            if self.time_since_harvest >= interval {
                self.time_since_harvest = 0.0;
                // Harvest and replenish (simplified)
                let harvest_volume = self.working_volume * 0.5;
                self.working_volume -= harvest_volume;
                // Refill with fresh media
                self.working_volume += harvest_volume;
            }
        }

        Ok(())
    }

    /// Feed operation for fed-batch mode.
    fn feed(&mut self, dt: Scalar, feed_rate: Scalar) {
        let feed_volume = feed_rate * dt;
        self.working_volume += feed_volume;

        // Add nutrients with feed
        let glucose = self.media.get_concentration("Glucose").unwrap_or(0.0);
        let added_glucose = self.feed_concentration * feed_volume;
        let new_glucose = (glucose * (self.working_volume - feed_volume) + added_glucose)
            / self.working_volume;
        self.media
            .set_concentration("Glucose", new_glucose)
            .ok();
    }

    /// Continuous culture operation.
    fn continuous_operation(&mut self, dt: Scalar, dilution_rate: Scalar) {
        let flow_rate = dilution_rate * self.working_volume * dt;
        // Remove volume and cells
        let cells_to_remove = (self.population.cells.len() as Scalar * dilution_rate * dt).round() as usize;
        let remove_count = cells_to_remove.min(self.population.cells.len() / 2);
        for _ in 0..remove_count {
            self.population.cells.pop();
        }
        self.working_volume -= flow_rate;
        // Add fresh media
        let fresh_glucose = self.feed_concentration * flow_rate;
        let current_glucose = self.media.get_concentration("Glucose").unwrap_or(0.0);
        let new_glucose = (current_glucose * (self.working_volume) + fresh_glucose) / self.working_volume.max(1e-15);
        self.media
            .set_concentration("Glucose", new_glucose)
            .ok();
    }

    /// Perfusion operation.
    fn perfusion_operation(&mut self, dt: Scalar, perfusion_rate: Scalar) {
        let exchange_volume = perfusion_rate * dt;
        // Cell retention: cells remain, media is replaced
        let current_glucose = self.media.get_concentration("Glucose").unwrap_or(0.0);
        let retained = current_glucose * (self.working_volume - exchange_volume);
        let added = self.feed_concentration * exchange_volume;
        let new_glucose = (retained + added) / self.working_volume;
        self.media
            .set_concentration("Glucose", new_glucose)
            .ok();
    }

    /// Simple proportional pH control.
    pub fn control_ph(&mut self) {
        let diff = self.ph_setpoint - self.media.ph;
        // P-controller with small gain
        let adjustment = diff * 0.01;
        self.media.ph = (self.media.ph + adjustment).clamp(6.5, 8.0);
    }

    /// Simple proportional DO control via agitation.
    pub fn control_o2(&mut self) {
        let current_o2 = self.media.dissolved_o2();
        let target_o2 = self.o2_setpoint / 100.0 * 0.2; // % of saturation
        let diff = target_o2 - current_o2;
        // Adjust aeration based on error
        self.aeration_rate = (self.aeration_rate + diff * 0.1).clamp(0.01, 2.0);
    }

    /// Calculate specific growth rate μ (h⁻¹).
    pub fn specific_growth_rate(&self) -> Scalar {
        if self.population.cells.is_empty() {
            return 0.0;
        }
        // Simplified: based on glucose concentration
        let glucose = self.media.get_concentration("Glucose").unwrap_or(0.0);
        let mu_max = 0.05; // max specific growth rate (h⁻¹)
        let ks = 0.1; // Monod constant (mM)
        mu_max * glucose / (ks + glucose)
    }

    /// Calculate productivity (cells/(mL·h)).
    pub fn productivity(&self, _component: &str) -> Scalar {
        let viable = self.population.viable_count() as Scalar;
        let volume_l = self.working_volume * 1000.0; // m³ → L
        if volume_l > 0.0 {
            viable / (volume_l * 1.0) // simplified
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bioreactor_creation() {
        let reactor = Bioreactor::new(BioreactorMode::Batch, 1e-4);
        assert!(reactor.working_volume > 0.0);
    }

    #[test]
    fn test_inoculation() {
        let mut reactor = Bioreactor::new(BioreactorMode::Batch, 1e-4);
        let mut cells = CellPopulation::new();
        reactor.inoculate(&mut cells, 1e5);
        assert!(!reactor.population.cells.is_empty());
    }

    #[test]
    fn test_batch_step() {
        let mut reactor = Bioreactor::new(BioreactorMode::Batch, 1e-4);
        let mut cells = CellPopulation::new();
        reactor.inoculate(&mut cells, 1e5);
        reactor.step(3600.0).unwrap();
        assert!(reactor.total_nutrient_consumed >= 0.0);
    }

    #[test]
    fn test_fed_batch_step() {
        let mut reactor = Bioreactor::new(
            BioreactorMode::FedBatch {
                feed_rate: 1e-8,
            },
            1e-4,
        );
        let mut cells = CellPopulation::new();
        reactor.inoculate(&mut cells, 1e5);
        reactor.step(3600.0).unwrap();
        assert!(reactor.working_volume > 1e-4);
    }

    #[test]
    fn test_specific_growth_rate() {
        let reactor = Bioreactor::new(BioreactorMode::Batch, 1e-4);
        let mu = reactor.specific_growth_rate();
        assert!(mu >= 0.0);
    }
}
