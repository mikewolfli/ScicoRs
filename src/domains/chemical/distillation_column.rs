//! Distillation column stage-by-stage calculations (McCabe-Thiele method).

use crate::core::types::Scalar;

/// Distillation column model.
#[derive(Debug, Clone)]
pub struct DistillationColumn {
    pub n_stages: usize, pub feed_stage: usize,
    pub reflux_ratio: Scalar, pub boilup_ratio: Scalar,
    pub alpha: Vec<Scalar>,
}

impl DistillationColumn {
    pub fn new(n_stages: usize, feed_stage: usize, reflux: Scalar, boilup: Scalar, components: usize) -> Self {
        Self { n_stages, feed_stage, reflux_ratio: reflux, boilup_ratio: boilup, alpha: vec![1.0; components] }
    }

    pub fn mesh_equations(&self, x_f: &[Scalar], q: Scalar) -> Result<(Vec<Scalar>, Vec<Scalar>), String> {
        let c = x_f.len();
        if c == 0 { return Err("No components".to_string()); }
        let mut x_d = vec![0.0; c];
        let mut x_b = vec![0.0; c];
        let r = self.reflux_ratio;
        for i in 0..c {
            x_d[i] = x_f[i] * (r + 1.0) * self.alpha[i] / self.alpha.iter().sum::<Scalar>();
            x_b[i] = x_f[i] * q * self.alpha[i] / self.alpha.iter().sum::<Scalar>();
        }
        Ok((x_d, x_b))
    }

    pub fn mccabe_thiele(&self, x_d: Scalar, x_b: Scalar, x_f: Scalar) -> usize {
        let r = self.reflux_ratio;
        let mut x = x_d;
        let mut stages = 0;
        let op_line = |x: Scalar| r / (r + 1.0) * x + x_d / (r + 1.0);
        while x > x_b && stages < 100 {
            let y = op_line(x);
            x = y / (self.alpha[0] - (self.alpha[0] - 1.0) * y);
            stages += 1;
            if stages == self.feed_stage { x = x_f; }
        }
        stages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_column_new() { let c = DistillationColumn::new(20, 10, 2.0, 3.0, 2); assert_eq!(c.n_stages, 20); }
    #[test]
    fn test_mesh_eq() {
        let c = DistillationColumn::new(20, 10, 2.0, 3.0, 2);
        let (xd, xb) = c.mesh_equations(&[0.5, 0.5], 1.0).unwrap();
        assert_eq!(xd.len(), 2);
        assert_eq!(xb.len(), 2);
    }
    #[test]
    fn test_mccabe_thiele() {
        let c = DistillationColumn::new(20, 10, 2.0, 3.0, 1);
        let s = c.mccabe_thiele(0.9, 0.1, 0.5);
        assert!(s > 0);
    }
}
