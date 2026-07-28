//! Optics & Photonics Simulation (Phase 18).
//!
//! Provides geometric optics (ray tracing, ABCD matrix), wave optics
//! (interference, diffraction, polarization, Gaussian beams), laser/fiber
//! waveguide models, photoelectric conversion, and optical system analysis.

pub mod analysis;
pub mod laser;
pub mod photoelectric;
pub mod physics;
pub mod ray;
pub mod wave;

pub use analysis::{
    AberrationEstimator, modulation_transfer_function, optical_efficiency, rayleigh_criterion,
    system_transmittance,
};
pub use laser::{Fiber, Grating, LaserSource, Waveguide};
pub use photoelectric::{PhotodetectorBlock, photocurrent, quantum_efficiency, solar_cell_iv};
pub use physics::{
    C, ConstantRefractiveIndex, EPSILON_0, H_PLANCK, MU_0, RefractiveIndex, SellmeierModel,
    SpectralBand, bk7_glass, freq_to_wavelength, fused_silica, photon_energy, silicon_n,
    wavelength_to_freq,
};
pub use ray::{
    Aperture, FlatInterface, FlatMirror, ImagingSystem, OpticalElement, Ray, SphericalMirror,
    ThinLens, TracePoint,
};
pub use wave::{
    CircularPolarization, GaussianBeam, PolarizationState, Wavefront, brewster_angle,
    circular_aperture_diffraction, double_slit_intensity, fresnel_reflection, fresnel_transmission,
    grating_diffraction, malus_law, michelson_intensity, single_slit_diffraction,
    thin_film_interference,
};
