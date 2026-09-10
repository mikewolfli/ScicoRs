//! Tumour-immune interaction model (simple ODE system).
use crate::core::types::Scalar;
use std::collections::HashMap;
#[derive(Debug, Clone)]
pub struct TumourImmuneModel {
    pub tumour_count: Scalar,
    pub t_cell_count: Scalar,
    pub nk_cell_count: Scalar,
    pub cytokine_levels: HashMap<String, Scalar>,
}
impl TumourImmuneModel {
    pub fn new(tumour_init: Scalar, t_init: Scalar, nk_init: Scalar) -> Self {
        let mut cytokines = HashMap::new();
        cytokines.insert("IL2".to_string(), 10.0);
        cytokines.insert("IFNg".to_string(), 5.0);
        Self {
            tumour_count: tumour_init,
            t_cell_count: t_init,
            nk_cell_count: nk_init,
            cytokine_levels: cytokines,
        }
    }
    pub fn step(&mut self, dt: Scalar) {
        let tk_t = 0.01 * self.t_cell_count * self.tumour_count;
        let tk_nk = 0.005 * self.nk_cell_count * self.tumour_count;
        let tg = 0.1 * self.tumour_count * (1.0 - self.tumour_count / 1e6);
        self.tumour_count += dt * (tg - tk_t - tk_nk);
        self.t_cell_count += dt * (0.001 * self.tumour_count - 0.01 * self.t_cell_count);
        self.nk_cell_count += dt * (0.01 - 0.005 * self.nk_cell_count);
        self.tumour_count = self.tumour_count.max(0.0);
        self.t_cell_count = self.t_cell_count.max(1.0);
        self.nk_cell_count = self.nk_cell_count.max(1.0);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_immune_new() {
        let m = TumourImmuneModel::new(1000.0, 100.0, 50.0);
        assert!(m.tumour_count > 0.0);
    }
    #[test]
    fn test_immune_step() {
        let mut m = TumourImmuneModel::new(1000.0, 100.0, 50.0);
        let tb = m.tumour_count;
        m.step(0.1);
        assert!(m.tumour_count >= 0.0);
        assert!((m.tumour_count - tb).abs() > 1e-10);
    }
}
