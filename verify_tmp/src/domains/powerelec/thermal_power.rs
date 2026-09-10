//! Power electronics thermal analysis.

use crate::core::types::Scalar;

/// Power device junction temperature.
pub fn device_junction_temp(total_loss: Scalar, n_devices: usize, rth_jc: Scalar, rth_ch: Scalar, rth_ha: Scalar, ambient: Scalar) -> Scalar {
    let rth_total = rth_jc + rth_ch + rth_ha;
    let loss_per_device = total_loss / n_devices as Scalar;
    ambient + loss_per_device * rth_total
}

/// Heat sink thermal resistance (simplified natural convection).
pub fn heatsink_thermal_resistance(volume: Scalar, fin_area: Scalar, airflow: Scalar) -> Scalar {
    if fin_area <= 0.0 { return Scalar::INFINITY; }
    let base_rth = 250.0 / f64::sqrt(volume * 1e6);
    let airflow_factor = if airflow > 0.0 { f64::powf(airflow / 2.0, -0.5).min(1.0) } else { 1.0 };
    base_rth * airflow_factor / fin_area * 0.001
}

/// Power loss breakdown.
#[derive(Debug, Clone)]
pub struct PowerLossBreakdown {
    pub conduction_loss: Scalar,
    pub switching_loss: Scalar,
    pub core_loss: Scalar,
    pub copper_loss: Scalar,
    pub mechanical_loss: Scalar,
}

impl PowerLossBreakdown {
    pub fn total(&self) -> Scalar {
        self.conduction_loss + self.switching_loss + self.core_loss + self.copper_loss + self.mechanical_loss
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_junction_temp() {
        let tj = device_junction_temp(50.0, 2, 1.0, 0.5, 10.0, 25.0);
        assert!((tj - 25.0 - 25.0 * 11.5).abs() < 0.01);
    }

    #[test]
    fn test_heatsink_resistance() {
        let rth = heatsink_thermal_resistance(1e-4, 0.01, 0.0);
        assert!(rth > 0.0);
    }

    #[test]
    fn test_loss_breakdown_total() {
        let lb = PowerLossBreakdown { conduction_loss: 10.0, switching_loss: 5.0, core_loss: 2.0, copper_loss: 3.0, mechanical_loss: 1.0 };
        assert!((lb.total() - 21.0).abs() < 1e-10);
    }
}
