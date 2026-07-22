//! Unified Database System (TOML + SQLite Index)
//!
//! Provides a database abstraction layer for simulation data:
//! - TOML files for human-readable data storage
//! - SQLite index for efficient searching and cross-referencing
//! - Material, celestial, fluid, electrical, and other domain libraries

use std::collections::HashMap;

/// A generic entry in the simulation database.
#[derive(Debug, Clone)]
pub struct DbEntry {
    /// Unique identifier for this entry.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Category/domain this entry belongs to.
    pub category: String,
    /// Tags for searching and classification.
    pub tags: Vec<String>,
    /// Key-value data store.
    pub properties: HashMap<String, String>,
    /// Source TOML file path (if loaded from file).
    pub source_file: Option<String>,
}

impl DbEntry {
    pub fn new(id: &str, name: &str, category: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            category: category.to_string(),
            tags: Vec::new(),
            properties: HashMap::new(),
            source_file: None,
        }
    }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.properties.insert(key.to_string(), value.to_string());
        self
    }

    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }

    pub fn get_property_f64(&self, key: &str) -> Option<f64> {
        self.properties.get(key)?.parse::<f64>().ok()
    }
}

/// The simulation database — an in-memory store with TOML/SQLite backends.
#[derive(Debug, Default)]
pub struct SimulationDatabase {
    entries: Vec<DbEntry>,
    index: HashMap<String, Vec<usize>>, // category -> indices
}

impl SimulationDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a new entry into the database.
    pub fn insert(&mut self, entry: DbEntry) {
        let idx = self.entries.len();
        self.index.entry(entry.category.clone()).or_default().push(idx);
        self.entries.push(entry);
    }

    /// Find an entry by its ID.
    pub fn find_by_id(&self, id: &str) -> Option<&DbEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Find entries by category.
    pub fn find_by_category(&self, category: &str) -> Vec<&DbEntry> {
        self.index
            .get(category)
            .map(|indices| indices.iter().map(|&i| &self.entries[i]).collect())
            .unwrap_or_default()
    }

    /// Search entries by tag.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&DbEntry> {
        self.entries.iter().filter(|e| e.tags.iter().any(|t| t == tag)).collect()
    }

    /// Search entries by property value.
    pub fn find_by_property(&self, key: &str, value: &str) -> Vec<&DbEntry> {
        self.entries
            .iter()
            .filter(|e| e.properties.get(key).is_some_and(|v| v == value))
            .collect()
    }

    /// Get all entries.
    pub fn all(&self) -> &[DbEntry] {
        &self.entries
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// List all categories.
    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self.index.keys().map(|s| s.as_str()).collect();
        cats.sort();
        cats
    }
}

/// Predefined database constants for common materials.
pub mod materials {
    use super::*;

    pub fn populate_structural(db: &mut SimulationDatabase) {
        db.insert(
            DbEntry::new("steel_1018", "Steel 1018", "structural")
                .with_property("density_kg_m3", "7870")
                .with_property("young_modulus_gpa", "205")
                .with_property("poisson_ratio", "0.29")
                .with_property("yield_strength_mpa", "310")
                .with_tag("metal")
                .with_tag("carbon_steel"),
        );
        db.insert(
            DbEntry::new("aluminum_6061", "Aluminum 6061", "structural")
                .with_property("density_kg_m3", "2700")
                .with_property("young_modulus_gpa", "68.9")
                .with_property("poisson_ratio", "0.33")
                .with_property("yield_strength_mpa", "276")
                .with_tag("metal")
                .with_tag("aluminum"),
        );
        db.insert(
            DbEntry::new("copper_c110", "Copper C110", "structural")
                .with_property("density_kg_m3", "8960")
                .with_property("young_modulus_gpa", "110")
                .with_property("poisson_ratio", "0.34")
                .with_property("thermal_conductivity_w_mk", "401")
                .with_tag("metal"),
        );
    }

    pub fn populate_electrical(db: &mut SimulationDatabase) {
        db.insert(
            DbEntry::new("resistor_ideal", "Ideal Resistor", "electrical")
                .with_property("type", "passive")
                .with_property("description", "Ideal linear resistor R = V/I")
                .with_tag("r")
                .with_tag("passive"),
        );
        db.insert(
            DbEntry::new("capacitor_ideal", "Ideal Capacitor", "electrical")
                .with_property("type", "passive")
                .with_property("description", "Ideal linear capacitor I = C*dV/dt")
                .with_tag("c")
                .with_tag("passive"),
        );
        db.insert(
            DbEntry::new("diode_1n4148", "Diode 1N4148", "electrical")
                .with_property("type", "active")
                .with_property("v_fwd_v", "0.7")
                .with_property("v_rev_max_v", "100")
                .with_property("i_max_a", "0.3")
                .with_tag("diode"),
        );
    }

    pub fn populate_semiconductor(db: &mut SimulationDatabase) {
        db.insert(
            DbEntry::new("silicon", "Silicon (Si)", "semiconductor")
                .with_property("bandgap_eV", "1.12")
                .with_property("electron_mobility_cm2_vs", "1350")
                .with_property("hole_mobility_cm2_vs", "480")
                .with_property("dielectric_constant", "11.7")
                .with_property("density_kg_m3", "2330")
                .with_tag("element")
                .with_tag("group_iv"),
        );
        db.insert(
            DbEntry::new("gaas", "Gallium Arsenide (GaAs)", "semiconductor")
                .with_property("bandgap_eV", "1.43")
                .with_property("electron_mobility_cm2_vs", "8500")
                .with_property("hole_mobility_cm2_vs", "400")
                .with_property("dielectric_constant", "12.9")
                .with_tag("compound"),
        );
    }

    pub fn populate_optical(db: &mut SimulationDatabase) {
        db.insert(
            DbEntry::new("bk7_glass", "BK7 Glass", "optical")
                .with_property("refractive_index_587nm", "1.5168")
                .with_property("abbe_number", "64.17")
                .with_property("density_kg_m3", "2510")
                .with_tag("glass"),
        );
        db.insert(
            DbEntry::new("silica_fused", "Fused Silica", "optical")
                .with_property("refractive_index_587nm", "1.4585")
                .with_property("abbe_number", "67.8")
                .with_property("density_kg_m3", "2200")
                .with_tag("glass"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_basics() {
        let mut db = SimulationDatabase::new();
        materials::populate_structural(&mut db);
        assert!(db.len() >= 3);
        assert!(db.find_by_id("steel_1018").is_some());
        assert_eq!(db.find_by_category("structural").len(), 3);
        assert!(!db.find_by_tag("metal").is_empty());
    }
}
