//! Board-level electro-thermal simulation.

use crate::core::block::{Block, BlockError, BlockId};
use crate::core::param::{Parameter, ParameterSet};
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{ComponentStatus, PortDirection, Scalar, SignalType, SignalValue, Time};

/// Chip junction temperature: T_j = T_a + θ_ja · P.
pub fn junction_temperature(ambient_temp: Scalar, theta_ja: Scalar, power: Scalar) -> Scalar {
    ambient_temp + theta_ja * power
}

/// PCB trace temperature rise (IPC-2151 simplified).
pub fn pcb_trace_temperature_rise(current: Scalar, width: Scalar, _thickness: Scalar, ambient_temp: Scalar) -> Scalar {
    let r_per_mm = 1.72e-8 / (width * 35e-6); // 1 oz copper
    let power_per_mm = current * current * r_per_mm;
    let temp_rise = 30.0 * f64::powf(power_per_mm * 1000.0, 0.5);
    ambient_temp + temp_rise
}

/// Thermal resistance network (chip → package → board → ambient).
#[derive(Debug, Clone)]
pub struct ThermalNetwork {
    pub theta_jc: Scalar,
    pub theta_cb: Scalar,
    pub theta_ba: Scalar,
}

impl ThermalNetwork {
    pub fn new(theta_jc: Scalar, theta_cb: Scalar, theta_ba: Scalar) -> Self {
        Self { theta_jc, theta_cb, theta_ba }
    }

    pub fn total_theta_ja(&self) -> Scalar {
        self.theta_jc + self.theta_cb + self.theta_ba
    }

    pub fn steady_state_temp(&self, power: Scalar, ambient: Scalar) -> Scalar {
        ambient + self.total_theta_ja() * power
    }

    pub fn thermal_time_constant(&self, thermal_capacitance: Scalar) -> Scalar {
        self.total_theta_ja() * thermal_capacitance
    }
}

/// PCB thermal Block: takes power array input, outputs temperature distribution.
pub struct PcbThermalBlock {
    id: BlockId,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    time: Time,
    n_cells: usize,
}

impl PcbThermalBlock {
    pub fn new(id: &str, n_cells: usize) -> Self {
        let mut ports = PortSet::new();
        ports.add(Port::new("power_in", PortDirection::Input, SignalType::Continuous).with_description("Input power (W)"));
        ports.add(Port::new("temp_out", PortDirection::Output, SignalType::Continuous).with_description("Output temperature (°C)"));
        let mut params = ParameterSet::new();
        params.add(Parameter::new_static("n_cells", SignalValue::Scalar(n_cells as Scalar), "Number of cells"));
        params.add(Parameter::new_config("theta_ja", SignalValue::Scalar(20.0), "Junction-to-ambient thermal resistance"));
        params.add(Parameter::new_config("ambient_temp", SignalValue::Scalar(25.0), "Ambient temperature (°C)"));
        Self { id: id.to_string(), ports, params, status: ComponentStatus::Inactive, time: 0.0, n_cells }
    }
}

impl Block for PcbThermalBlock {
    fn id(&self) -> &BlockId { &self.id }
    fn block_type(&self) -> &str { "PcbThermal" }
    fn ports(&self) -> &PortSet { &self.ports }
    fn ports_mut(&mut self) -> &mut PortSet { &mut self.ports }
    fn params(&self) -> &ParameterSet { &self.params }
    fn params_mut(&mut self) -> &mut ParameterSet { &mut self.params }
    fn status(&self) -> ComponentStatus { self.status }
    fn set_status(&mut self, status: ComponentStatus) { self.status = status; }
    fn set_time(&mut self, time: Time) { self.time = time; }
    fn time(&self) -> Time { self.time }

