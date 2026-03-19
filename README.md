# SCIcoRS: Unified Simulation Kernel for All Humanity

## Overview
SCIcoRS is a universal simulation kernel designed to unify all engineering and scientific simulation scenarios across every discipline, scale, and field. It provides a single architecture for modeling, simulation, and data management, enabling seamless integration from the smallest chip to the largest cosmic system.

- **Unified Architecture:**
  - One coordinate system
  - One dimensional/unit system
  - One solver engine
  - One extensible database (TOML + SQLite)
- **Coverage:**
  - Nanometer (molecule/chip) → Micrometer (cell/device) → Millimeter (PCB/organ) → Meter (mechanical/human body) → Kilometer (equipment/building) → Light-year (star/galaxy)

## Key Features
- Modular block/port/link/diagram system
- Unified simulation context and time management
- General numerical solver suite (ODE, DAE, stiff/non-stiff, sparse, nonlinear)
- Scheduling and execution engine for mixed systems
- Event and trigger system (time, signal, external, conditional)
- Discrete and multi-rate system support
- Algebraic loop and numerical stability management
- Comprehensive math, signal, and control libraries
- Universal coordinate and unit system
- Extensible database for materials, components, biological, chemical, and physical data
- Specialized simulation modules for:
  - Semiconductor device physics (TCAD)
  - Analog/digital/RTL circuit simulation
  - Molecular dynamics, cell/tissue growth
  - Optics, acoustics, board-level/PCB, power electronics, RF/microwave
  - Structural, thermal, fluid, multibody, aerospace, quantum, astrophysics
  - Chemical, physiological, and biomedical processes
- Multi-physics coupling and cross-scale integration
- Advanced post-processing, visualization, and reporting
- Cross-platform, scripting, plugin, and cloud/distributed support

## Roadmap (Phases)
1. Core Model Kernel (Block/Port/Link/Diagram)
2. Simulation Context & Time System
3. General Numerical Solver System
4. Scheduling & Execution Engine
5. Event & Trigger System
6. Discrete & Multi-Rate Systems
7. Algebraic Loops & Numerical Stability
8. Math, Signal & Control Foundation Library
9. Unified Coordinate System
10. Unified Dimension & Unit System
11. Unified Database System (TOML + SQLite)
12. Semiconductor Device Physics Simulation (TCAD)
13. Analog Circuit Simulation (SPICE)
14. Digital Logic & RTL Simulation
15. Molecular Dynamics & Biomolecular Simulation
16. Cell Culture & Tissue Growth Simulation
17. Optics & Photonics Simulation
18. Acoustics & Vibration Simulation
19. Board-Level Circuit & PCB System Simulation
20. Power Electronics & Motor Drive Simulation
21. Electromagnetic Field & RF/Microwave Simulation
22. Physiological Systems & Biomedical Simulation
23. Chemical Reaction & Process Simulation
24. Structural Mechanics & FEA Simulation
25. Thermodynamics & Heat Transfer Simulation
26. Fluid Dynamics (CFD) Simulation
27. Multibody Dynamics & Mechanical System Simulation
28. Aerospace & Aerodynamic Simulation
29. Quantum Physics & Quantum Computing Simulation
30. Astrophysics & Celestial Orbit Simulation
31. Unified Multi-Physics Coupling Bus
32. Advanced Industrial Features, Post-Processing & Visualization
33. Cross-Platform, Scripting Ecosystem & Extension System

## Data & Extensibility
- **Database:** TOML for real data, SQLite for indexing
- **Libraries:** Materials, celestial, fluids, sections, electrical, logic, chips, board-level, optics, acoustics, chemicals, biomolecules, cells, media, semiconductor process
- **Extensible:** Public/private libraries, custom data, import/export, versioning

## Status
See [todo.md](todo.md) and [CHECKLIST.md](CHECKLIST.md) for detailed progress and feature tracking.


## License
This project is dual-licensed under either:

- MIT License (see [LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 (see [LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.

## Contributing
Contributions are welcome! Please see CONTRIBUTING.md for guidelines.

---

For the full technical roadmap and details, see [ROADMAP.md](ROADMAP.md).
