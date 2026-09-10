//! Hypersonic flow analysis.
use crate::core::types::Scalar;
use crate::domains::aerospace::physics::IsaAtmosphere;
#[derive(Debug, Clone)]
pub struct HypersonicFlow {
    pub mach: Scalar,
    pub altitude: Scalar,
    pub angle_of_attack: Scalar,
}
impl HypersonicFlow {
    pub fn new(mach: Scalar, altitude: Scalar, aoa: Scalar) -> Self {
        Self {
            mach,
            altitude,
            angle_of_attack: aoa,
        }
    }
    pub fn stagnation_temperature(&self) -> Scalar {
        let t = IsaAtmosphere::temperature(self.altitude);
        t * (1.0 + 0.2 * self.mach * self.mach)
    }
    pub fn stagnation_pressure(&self) -> Scalar {
        let p = IsaAtmosphere::pressure(self.altitude);
        p * (1.0 + 0.2 * self.mach * self.mach).powf(3.5)
    }
    pub fn convective_heating(&self, nose_radius: Scalar) -> Scalar {
        let rho = IsaAtmosphere::density(self.altitude);
        let v = self.mach * IsaAtmosphere::speed_of_sound(self.altitude);
        let c = 1.83e-8 * nose_radius.powf(-0.5);
        c * rho.powf(0.5) * v.powi(3)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_hypersonic_new() {
        let h = HypersonicFlow::new(5.0, 30000.0, 0.0);
        assert!((h.mach - 5.0).abs() < 1e-10);
    }
    #[test]
    fn test_stagnation_temp() {
        let h = HypersonicFlow::new(5.0, 30000.0, 0.0);
        let t = h.stagnation_temperature();
        assert!(t > 200.0);
    }
    #[test]
    fn test_stagnation_pressure() {
        let h = HypersonicFlow::new(5.0, 30000.0, 0.0);
        assert!(h.stagnation_pressure() > 0.0);
    }
    #[test]
    fn test_convective_heating() {
        let h = HypersonicFlow::new(5.0, 30000.0, 0.0);
        assert!(h.convective_heating(0.5) >= 0.0);
    }
}
