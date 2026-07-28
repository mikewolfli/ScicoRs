//! Acoustics & Vibration Simulation (Phase 19).
//!
//! Provides sound field propagation (air/water/ultrasound), vibration and
//! resonance analysis, cavity acoustics, transducer models (speaker,
//! microphone, accelerometer), structure-acoustic coupling, and acoustic
//! analysis tools (SPL, RT60, FRF, damping).

pub mod physics;
pub mod wave_prop;
pub mod cavity;
pub mod transducer;
pub mod vibro_acoustic;
pub mod analysis;

pub use physics::{
    characteristic_impedance, speed_of_sound_air, speed_of_sound_water,
    P_REF_AIR, P_REF_WATER, SPEED_OF_SOUND_AIR, SPEED_OF_SOUND_STEEL,
    SPEED_OF_SOUND_WATER, Z0_AIR, Z0_WATER,
};
pub use wave_prop::{
    air_attenuation_coefficient, sound_intensity, sound_power, sound_pressure_level,
    spherical_spreading, spl_at_distance, SoundField,
};
pub use cavity::{
    critical_distance, helmholtz_resonance, rectangular_room_modes, rt60_sabine, Cavity,
};
pub use transducer::{
    Accelerometer, Loudspeaker, Microphone,
};
pub use vibro_acoustic::{
    critical_frequency, radiation_efficiency, sound_transmission_loss,
    transmission_loss_mass_law, vibration_transfer_function,
};
pub use analysis::{
    a_weighting, damping_ratio_from_peak, equivalent_sound_level,
    frequency_response_function, octave_band_center_frequencies,
};
