//! Photoelectric conversion: photodetectors, solar cells, optoelectronic coupling.

use crate::core::block::{Block, BlockError, BlockId};
use crate::core::io::{IODeclaration, InputDecl, OutputDecl};
use crate::core::param::{Parameter, ParameterSet};
use crate::core::port::{Port, PortSet};
use crate::core::signal::Signal;
use crate::core::types::{
    ComponentStatus, PortDirection, Scalar, SignalType, SignalValue, Time,
};

/// Photocurrent: I_photo = R · P, where R = responsivity (A/W).
pub fn photocurrent(responsivity: Scalar, optical_power: Scalar) -> Scalar {
    responsivity * optical_power
}

/// Quantum efficiency: QE = (I_photo/q) / (P·λ/(h·c)).
pub fn quantum_efficiency(
    photocurrent_val: Scalar,
    optical_power: Scalar,
    wavelength: Scalar,
) -> Scalar {
    if optical_power <= 0.0 || wavelength <= 0.0 {
        return 0.0;
    }
    let n_photons = optical_power * wavelength / (super::physics::H_PLANCK * super::physics::C);
    if n_photons <= 0.0 {
        return 0.0;
    }
    let n_electrons = photocurrent_val / 1.602176634e-19;
    n_electrons / n_photons
}

/// Solar cell I-V characteristic (single-diode model).
///
/// I = I_ph - I₀·(exp(qV/(nkT)) - 1)
pub fn solar_cell_iv(
    photocurrent_val: Scalar,
    saturation_current: Scalar,
    voltage: Scalar,
    n: Scalar,
    temp: Scalar,
) -> Scalar {
    let vt = 1.380649e-23 * temp / 1.602176634e-19;
    let exp_arg = voltage / (n * vt);
    photocurrent_val - saturation_current * (exp_arg.exp() - 1.0)
}

/// Photodetector Block: converts optical power input to current output.
pub struct PhotodetectorBlock {
    id: BlockId,
    ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    time: Time,
    responsivity: Scalar,
    dark_current: Scalar,
}

impl PhotodetectorBlock {
    pub fn new(id: &str, responsivity: Scalar, dark_current: Scalar) -> Self {
        let mut ports = PortSet::new();
        ports.add(
            Port::new(
                "optical_power",
                PortDirection::Input,
                SignalType::Continuous,
            )
            .with_description("Input optical power (W)"),
        );
        ports.add(
            Port::new("current", PortDirection::Output, SignalType::Continuous)
                .with_description("Output photocurrent (A)"),
        );
        let mut params = ParameterSet::new();
        params.add(Parameter::new_static(
            "responsivity",
            SignalValue::Scalar(responsivity),
            "Responsivity (A/W)",
        ));
        params.add(Parameter::new_static(
            "dark_current",
            SignalValue::Scalar(dark_current),
            "Dark current (A)",
        ));
        Self {
            id: id.to_string(),
            ports,
            params,
            status: ComponentStatus::Inactive,
            time: 0.0,
            responsivity,
            dark_current,
        }
    }
}

impl Block for PhotodetectorBlock {
    fn id(&self) -> &BlockId {
        &self.id
    }
    fn block_type(&self) -> &str {
        "Photodetector"
    }
    fn ports(&self) -> &PortSet {
        &self.ports
    }
    fn ports_mut(&mut self) -> &mut PortSet {
        &mut self.ports
    }
    fn params(&self) -> &ParameterSet {
        &self.params
    }
    fn params_mut(&mut self) -> &mut ParameterSet {
        &mut self.params
    }
    fn status(&self) -> ComponentStatus {
        self.status
    }
    fn set_status(&mut self, status: ComponentStatus) {
        self.status = status;
    }
    fn set_time(&mut self, time: Time) {
        self.time = time;
    }
    fn time(&self) -> Time {
        self.time
    }

    fn io_declaration(&self) -> IODeclaration {
        let mut io = IODeclaration::new();
        io.add_input(
            InputDecl::new("optical_power", SignalType::Continuous)
                .with_description("Input optical power (W)"),
        );
        io.add_output(
            OutputDecl::new("current", SignalType::Continuous)
                .with_description("Output photocurrent (A)"),
        );
        io
    }

    fn init(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Ready;
        Ok(())
    }

    fn output(&mut self) -> Result<(), BlockError> {
        let power = self
            .ports
            .get("optical_power")
            .and_then(|p| p.read())
            .and_then(|s| {
                if let SignalValue::Scalar(v) = &s.value {
                    Some(*v)
                } else {
                    None
                }
            })
            .unwrap_or(0.0);
        let current = photocurrent(self.responsivity, power) + self.dark_current;
        if let Some(p) = self.ports.get_mut("current") {
            p.write(Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(current),
                self.time,
            ));
        }
        Ok(())
    }

    fn derivative(&self) -> Result<Vec<Scalar>, BlockError> {
        Ok(vec![])
    }
    fn update(&mut self) -> Result<(), BlockError> {
        Ok(())
    }
    fn zero_crossings(&self) -> Vec<Scalar> {
        vec![]
    }
    fn terminate(&mut self) -> Result<(), BlockError> {
        self.status = ComponentStatus::Completed;
        Ok(())
    }

    fn clone_block(&self) -> Box<dyn Block> {
        Box::new(Self {
            id: self.id.clone(),
            ports: PortSet::new(),
            params: ParameterSet::new(),
            status: ComponentStatus::Inactive,
            time: 0.0,
            responsivity: self.responsivity,
            dark_current: self.dark_current,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_photocurrent_const() {
        let i = photocurrent(0.5, 1e-3);
        assert!((i - 5e-4).abs() < 1e-15);
    }

    #[test]
    fn test_quantum_efficiency_ideal() {
        let power = 1e-6;
        let lambda = 500e-9;
        let n_photons = power * lambda / (crate::domains::optical::physics::H_PLANCK * crate::domains::optical::physics::C);
        let ideal_current = n_photons * 1.602176634e-19;
        let qe = quantum_efficiency(ideal_current, power, lambda);
        assert!((qe - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_solar_cell_open_circuit() {
        let i_ph = 1.0;
        let i0 = 1e-10;
        let n = 1.0;
        let temp = 300.0;
        let vt = 1.380649e-23 * temp / 1.602176634e-19;
        let voc_expected = n * vt * (i_ph / i0 as Scalar).ln();
        let i_at_voc = solar_cell_iv(i_ph, i0, voc_expected, n, temp);
        assert!(i_at_voc.abs() < 0.01 * i_ph);
    }

    #[test]
    fn test_photodetector_create() {
        let pd = PhotodetectorBlock::new("pd1", 0.5, 0.0);
        assert_eq!(*pd.id(), "pd1");
        assert_eq!(pd.block_type(), "Photodetector");
    }
}
