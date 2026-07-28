//! Power semiconductor device models: diode, MOSFET, IGBT, thyristor.

use crate::core::types::Scalar;

/// Power diode model.
#[derive(Debug, Clone)]
pub struct PowerDiode {
    pub vf: Scalar,
    pub r_on: Scalar,
    pub trr: Scalar,
    pub v_br: Scalar,
    pub i_max: Scalar,
}

impl PowerDiode {
    pub fn forward_voltage(&self, current: Scalar) -> Scalar {
        self.vf + current * self.r_on
    }

    pub fn conduction_loss(&self, current: Scalar, duty: Scalar) -> Scalar {
        self.forward_voltage(current) * current * duty
    }

    pub fn switching_loss(&self, current: Scalar, v_dc: Scalar, freq: Scalar) -> Scalar {
        0.5 * v_dc * current * self.trr * freq
    }
}

/// Power MOSFET model.
#[derive(Debug, Clone)]
pub struct PowerMosfet {
    pub r_ds_on: Scalar,
    pub v_th: Scalar,
    pub q_g: Scalar,
    pub c_iss: Scalar,
    pub c_rss: Scalar,
    pub v_dss: Scalar,
    pub i_d_max: Scalar,
}

impl PowerMosfet {
    pub fn conduction_loss(&self, i_d: Scalar, rds_on: Scalar, duty: Scalar) -> Scalar {
        i_d * i_d * rds_on * duty
    }

    pub fn switching_loss(&self, i_d: Scalar, v_dc: Scalar, freq: Scalar) -> Scalar {
        0.5 * v_dc * i_d * (self.q_g / self.c_iss) * freq
    }

    pub fn gate_drive_power(&self, v_gs: Scalar, freq: Scalar) -> Scalar {
        self.q_g * v_gs * freq
    }

    pub fn rds_on_temp(&self, temp_c: Scalar) -> Scalar {
        self.r_ds_on * (1.0 + 0.005 * (temp_c - 25.0))
    }
}

/// IGBT model.
#[derive(Debug, Clone)]
pub struct Igbt {
    pub v_ce_sat: Scalar,
    pub r_on: Scalar,
    pub e_on: Scalar,
    pub e_off: Scalar,
    pub v_ces: Scalar,
    pub i_c_max: Scalar,
}

impl Igbt {
    pub fn conduction_loss(&self, i_c: Scalar, duty: Scalar) -> Scalar {
        (self.v_ce_sat * i_c + self.r_on * i_c * i_c) * duty
    }

    pub fn switching_loss(&self, i_c: Scalar, v_dc: Scalar, freq: Scalar) -> Scalar {
        (self.e_on + self.e_off) * i_c * v_dc * freq / (25.0 * 600.0)
    }

    pub fn total_loss(&self, i_c: Scalar, v_dc: Scalar, freq: Scalar, duty: Scalar) -> Scalar {
        self.conduction_loss(i_c, duty) + self.switching_loss(i_c, v_dc, freq)
    }
}

/// Thyristor model.
#[derive(Debug, Clone)]
pub struct Thyristor {
    pub v_ak_on: Scalar,
    pub i_l: Scalar,
    pub i_h: Scalar,
    pub v_rrm: Scalar,
    pub t_q: Scalar,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diode_forward_voltage() {
        let d = PowerDiode { vf: 0.8, r_on: 0.01, trr: 50e-9, v_br: 600.0, i_max: 10.0 };
        let v = d.forward_voltage(5.0);
        assert!((v - 0.85).abs() < 1e-10);
    }

    #[test]
    fn test_diode_conduction_loss() {
        let d = PowerDiode { vf: 0.8, r_on: 0.01, trr: 50e-9, v_br: 600.0, i_max: 10.0 };
        let p = d.conduction_loss(5.0, 0.5);
        assert!((p - 0.85 * 5.0 * 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_mosfet_conduction_loss() {
        let m = PowerMosfet { r_ds_on: 0.01, v_th: 3.0, q_g: 20e-9, c_iss: 1e-9, c_rss: 10e-12, v_dss: 100.0, i_d_max: 50.0 };
        let p = m.conduction_loss(10.0, 0.01, 0.5);
        assert!((p - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_mosfet_temp_dep() {
        let m = PowerMosfet { r_ds_on: 0.01, v_th: 3.0, q_g: 20e-9, c_iss: 1e-9, c_rss: 10e-12, v_dss: 100.0, i_d_max: 50.0 };
        let r_hot = m.rds_on_temp(125.0);
        assert!(r_hot > 0.01);
    }

    #[test]
    fn test_igbt_total_loss() {
        let igbt = Igbt { v_ce_sat: 1.8, r_on: 0.005, e_on: 0.002, e_off: 0.001, v_ces: 1200.0, i_c_max: 100.0 };
        let p = igbt.total_loss(50.0, 600.0, 10e3, 0.5);
        assert!(p > 0.0);
    }
}
