//! 1D arterial network blood flow model.
use crate::core::types::Scalar;
#[derive(Debug, Clone)]
pub struct ArterialSegment {
    pub length: Scalar,
    pub radius: Scalar,
    pub wall_thickness: Scalar,
    pub young_modulus: Scalar,
}
impl ArterialSegment {
    pub fn new(length: Scalar, radius: Scalar, wt: Scalar, young: Scalar) -> Self {
        Self {
            length,
            radius,
            wall_thickness: wt,
            young_modulus: young,
        }
    }
    pub fn pulse_wave_velocity(&self, rho: Scalar) -> Scalar {
        ((self.young_modulus * self.wall_thickness) / (2.0 * rho * self.radius)).sqrt()
    }
}
#[derive(Debug, Clone)]
pub struct CirculatoryNetwork {
    pub segments: Vec<ArterialSegment>,
    pub rho_blood: Scalar,
}
impl CirculatoryNetwork {
    pub fn new(rho_blood: Scalar) -> Self {
        Self {
            segments: Vec::new(),
            rho_blood,
        }
    }
    pub fn add_segment(&mut self, seg: ArterialSegment) {
        self.segments.push(seg);
    }
    pub fn pressure_wave(&self, t_end: Scalar, dt: Scalar) -> Result<Vec<Vec<Scalar>>, String> {
        if dt <= 0.0 {
            return Err("dt must be positive".to_string());
        }
        let n = self.segments.len();
        let n_steps = (t_end / dt) as usize;
        let mut results = vec![vec![0.0; n_steps + 1]; n];
        for step in 0..=n_steps {
            let t = step as Scalar * dt;
            for (i, seg) in self.segments.iter().enumerate() {
                let z0 = seg.pulse_wave_velocity(self.rho_blood);
                let q_in = (t * 2.0 * std::f64::consts::PI).sin().max(0.0) * 5e-5;
                results[i][step] = z0 * q_in;
            }
        }
        Ok(results)
    }
    pub fn reflection_coefficient(&self, i: usize) -> Scalar {
        if i >= self.segments.len().saturating_sub(1) {
            return 0.0;
        }
        let z1 = self.segments[i].pulse_wave_velocity(self.rho_blood);
        let z2 = self.segments[i + 1].pulse_wave_velocity(self.rho_blood);
        (z2 - z1) / (z2 + z1).max(1e-30)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_segment() {
        let s = ArterialSegment::new(0.1, 0.01, 0.001, 5e6);
        assert!(s.pulse_wave_velocity(1060.0) > 0.0);
    }
    #[test]
    fn test_pressure() {
        let mut net = CirculatoryNetwork::new(1060.0);
        net.add_segment(ArterialSegment::new(0.1, 0.01, 0.001, 5e6));
        let pw = net.pressure_wave(1.0, 0.01).unwrap();
        assert_eq!(pw.len(), 1);
    }
    #[test]
    fn test_reflection() {
        let mut net = CirculatoryNetwork::new(1060.0);
        net.add_segment(ArterialSegment::new(0.1, 0.01, 0.001, 5e6));
        net.add_segment(ArterialSegment::new(0.1, 0.005, 0.001, 5e6));
        assert!(net.reflection_coefficient(0).is_finite());
    }
}
