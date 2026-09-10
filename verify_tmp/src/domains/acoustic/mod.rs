//! Acoustics & Vibration Simulation (Phase 19).
//!
//! Provides sound field propagation (air/water/ultrasound), vibration and
//! resonance analysis, cavity acoustics, transducer models (speaker,
//! microphone, accelerometer), structure-acoustic coupling, and acoustic
//! analysis tools (SPL, RT60, FRF, damping).

pub mod analysis;
pub mod bem_acoustic;
pub mod cavity;
pub mod physics;
pub mod transducer;
pub mod vibro_acoustic;
pub mod wave_prop;
pub mod ultrasound;

pub use analysis::{
    a_weighting, damping_ratio_from_peak, equivalent_sound_level, frequency_response_function,
    octave_band_center_frequencies,
};
pub use bem_acoustic::AcousticBEM;
pub use cavity::{
    Cavity, critical_distance, helmholtz_resonance, rectangular_room_modes, rt60_sabine,
};
pub use physics::{
    P_REF_AIR, P_REF_WATER, SPEED_OF_SOUND_AIR, SPEED_OF_SOUND_STEEL, SPEED_OF_SOUND_WATER, Z0_AIR,
    Z0_WATER, characteristic_impedance, speed_of_sound_air, speed_of_sound_water,
};
pub use transducer::{Accelerometer, Loudspeaker, Microphone};
pub use vibro_acoustic::{
    critical_frequency, radiation_efficiency, sound_transmission_loss, transmission_loss_mass_law,
    vibration_transfer_function,
};
pub use wave_prop::{
    SoundField, air_attenuation_coefficient, sound_intensity, sound_power, sound_pressure_level,
    spherical_spreading, spl_at_distance,
};
