//! SerDes Channel Operating Margin (COM) computation.

use crate::core::types::Scalar;
type CS = num_complex::Complex<f64>;

/// Channel Operating Margin (COM) for SerDes link compliance.
#[derive(Debug, Clone)]
pub struct ChannelOperatingMargin {
    pub tx_eq: Vec<Scalar>, pub rx_eq: Vec<Scalar>,
    pub channel_s4p: Vec<Vec<CS>>,
    pub baud_rate: Scalar, pub ber_target: Scalar,
}

impl ChannelOperatingMargin {
    pub fn new(baud: Scalar) -> Self {
        Self { tx_eq: vec![1.0], rx_eq: vec![1.0], channel_s4p: Vec::new(), baud_rate: baud, ber_target: 1e-12 }
    }

    pub fn compute_com(&self) -> Scalar {
        let _f_nyquist = self.baud_rate / 2.0;
        let mut signal = 0.0; let mut noise = 1e-30;
        for s_params in &self.channel_s4p {
            if s_params.len() >= 2 {
                let h = s_params[1].norm(); // S21 magnitude
                signal += h;
                noise += (1.0 - h).abs();
            }
        }
        20.0 * (signal / noise).log10()
    }

    pub fn eye_height_at_ber(&self, target_ber: Scalar) -> Scalar {
        let com = self.compute_com();
        let sigma = 1.0;
        2.0 * (com / 20.0).exp() * sigma * (1.0 / (target_ber * 2.0)).ln().sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_com_new() { let c = ChannelOperatingMargin::new(25e9); assert!((c.baud_rate - 25e9).abs() < 1.0); }
    #[test]
    fn test_compute_com() {
        let mut c = ChannelOperatingMargin::new(25e9);
        c.channel_s4p = vec![vec![CS::new(0.5, 0.0), CS::new(0.8, 0.0)]];
        let com = c.compute_com();
        assert!(com.is_finite());
    }
    #[test]
    fn test_eye_height() {
        let c = ChannelOperatingMargin::new(25e9);
        let eh = c.eye_height_at_ber(1e-12);
        assert!(eh.is_finite());
    }
}