    fn init(&mut self) -> Result<(), BlockError> { self.status = ComponentStatus::Ready; Ok(()) }
    fn output(&mut self) -> Result<(), BlockError> {
        let power = self.ports.get("power_in").and_then(|p| p.read()).and_then(|s| {
            if let SignalValue::Scalar(v) = &s.value { Some(*v) } else { None }
        }).unwrap_or(0.0);
        let ambient = self.params.get_scalar("ambient_temp").unwrap_or(25.0);
        let theta_ja = self.params.get_scalar("theta_ja").unwrap_or(20.0);
        let temp = ambient + theta_ja * power;
        if let Some(p) = self.ports.get_mut("temp_out") {
            p.write(Signal::new(SignalType::Continuous, SignalValue::Scalar(temp), self.time));
        }
        Ok(())
    }
    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> { Ok(vec![]) }
    fn update(&mut self) -> Result<(), BlockError> { Ok(()) }
    fn zero_crossings(&self) -> Vec<Scalar> { vec![] }
    fn terminate(&mut self) -> Result<(), BlockError> { self.status = ComponentStatus::Completed; Ok(()) }
    fn clone_block(&self) -> Box<dyn Block> {
        Box::new(Self { id: self.id.clone(), ports: PortSet::new(), params: ParameterSet::new(),
            status: ComponentStatus::Inactive, time: 0.0, n_cells: self.n_cells })
    }
}

/// Hot spot temperature estimation (simplified).
pub fn hot_spot_temperature(
    power_map: &[Vec<Scalar>],
    via_count: usize,
    board_thickness: Scalar,
    copper_coverage: Scalar,
) -> Scalar {
    if power_map.is_empty() || power_map[0].is_empty() {
        return 25.0;
    }

    let mut total_power: Scalar = 0.0;
    let mut peak_power: Scalar = 0.0;
    let mut cell_count: Scalar = 0.0;
    for row in power_map {
        for &power in row {
            let power = power.max(0.0);
            total_power += power;
            peak_power = peak_power.max(power);
            cell_count += 1.0;
        }
    }

    if cell_count <= 0.0 {
        return 25.0;
    }

    let mean_power = total_power / cell_count;
    let via_factor = 1.0 / (1.0 + via_count as Scalar * 0.08);
    let thickness_factor = (board_thickness.max(0.2) / 1.6).clamp(0.25, 4.0);
    let copper_factor = (1.0 - copper_coverage.clamp(0.0, 1.0)) * 0.75 + 0.25;
    let thermal_load = 0.6 * peak_power + 0.4 * mean_power;

    25.0 + thermal_load * 35.0 * thickness_factor * copper_factor * via_factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_junction_temperature() {
        let tj = junction_temperature(25.0, 20.0, 2.0);
        assert!((tj - 65.0).abs() < 1e-10);
    }

    #[test]
    fn test_thermal_network_total() {
        let tn = ThermalNetwork::new(5.0, 3.0, 12.0);
        assert!((tn.total_theta_ja() - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_thermal_network_steady_state() {
        let tn = ThermalNetwork::new(5.0, 3.0, 12.0);
        let t = tn.steady_state_temp(2.0, 25.0);
        assert!((t - 65.0).abs() < 1e-10);
    }

    #[test]
    fn test_pcb_thermal_block() {
        let mut b = PcbThermalBlock::new("thermal1", 4);
        assert_eq!(*b.id(), "thermal1");
        b.init().unwrap();
        b.output().unwrap();
    }

    #[test]
    fn test_hot_spot_temperature_increases_with_power() {
        let low = hot_spot_temperature(&[vec![1.0, 1.0], vec![1.0, 1.0]], 2, 1.6, 0.4);
        let high = hot_spot_temperature(&[vec![4.0, 4.0], vec![4.0, 4.0]], 2, 1.6, 0.4);
        assert!(high > low);
    }

    #[test]
    fn test_hot_spot_temperature_improves_with_vias_and_copper() {
        let base = hot_spot_temperature(&[vec![3.0, 3.0], vec![3.0, 3.0]], 0, 1.6, 0.1);
        let cooled = hot_spot_temperature(&[vec![3.0, 3.0], vec![3.0, 3.0]], 8, 1.6, 0.8);
        assert!(cooled < base);
    }
}
