
# SCIcoRS Development TODO

Each roadmap item is listed with its core, basic, and advanced features. Completion: 0% for all items. All features are marked as incomplete.

---

## Phase 1: Core Model Kernel (Block / Port / Link / Diagram System) [0%]
- [ ] Core Features:
    - [ ] Define Block (functional module), Port (input/output), Link (signal connection), Diagram (topology)
    - [ ] Support scalar, vector, matrix, complex, tensor, boolean, string types
- [ ] Basic Features:
    - [ ] Parameter system: static config, runtime variable, expression-bound parameter
    - [ ] Signal types: continuous, discrete, event, bus
    - [ ] Module encapsulation: input/output declaration, state declaration, dependency declaration, reusable component definition
- [ ] Advanced Features:
    - [ ] Diagram load, save, parse, and validation mechanisms

## Phase 2: Simulation Context and Time System [0%]
- [ ] Core Features:
    - [ ] Simulation time management: current time t, step size dt, end time t_end, minimum precision limit
    - [ ] Unified management of continuous state x, discrete state z, state derivative dx/dt
- [ ] Basic Features:
    - [ ] Simulation modes: normal, real-time, single-step, pause, breakpoint
    - [ ] Simulation lifecycle: Init, Start, Run, Pause, Stop, Reset
- [ ] Advanced Features:
    - [ ] Global context sharing: shared data, logs, errors, runtime status

## Phase 3: General Numerical Solver System [0%]
- [ ] Core Features:
    - [ ] Fixed-step solvers: Euler, Runge-Kutta 4th order (RK4), Heun, Midpoint
    - [ ] Variable-step adaptive solvers: RK45, RK23, Cash-Karp
- [ ] Basic Features:
    - [ ] Stiff system solvers: BDF, Trapezoidal, Backward Euler
    - [ ] DAE solvers
- [ ] Advanced Features:
    - [ ] Nonlinear equation solver: Newton-Raphson iteration
    - [ ] Sparse matrix solver, eigenvalue solver, linear system solver

## Phase 4: Scheduling and Execution Engine [0%]
- [ ] Core Features:
    - [ ] Diagram topological sorting, signal flow direction analysis
    - [ ] Mixed scheduling for continuous, discrete, event, and multi-rate systems
- [ ] Basic Features:
    - [ ] Port signal update, propagation, caching, synchronization
    - [ ] Standard module execution flow: Init → Output → Deriv → Update → Event → Terminate
- [ ] Advanced Features:
    - [ ] Multi-rate task scheduling, clock domain isolation, data synchronization

## Phase 5: Event and Trigger System [0%]
- [ ] Core Features:
    - [ ] Time event queue: sorted by time, precise triggering
    - [ ] Zero-crossing detection, rising/falling edge detection for continuous signals
- [ ] Basic Features:
    - [ ] External, interrupt, and conditional triggers
    - [ ] Event-driven solver restart, state refresh
- [ ] Advanced Features:
    - [ ] Event log, statistics, and replay

## Phase 6: Discrete and Multi-Rate Systems [0%]
- [ ] Core Features:
    - [ ] Multi-rate sampling, sample-and-hold, resampling
    - [ ] Digital filters, discrete integration, discrete state holding
- [ ] Basic Features:
    - [ ] Synchronous/asynchronous clocks, phase offset
    - [ ] Embedded logic, PLC logic, counters, timers
- [ ] Advanced Features:
    - [ ] Digital logic timing, race and hazard handling

## Phase 7: Algebraic Loops and Numerical Stability [0%]
- [ ] Core Features:
    - [ ] Automatic detection, marking, and classification of algebraic loops
    - [ ] Direct feedthrough path identification and handling
- [ ] Basic Features:
    - [ ] Fixed-point iteration, relaxation iteration
    - [ ] Numerical stability control, overflow protection, NaN/Inf capture
- [ ] Advanced Features:
    - [ ] Solver convergence threshold, max iteration control

## Phase 8: Math, Signal, and Control Foundation Library [0%]
- [ ] Core Features:
    - [ ] Signal sources: constant, sine, square, step, pulse, Gaussian white noise, pseudo-random sequence
    - [ ] Math operations: add, subtract, multiply, divide, trig functions, exp/log, matrix operations, inverse, dot/cross product
