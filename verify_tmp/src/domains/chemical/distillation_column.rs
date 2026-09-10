//! Distillation column stage-by-stage calculations (McCabe-Thiele method).

use crate::core::types::Scalar;

/// Distillation column model.
#[derive(Debug, Clone)]
pub struct DistillationColumn {
    pub n_stages: usize,
    pub feed_stage: usize,
    pub reflux_ratio: Scalar,
    pub boilup_ratio: Scalar,
    pub alpha: Vec<Scalar>,
}

impl DistillationColumn {
    pub fn new(
        n_stages: usize,
        feed_stage: usize,
        reflux: Scalar,
        boilup: Scalar,
        components: usize,
    ) -> Self {
        Self {
            n_stages,
            feed_stage,
            reflux_ratio: reflux,
            boilup_ratio: boilup,
            alpha: vec![1.0; components],
        }
    }

    pub fn mesh_equations(
        &self,
        x_f: &[Scalar],
        q: Scalar,
    ) -> Result<(Vec<Scalar>, Vec<Scalar>), String> {
        let c = x_f.len();
        if c == 0 {
            return Err("No components".to_string());
        }
        let mut x_d = vec![0.0; c];
        let mut x_b = vec![0.0; c];
        let r = self.reflux_ratio;
        for i in 0..c {
            x_d[i] = x_f[i] * (r + 1.0) * self.alpha[i] / self.alpha.iter().sum::<Scalar>();
            x_b[i] = x_f[i] * q * self.alpha[i] / self.alpha.iter().sum::<Scalar>();
        }
        Ok((x_d, x_b))
    }

    /// McCabe-Thiele graphical stage count using rectifying and stripping
    /// operating lines.
    ///
    /// Steps down from the distillate composition along the rectifying line
    /// (slope R/(R+1)) until the feed stage, then the stripping line
    /// (slope (V̄/B − 1)/(V̄/B), intercept x_B/(V̄/B)) until the bottoms
    /// composition is reached. A physically valid relative volatility
    /// (α > 1) is required; `alpha` values ≤ 1 (no separation) are replaced
    /// by a default of 2.0.
    pub fn mccabe_thiele(&self, x_d: Scalar, x_b: Scalar, _x_f: Scalar) -> usize {
        let r = self.reflux_ratio.max(1e-9);
        let vb = self.boilup_ratio.max(1e-9); // V̄/B
        let alpha = if self.alpha.is_empty() || self.alpha[0] <= 1.0 {
            2.0
        } else {
            self.alpha[0]
        };
        let mut x = x_d;
        let mut stages = 0;
        let rect = |x: Scalar| r / (r + 1.0) * x + x_d / (r + 1.0);
        let strip = |x: Scalar| (vb - 1.0) / vb * x + x_b / vb;
        while x > x_b && stages < self.n_stages.max(2) {
            let y = if stages < self.feed_stage {
                rect(x)
            } else {
                strip(x)
            };
            x = y / (alpha - (alpha - 1.0) * y);
            stages += 1;
        }
        stages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_column_new() {
        let c = DistillationColumn::new(20, 10, 2.0, 3.0, 2);
        assert_eq!(c.n_stages, 20);
    }
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
