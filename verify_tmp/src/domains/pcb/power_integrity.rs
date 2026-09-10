//! Power integrity: IR drop, decoupling network, PDN impedance.

use crate::core::types::Scalar;
use num_complex::Complex;

/// DC voltage drop: V = I·R.
pub fn ir_drop(current: Scalar, resistance: Scalar) -> Scalar {
    current * resistance
}

/// Buck converter output ripple voltage (simplified).
pub fn buck_ripple_voltage(vin: Scalar, vout: Scalar, l: Scalar, c: Scalar, freq: Scalar, esr: Scalar) -> Scalar {
    if freq <= 0.0 || l <= 0.0 || c <= 0.0 { return 0.0; }
    let d = vout / vin;
    let di = (vin - vout) * d / (l * freq);
    let dv_c = di / (8.0 * c * freq);
    let dv_esr = di * esr;
    f64::sqrt(dv_c * dv_c + dv_esr * dv_esr)
}

/// Target impedance for PDN: Z_target = V·ripple_pct / I_max.
pub fn target_impedance(voltage: Scalar, ripple_pct: Scalar, max_current: Scalar) -> Scalar {
    if max_current <= 0.0 { return Scalar::INFINITY; }
    voltage * ripple_pct / max_current
}

/// Decoupling capacitor model.
#[derive(Debug, Clone)]
pub struct Decap {
    pub capacitance: Scalar,
    pub esr: Scalar,
    pub esl: Scalar,
    pub count: usize,
}

/// Network of parallel decoupling capacitors.
#[derive(Debug, Clone)]
pub struct DecapNetwork {
    pub capacitors: Vec<Decap>,
}

impl DecapNetwork {
    pub fn new() -> Self { Self { capacitors: Vec::new() } }

    pub fn add(&mut self, c: Decap) { self.capacitors.push(c); }

    /// Total impedance at frequency f.
    pub fn impedance(&self, freq: Scalar) -> Complex<Scalar> {
        let omega = 2.0 * std::f64::consts::PI * freq;
        // Compute admittance sum: Y_total = Σ 1/Z_i
        let mut y_sum = Complex::new(0.0, 0.0);
        for cap in &self.capacitors {
            let z_c = Complex::new(cap.esr, omega * cap.esl - 1.0 / (omega * cap.capacitance));
            let n = cap.count as Scalar;
            // Each capacitor contributes n * (1/Z_i)
            if z_c.norm_sqr() > 0.0 {
                y_sum += Complex::new(n * z_c.re / z_c.norm_sqr(), -n * z_c.im / z_c.norm_sqr());
            }
        }
        if y_sum.norm_sqr() > 0.0 {
            Complex::new(y_sum.re / y_sum.norm_sqr(), -y_sum.im / y_sum.norm_sqr())
        } else {
            Complex::new(Scalar::INFINITY, 0.0)
        }
    }

    /// Self-resonant frequency of the i-th capacitor.
    pub fn self_resonant_freq(&self, idx: usize) -> Scalar {
        if idx >= self.capacitors.len() { return 0.0; }
        let c = &self.capacitors[idx];
        if c.capacitance <= 0.0 || c.esl <= 0.0 { return 0.0; }
        1.0 / (2.0 * std::f64::consts::PI * f64::sqrt(c.capacitance * c.esl))
    }

    /// Frequencies of parallel resonance peaks.
    pub fn parallel_resonance_peaks(&self) -> Vec<Scalar> {
        let mut peaks = Vec::new();
        for i in 0..self.capacitors.len().saturating_sub(1) {
            let f1 = self.self_resonant_freq(i);
            let f2 = self.self_resonant_freq(i + 1);
            if f1 > 0.0 && f2 > 0.0 {
                peaks.push(f64::sqrt(f1 * f2));
            }
        }
        peaks
    }
}

impl Default for DecapNetwork { fn default() -> Self { Self::new() } }

/// PDN impedance at frequency f.
///
/// The VRM output impedance `vrm_output` (Ω) is placed in series with the
/// parallel combination of the plane capacitance and the decoupling network:
/// `Z_pdn = Z_vrm + (Z_plane ∥ Z_decap)`.
pub fn pdn_impedance(vrm_output: Scalar, decap_network: &DecapNetwork, plane_cap: Scalar, freq: Scalar) -> Complex<Scalar> {
    let omega = 2.0 * std::f64::consts::PI * freq;
    let z_plane = if plane_cap > 0.0 { Complex::new(0.0, -1.0 / (omega * plane_cap)) } else { Complex::new(0.0, 0.0) };
    let z_decap = decap_network.impedance(freq);
    let z_parallel = if z_decap.norm() > 0.0 && z_plane.norm() > 0.0 {
        (z_decap * z_plane) / (z_decap + z_plane)
    } else if z_decap.norm() > 0.0 {
        z_decap
    } else {
        z_plane
    };
    // VRM source impedance in series with the rest of the PDN.
    Complex::new(vrm_output, 0.0) + z_parallel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ir_drop() {
        assert!((ir_drop(1.0, 0.1) - 0.1).abs() < 1e-15);
    }

    #[test]
    fn test_buck_ripple() {
        let dv = buck_ripple_voltage(12.0, 5.0, 10e-6, 22e-6, 500e3, 0.005);
        assert!(dv > 0.0 && dv < 1.0);
    }

    #[test]
    fn test_target_impedance() {
        let z = target_impedance(1.8, 0.05, 10.0);
        assert!((z - 0.009).abs() < 1e-10);
    }

    #[test]
    fn test_decap_srf() {
        let mut net = DecapNetwork::new();
        net.add(Decap { capacitance: 10e-6, esr: 0.01, esl: 1e-9, count: 2 });
        let srf = net.self_resonant_freq(0);
        assert!(srf > 1e3 && srf < 1e8);
    }

    #[test]
    fn test_decap_network_impedance() {
        let mut net = DecapNetwork::new();
        net.add(Decap { capacitance: 10e-6, esr: 0.01, esl: 1e-9, count: 1 });
        let z = net.impedance(1e5);
        assert!(z.norm() > 0.0);
    }
}
