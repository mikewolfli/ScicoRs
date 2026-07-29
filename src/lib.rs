//! SCIcoRS — Unified Simulation Kernel for All Humanity
//!
//! A universal simulation kernel designed to unify all engineering and
//! scientific simulation scenarios across every discipline, scale, and field.
//!
//! Architecture (7 layers):
//!   core/      — Data model layer: Block, Port, Link, Diagram, Types
//!   runtime/   — Simulation runtime: Context, State, Engine, Solvers
//!   blocks/    — Standard block library: sources, math, logic, sinks
//!   domains/   — Domain-specific simulations: circuits, fluid, quantum, etc.
//!   coupling/  — Multi-physics coupling bus
//!   postproc/  — Post-processing & visualization
//!   bindings/  — Python bindings & plugin system

// Crate-level style lint allowances for domain module test code.
// These are style preferences, not correctness issues.
#![allow(
    clippy::needless_range_loop,
    clippy::doc_lazy_continuation,
    clippy::approx_constant,
    clippy::collapsible_if,
    clippy::let_and_return,
    clippy::manual_range_contains,
    clippy::single_match,
    clippy::unnecessary_unwrap,
    clippy::if_same_then_else,
    clippy::needless_borrowed_reference,
    clippy::new_without_default,
    clippy::ptr_arg,
    clippy::assertions_on_constants
)]

pub mod core;
pub mod runtime;

// Future module placeholders (declared for architecture completeness)
pub mod bindings;
pub mod blocks;
pub mod coupling;
pub mod db;
pub mod domains;
pub mod postproc;

// Re-export commonly used types at the crate root for convenience.
pub use core::block::{Block, BlockError, SimpleBlock};
pub use core::coord::{Coord1D, Coord2D, Coord3D, CoordSystem, Transform4x4};
pub use core::diagram::Diagram;
pub use core::error::{ErrorCode, SimError};
pub use core::link::Link;
pub use core::param::{Parameter, ParameterSet};
pub use core::port::Port;
pub use core::types::{PortDirection, Scalar, SignalType, SignalValue, Time};
pub use core::units::{Dimension, Quantity, Unit};
pub use runtime::algebraic::{
    AlgebraicLoop, AlgebraicLoopDetector, AlgebraicSolveResult, AlgebraicSolverConfig,
    FixedPointIteration, LoopAnalysis, NumericalGuard, RelaxationIteration,
};
pub use runtime::context::{LogLevel, SimContext, SimLifecycle, SimRunMode, TimeConfig};
pub use runtime::discrete::{
    Counter, CounterDirection, DiscreteIntegrator, EdgeDetector, FIRFilter, IIRFilter,
    MovingAverage, RSFlipFlop, SampleHold, Timer,
};
pub use runtime::engine::{SimEngine, SimStepResult, SimSummary};
pub use runtime::event::{Event, EventQueue, EventTriggerManager, EventType, ZeroCrossingDetector};
pub use runtime::state::{ContinuousState, DiscreteState, SimStateManager, StateSnapshot};
pub use runtime::workflow::{WorkflowDAG, WorkflowEdge, WorkflowEngine, WorkflowTask};

pub use db::{
    DbConfig, DbError, LibraryCategory, LibraryDb, LibraryEntry, LibraryManager, TomlLoader,
    load_sample_entries,
};

// Re-export molbio key types
pub use domains::molbio::analysis::{
    compute_dihedral_angle, compute_rmsd, detect_hydrogen_bonds, radius_of_gyration,
};
pub use domains::molbio::{
    EnergyMinimizer, ForceField, HarmonicBond, Integrator, LennardJones, MolecularDynamics,
    Molecule, SimParams, Vec3,
};

// Re-export cellbio key types
pub use domains::cellbio::analysis::{doubling_time, monod_growth_rate, specific_growth_rate};
pub use domains::cellbio::{
    Bioreactor, BioreactorMode, Cell, CellPopulation, CellState, CultureMedia, GridModel,
    MediumComponent,
};

