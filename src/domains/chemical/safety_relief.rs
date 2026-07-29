//! Safety relief valve and flare network sizing.

use crate::core::types::Scalar;

pub struct GasProperties { pub gamma: Scalar, pub mw: Scalar, pub z: Scalar, pub t: Scalar }

pub fn relief_valve_flow_area(set_pressure: Scalar, mass_flow: Scalar, gas: &GasProperties) -> Scalar {
    let r = 8314.0 / gas.mw;
    let rho = set_pressure * gas.mw / (gas.z * 8314.0 * gas.t);
    let _v = mass_flow / rho.max(1e-30);
    let c0 = (gas.gamma * r * gas.t).sqrt();
    mass_flow / (c0 * set_pressure * 0.6).max(1e-30)
}

pub struct PipeSegment { pub length: Scalar, pub diameter: Scalar, pub roughness: Scalar }

pub fn flare_network_backpressure(pipes: &[PipeSegment], relief_rate: Scalar) -> Scalar {
    let mut dp = 0.0;
    for pipe in pipes {
        let area = std::f64::consts::PI * pipe.diameter * pipe.diameter / 4.0;
        let vel = relief_rate / area.max(1e-30);
        let f = 0.02; // Darcy friction factor approximation
        dp += f * pipe.length / pipe.diameter * 0.5 * 1.2 * vel * vel;
    }
    dp
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_relief_area() {
        let gas = GasProperties { gamma: 1.4, mw: 28.0, z: 1.0, t: 300.0 };
        let a = relief_valve_flow_area(1e6, 10.0, &gas);
        assert!(a > 0.0);
    }
    #[test]
    fn test_flare_backpressure() {
        let pipes = vec![PipeSegment { length: 100.0, diameter: 0.3, roughness: 0.0001 }];
        let dp = flare_network_backpressure(&pipes, 50.0);
        assert!(dp >= 0.0);
    }
}
