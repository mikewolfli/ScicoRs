//! Simulation Modules
//!
//! Specialized simulation modules for various domains including
//! semiconductor physics, circuit simulation, molecular dynamics,
//! optics, acoustics, thermal, fluid, structural, and more.
//!
//! Each domain module defines domain-specific block types, models,
//! and solvers built on top of the core kernel.

use crate::core::types::Scalar;

/// Common parameters shared across simulation domains.
#[derive(Debug, Clone)]
pub struct DomainConfig {
    pub name: String,
    pub enabled: bool,
    pub parameters: std::collections::HashMap<String, Scalar>,
}

impl DomainConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            enabled: true,
            parameters: std::collections::HashMap::new(),
        }
    }
}

/// Semiconductor device physics (TCAD) domain.
pub mod semiconductor {
    use crate::core::block::{Block, BlockError, SimpleBlock};
    use crate::core::error::SimError;
    use crate::core::param::ParameterSet;
    use crate::core::port::{Port, PortDirection, PortSet};
    use crate::core::signal::Signal;
    use crate::core::types::{
        ComponentStatus, Scalar, SignalType, SignalValue, Time,
    };

    /// A simple drift-diffusion transport model block.
    #[derive(Debug)]
    pub struct DriftDiffusionModel {
        inner: SimpleBlock,
        electron_mobility: Scalar,
        hole_mobility: Scalar,
    }

    impl DriftDiffusionModel {
        pub fn new(id: &str) -> Self {
            let mut inner = SimpleBlock::new(id, "DriftDiffusion");
            inner.add_port(Port::new("E", PortDirection::Input, SignalType::Continuous));
            inner.add_port(Port::new("n", PortDirection::Input, SignalType::Continuous));
            inner.add_port(Port::new("p", PortDirection::Input, SignalType::Continuous));
            inner.add_port(Port::new("Jn", PortDirection::Output, SignalType::Continuous));
            inner.add_port(Port::new("Jp", PortDirection::Output, SignalType::Continuous));
            Self {
                inner,
                electron_mobility: 1350.0,
                hole_mobility: 480.0,
            }
        }
    }

    impl Block for DriftDiffusionModel {
        fn id(&self) -> &String { self.inner.id() }
        fn block_type(&self) -> &str { self.inner.block_type() }
        fn ports(&self) -> &PortSet { self.inner.ports() }
        fn ports_mut(&mut self) -> &mut PortSet { self.inner.ports_mut() }
        fn params(&self) -> &ParameterSet { self.inner.params() }
        fn params_mut(&mut self) -> &mut ParameterSet { self.inner.params_mut() }
        fn status(&self) -> ComponentStatus { self.inner.status() }
        fn set_status(&mut self, s: ComponentStatus) { self.inner.set_status(s); }
        fn set_time(&mut self, t: Time) { self.inner.set_time(t); }
        fn time(&self) -> Time { self.inner.time() }

        fn init(&mut self) -> Result<(), BlockError> {
            self.inner.init()
        }

        fn output(&mut self) -> Result<(), BlockError> {
            let e = self.ports().get("E")
                .and_then(|p| p.read()).and_then(|s| s.as_scalar())
                .ok_or_else(|| SimError::missing_port("E"))?;
            let n = self.ports().get("n")
                .and_then(|p| p.read()).and_then(|s| s.as_scalar())
                .ok_or_else(|| SimError::missing_port("n"))?;
            let p = self.ports().get("p")
                .and_then(|p| p.read()).and_then(|s| s.as_scalar())
                .ok_or_else(|| SimError::missing_port("p"))?;

            let q = 1.602e-19; // elementary charge
            let jn = q * self.electron_mobility * n * e;
            let jp = q * self.hole_mobility * p * e;
            let t = self.time();

            if let Some(port) = self.inner.ports_mut().get_mut("Jn") {
                port.write(Signal::new(SignalType::Continuous, SignalValue::Scalar(jn), t));
            }
            if let Some(port) = self.inner.ports_mut().get_mut("Jp") {
                port.write(Signal::new(SignalType::Continuous, SignalValue::Scalar(jp), t));
            }
            Ok(())
        }

        fn derivative(&self) -> Result<Vec<Scalar>, BlockError> { Ok(Vec::new()) }
        fn update(&mut self) -> Result<(), BlockError> { Ok(()) }
        fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
        fn terminate(&mut self) -> Result<(), BlockError> { self.inner.terminate() }
    }
}

/// Analog circuit simulation (SPICE-level) domain.
pub mod analog_circuit {
    use crate::core::block::{Block, BlockError, SimpleBlock};
    use crate::core::param::ParameterSet;
    use crate::core::port::{Port, PortDirection, PortSet};
    use crate::core::signal::Signal;
    use crate::core::types::{
        ComponentStatus, Scalar, SignalType, SignalValue, Time,
    };

    /// A simple linear resistor model.
    #[derive(Debug)]
    pub struct Resistor {
        inner: SimpleBlock,
        resistance: Scalar,
    }

    impl Resistor {
        pub fn new(id: &str, resistance: Scalar) -> Self {
            let mut inner = SimpleBlock::new(id, "Resistor");
            inner.add_port(Port::new("p", PortDirection::InOut, SignalType::Continuous));
            inner.add_port(Port::new("n", PortDirection::InOut, SignalType::Continuous));
            Self { inner, resistance }
        }
    }

    impl Block for Resistor {
        fn id(&self) -> &String { self.inner.id() }
        fn block_type(&self) -> &str { self.inner.block_type() }
        fn ports(&self) -> &PortSet { self.inner.ports() }
        fn ports_mut(&mut self) -> &mut PortSet { self.inner.ports_mut() }
        fn params(&self) -> &ParameterSet { self.inner.params() }
        fn params_mut(&mut self) -> &mut ParameterSet { self.inner.params_mut() }
        fn status(&self) -> ComponentStatus { self.inner.status() }
        fn set_status(&mut self, s: ComponentStatus) { self.inner.set_status(s); }
        fn set_time(&mut self, t: Time) { self.inner.set_time(t); }
        fn time(&self) -> Time { self.inner.time() }

        fn init(&mut self) -> Result<(), BlockError> { self.inner.init() }

        fn output(&mut self) -> Result<(), BlockError> {
            let v_p = self.ports().get("p")
                .and_then(|p| p.read()).and_then(|s| s.as_scalar())
                .unwrap_or(0.0);
            let v_n = self.ports().get("n")
                .and_then(|p| p.read()).and_then(|s| s.as_scalar())
                .unwrap_or(0.0);

            let current = if self.resistance > 0.0 {
                (v_p - v_n) / self.resistance
            } else {
                0.0
            };
            let t = self.time();

            if let Some(port) = self.inner.ports_mut().get_mut("p") {
                port.write(Signal::new(SignalType::Continuous, SignalValue::Scalar(current), t));
            }
            Ok(())
        }

        fn derivative(&self) -> Result<Vec<Scalar>, BlockError> { Ok(Vec::new()) }
        fn update(&mut self) -> Result<(), BlockError> { Ok(()) }
        fn zero_crossings(&self) -> Vec<Scalar> { Vec::new() }
        fn terminate(&mut self) -> Result<(), BlockError> { self.inner.terminate() }
    }
}