- [ ] Basic Features:
    - [ ] Logic operations: and, or, not, xor, comparison, multiplexer
    - [ ] Continuous systems: integrator, differentiator, transfer function, state-space, PID controller
- [ ] Advanced Features:
    - [ ] Discrete systems: unit delay, discrete integration, discrete filter, discrete PID
    - [ ] Observation modules: oscilloscope, data recorder, numeric display, chart output

## Phase 9: Unified Coordinate System (All-Scale Universal) [0%]
- [ ] Core Features:
    - [ ] Coordinate dimensions: 0D, 1D, 2D, 3D, high-dimensional abstract space
    - [ ] Coordinate systems: Cartesian, polar, cylindrical, spherical, natural, finite element isoparametric
- [ ] Basic Features:
    - [ ] Reference frames: inertial, non-inertial, local part, node
    - [ ] Coordinate transformations: translation, rotation, scaling, mirroring, homogeneous, system conversion
- [ ] Advanced Features:
    - [ ] Field coordinate binding: scalar, vector, tensor, matrix fields
    - [ ] Unified compatibility for chip/PCB/device, cell/tissue, celestial orbit coordinates
    - [ ] Decoupled interfaces: simulation ↔ 3D rendering ↔ CAD coordinates

## Phase 10: Unified Dimension and Unit System [0%]
- [ ] Core Features:
    - [ ] Seven SI base dimensions: length, mass, time, current, temperature, amount of substance, luminous intensity
    - [ ] Automatic derivation of composite dimensions: velocity, acceleration, force, energy, power, voltage, current, field
- [ ] Basic Features:
    - [ ] All-discipline unit support: nm, μm, mm, m, km, AU, light-year, s, ms, μs
    - [ ] Electrical dimensions: V, A, Ω, F, H, W, J
    - [ ] Chemical/biological: mol, concentration, pH, reaction rate
    - [ ] Biological: cell density, protein concentration, growth rate
- [ ] Advanced Features:
    - [ ] Core normalization [-1,1] to avoid numerical precision explosion
    - [ ] Port dimension check, auto-conversion, unit consistency
    - [ ] Bridges to OCCT geometry, CAD, CAE units

## Phase 11: Unified Database System (TOML + SQLite Index, Complete Edition) [0%]
- [ ] Core Features:
    - [ ] TOML data storage for real data
    - [ ] SQLite index.db for indexing
- [ ] Basic Features:
    - [ ] Material library: structural, thermal, electromagnetic, fluid, semiconductor, biological
    - [ ] Celestial library: planets, satellites, stars, orbits, gravity, mass, radius
    - [ ] Fluid media library: air, water, oil, plasma, multiphase parameters
    - [ ] Section properties library: I-beam, channel, beam, square/round tube
    - [ ] Electrical components library: R, C, L, diode, MOSFET, BJT, op-amp, power supply
    - [ ] Logic gates library: AND, OR, NOT, NAND, NOR, XOR, flip-flop
    - [ ] Chip devices library: 74 series, CPU models, interface chips, memory chips
    - [ ] Board-level components library: PCB package, connector, crystal, sensor, power module
    - [ ] Optical components library: lens, mirror, fiber, laser, grating
    - [ ] Acoustic components library: speaker, microphone, cavity, acoustic impedance
    - [ ] Chemicals library: reactants, solvents, catalysts, thermodynamic parameters
    - [ ] Biomolecules library: protein, DNA, RNA, enzyme, ligand, small molecule
    - [ ] Cells library: cell lines, colonies, tissues, tumor cells
    - [ ] Culture media library: nutrient solution, pH, temperature, dissolved oxygen, growth factors
    - [ ] Semiconductor process library: doping, implantation, etching, deposition, oxidation
- [ ] Advanced Features:
    - [ ] Public/private libraries, built-in/custom data
    - [ ] Search, classification, import/export, version management

## Phase 12: Semiconductor Device Physics Simulation (TCAD, Chip Bottom Layer) [0%]
- [ ] Core Features:
    - [ ] Transistor physics: MOSFET, BJT, FinFET, channel carrier transport
    - [ ] Drift-diffusion model, carrier concentration, mobility, saturation velocity