// Re-export tcad (Phase 13) key types
pub use domains::tcad::{
    BjtBlock, BjtModel, MobilityModel, MosfetBlock, MosfetModel, OxidationAmbient,
    bjt_base_current, bjt_collector_current, built_in_potential, depletion_width,
    diffusion_profile, drift_diffusion_current, implant_range, mosfet_cv_curve,
    mosfet_drain_current, mosfet_gds, mosfet_gm, mosfet_iv_curve, mosfet_transfer_curve,
    oxide_thickness, thermal_voltage,
};

// Re-export analog (Phase 14) key types
pub use domains::analog::{
    AcResult, AcSweepConfig, AnalysisType, BjtStamp, CapacitorBlock, CapacitorStamp,
    CurrentSourceStamp, DcOpResult, DcSweepConfig, DiodeBlock, DiodeStamp, FreqScale,
    InductorBlock, InductorStamp, MnaMatrix, MnaSolution, MosfetStamp, NoiseConfig, OpAmpStamp,
    ResistorBlock, ResistorStamp, TransientConfig, TransientResult, VoltageSourceStamp,
    flicker_noise_psd, run_ac_sweep, run_dc_op, run_dc_sweep, run_transient, shot_noise_psd,
    solve_mna, thermal_noise_psd,
};

// Re-export digital (Phase 15) key types
pub use domains::digital::{
    ALUBlock, ALUOp, AdderBlock, CpuInstruction, CpuProgram, DFlipFlopBlock, DecoderBlock,
    GateConnection, JKFlipFlopBlock, LatchBlock, LogicBuffer, LogicNand, LogicNor, LogicNotBlock,
    LogicXnor, MultiplierBlock, PipelineStages, ShiftRegisterBlock, SimpleCpu, TFlipFlopBlock,
    TimingAnalyzer, TriStateBuffer,
};

// Re-export optical (Phase 18) key types
pub use domains::optical::{
    AberrationEstimator, Aperture, CircularPolarization, ConstantRefractiveIndex, Fiber,
    FlatInterface, FlatMirror, GaussianBeam, Grating, ImagingSystem, JonesMatrix, LaserSource,
    MuellerMatrix, OpticalElement, PhotodetectorBlock, PolarizationState, Ray, RefractiveIndex,
    SellmeierModel, SpectralBand, SphericalMirror, ThinLens, TracePoint, Wavefront, Waveguide,
    bk7_glass, brewster_angle, circular_aperture_diffraction, degree_of_polarisation,
    double_slit_intensity, freq_to_wavelength, fresnel_reflection, fresnel_transmission,
    fused_silica, grating_diffraction, jones_vector, malus_law, michelson_intensity,
    modulation_transfer_function, optical_efficiency, photocurrent, photon_energy,
    quantum_efficiency, rayleigh_criterion, silicon_n, single_slit_diffraction, solar_cell_iv,
    stokes_from_jones, system_transmittance, thin_film_interference, wavelength_to_freq,
};

// Re-export acoustic (Phase 19) key types
pub use domains::acoustic::{
    Accelerometer, AcousticBEM, Cavity, Loudspeaker, Microphone, SoundField, a_weighting,
    air_attenuation_coefficient, characteristic_impedance, critical_distance, critical_frequency,
    damping_ratio_from_peak, equivalent_sound_level, frequency_response_function,
    helmholtz_resonance, octave_band_center_frequencies, radiation_efficiency,
    rectangular_room_modes, rt60_sabine, sound_intensity, sound_power, sound_pressure_level,
    sound_transmission_loss, speed_of_sound_air, speed_of_sound_water, spherical_spreading,
    spl_at_distance, transmission_loss_mass_law, vibration_transfer_function,
};

