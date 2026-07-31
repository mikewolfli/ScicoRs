//! PCB via modeling with stub resonance effects.

use crate::core::types::Scalar;

/// PCB via model.
#[derive(Debug, Clone)]
pub struct ViaModel {
    pub drill_diameter: Scalar,
    pub pad_diameter: Scalar,
    pub stub_length: Scalar,
    pub er: Scalar,
}

impl ViaModel {
    pub fn new(drill: Scalar, pad: Scalar, stub: Scalar, er: Scalar) -> Self {
        Self {
            drill_diameter: drill,
            pad_diameter: pad,
            stub_length: stub,
            er,
        }
    }
    pub fn resonant_frequency(&self) -> Scalar {
        if self.stub_length <= 0.0 {
            return 0.0;
        }
        let c0 = 2.99792458e8;
        c0 / (4.0 * self.stub_length * self.er.sqrt())
    }
    pub fn insertion_loss(&self, freq: Scalar) -> Scalar {
        let fr = self.resonant_frequency();
        if fr <= 0.0 {
            return 0.0;
        }
        // Quarter-wave stub resonator: loss ∝ |sin(π/2 · f/fr)|, maximum at f=fr.
        -20.0
            * (std::f64::consts::PI * 0.5 * freq / fr)
                .sin()
                .abs()
                .log10()
                .max(-40.0)
    }
    pub fn stub_equalization(&mut self, back_drill_depth: Scalar) {
        self.stub_length = (self.stub_length - back_drill_depth).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_via_resonant_freq() {
        let v = ViaModel::new(0.3e-3, 0.5e-3, 1.0e-3, 4.0);
        let fr = v.resonant_frequency();
        assert!(fr > 0.0);
    }
    #[test]
    fn test_insertion_loss() {
        let v = ViaModel::new(0.3e-3, 0.5e-3, 1.0e-3, 4.0);
        let il = v.insertion_loss(10e9);
        assert!(il.is_finite());
    }
    #[test]
    fn test_stub_equalization() {
        let mut v = ViaModel::new(0.3e-3, 0.5e-3, 1.0e-3, 4.0);
        v.stub_equalization(0.3e-3);
        assert!((v.stub_length - 0.7e-3).abs() < 1e-15);
    }
}