- [ ] Basic Features:
    - [ ] Electric field, potential, current density, band structure
    - [ ] Breakdown, ESD, latchup, hot carrier effect
    - [ ] Semiconductor process simulation: diffusion, ion implantation, oxidation, etching, deposition
- [ ] Advanced Features:
    - [ ] Device electrical characteristics: IV, CV curves, threshold voltage
    - [ ] Full unification with database, dimension, and coordinate systems

## Phase 13: Analog Circuit Simulation (SPICE Level, Transistor Level) [0%]
- [ ] Core Features:
    - [ ] Passive devices: ideal/non-ideal R, C, L, mutual inductance, transformer
    - [ ] Active devices: diode, BJT, MOSFET, op-amp, comparator
- [ ] Basic Features:
    - [ ] Simulation analysis: DC operating point, AC small signal, transient, noise
    - [ ] Improved node analysis (MNA) equation construction and solving
    - [ ] Parasitic resistance, capacitance, inductance modeling
- [ ] Advanced Features:
    - [ ] Circuit stability, gain, bandwidth, phase margin analysis
    - [ ] Integration with electrical and chip device libraries

## Phase 14: Digital Logic and RTL Simulation (Gate/CPU Core Level) [0%]
- [ ] Core Features:
    - [ ] Basic logic gates: AND, OR, NOT, NAND, NOR, XOR, XNOR
    - [ ] Sequential units: D flip-flop, JK flip-flop, latch, shift register
- [ ] Basic Features:
    - [ ] Combinational logic: adder, subtractor, multiplier, multiplexer, decoder
    - [ ] CPU microarchitecture: ALU, register file, bus, control unit, pipeline
    - [ ] Instruction set simulation: fetch, decode, execute, memory, write-back
- [ ] Advanced Features:
    - [ ] Timing analysis: delay, setup/hold time, clock jitter
    - [ ] Integration with logic gate and chip device libraries

## Phase 15: Molecular Dynamics and Biomolecular Simulation [0%]
- [ ] Core Features:
    - [ ] Protein folding, conformational change, molecular docking
    - [ ] DNA/RNA structure, base pairing, transcription, translation
- [ ] Basic Features:
    - [ ] Molecular force fields, bond length/angle, dihedral, van der Waals
    - [ ] Small molecule-protein binding, enzyme catalysis
    - [ ] Molecular trajectory, energy minimization, dynamics simulation
- [ ] Advanced Features:
    - [ ] Temperature, pressure, solvent effects
    - [ ] Integration with biomolecule and chemical libraries

## Phase 16: Cell Culture and Tissue Growth Simulation [0%]
- [ ] Core Features:
    - [ ] Cell proliferation, division, apoptosis, migration, adhesion
    - [ ] Nutrient diffusion/consumption, concentration fields in media
- [ ] Basic Features:
    - [ ] Effects of pH, oxygen, temperature, osmotic pressure on growth
    - [ ] Cell colonies, tissue layer growth morphology
    - [ ] Bioreactor cell culture dynamics
- [ ] Advanced Features:
    - [ ] Cell density, activity, metabolic rate modeling
    - [ ] Integration with cell, media, and biological material libraries

## Phase 17: Optics and Photonics Simulation [0%]
- [ ] Core Features:
    - [ ] Geometric optics: reflection, refraction, transmission, focusing, imaging
    - [ ] Wave optics: interference, diffraction, polarization, Gaussian beam
- [ ] Basic Features:
    - [ ] Laser, fiber transmission, waveguide, grating, lens systems
    - [ ] IR, visible, UV spectral response
    - [ ] Optical system aberration, transmittance, efficiency analysis
- [ ] Advanced Features:
    - [ ] Photoelectric conversion, optoelectronic coupling simulation
    - [ ] Integration with optical and material libraries

## Phase 18: Acoustics and Vibration Simulation [0%]
- [ ] Core Features:
    - [ ] Sound field propagation: air, water, ultrasound
    - [ ] Vibration, resonance, noise, damping, frequency response
- [ ] Basic Features:
    - [ ] Cavity acoustics, speaker, microphone, sensor modeling
    - [ ] Acoustic impedance, pressure, intensity, power
    - [ ] Structure-acoustic coupling, vibration noise transmission
- [ ] Advanced Features:
    - [ ] Integration with acoustic and structural libraries