// Re-export pcb (Phase 20) key types
pub use domains::pcb::{
    Decap, DecapNetwork, EyeDiagram, PackageParasitics, PcbThermalBlock, ThermalNetwork,
    TransmissionLine, bga_ball_capacitance, bond_wire_inductance, buck_ripple_voltage, cpw_z0,
    crosstalk_peak, eye_diagram_analysis, hot_spot_temperature, insertion_loss, ir_drop,
    junction_temperature, microstrip_z0, pcb_trace_temperature_rise, pdn_impedance,
    propagation_delay, reflection_coefficient, return_loss, ringing_overshoot, s2p_to_t_params,
    stripline_z0, target_impedance, tdr_waveform,
};

// Re-export powerelec (Phase 21) key types
pub use domains::powerelec::{
    BoostConverter, BuckConverter, Chopper, ChopperMode, DcMotor, FocController,
    FullBridgeInverter, Igbt, InductionMotor, PiController, Pmsm, PowerDiode, PowerLossBreakdown,
    PowerMosfet, StepperMotor, Thyristor, device_junction_temp, drive_efficiency, pwm_signal,
    single_phase_rectifier, three_phase_rectifier, torque_speed_curve,
};

// Re-export emag (Phase 22) key types
pub use domains::emag::{
    Antenna, Antenna3D, BoundaryType3D, CavityShape, DipoleAntenna, DispersionModel,
    ElectrostaticSolver1D, Fdtd1D, Fdtd3D, FieldComponent, MagnetShape, PermanentMagnet,
    ResonantCavity, RfAmplifier, Source3D, Transformer, Waveform, antenna_gain_dbi, cascade_s2p,
    coil_inductance, debye_water, drude_gold, drude_silver, eddy_current_loss, gamma_to_z,
    hysteresis_loss, mutual_inductance, parallel_plate_capacitance, point_charge_field,
    radar_range_eq, rcs_3d, rcs_monostatic, shielding_effectiveness, skin_depth,
    smith_chart_impedance, solenoid_field, transmission_line_resonator, wave_impedance,
    wave_number, wavelength, wire_magnetic_field,
};

// Re-export bio_medical (Phase 23) key types
pub use domains::bio_medical::{
    CompartmentModel, HodgkinHuxley, NeuronModel, PkPdParams, TissueDiffusion2D, TissueMaterial,
    TissueMechanics, TumorModel, VesselSegment, WindkesselModel, artery_wall, articular_cartilage,
    body_surface_area, cardiac_output, cortical_bone, egfr_ckd_epi, emax_model, perfusion_pressure,
    pulse_wave_velocity, skeletal_muscle, trabecular_bone,
};

// Re-export chemical (Phase 24) key types
pub use domains::chemical::{
    BatchReactor, Cstr, Pfr, ProcessFlowsheet, ProcessUnit, ReactionKinetics, absorption_factor,
    adiabatic_flame_temperature, arrhenius_rate, auto_catalytic_conversion, conversion,
    distribution_coefficient, equilibrium_constant, explosive_limits, fenske_equation,
    half_life_first_order, heat_exchanger_ntu, laminar_flame_speed, minimum_reflux_ratio,
    rachford_rice, reaction_enthalpy, reaction_rate, reversible_rate, selectivity, yield_ratio,
};

// Re-export structural (Phase 25) key types
pub use domains::structural::{
    BeamElement, ExplicitDynamics, FemElement, FemSystem, MaterialProperties, NonlinearFem,
    SdofSystem, ShellElement, SolidElement, SpringElement, TrussElement, aluminum_6061,
    beam_deflection_simple, bolt_preload, concrete_30mpa, coulomb_friction, euler_buckling_load,
    hertz_contact_stress, hookes_law_1d, hookes_law_3d, miner_damage, point_to_point_distance,
    safety_factor, sn_curve, steel_structural, titanium_ti6al4v, von_mises_stress,
};

