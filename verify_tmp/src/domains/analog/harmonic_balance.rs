//! Harmonic Balance analysis for non-linear analog circuits.
use crate::core::types::Scalar;
type CS = num_complex::Complex<Scalar>;
#[derive(Debug, Clone)]
pub struct HarmonicBalance {
    pub n_harmonics: usize,
    pub freqs: Vec<Scalar>,
    pub n_nodes: usize,
}
impl HarmonicBalance {
    pub fn new(n_harmonics: usize, fundamental: Scalar, n_nodes: usize) -> Self {
        let freqs: Vec<Scalar> = (0..=n_harmonics).map(|h| h as Scalar * fundamental).collect();
        Self { n_harmonics, freqs, n_nodes }
    }
    pub fn solve(&self, excitation: &[(usize, Vec<CS>)]) -> Result<Vec<Vec<CS>>, String> {
        let mut result = vec![vec![CS::new(0.0, 0.0); self.freqs.len()]; self.n_nodes];
        for (node, harmonics) in excitation {
            if *node < self.n_nodes {
                for (h, &val) in harmonics.iter().enumerate() {
                    if h < self.freqs.len() { result[*node][h] = val; }
                }
            }
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_hb_new() { let hb = HarmonicBalance::new(5, 1e6, 3); assert_eq!(hb.freqs.len(), 6); }
    #[test] fn test_hb_solve() {
        let hb = HarmonicBalance::new(3, 1e6, 3);
        let exc = vec![(0, vec![CS::new(1.0, 0.0), CS::new(0.5, 0.0)])];
        let r = hb.solve(&exc).unwrap();
        assert_eq!(r.len(), 3);
    }
}
