//! Hemodynamics: vessel flow resistance, cardiac electrophysiology, Windkessel model.

use crate::core::types::Scalar;

/// Geometric and mechanical properties of a blood vessel segment.
pub struct VesselSegment {
    pub length: Scalar,
    pub radius: Scalar,
    pub wall_thickness: Scalar,
    pub young_modulus: Scalar,
}

impl VesselSegment {
    pub fn new(
        length: Scalar,
        radius: Scalar,
        wall_thickness: Scalar,
        young_modulus: Scalar,
    ) -> Self {
        Self {
            length,
            radius,
            wall_thickness,
            young_modulus,
        }
    }

    /// Poiseuille flow resistance: R = 8·η·L / (π·r⁴)
    pub fn flow_resistance(&self, viscosity: Scalar) -> Scalar {
        (8.0 * viscosity * self.length) / (std::f64::consts::PI * self.radius.powi(4))
    }

    /// Vascular compliance: C = 3·π·r³·L / (2·E·h)
    pub fn compliance(&self) -> Scalar {
        (3.0 * std::f64::consts::PI * self.radius.powi(3) * self.length)
            / (2.0 * self.young_modulus * self.wall_thickness)
    }

    /// Inertance: L = ρ·L / (π·r²)
    pub fn inertance(&self, density: Scalar) -> Scalar {
        (density * self.length) / (std::f64::consts::PI * self.radius * self.radius)
    }

    /// Volumetric flow rate: Q = ΔP / R
    pub fn flow_rate(&self, pressure_drop: Scalar, viscosity: Scalar) -> Scalar {
        pressure_drop / self.flow_resistance(viscosity)
    }
}

/// Hodgkin-Huxley neuron / cardiac cell model.
pub struct HodgkinHuxley {
    pub v_rest: Scalar,
    pub v_threshold: Scalar,
    pub g_na: Scalar,
    pub g_k: Scalar,
    pub g_l: Scalar,
    pub v: Scalar,
    pub m: Scalar,
    pub n: Scalar,
    pub h: Scalar,
}

impl HodgkinHuxley {
    /// Reversal potentials (mV relative to resting potential).
    const E_NA: Scalar = 115.0;
    const E_K: Scalar = -12.0;
    const E_L: Scalar = 10.6;
    const C_M: Scalar = 1.0; // membrane capacitance (µF/cm²)

    pub fn new(
        v_rest: Scalar,
        v_threshold: Scalar,
        g_na: Scalar,
        g_k: Scalar,
        g_l: Scalar,
    ) -> Self {
        Self {
            v_rest,
            v_threshold,
            g_na,
            g_k,
            g_l,
            v: 0.0, // start at rest (deviation = 0)
            m: 0.05,
            n: 0.32,
            h: 0.6,
        }
    }

    /// Derivative of membrane potential: C·dV/dt = I - Σ g·(V - E)
    pub fn membrane_potential_derivative(
        &self,
        v: Scalar,
        m: Scalar,
        n: Scalar,
        h: Scalar,
        i_stim: Scalar,
    ) -> Scalar {
        let i_na = self.g_na * m.powi(3) * h * (v - Self::E_NA);
        let i_k = self.g_k * n.powi(4) * (v - Self::E_K);
        let i_l = self.g_l * (v - Self::E_L);
        (i_stim - i_na - i_k - i_l) / Self::C_M
    }

    /// Gate rate equations. Returns (dm/dt, dn/dt, dh/dt).
    ///
    /// The gate kinetics use the *absolute* membrane potential
    /// `V = v_rest + v` (the standard Hodgkin-Huxley offsets v+40, v+65, ...),
    /// while `v` here is the deviation-from-rest voltage used by the ionic
    /// current equations.
    pub fn gate_derivatives(&self, v: Scalar) -> (Scalar, Scalar, Scalar) {
        let v_abs = self.v_rest + v;
        let alpha_m = if (v_abs + 40.0).abs() > 1e-10 {
            0.1 * (v_abs + 40.0) / (1.0 - (-(v_abs + 40.0) / 10.0).exp())
        } else {
            1.0
        };
        let beta_m = 4.0 * (-(v_abs + 65.0) / 18.0).exp();
        let alpha_n = if (v_abs + 55.0).abs() > 1e-10 {
            0.01 * (v_abs + 55.0) / (1.0 - (-(v_abs + 55.0) / 10.0).exp())
        } else {
            0.1
        };
        let beta_n = 0.125 * (-(v_abs + 65.0) / 80.0).exp();
        let alpha_h = 0.07 * (-(v_abs + 65.0) / 20.0).exp();
        let beta_h = 1.0 / (1.0 + (-(v_abs + 35.0) / 10.0).exp());

        let dm = alpha_m * (1.0 - self.m) - beta_m * self.m;
        let dn = alpha_n * (1.0 - self.n) - beta_n * self.n;
        let dh = alpha_h * (1.0 - self.h) - beta_h * self.h;
        (dm, dn, dh)
    }

    /// Advance one time step with forward Euler integration.
    pub fn step(&mut self, dt: Scalar, i_stim: Scalar) {
        let dv = self.membrane_potential_derivative(self.v, self.m, self.n, self.h, i_stim);
        let (dm, dn, dh) = self.gate_derivatives(self.v);
        self.v += dv * dt;
        self.m += dm * dt;
        self.n += dn * dt;
        self.h += dh * dt;
    }
}

