//! BSIM4 MOSFET compact model for advanced node simulation.

use crate::core::types::Scalar;

/// BSIM4 model parameters (simplified set).
#[derive(Debug, Clone)]
pub struct Bsim4Model {
    pub vth0: Scalar,
    pub u0: Scalar,
    pub vsat: Scalar,
    pub rdsw: Scalar,
    pub alpha0: Scalar,
    pub beta0: Scalar,
}

impl Bsim4Model {
    pub fn new(vth0: Scalar, u0: Scalar, vsat: Scalar) -> Self {
        Self {
            vth0,
            u0,
            vsat,
            rdsw: 100.0,
            alpha0: 0.0,
            beta0: 1.0,
        }
    }

    pub fn drain_current(&self, vgs: Scalar, vds: Scalar, vbs: Scalar) -> Scalar {
        let vth = self.vth0 + 0.5 * ((1.0 + 0.5 * vbs).sqrt() - 1.0);
        let vgt = vgs - vth;
        if vgt <= 0.0 {
            return 0.0;
        }
        let vdsat = vgt / self.alpha0.max(0.01);
        let vdseff = vds.min(vdsat);
        let id_lin = self.u0 * vgt * vdseff * (1.0 - 0.5 * vdseff / vgt);
        let lam = 0.01;
        id_lin * (1.0 + lam * vds)
    }

    pub fn capacitances(&self, vgs: Scalar, _vds: Scalar, vbs: Scalar) -> [Scalar; 4] {
        let vth = self.vth0 + 0.5 * ((1.0 + 0.5 * vbs).sqrt() - 1.0);
        let vgt = (vgs - vth).max(0.1);
        let cox = 1e-3;
        let cgg = cox * vgt;
        let cgd = 0.3 * cox;
        let csg = 0.3 * cox;
        let cds = 0.1 * cox;
        [cgg, cgd, csg, cds]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_bsim_new() {
        let m = Bsim4Model::new(0.3, 0.05, 1e5);
        assert!((m.vth0 - 0.3).abs() < 1e-10);
    }
    #[test]
    fn test_drain_current_off() {
        let m = Bsim4Model::new(0.3, 0.05, 1e5);
        assert!((m.drain_current(0.0, 1.0, 0.0) - 0.0).abs() < 1e-10);
    }
    #[test]
    fn test_drain_current_on() {
        let m = Bsim4Model::new(0.3, 0.05, 1e5);
        assert!(m.drain_current(1.0, 0.5, 0.0) > 0.0);
    }
    #[test]
    fn test_capacitances() {
        let m = Bsim4Model::new(0.3, 0.05, 1e5);
        let c = m.capacitances(1.0, 0.5, 0.0);
        assert_eq!(c.len(), 4);
    }
}
