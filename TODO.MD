# ScicoRs Simulation Kernel TODO List

## Phase 1: Basic Model Layer (Block Diagram Data Structure)
**Status: Pending**

- [ ] 1.1 Define Block structure with input/output ports, states, parameters, sampling time, and working memory
- [ ] 1.2 Define Port with data types, signal direction, and dimensions
- [ ] 1.3 Define Link for inter-block connections, signal duplication, and broadcasting
- [ ] 1.4 Define Diagram with global state management, module list, connection table, and sampling time groups
- [ ] 1.5 Implement parameter system with static parameters, runtime variable parameters, and Python script binding

## Phase 2: Simulation Core Context (Time and State)
**Status: Pending**

- [ ] 2.1 Implement simulation time system with current time, step size, end time, and time precision control
- [ ] 2.2 Implement state vector management for continuous and discrete states
- [ ] 2.3 Implement simulation modes: normal, real-time, step, and pause modes
- [ ] 2.4 Implement simulation lifecycle: initialization, start, run, pause, stop, reset

## Phase 3: Solver System (Full Scene Coverage)
**Status: Pending**

- [ ] 3.1 Implement fixed step solvers: Euler, RK4, Heun's method, Midpoint method
- [ ] 3.2 Implement variable step solvers: RK45, RK23, Cash-Karp
- [ ] 3.3 Implement stiff system solvers: BDF, Trapezoidal/Crank-Nicolson
- [ ] 3.4 Implement solver interface with state input, derivative input, step size calculation, error estimation, and automatic step size adjustment
- [ ] 3.5 Implement algebraic loop solvers: fixed-point iteration, direct solving, relaxation iteration

## Phase 4: Scheduling and Execution System (Simulation Soul)
**Status: Pending**

- [ ] 4.1 Implement topological sorting for directed acyclic graphs, signal flow order, and feedback loop processing
- [ ] 4.2 Implement module execution scheduling for continuous, discrete, multi-rate, and event-triggered modules
- [ ] 4.3 Implement signal propagation with input update, output calculation, and connection transfer
- [ ] 4.4 Implement module execution phases: Init, Output, Derivatives, Update, Event, Terminate

## Phase 5: Event System (Essential for All Scenarios)
**Status: Pending**

- [ ] 5.1 Implement event queue with time sorting, event insertion, and event retrieval
- [ ] 5.2 Implement zero-crossing detection for continuous signals, rising/falling edges, and precise event time localization
- [ ] 5.3 Implement discrete events: timer events, external triggers, interrupt events
- [ ] 5.4 Implement event handling: state transitions, module restart, solver restart, real-time response

## Phase 6: Discrete and Multi-rate Systems (Essential for Industrial Control)
**Status: Pending**

- [ ] 6.1 Implement discrete sampling systems with fixed sampling time, multi-rate mixing, and phase offset
- [ ] 6.2 Implement discrete state updates: unit delay, discrete integration, digital filters
- [ ] 6.3 Implement synchronization systems: clock synchronization, asynchronous events, triggered subsystems

## Phase 7: Algebraic Loops and Direct Feedthrough (Advanced Scenarios)
**Status: Pending**

- [ ] 7.1 Implement algebraic loop detection
- [ ] 7.2 Implement algebraic loop solving
- [ ] 7.3 Implement direct feedthrough module processing
- [ ] 7.4 Implement iterative solving

## Phase 8: Standard Module Library (100% Coverage for All Scenarios)
**Status: Pending**

- [ ] 8.1 Implement signal source modules: constant, sine wave, step, pulse, noise, function generator
- [ ] 8.2 Implement mathematical operation modules: addition, gain, multiplication, division, logic operations, comparison, trigonometric functions
- [ ] 8.3 Implement continuous system modules: integrator, differentiator, transfer function, state space, PID, lag
- [ ] 8.4 Implement discrete system modules: unit delay, discrete integrator, discrete filter, discrete PID
- [ ] 8.5 Implement routing and signal modules: multiplexer, demultiplexer, merge, switch
- [ ] 8.6 Implement observation and output modules: oscilloscope, data recording, display, data export

## Phase 9: 3D Physics / Multi-body Dynamics Co-simulation (OCCT Specific)
**Status: Pending**

- [ ] 9.1 Implement rigid body interface with position, velocity, attitude, force/torque
- [ ] 9.2 Implement constraint modules: revolute joint, prismatic joint, fixed constraint, gear constraint
- [ ] 9.3 Implement collision and contact: collision detection, contact force, bounce
- [ ] 9.4 Implement 3D geometry driver module with position output, attitude output, coordinate transformation
- [ ] 9.5 Implement time synchronization: simulation time = 3D rendering time

## Phase 10: Advanced Industrial Simulation Features (Complete Commercial Grade)
**Status: Pending**

- [ ] 10.1 Implement subsystems and encapsulation: atomic subsystems, conditional execution subsystems, function subsystems
- [ ] 10.2 Implement data recording and playback: state playback, signal recording, offline analysis
- [ ] 10.3 Implement exception handling: numerical overflow, singular solution, solver failure
- [ ] 10.4 Implement real-time simulation support: clock synchronization, delay control, hardware linkage
- [ ] 10.5 Implement script interface (Python): block diagram creation, run control, data acquisition, module extension

## Phase 11: Quantum Physics Extension Layer (Complete Industrial Grade)
**Status: Pending**

### 11.1 Quantum Data Structures and State System (Basic Layer)
- [ ] 11.1.1 Implement quantum state types: QStateVector, QDensityMatrix, Qubit
- [ ] 11.1.2 Implement quantum ports (QPort) for quantum input/output and classical control
- [ ] 11.1.3 Implement quantum links (QLink) for quantum signal transmission

### 11.2 Quantum Gates and Operator Library (Core Building Blocks)
- [ ] 11.2.1 Implement single-qubit gates: Pauli gates, rotation gates, phase gates, Hadamard
- [ ] 11.2.2 Implement multi-qubit gates: CNOT, CY, CZ, SWAP, controlled rotations, Toffoli
- [ ] 11.2.3 Implement universal operator interface for arbitrary unitary transformations and quantum channels

### 11.3 Quantum Evolution and Solvers (Soul Layer)
- [ ] 11.3.1 Implement closed system evolution with Schrödinger equation solver
- [ ] 11.3.2 Implement open system evolution with Lindblad master equation solver
- [ ] 11.3.3 Implement quantum measurement module: projection measurement, partial measurement, quantum phase estimation

### 11.4 Classical-Quantum Hybrid Simulation (Most Powerful Scenario)
- [ ] 11.4.1 Implement conditional execution for quantum gates based on classical signals
- [ ] 11.4.2 Implement hybrid system modules: classical control quantum gate block, quantum feedback block
- [ ] 11.4.3 Implement multi-physics coupling: quantum-mechanical coupling, quantum-electromagnetic coupling

### 11.5 Quantum Algorithms and Specialized Modules (Application Layer)
- [ ] 11.5.1 Implement quantum chemistry modules: VQE, QAOA
- [ ] 11.5.2 Implement quantum algorithm modules: Grover, Shor, HHL
- [ ] 11.5.3 Implement quantum optics modules: mode field operators, beam splitters, photon statistics