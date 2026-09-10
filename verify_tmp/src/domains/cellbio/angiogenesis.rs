//! Angiogenesis model.
use crate::core::types::Scalar;
#[derive(Debug, Clone)]
pub struct Angiogenesis {
    pub vegf: Vec<Vec<Scalar>>, pub vessel_density: Vec<Vec<Scalar>>, pub nx: usize, pub ny: usize,
}
impl Angiogenesis {
    pub fn new(nx: usize, ny: usize) -> Self { Self { vegf: vec![vec![0.0; nx]; ny], vessel_density: vec![vec![0.0; nx]; ny], nx, ny } }
    pub fn step(&mut self, dt: Scalar) {
        let mut new_g = self.vegf.clone();
        for j in 1..self.ny.saturating_sub(1) { for i in 1..self.nx.saturating_sub(1) {
            new_g[j][i] += dt * (self.vegf[j][i+1] + self.vegf[j][i.saturating_sub(1)] + self.vegf[j+1][i] + self.vegf[j.saturating_sub(1)][i] - 4.0 * self.vegf[j][i]);
        }}
        self.vegf = new_g;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_new() { let a = Angiogenesis::new(10, 10); assert_eq!(a.nx, 10); }
    #[test] fn test_step() { let mut a = Angiogenesis::new(10, 10); a.vegf[5][5] = 1.0; a.step(0.1); }
}