// Re-export thermal (Phase 26) key types
pub use domains::thermal::{
    BoundaryCondition, BoundaryCondition3D, ConjugateHeatTransfer, DomQuadrature, DomRadiation3D,
    HeatConduction1D, HeatConduction2D, HeatConduction3D, PhaseChange1D, ThermalResistance,
    convection_coefficient, convective_heat_transfer, cooling_cop, evaporation_rate,
    forced_convection_nu_laminar, forced_convection_nu_turbulent, fourier_law_1d, friction_heating,
    grashof_number, heat_pipe_effective_k, heatsink_thermal_resistance, joule_heating,
    natural_convection_nu, nucleate_boiling_h, radiation_exchange, stefan_boltzmann,
    temperature_gradient, thermal_strain, view_factor_parallel_disks,
    view_factor_perpendicular_rectangles,
};

// Re-export fluid (Phase 27) key types
pub use domains::fluid::{
    CompressibleNS2D, FlowRegime, KEpsilon, NavierStokes2D, NavierStokes3D, Smagorinsky,
    TurbulenceModel, VofSolver2D, WallCondition, WallCondition3D, bubble_terminal_velocity,
    darcy_friction_factor, drag_coefficient, drag_force, dynamic_pressure, flow_regime,
    homogeneous_density, hydraulic_diameter, lift_coefficient, lift_force, mach_number,
    manning_flow, mass_flow, mixing_length_turbulent_viscosity, orifice_flow, pipe_pressure_drop,
    pressure_coefficient, reynolds_number, turbulent_boundary_layer_thickness, volumetric_flow,
    water_hammer_pressure, weir_flow,
};

// Re-export multibody (Phase 28) key types
pub use domains::multibody::{
    Aabb, CollisionResult, CollisionShape, Constraint, ConstraintJacobian, ConstraintSolver,
    ConstraintType, ExternalForce, MultibodySystem, Quaternion, RigidBody, RigidBodyProperties,
    center_of_mass, collision_impulse, contact_force_spring_damper, friction_force, linkage_ratio,
    sphere_sphere_collision, total_angular_momentum, total_momentum, trajectory_length,
};

// Re-export aerospace (Phase 29) key types
pub use domains::aerospace::{
    AircraftAerodynamics, Autopilot, HighAltitudeAtmosphere, IsaAtmosphere, SixDofAircraft,
    ThermalProtectionSystem, TpsLayer, aerodynamic_heating, airfoil_cd, airfoil_cm,
    ambient_temperature, breguet_range, characteristic_velocity, euler_to_quaternion,
    gravity_at_altitude, isentropic_flow, lift_to_drag_ratio, load_factor,
    normal_shock_pressure_ratio, nozzle_area_ratio, oblique_shock_angle, prandtl_meyer_angle,
    quaternion_to_euler, rate_of_climb, rocket_thrust, shock_response_sweep, specific_impulse,
    thin_airfoil_cl, thrust_specific_fuel_consumption, turbojet_thrust, wing_loading,
};

// Re-export quantum (Phase 30) key types
pub use domains::quantum::{
    GateOperation, GateType, LogicalQubit, MatrixProductState, MultiQubitGate, NoiseChannel,
    NoiseDensityMatrix, PAULI_X, PAULI_Y, PAULI_Z, QuantumCircuit, QuantumCode, SingleQubitGate,
    VqeOptimizer, VqeSolver, cnot_matrix, hadamard_matrix, pauli_x_matrix, pauli_y_matrix,
    pauli_z_matrix, pure_state_density, rotation_x, rotation_y, rotation_z, swap_matrix,
    toffoli_matrix,
};

// Re-export postproc (Phase 33) key types
pub use postproc::{
    BatchSimManager, BatchTask, BatchTaskStatus, ChartGenerator, ChartType, ContourGenerator,
    CurveData, DataRecorder, DataReplayer, DesignParam, ExportFormat, FieldRecorder3D,
    FieldSnapshot3D, HilConfig, HilIoChannels, HilRunner, IsoSurface3D, OfflineAnalysis,
    OptimizationLoop, ParameterSweep, RecorderConfig, ReportSection, ReportTable, SimulationReport,
    SolverBenchConfig, SolverBenchmarkResult, VectorFieldVisualization, VolumeSlice3D,
    bench_solver, benchmark_report, benchmark_speedup, run_benchmark_suite, vector_field_slice,
};
