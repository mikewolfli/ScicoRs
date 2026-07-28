//! Power Electronics & Motor Drive Simulation (Phase 21).
//!
//! Provides power device models (diode, MOSFET, IGBT, thyristor), converter
//! topologies (Buck, Boost, inverter, chopper, rectifier), motor models (DC,
//! stepper, PMSM, induction), drive control (FOC, PI), and thermal analysis.

pub mod converters;
pub mod devices;
pub mod drive_ctrl;
pub mod motors;
pub mod thermal_power;

pub use converters::{
    BoostConverter, BuckConverter, Chopper, ChopperMode, FullBridgeInverter, buck_ripple_voltage,
    pwm_signal, single_phase_rectifier, three_phase_rectifier,
};
pub use devices::{Igbt, PowerDiode, PowerMosfet, Thyristor};
pub use drive_ctrl::{FocController, PiController, drive_efficiency, torque_speed_curve};
pub use motors::{DcMotor, InductionMotor, Pmsm, StepperMotor};
pub use thermal_power::{PowerLossBreakdown, device_junction_temp, heatsink_thermal_resistance};
