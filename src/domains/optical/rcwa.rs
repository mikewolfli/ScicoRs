//! RCWA grating diffraction.
use crate::core::types::Scalar;
type CS = num_complex::Complex<f64>;
#[derive(Debug, Clone)]
pub struct RcwaGrating {
    pub period: Scalar, pub depth: Scalar,
    pub n1: CS, pub n2: CS, pub harmonics: usize,
}
impl RcwaGrating {
    pub fn new(p: Scalar, d: Scalar, n1: CS, n2: CS, h: usize) -> Self {
        Self { period: p, depth: d, n1, n2, harmonics: h.max(1) }
    }
    pub fn diffraction_efficiency(&self, wl: Scalar, theta: Scalar) -> Vec<Scalar> {
        let k0 = 2.0 * 3.141592653589793 / wl;
        let nh = self.harmonics as isize;
        let mut de = vec![0.0; (2 * nh + 1) as usize];
        for m in -nh..=nh {
            let kxm = k0 * (self.n1.re * theta.sin() + (m as Scalar) * wl / self.period);
            let kz1 = (k0 * k0 * self.n1.norm_sqr() - kxm * kxm).sqrt().max(0.0);
            let kz2 = (k0 * k0 * self.n2.norm_sqr() - kxm * kxm).sqrt().max(0.0);
            let idx = (m + nh) as usize;
            let r = (kz1 - kz2) / (kz1 + kz2).max(1e-30);
            de[idx] = (r * r).max(0.0);
        }
        de
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_new() { let g = RcwaGrating::new(1e-6, 5e-7, CS::new(1.5,0.0), CS::new(1.0,0.0), 3); assert_eq!(g.harmonics, 3); }
    #[test] fn test_de() { let g = RcwaGrating::new(1e-6, 5e-7, CS::new(1.5,0.0), CS::new(1.0,0.0), 2); let de = g.diffraction_efficiency(633e-9, 0.0); assert_eq!(de.len(), 5); }
}
