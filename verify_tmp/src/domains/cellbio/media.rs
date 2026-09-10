//! Culture media model for cell culture simulation.
//!
//! Provides CultureMedia and MediumComponent types for defining
//! the chemical environment of cell culture, including standard
//! media recipes (DMEM, RPMI-1640).

use crate::core::types::Scalar;
use std::collections::HashMap;

// ──────────────────────────────────────────────
// MediumComponent
// ──────────────────────────────────────────────

/// A single component (nutrient, gas, metabolite) in the culture medium.
#[derive(Debug, Clone)]
pub struct MediumComponent {
    /// Component name (e.g., "Glucose", "Glutamine", "O2").
    pub name: String,
    /// Concentration (mol/m³).
    pub concentration: Scalar,
    /// Diffusion coefficient (m²/s).
    pub diffusion_coeff: Scalar,
    /// Cell consumption rate (mol/(cell·s)).
    pub consumption_rate: Scalar,
    /// Cell production rate (mol/(cell·s)).
    pub production_rate: Scalar,
}

impl MediumComponent {
    pub fn new(name: &str, concentration: Scalar) -> Self {
        Self {
            name: name.to_string(),
            concentration,
            diffusion_coeff: 1e-9,
            consumption_rate: 0.0,
            production_rate: 0.0,
        }
    }

    /// Default glucose component (25 mM = 25 mol/m³).
    pub fn glucose(conc: Scalar) -> Self {
        Self {
            name: "Glucose".to_string(),
            concentration: conc,
            diffusion_coeff: 6.7e-10,
            consumption_rate: 1e-16,
            production_rate: 0.0,
        }
    }

    /// Default oxygen component (~0.2 mM at 37°C in equilibrium with air).
    pub fn oxygen(conc: Scalar) -> Self {
        Self {
            name: "O2".to_string(),
            concentration: conc,
            diffusion_coeff: 2.1e-9,
            consumption_rate: 5e-17,
            production_rate: 0.0,
        }
    }

    /// Default lactate component.
    pub fn lactate() -> Self {
        Self {
            name: "Lactate".to_string(),
            concentration: 0.0,
            diffusion_coeff: 1.1e-9,
            consumption_rate: 0.0,
            production_rate: 1e-16,
        }
    }
}

// ──────────────────────────────────────────────
// CultureMedia
// ──────────────────────────────────────────────

/// Complete culture medium definition.
#[derive(Debug, Clone)]
pub struct CultureMedia {
    /// Medium components.
    pub components: Vec<MediumComponent>,
    /// pH value.
    pub ph: Scalar,
    /// Temperature (K).
    pub temperature: Scalar,
    /// Osmolarity (mOsm/L).
    pub osmolarity: Scalar,
    /// Volume (m³).
    pub volume: Scalar,
    /// Gas phase O₂ fraction.
    pub gas_o2_fraction: Scalar,
    /// Gas phase CO₂ fraction.
    pub gas_co2_fraction: Scalar,
    /// Component lookup cache.
    component_map: HashMap<String, usize>,
}