## Phase 19: Board-Level Circuit and PCB System Simulation [0%]
- [ ] Core Features:
    - [ ] PCB routing, impedance control, transmission line effects
    - [ ] Power integrity (PI): voltage drop, ripple, decoupling capacitors
- [ ] Basic Features:
    - [ ] Signal integrity (SI): reflection, crosstalk, ringing, S-parameters
    - [ ] Board-level electro-thermal coupling, chip power → temperature distribution
    - [ ] Packaging, BGA, wire bonding, parasitic parameters
- [ ] Advanced Features:
    - [ ] Integration with board-level and electrical component libraries

## Phase 20: Power Electronics and Motor Drive Simulation [0%]
- [ ] Core Features:
    - [ ] Power devices: diode, MOSFET, IGBT, thyristor
    - [ ] Rectification, inversion, PWM, chopper, DC-DC, AC-DC
- [ ] Basic Features:
    - [ ] Motor models: DC, stepper, servo, asynchronous, PMSM
    - [ ] Drive control, closed-loop speed, torque control, efficiency analysis
    - [ ] Power flow, energy conversion, loss, heating
- [ ] Advanced Features:
    - [ ] Integration with electrical and material libraries

## Phase 21: Electromagnetic Field and RF/Microwave Simulation [0%]
- [ ] Core Features:
    - [ ] Maxwell equations full-frequency solution
    - [ ] Electrostatic, magnetostatic, quasi-static, transient, harmonic fields
- [ ] Basic Features:
    - [ ] Coil, transformer, permanent magnet, antenna, radar
    - [ ] Eddy current, hysteresis, Joule heating, radiation efficiency
    - [ ] RF circuits, microwave networks, resonant cavity, transmission line
- [ ] Advanced Features:
    - [ ] Integration with material and electrical component libraries

## Phase 22: Physiological Systems and Biomedical Simulation [0%]
- [ ] Core Features:
    - [ ] Tissue mechanics: muscle, bone, cartilage, blood vessel
    - [ ] Hemodynamics, cardiac electrophysiology, pulse propagation
- [ ] Basic Features:
    - [ ] Pharmacokinetics/dynamics (PK/PD), absorption, distribution, excretion
    - [ ] Neural networks, action potential, signal transmission
    - [ ] Tumor growth, invasion, diffusion, treatment response
- [ ] Advanced Features:
    - [ ] Integration with cell, biomolecule, and material libraries

## Phase 23: Chemical Reaction and Process Simulation [0%]
- [ ] Core Features:
    - [ ] Reaction kinetics, rate equations, equilibrium constants
    - [ ] Combustion, explosion, polymerization, oxidation, reduction
- [ ] Basic Features:
    - [ ] Mixing, separation, distillation, absorption, extraction
    - [ ] Reactor, heat exchanger, column, pipeline processes
    - [ ] Effects of temperature, pressure, concentration
- [ ] Advanced Features:
    - [ ] Integration with chemical and material libraries

## Phase 24: Structural Mechanics and FEA Simulation [0%]
- [ ] Core Features:
    - [ ] Stress, strain, Young's modulus, Poisson's ratio, shear modulus
    - [ ] Beam, shell, solid, spring elements
- [ ] Basic Features:
    - [ ] Static, modal, buckling, vibration, fatigue analysis
    - [ ] Contact, friction, connection, fastening, constraints
    - [ ] Deformation, failure, strength, life prediction
- [ ] Advanced Features:
    - [ ] Integration with material and section libraries

## Phase 25: Thermodynamics and Heat Transfer Simulation [0%]
- [ ] Core Features:
    - [ ] Conduction, convection, radiation
    - [ ] Heat capacity, resistance, flux, temperature field
- [ ] Basic Features:
    - [ ] Phase change, latent heat, solidification, melting, evaporation
    - [ ] Cooling systems, heat pipes, radiators, cooling
    - [ ] Multi-physics thermal coupling: electro-thermal, mech-thermal, fluid-thermal
- [ ] Advanced Features:
    - [ ] Integration with material and fluid libraries

## Phase 26: Fluid Dynamics (CFD) Simulation [0%]
- [ ] Core Features:
    - [ ] Continuity, momentum, energy equations
    - [ ] Laminar, turbulent, multiphase, compressible/incompressible
