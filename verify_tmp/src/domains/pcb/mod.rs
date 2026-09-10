//! Board-Level Circuit & PCB System Simulation (Phase 20).
//!
//! Provides PCB transmission line models, power integrity (IR drop, ripple,
//! decoupling network), signal integrity (reflection, crosstalk, eye diagram),
//! board-level electro-thermal coupling, and package parasitics.

pub mod package;
pub mod power_integrity;
pub mod signal_integrity;
pub mod thermal;
pub mod transmission;

pub use package::{PackageParasitics, bga_ball_capacitance, bond_wire_inductance};
pub use power_integrity::{
    Decap, DecapNetwork, buck_ripple_voltage, ir_drop, pdn_impedance, target_impedance,
};
pub use signal_integrity::{
    EyeDiagram, crosstalk_peak, eye_diagram_analysis, insertion_loss, reflection_coefficient,
    return_loss, ringing_overshoot, tdr_waveform,
};
pub use thermal::{
    PcbThermalBlock, ThermalNetwork, hot_spot_temperature, junction_temperature,
    pcb_trace_temperature_rise,
};
pub use transmission::{
    TransmissionLine, cpw_z0, microstrip_z0, propagation_delay, s2p_to_t_params, stripline_z0,
};

pub mod via_model;
pub mod serdes_com;