/// Pulse wave velocity in an elastic vessel (Moens–Korteweg).
///
/// c = √(E·h / (2·r·ρ))
pub fn pulse_wave_velocity(e: Scalar, h: Scalar, r: Scalar, rho: Scalar) -> Scalar {
    ((e * h) / (2.0 * r * rho)).sqrt()
}

/// Three-element Windkessel model (R_proximal, C, R_peripheral).
pub struct WindkesselModel {
    pub r_proximal: Scalar,
    pub compliance: Scalar,
    pub r_peripheral: Scalar,
}

impl WindkesselModel {
    pub fn new(r_proximal: Scalar, compliance: Scalar, r_peripheral: Scalar) -> Self {
        Self {
            r_proximal,
            compliance,
            r_peripheral,
        }
    }

    /// Aortic pressure update (forward Euler on two-element core).
    ///
    /// dP/dt = (Q - P/R_peripheral) / compliance
    pub fn aortic_pressure(&self, flow: Scalar, p_prev: Scalar, dt: Scalar) -> Scalar {
        let dp = (flow - p_prev / self.r_peripheral) / self.compliance;
        p_prev + dp * dt
    }

    /// Input impedance magnitude and phase at frequency ω (rad/s).
    ///
    /// Z(ω) = R_proximal + R_peripheral / (1 + j·ω·C·R_peripheral)
    /// Returns (magnitude, phase_radians).
    pub fn impedance(&self, omega: Scalar) -> (Scalar, Scalar) {
        let denom_real = 1.0;
        let denom_imag = omega * self.compliance * self.r_peripheral;
        let denom_mag_sq = denom_real * denom_real + denom_imag * denom_imag;
        let z_real = self.r_proximal + self.r_peripheral * denom_real / denom_mag_sq;
        let z_imag = -self.r_peripheral * denom_imag / denom_mag_sq;
        let magnitude = (z_real * z_real + z_imag * z_imag).sqrt();
        let phase = z_imag.atan2(z_real);
        (magnitude, phase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vessel_flow_resistance() {
        let v = VesselSegment::new(0.1, 0.005, 0.001, 1.3e6);
        let r = v.flow_resistance(0.0035);
        assert!(r > 0.0);
        // Narrower vessel gives higher resistance
        let v2 = VesselSegment::new(0.1, 0.0025, 0.001, 1.3e6);
        let r2 = v2.flow_resistance(0.0035);
        assert!(r2 > r);
    }

    #[test]
    fn test_vessel_compliance() {
        let v = VesselSegment::new(0.1, 0.005, 0.001, 1.3e6);
        let c = v.compliance();
        assert!(c > 0.0);
    }

    #[test]
    fn test_vessel_inertance() {
        let v = VesselSegment::new(0.1, 0.005, 0.001, 1.3e6);
        let l = v.inertance(1060.0);
        assert!(l > 0.0);
    }

    #[test]
    fn test_vessel_flow_rate() {
        let v = VesselSegment::new(0.1, 0.005, 0.001, 1.3e6);
        let q = v.flow_rate(100.0, 0.0035);
        assert!(q > 0.0);
    }

    #[test]
    fn test_pulse_wave_velocity() {
        let c = pulse_wave_velocity(1.3e6, 0.001, 0.005, 1060.0);
        assert!(c > 0.0);
        assert!(c < 100.0);
    }

    #[test]
    fn test_windkessel_aortic_pressure() {
        let w = WindkesselModel::new(0.1, 1e-8, 1.0e8);
        let p = w.aortic_pressure(5e-5, 80.0, 0.001);
        // Pressure should rise when flow is positive
        assert!(p > 80.0);
    }

    #[test]
    fn test_windkessel_impedance_dc() {
        let w = WindkesselModel::new(0.1, 1e-8, 1.0e8);
        let (mag, phase) = w.impedance(0.0);
        // DC impedance = R_proximal + R_peripheral
        assert!((mag - 100_000_000.1).abs() < 1.0);
        assert!((phase).abs() < 1e-10);
    }

    #[test]
    fn test_hodgkin_huxley_step() {
        let mut hh = HodgkinHuxley::new(-65.0, -55.0, 120.0, 36.0, 0.3);
        hh.step(0.01, 10.0);
        // v should change after stimulation
        assert!(hh.v.abs() > 0.0);
        // gate variables should have been updated
        assert!(hh.m > 0.0 && hh.m <= 1.0);
        assert!(hh.n > 0.0 && hh.n <= 1.0);
        assert!(hh.h > 0.0 && hh.h <= 1.0);
    }

    #[test]
    fn test_hodgkin_huxley_gate_derivatives() {
        let hh = HodgkinHuxley::new(-65.0, -55.0, 120.0, 36.0, 0.3);
        let (dm, dn, dh) = hh.gate_derivatives(0.0);
        assert!(dm.is_finite());
        assert!(dn.is_finite());
        assert!(dh.is_finite());
    }

    #[test]
    fn test_vessel_segment_new() {
        let v = VesselSegment::new(0.05, 0.004, 0.0008, 1.0e6);
        assert!((v.length - 0.05).abs() < 1e-10);
        assert!((v.radius - 0.004).abs() < 1e-10);
        assert!((v.wall_thickness - 0.0008).abs() < 1e-10);
    }
}