- [ ] Basic Features:
    - [ ] Aerodynamics, hydraulics, pipe flow, boundary layer
    - [ ] Lift, drag, pressure, velocity, flow rate
    - [ ] Plasma, high-temp/high-speed fluids
- [ ] Advanced Features:
    - [ ] Integration with fluid media and material libraries

## Phase 27: Multibody Dynamics and Mechanical System Simulation [0%]
- [ ] Core Features:
    - [ ] Rigid/flexible bodies, mass, inertia, pose, coordinates
    - [ ] Motion constraints: revolute, prismatic, gear, linkage, belt
- [ ] Basic Features:
    - [ ] Collision detection, contact force, friction, rebound
    - [ ] Mechanical system motion, trajectory, velocity, acceleration
    - [ ] 3D geometry and OCCT kernel integration
- [ ] Advanced Features:
    - [ ] Integration with material, section, and structural libraries

## Phase 28: Aerospace and Aerodynamic Simulation [0%]
- [ ] Core Features:
    - [ ] Aircraft aerodynamic layout, lift-drag ratio, shock, boundary layer separation
    - [ ] Rocket engine, propulsion, fuel combustion
- [ ] Basic Features:
    - [ ] Flight control, attitude control, orbital insertion
    - [ ] High-altitude, low-temp, high-speed environments
    - [ ] Thermal protection, structural load, vibration/shock

## Phase 29: Quantum Physics and Quantum Computing Simulation [0%]
- [ ] Core Features:
    - [ ] Quantum states, state vectors, density matrix, qubits
    - [ ] Quantum gates: H, X, Y, Z, CNOT, CZ, SWAP, Toffoli
- [ ] Basic Features:
    - [ ] Schrödinger equation, Lindblad master equation, open systems
    - [ ] Quantum measurement, collapse, entanglement, superposition
    - [ ] Quantum algorithms: VQE, QAOA, Grover, HHL
- [ ] Advanced Features:
    - [ ] Quantum-classical hybrid, quantum-electromagnetic coupling

## Phase 30: Astrophysics and Celestial Orbit Simulation [0%]
- [ ] Core Features:
    - [ ] Planets, satellites, stars, black holes, galaxies
    - [ ] Gravity, orbital mechanics, two/multi-body problems
- [ ] Basic Features:
    - [ ] Revolution, rotation, precession, nutation, orbital perturbation
    - [ ] Galaxy evolution, cosmic expansion, gravitational lensing
    - [ ] Space orbit design, transfer, docking, escape velocity
- [ ] Advanced Features:
    - [ ] Integration with celestial and material libraries

## Phase 31: Unified Multi-Physics Coupling Bus [0%]
- [ ] Core Features:
    - [ ] Multi-physics interface standards, data exchange protocols
    - [ ] Field mapping, interpolation, projection, data transfer
- [ ] Basic Features:
    - [ ] Cross-scale coupling: nano → micro → meter → universe
    - [ ] Automatic dimension matching, unit conversion
    - [ ] Unified convergence control, solver scheduling, time synchronization
- [ ] Advanced Features:
    - [ ] Free combination and arbitrary coupling of all simulation modules

## Phase 32: Advanced Industrial Features, Post-Processing, and Visualization [0%]
- [ ] Core Features:
    - [ ] Data recording, playback, offline analysis, batch processing
    - [ ] Charts, curves, contour maps, vector plots, isosurfaces
- [ ] Basic Features:
    - [ ] Report generation, data export, reporting, screenshots
    - [ ] Exception capture, warnings, error handling, stability assurance
    - [ ] Real-time simulation, hardware-in-the-loop (HIL) support
- [ ] Advanced Features:
    - [ ] Batch tasks, automated simulation, optimization iteration

## Phase 33: Cross-Platform, Scripting Ecosystem, and Extension System [0%]
- [ ] Core Features:
    - [ ] Full Python scripting interface, command simulation, batch modeling
    - [ ] Plugin system, third-party modules, custom modules
- [ ] Basic Features:
    - [ ] CAD/CAE/PLM data interfaces: STEP, STL, mesh
    - [ ] Cross-OS: Windows/Linux/macOS
    - [ ] Cloud deployment, distributed computing, parallel acceleration

---