impl CultureMedia {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            ph: 7.4,
            temperature: 310.15, // 37°C
            osmolarity: 300.0,
            volume: 1e-4, // 100 mL
            gas_o2_fraction: 0.21,
            gas_co2_fraction: 0.05,
            component_map: HashMap::new(),
        }
    }

    /// Add or replace a component.
    pub fn add_component(&mut self, component: MediumComponent) {
        if let Some(&idx) = self.component_map.get(&component.name) {
            self.components[idx] = component;
        } else {
            let idx = self.components.len();
            self.component_map.insert(component.name.clone(), idx);
            self.components.push(component);
        }
    }

    /// Get concentration of a component by name.
    pub fn get_concentration(&self, name: &str) -> Option<Scalar> {
        self.component_map
            .get(name)
            .map(|&idx| self.components[idx].concentration)
    }

    /// Set concentration of a component by name.
    pub fn set_concentration(&mut self, name: &str, conc: Scalar) -> Result<(), String> {
        self.component_map
            .get(name)
            .map(|&idx| {
                self.components[idx].concentration = conc;
            })
            .ok_or_else(|| format!("component '{name}' not found"))
    }

    /// Update pH based on CO₂/bicarbonate buffer system.
    pub fn update_ph(&mut self) {
        // Simplified Henderson-Hasselbalch: pH = pKa + log([HCO₃⁻]/[CO₂])
        // pKa of carbonic acid at 37°C = 6.1
        // [CO₂] = kH * pCO₂
        let kh_co2 = 0.034; // mol/(L·atm) at 37°C
        let pco2 = self.gas_co2_fraction; // atm
        let co2_conc = kh_co2 * pco2; // M

        // Assume bicarbonate is the main buffer at ~24 mM
        let hco3_conc = 0.024; // M
        self.ph = 6.1 + (hco3_conc / co2_conc).log10();
        // Clamp to physiological range
        self.ph = self.ph.clamp(6.0, 8.5);
    }

    /// Calculate dissolved oxygen concentration (mol/m³).
    pub fn dissolved_o2(&self) -> Scalar {
        // Henry's law: [O₂] = kH * pO₂
        let kh_o2 = o2_henry(self.temperature); // mol/(m³·atm)
        kh_o2 * self.gas_o2_fraction
    }

    /// Check if osmolarity is in the valid range for mammalian cells.
    pub fn is_osmolarity_valid(&self) -> bool {
        self.osmolarity > 250.0 && self.osmolarity < 400.0
    }

    /// Create DMEM high-glucose medium.
    pub fn dmem_high_glucose() -> Self {
        let mut media = Self::new();
        media.add_component(MediumComponent::glucose(25.0)); // 25 mM
        media.add_component(MediumComponent::oxygen(0.2)); // ~0.2 mM
        media.add_component(MediumComponent::lactate());
        media.add_component(MediumComponent::new("Glutamine", 4.0));
        media.add_component(MediumComponent::new("Sodium", 155.0));
        media.add_component(MediumComponent::new("Potassium", 5.3));
        media.add_component(MediumComponent::new("Calcium", 1.8));
        media.add_component(MediumComponent::new("Bicarbonate", 44.0));
        media.osmolarity = 320.0;
        media
    }

    /// Create RPMI-1640 medium.
    pub fn rpmi_1640() -> Self {
        let mut media = Self::new();
        media.add_component(MediumComponent::glucose(11.0)); // 11 mM
        media.add_component(MediumComponent::oxygen(0.2));
        media.add_component(MediumComponent::lactate());
        media.add_component(MediumComponent::new("Glutamine", 2.0));
        media.add_component(MediumComponent::new("Sodium", 140.0));
        media.add_component(MediumComponent::new("Potassium", 5.3));
        media.add_component(MediumComponent::new("Calcium", 0.42));
        media.add_component(MediumComponent::new("Bicarbonate", 24.0));
        media.osmolarity = 270.0;
        media
    }
}

impl Default for CultureMedia {
    fn default() -> Self {
        Self::new()
    }
}

/// Oxygen Henry's constant (mol/(m³·atm)) at temperature (K).
fn o2_henry(temp: Scalar) -> Scalar {
    1.3 * (-1500.0 * (1.0 / temp - 1.0 / 298.15)).exp()
}

/// Standard cell culture conditions.
pub fn standard_conditions() -> (Scalar, Scalar, Scalar) {
    (7.4, 310.15, 300.0) // pH, temp_K, osmolarity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_medium_creation() {
        let media = CultureMedia::new();
        assert!((media.ph - 7.4).abs() < 0.1);
    }

    #[test]
    fn test_dmem_high_glucose() {
        let dmem = CultureMedia::dmem_high_glucose();
        let glucose = dmem.get_concentration("Glucose").unwrap();
        assert!((glucose - 25.0).abs() < 0.1);
    }

    #[test]
    fn test_rpmi_1640() {
        let rpmi = CultureMedia::rpmi_1640();
        let glucose = rpmi.get_concentration("Glucose").unwrap();
        assert!((glucose - 11.0).abs() < 0.1);
    }

    #[test]
    fn test_concentration_set_get() {
        let mut media = CultureMedia::new();
        media.add_component(MediumComponent::glucose(10.0));
        assert!((media.get_concentration("Glucose").unwrap() - 10.0).abs() < 0.1);
        media.set_concentration("Glucose", 20.0).unwrap();
        assert!((media.get_concentration("Glucose").unwrap() - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_dissolved_o2() {
        let media = CultureMedia::new();
        let o2 = media.dissolved_o2();
        assert!(o2 > 0.0);
        assert!(o2 < 1.0);
    }

    #[test]
    fn test_osmolarity_valid() {
        let dmem = CultureMedia::dmem_high_glucose();
        assert!(dmem.is_osmolarity_valid());
    }
}
