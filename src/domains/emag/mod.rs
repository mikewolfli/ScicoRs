//! Electromagnetic Field & RF/Microwave Simulation (Phase 22).
//!
//! Provides Maxwell equation-based models: electrostatic/magnetostatic fields,
//! transient EM (FDTD 1D), EM devices (coils, transformers, antennas, magnets),
//! RF/microwave circuits (S-parameters, resonant cavities, amplifiers), and
//! EM analysis (eddy current, hysteresis, shielding, radar equation).

pub mod analysis;
pub mod devices;
pub mod physics;
pub mod rf_microwave;
pub mod static_fields;
pub mod transient_em;

pub use analysis::{
    antenna_gain_dbi, eddy_current_loss, hysteresis_loss, joule_heating, radar_range_eq,
    radiation_efficiency, shielding_effectiveness,
};
pub use devices::{
    Antenna, DipoleAntenna, MagnetShape, PermanentMagnet, Transformer, coil_inductance,
    mutual_inductance,
};
pub use physics::{C, EPSILON_0, MU_0, Z0, skin_depth, wave_impedance, wave_number, wavelength};
pub use rf_microwave::{
    CavityShape, ResonantCavity, RfAmplifier, cascade_s2p, gamma_to_z, smith_chart_impedance,
    transmission_line_resonator,
};
pub use static_fields::{
    ElectrostaticSolver1D, parallel_plate_capacitance, point_charge_field, solenoid_field,
    wire_magnetic_field,
};
pub use transient_em::{BoundaryType, Fdtd1D, Phasor, PlaneWave, fdtd_energy};
