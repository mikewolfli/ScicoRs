# BLUE12 — 全物理场下一代扩展：3D/高级物理/并行化/统一计算平台

## 1. 概述

BLUE12 在阶段 13~34 已有实现的基础上，对全部 19 个领域模块进行**下一代扩展**，目标：

1. **维度升级** — 1D/2D → 3D 求解器，覆盖所有空间维度
2. **物理精度升级** — 简化模型 → 高级物理模型（湍流、非线性、多相、非平衡）
3. **计算统一** — 所有领域使用 `core::compute` 共享数学原语
4. **全并行化** — 所有可并行热点使用 rayon
5. **跨场耦合** — 构建统一的多物理场耦合接口
6. **测试全覆盖** — 每一新功能附带真实行为验证测试

**状态目标：** 编译通过、零 clippy 警告、全部测试通过、无占位代码、与已有 Block/Diagram/Engine 系统完全集成。

---

## 2. 扩展总览

| 领域 | 当前状态 | BLUE12 扩展 | 新增文件数 |
|------|---------|------------|-----------|
| fluid | 2D 不可压缩 NS | → 3D NS, 可压缩 NS, 湍流(k-ε/SST), 多相流(VOF), LES | +5 |
| thermal | 1D/2D 传导, 集总辐射 | → 3D FVM 传导, 热辐射 RTE, 共轭传热, 热网络 | +4 |
| structural | 线性静力, 模态占位 | → 非线性 FEA(NR), 几何刚度, 接触非线性, 显式动力 | +4 |
| emag | 1D FDTD, 静电场 | → 3D FDTD, FDFD, PML, 色散材料, 天线 3D 方向图 | +4 |
| quantum | 态向量, 密度矩阵 | → 张量网络 MPS, QEC 纠错码, 量子噪声信道 | +3 |
| astrophysics | N-body, ΛCDM | → GR 弱场, MHD, 恒星演化, N体 SPH | +4 |
| multibody | 刚体, 约束 | → 柔体 FFR, 接触动力学, 递归算法 | +3 |
| acoustic | SPL, 房间模态 | → BEM 声辐射, SEA, 超声相控阵, 声学超材料 | +3 |
| optical | 光线追迹, 标量衍射 | → 非序列追迹, Jones 矩阵, RCWA, 非线性光学 | +4 |
| bio_medical | HH, PK/PD, Windkessel | → 3D 组织 PDE, PBPK, 循环网络 1D, 心脏机电 | +4 |
| chemical | CSTR/PFR, 蒸馏 | → 动态流程, 精馏塔板, 催化反应器, 安全泄放 | +4 |
| pcb | 传输线, PI/SI | → 3D 场求解, IBIS, SerDes COM, EMI 辐射 | +3 |
| powerelec | Buck/Boost, 电机 | → 谐振 LLC, SiC/GaN 开关, 并网逆变, DAB | +4 |
| digital | 门级, CPU | → 事件驱动, SDF, CDC, 功耗估计, UVM | +3 |
| analog | MNA, DC/AC/瞬态 | → PSS, 谐波平衡, 混合信号, Verilog-A IF | +3 |
| tcad | MOSFET L1, BJT | → BSIM, 短沟道, GaN/SiC, 可靠性 NBTI | +3 |
| molbio | 古典力场, Verlet | → PME, SHAKE, NPT, QM/MM, Martini CoarseGrain | +4 |
| cellbio | 3D 格子, 生物反应器 | → 血管新生, EMT, 免疫交互, 类器官 | +3 |
| aerospace | 6DOF, ISA, 火箭 | → 高超音速, 再入, 制导, 轨道机动 | +3 |

---

## 3. 详细规格

### 3.1 流体动力学 Phase 27 扩展（`src/domains/fluid/`）

#### 3.1.1 `navier_stokes_3d.rs` — 3D 不可压缩 NS 求解器

```rust
/// 3D incompressible Navier-Stokes solver (projection method, staggered grid).
pub struct NavierStokes3D {
    pub nx: usize, pub ny: usize, pub nz: usize,
    pub dx: Scalar, pub dy: Scalar, pub dz: Scalar,
    pub dt: Scalar, pub re: Scalar,
    pub u: Vec<Vec<Vec<Scalar>>>, // (nx+1)×ny×nz
    pub v: Vec<Vec<Vec<Scalar>>>, // nx×(ny+1)×nz
    pub w: Vec<Vec<Vec<Scalar>>>, // nx×ny×(nz+1)
    pub p: Vec<Vec<Vec<Scalar>>>,
}

impl NavierStokes3D {
    pub fn new(nx, ny, nz, dx, dy, dz, dt, re) -> Self;
    pub fn projection_step(&mut self) -> Result<(), String>;
    pub fn compute_intermediate_velocity(&self) -> (Vec3D, Vec3D, Vec3D); // par_iter 3D
    pub fn solve_pressure_poisson(&mut self, u_star, v_star, w_star) -> Result<(), String>;
    pub fn velocity_correction(&mut self, u_star, v_star, w_star);
    pub fn set_bc(&mut self, boundary: &[WallCondition3D]);
}

/// 3D 壁面条件
pub enum WallCondition3D {
    NoSlip, FreeSlip,
    Inlet(Scalar, Scalar, Scalar),
    Outflow, MovingWall(Scalar, Scalar, Scalar),
}
```

#### 3.1.2 `compressible_ns.rs` — 可压缩 NS 求解器（密度基）

```rust
/// 2D compressible Navier-Stokes solver (finite volume, Roe scheme).
pub struct CompressibleNS2D {
    pub nx: usize, pub ny: usize,
    pub dx: Scalar, pub dy: Scalar, pub dt: Scalar,
    pub gamma: Scalar, pub pr: Scalar, pub mu: Scalar,
    /// Conserved variables: [ρ, ρu, ρv, ρE] at each cell
    pub Q: Vec<Vec<[Scalar; 4]>>,
}

impl CompressibleNS2D {
    pub fn new(nx, ny, dx, dy, dt, gamma, pr, mu) -> Self;
    /// Compute flux using Roe approximate Riemann solver
    pub fn compute_flux(&self) -> (Vec<Vec<[Scalar; 4]>>, Vec<Vec<[Scalar; 4]>>);
    /// Forward Euler step (parallel over cells)
    pub fn step(&mut self) -> Result<(), String>;
    /// Primitive variables: [ρ, u, v, p, T]
    pub fn primitive(&self) -> Vec<Vec<[Scalar; 5]>>;
}
```

> **注意**：`compressible_ns.rs` 中的 `gamma`、`pr`、`mu` 分别为比热比、普朗特数和动力粘度，均为 `Scalar` 类型，与 `core::types` 保持一致。为避免与已定义的 `physics.rs` 常量同名冲突，本模块内的常量在代码中直接使用 `std::f64::consts` 或 `crate::domains::fluid::physics` 的全路径引用。

#### 3.1.3 `turbulence.rs` — 湍流模型

```rust
pub enum TurbulenceModel { KEpsilon, KOmegaSST, SpalartAllmaras, LESSmagorinsky }

/// k-ε turbulence model (2D).
pub struct KEpsilon {
    pub k: Vec<Vec<Scalar>>,        // Turbulent kinetic energy
    pub epsilon: Vec<Vec<Scalar>>,  // Dissipation rate
    pub nut: Vec<Vec<Scalar>>,     // Eddy viscosity
    pub c_mu: Scalar, pub c1: Scalar, pub c2: Scalar,
    pub sigma_k: Scalar, pub sigma_e: Scalar,
}

impl KEpsilon {
    pub fn step(&mut self, u: &[Vec<Scalar>], v: &[Vec<Scalar>], rho: Scalar, mu: Scalar, dt: Scalar);
    pub fn turbulent_viscosity(&self) -> &[Vec<Scalar>];
}
```

#### 3.1.4 `multiphase.rs` — 多相流（VOF 方法）

```rust
/// Volume-of-Fluid two-phase flow solver.
pub struct VofSolver2D {
    pub nx: usize, pub ny: usize,
    pub dx: Scalar, pub dy: Scalar, pub dt: Scalar,
    pub re: Scalar,
    pub u: Vec<Vec<Scalar>>,
    pub v: Vec<Vec<Scalar>>,
    pub p: Vec<Vec<Scalar>>,
    pub phi: Vec<Vec<Scalar>>,  // Volume fraction [0,1]
    pub rho1: Scalar, pub rho2: Scalar,  // Densities
    pub mu1: Scalar, pub mu2: Scalar,    // Viscosities
}

impl VofSolver2D {
    pub fn new(nx, ny, dx, dy, dt, re, rho1, rho2, mu1, mu2) -> Self;
    pub fn step(&mut self) -> Result<(), String>;
    /// Advect volume fraction (CICSAM/HRIC scheme)
    pub fn advect_phi(&mut self, u: &[Vec<Scalar>], v: &[Vec<Scalar>]);
    /// Compute density and viscosity from phi
    pub fn mean_properties(&self) -> (Vec<Vec<Scalar>>, Vec<Vec<Scalar>>);
}
```

#### 3.1.5 `analysis.rs` 扩展 — 添加：

```rust
pub fn mass_flow_rate_3d(density: &[Vec<Vec<Scalar>>], u: &[Vec<Vec<Scalar>>], area: Scalar) -> Scalar;
pub fn circulation(v: &[Vec<Scalar>], u: &[Vec<Scalar>], dx: Scalar, dy: Scalar) -> Scalar;
pub fn vorticity_3d(u: &[Vec<Vec<Vec<Scalar>>], v: &[Vec<Vec<Vec<Scalar>>], w: &[Vec<Vec<Vec<Scalar>>], dx, dy, dz) -> Vec<Vec<Vec<[Scalar;3]>>>;
pub fn turbulent_kinetic_energy(u_prime: &[Vec<Scalar>], v_prime: &[Vec<Scalar>]) -> Vec<Vec<Scalar>>;
```

---

### 3.2 热传导 Phase 26 扩展（`src/domains/thermal/`）

#### 3.2.1 `conduction_3d.rs` — 3D 瞬态热传导（有限体积法）

```rust
/// 3D transient heat conduction solver (finite volume, implicit Euler).
pub struct HeatConduction3D {
    pub nx: usize, pub ny: usize, pub nz: usize,
    pub dx: Scalar, pub dy: Scalar, pub dz: Scalar,
    pub alpha: Scalar,
    pub temperature: Vec<Vec<Vec<Scalar>>>,
}

impl HeatConduction3D {
    pub fn new(nx, ny, nz, dx, dy, dz, alpha, initial_temp) -> Self;
    /// Alternating Direction Implicit (ADI) step
    pub fn adi_step(&mut self, dt: Scalar, boundary: &[BoundaryCondition3D]) -> Result<(), String>;
    /// Gauss-Seidel steady-state with SOR
    pub fn sor_step(&mut self, omega: Scalar, boundary: &[BoundaryCondition3D]) -> Result<(), String>;
}

pub enum BoundaryCondition3D {
    FixedTemp(Scalar), FixedHeatFlux(Scalar),
    Convection(Scalar, Scalar), Adiabatic,
}
```

#### 3.2.2 `radiation_3d.rs` — 参与性介质辐射换热（离散坐标法）

```rust
/// Discrete Ordinates Method (DOM) for radiative transfer.
pub struct DomRadiation {
    pub nx: usize, pub ny: usize, pub nz: usize,
    pub dx: Scalar, pub dy: Scalar, pub dz: Scalar,
    pub absorption: Scalar, pub scattering: Scalar,
    pub temperature: Vec<Vec<Vec<Scalar>>>,
    pub incident_radiation: Vec<Vec<Vec<Scalar>>>,
}

impl DomRadiation {
    pub fn solve(&mut self, boundary: &[WallRadiation]) -> Result<(), String>;
    pub fn radiative_heat_source(&self) -> Vec<Vec<Vec<Scalar>>>;
}
```

#### 3.2.3 `conjugate_heat.rs` — 共轭传热（流-固耦合）

```rust
pub struct ConjugateHeatTransfer {
    pub solid_temp: Vec<Vec<Scalar>>,
    pub fluid_temp: Vec<Vec<Scalar>>,
    pub htc: Vec<Vec<Scalar>>,  // Heat transfer coefficient map
}

impl ConjugateHeatTransfer {
    pub fn steady_state(&mut self, t_fluid_inlet: Scalar, t_ambient: Scalar) -> Result<(), String>;
}
```

#### 3.2.4 `analysis.rs` 扩展 — 添加热网络系统级分析

```rust
pub struct ThermalNetworkBuilder {
    pub nodes: HashMap<String, ThermalNode>,
    pub resistors: Vec<ThermalResistor>,
    pub capacitors: Vec<ThermalCapacitor>,
    pub sources: Vec<HeatSource>,
}

impl ThermalNetworkBuilder {
    pub fn build(&self) -> Result<Vec<Vec<Scalar>>, String>;
    pub fn steady_state_temp(&self) -> HashMap<String, Scalar>;
    pub fn transient(&self, t_end: Scalar, dt: Scalar) -> Result<HashMap<Scalar, HashMap<String, Scalar>>, String>;
}
```

---

### 3.3 结构力学 Phase 25 扩展（`src/domains/structural/`）

#### 3.3.1 `nonlinear_fea.rs` — 非线性有限元

```rust
pub struct NonlinearFemSystem {
    pub nodes: Vec<Coord3D>,
    pub elements: Vec<FemElement>,
    pub constraints: Vec<(usize, usize, Scalar)>,
    pub loads: Vec<(usize, usize, Scalar)>,
    pub material: MaterialProperties,
}

impl NonlinearFemSystem {
    pub fn solve_newton_raphson(&self, max_iter: usize, tolerance: Scalar) -> Result<Vec<Scalar>, String>;
    pub fn tangent_stiffness(&self, u: &[Scalar]) -> Vec<Vec<Scalar>>;
    pub fn internal_force(&self, u: &[Scalar]) -> Vec<Scalar>;
    pub fn residual(&self, u: &[Scalar], f_ext: &[Scalar]) -> Vec<Scalar>;
}
```

#### 3.3.2 `explicit_dynamics.rs` — 显式动力学（中心差分法）

```rust
pub struct ExplicitDynamics {
    pub mass: Vec<Scalar>,           // Lumped mass
    pub u: Vec<Scalar>, pub v: Vec<Scalar>, pub a: Vec<Scalar>,
    pub dt: Scalar, pub t: Scalar,
}

impl ExplicitDynamics {
    pub fn step(&mut self, f_ext: &[Scalar], f_int: &[Scalar]) -> Result<(), String>;
    pub fn critical_dt(&self, element_sizes: &[Scalar], wave_speed: Scalar) -> Scalar;
}
```

#### 3.3.3 `fea_assembly_parallel.rs` — 并行要素矩阵组装加速

```rust
/// Parallel element stiffness computation (already in assemble_stiffness).
/// Additional: parallel mass matrix assembly.
pub fn assemble_mass_parallel(system: &FemSystem) -> Vec<Vec<Scalar>> {
    // rayon par_iter over elements, then serial assembly
}
```

#### 3.3.4 `elements.rs` 扩展 — 添加：

```rust
impl BeamElement {
    pub fn geometric_stiffness(&self, axial_force: Scalar) -> Vec<Vec<Scalar>>;
}
impl SolidElement {
    pub fn strain_displacement_matrix(&self, xi: Scalar, eta: Scalar, zeta: Scalar) -> Vec<Vec<Scalar>>;
}
```

---

### 3.4 电磁场 Phase 22 扩展（`src/domains/emag/`）

#### 3.4.1 `fdtd3d.rs` — 3D FDTD 求解器

```rust
pub struct Fdtd3D {
    pub nx: usize, pub ny: usize, pub nz: usize,
    pub dx: Scalar, pub dy: Scalar, pub dz: Scalar, pub dt: Scalar,
    pub ex: Vec<Vec<Vec<Scalar>>>, pub ey: Vec<Vec<Vec<Scalar>>>, pub ez: Vec<Vec<Vec<Scalar>>>,
    pub hx: Vec<Vec<Vec<Scalar>>>, pub hy: Vec<Vec<Vec<Scalar>>>, pub hz: Vec<Vec<Vec<Scalar>>>,
    pub boundary: BoundaryType3D,
}

pub enum BoundaryType3D { PEC, PMC, Cpml(Scalar, usize) }

impl Fdtd3D {
    pub fn new(nx, ny, nz, dx, dy, dz, dt) -> Self;
    pub fn update_e(&mut self);  // par_iter over cells
    pub fn update_h(&mut self);  // par_iter over cells
    pub fn step(&mut self) -> Result<(), String>;
    pub fn inject_source(&mut self, source: &Source3D);
    pub fn total_energy(&self) -> Scalar;
}
```

#### 3.4.2 `dispersion.rs` — 色散材料模型

```rust
pub enum DispersionModel {
    Drude { wp: Scalar, gamma: Scalar },
    Debye { eps_s: Scalar, eps_inf: Scalar, tau: Scalar },
    Lorentz { eps_s: Scalar, eps_inf: Scalar, wp: Scalar, gamma: Scalar },
}

pub struct DispersiveFdtd3D {
    pub fdtd: Fdtd3D,
    pub materials: Vec<Vec<Vec<DispersionModel>>>,
    pub polarization_currents: Vec<Vec<Vec<[Scalar; 3]>>>,
}

impl DispersiveFdtd3D {
    pub fn update_e_with_dispersion(&mut self) -> Result<(), String>;
}
```

#### 3.4.3 `antenna_3d.rs` — 3D 天线分析

```rust
pub struct Antenna3D {
    pub geometry: Vec<Coord3D>,
    pub excitation: Vec<(usize, Scalar)>,  // (feed_point_index, voltage)
}

impl Antenna3D {
    pub fn radiation_pattern(&self, fdtd: &Fdtd3D) -> Vec<[Scalar; 3]>;  // (θ, φ, gain)
    pub fn directivity(&self, fdtd: &Fdtd3D) -> Scalar;
    pub fn input_impedance(&self, fdtd: &Fdtd3D) -> ComplexScalar;
    pub fn s11(&self, fdtd: &Fdtd3D, z0: Scalar) -> ComplexScalar;
}
```

#### 3.4.4 `scattering.rs` — 雷达散射截面

```rust
pub fn rcs_3d(fdtd: &Fdtd3D, incident_angle_theta: Scalar, incident_angle_phi: Scalar) -> Vec<Vec<Scalar>>;
```

---

### 3.5 量子物理 Phase 30 扩展（`src/domains/quantum/`）

#### 3.5.1 `mps.rs` — 矩阵乘积态（张量网络）

```rust
/// Matrix Product State representation for efficient 1D quantum simulation.
pub struct MatrixProductState {
    pub tensors: Vec<Vec<Vec<Vec<ComplexScalar>>>>,  // [bond_left, phys, bond_right]
    pub num_qubits: usize,
    pub max_bond_dim: usize,
}

impl MatrixProductState {
    pub fn ground_state(num_qubits: usize, max_bond: usize) -> Self;
    pub fn apply_gate(&mut self, gate: &[Vec<ComplexScalar>], qubit: usize) -> Result<(), String>;
    pub fn canonicalize(&mut self);
    pub fn truncate(&mut self, max_bond: usize);
    pub fn expectation(&self, operator: &[Vec<ComplexScalar>], qubit: usize) -> ComplexScalar;
    pub fn entanglement_entropy(&self, bond: usize) -> Scalar;
}
```

#### 3.5.2 `qec.rs` — 量子纠错码

```rust
pub enum QuantumCode { Repetition3, Steane7, Shor9, SurfaceCode { d: usize } }

pub struct QuantumErrorCorrection {
    pub code: QuantumCode,
    pub logical_state: QuantumState,
    pub syndrome: Vec<usize>,
}

impl QuantumErrorCorrection {
    pub fn encode(&self, physical: &QuantumState) -> Result<QuantumState, String>;
    pub fn detect_error(&self, noisy: &QuantumState) -> Vec<usize>;
    pub fn correct(&self, noisy: &QuantumState, syndrome: &[usize]) -> Result<QuantumState, String>;
    pub fn logical_error_rate(&self, physical_error_rate: Scalar, n_rounds: usize) -> Scalar;
}
```

#### 3.5.3 `noise_channel.rs` — 量子噪声信道

```rust
pub enum NoiseChannel {
    Depolarizing { p: Scalar },
    AmplitudeDamping { gamma: Scalar },
    PhaseDamping { gamma: Scalar },
    BitFlip { p: Scalar },
    PhaseFlip { p: Scalar },
    Custom { kraus_ops: Vec<Vec<Vec<ComplexScalar>>> },
}

impl NoiseChannel {
    pub fn apply(&self, state: &DensityMatrix) -> Result<DensityMatrix, String>;
    pub fn apply_to_circuit(&self, circuit: &QuantumCircuit, noise_prob: Scalar) -> QuantumCircuit;
    pub fn channel_fidelity(&self, input: &DensityMatrix, output: &DensityMatrix) -> Scalar;
}
```

#### 3.5.4 `analysis.rs` 扩展 — 添加量子过程层析

```rust
pub fn quantum_process_tomography(channel: &dyn Fn(&DensityMatrix) -> DensityMatrix) -> Vec<Vec<ComplexScalar>>;
pub fn pauli_twirl(channel: &[Vec<ComplexScalar>]) -> Vec<Vec<ComplexScalar>>;
```

---

### 3.6 天体物理 Phase 31 扩展（`src/domains/astrophysics/`）

#### 3.6.1 `gr_weak_field.rs` — 弱场广义相对论修正

```rust
pub struct GRCorrection {
    pub mass_central: Scalar,  // Central mass (kg)
}

impl GRCorrection {
    pub fn schwarzschild_radius(&self) -> Scalar;
    pub fn perihelion_precession(&self, semi_major: Scalar, eccentricity: Scalar) -> Scalar;
    pub fn gravitational_time_dilation(&self, r: Scalar) -> Scalar;
    pub fn light_deflection(&self, impact_parameter: Scalar) -> Scalar;
    pub fn shapiro_delay(&self, r_source: Scalar, r_observer: Scalar) -> Scalar;
}
```

#### 3.6.2 `magnetohydrodynamics.rs` — 磁流体动力学

```rust
pub struct Mhd2D {
    pub nx: usize, pub ny: usize,
    pub dx: Scalar, pub dy: Scalar, pub dt: Scalar,
    pub rho: Vec<Vec<Scalar>>,           // Density
    pub p: Vec<Vec<Scalar>>,             // Pressure
    pub vx: Vec<Vec<Scalar>>, pub vy: Vec<Vec<Scalar>>,  // Velocity
    pub bx: Vec<Vec<Scalar>>, pub by: Vec<Vec<Scalar>>,  // Magnetic field
    pub gamma: Scalar,
}

impl Mhd2D {
    pub fn step(&mut self) -> Result<(), String>;
    pub fn alfven_speed(&self) -> Vec<Vec<Scalar>>;
    pub fn plasma_beta(&self) -> Vec<Vec<Scalar>>;
}
```

#### 3.6.3 `stellar_evolution.rs` — 恒星结构/演化

```rust
pub struct StellarStructure {
    pub mass: Scalar, pub radius: Scalar, pub luminosity: Scalar,
    pub core_temp: Scalar, pub metallicity: Scalar,
}

impl StellarStructure {
    pub fn polytropic_profile(&self, n: Scalar) -> Vec<(Scalar, Scalar, Scalar)>;
    pub fn main_sequence_lifetime(&self) -> Scalar;
    pub fn eddington_luminosity(&self) -> Scalar;
}
```

#### 3.6.4 `sph.rs` — 光滑粒子流体动力学

```rust
pub struct SPHSimulation {
    pub particles: Vec<SPHParticle>,
    pub h: Scalar,        // Smoothing length
    pub kernel: KernelType,
}

pub struct SPHParticle {
    pub pos: Coord3D, pub vel: [Scalar; 3],
    pub mass: Scalar, pub rho: Scalar, pub p: Scalar,
    pub u: Scalar, pub h: Scalar,
}

pub enum KernelType { CubicSpline, WendlandC2, Gaussian }

impl SPHSimulation {
    pub fn density(&mut self);   // par_iter
    pub fn forces(&mut self);    // par_iter
    pub fn step(&mut self, dt: Scalar);
}
```

---

### 3.7 多体动力学 Phase 28 扩展（`src/domains/multibody/`）

#### 3.7.1 `flexible_body.rs` — 柔体（浮动坐标系 FFR）

```rust
pub struct FlexibleBody {
    pub rigid: RigidBody,
    pub modal_coords: Vec<Scalar>,
    pub mode_shapes: Vec<Vec<Scalar>>,
    pub natural_frequencies: Vec<Scalar>,
}

impl FlexibleBody {
    pub fn new(rigid: RigidBody, modes: Vec<Vec<Scalar>>, freqs: Vec<Scalar>) -> Self;
    pub deflected_position(&self, local_pos: &Coord3D) -> Coord3D;
    pub strain_energy(&self) -> Scalar;
}
```

#### 3.7.2 `contact_dynamics.rs` — 接触动力学（互补条件）

```rust
pub struct ContactSolver {
    pub restitution: Scalar,
    pub mu_static: Scalar, pub mu_kinetic: Scalar,
}

impl ContactSolver {
    pub fn solve_contacts(&self, bodies: &mut [RigidBody], dt: Scalar) -> Result<(), String>;
    pub fn lcp_solve(&self, a: &[Vec<Scalar>], b: &[Scalar]) -> Vec<Scalar>;
}
```

#### 3.7.3 `articulated_body.rs` — 递归刚体算法（ARB）

```rust
pub struct ArticulatedBody {
    pub bodies: Vec<RigidBody>,
    pub joints: Vec<Constraint>,
    pub parent: Vec<Option<usize>>,
}

impl ArticulatedBody {
    pub fn forward_kinematics(&mut self);
    pub fn inverse_dynamics(&self, qdd: &[Scalar]) -> Vec<[Scalar; 3]>;
    pub fn recursive_newton_euler(&mut self, forces: &[ExternalForce], dt: Scalar);
}
```

---

### 3.8 声学 Phase 19 扩展（`src/domains/acoustic/`）

#### 3.8.1 `bem_acoustic.rs` — 边界元法声辐射

```rust
pub struct AcousticBEM {
    pub nodes: Vec<Coord3D>,
    pub elements: Vec<(usize, usize, usize)>,  // Triangle connectivity
    pub frequency: Scalar,
    pub c: Scalar, pub rho: Scalar,
}

impl AcousticBEM {
    pub fn surface_pressure(&self, v_n: &[Scalar]) -> Result<Vec<ComplexScalar>, String>;
    pub fn far_field_pattern(&self, theta: Scalar, phi: Scalar) -> ComplexScalar;
}
```

#### 3.8.2 `ultrasound.rs` — 超声相控阵

```rust
pub struct PhasedArray {
    pub elements: Vec<Coord3D>,
    pub delays: Vec<Scalar>,
    pub frequency: Scalar,
    pub amplitude: Vec<Scalar>,
}

impl PhasedArray {
    pub fn focus_at(&self, target: &Coord3D) -> Vec<Scalar>;
    pub fn beam_pattern(&self, theta_range: (Scalar, Scalar, usize)) -> Vec<Scalar>;
}
```

---

### 3.9 光学 Phase 18 扩展（`src/domains/optical/`）

#### 3.9.1 `non_sequential.rs` — 非序列光线追迹

```rust
pub struct NonSequentialRayTracer {
    pub objects: Vec<Box<dyn OpticalObject>>,
    pub rays: Vec<Ray>,
    pub max_bounces: usize,
}

pub trait OpticalObject: Send + Sync {
    fn intersect(&self, ray: &Ray) -> Option<Intersection>;
    fn scatter(&self, ray: &Ray, hit: &Intersection) -> Vec<Ray>;
}

impl NonSequentialRayTracer {
    pub fn trace(&mut self) -> Result<(), String>;  // par_iter over rays
    pub fn irradiance_map(&self, detector_plane: Scalar) -> Vec<Vec<Scalar>>;
}
```

#### 3.9.2 `jones_mueller.rs` — 偏振琼斯/缪勒矩阵

```rust
pub struct JonesMatrix {
    pub data: [[ComplexScalar; 2]; 2],
}

impl JonesMatrix {
    pub fn linear_polarizer(angle: Scalar) -> Self;
    pub fn quarter_wave_plate(fast_axis: Scalar) -> Self;
    pub fn apply(&self, state: &[ComplexScalar; 2]) -> [ComplexScalar; 2];
    pub fn multiply(&self, other: &JonesMatrix) -> JonesMatrix;
}
```

#### 3.9.3 `rcwa.rs` — 严格耦合波分析（光栅衍射）

```rust
pub struct RcwaGrating {
    pub period: Scalar, pub depth: Scalar,
    pub n1: ComplexScalar, pub n2: ComplexScalar,
    pub harmonics: usize,
}

impl RcwaGrating {
    pub fn diffraction_efficiency(&self, wavelength: Scalar, theta: Scalar, polarization: &str) -> Vec<Scalar>;
}
```

---

### 3.10 生物医学 Phase 23 扩展（`src/domains/bio_medical/`）

#### 3.10.1 `tissue_diffusion.rs` — 组织内药物扩散 PDE

```rust
pub struct TissueDiffusion2D {
    pub nx: usize, pub ny: usize,
    pub dx: Scalar, pub dy: Scalar,
    pub diffusivity: Scalar,
    pub concentration: Vec<Vec<Scalar>>,
    pub k_clearance: Scalar,  // Drug clearance rate
}

impl TissueDiffusion2D {
    pub fn step(&mut self, dt: Scalar) -> Result<(), String>;
    pub fn inject(&mut self, x: usize, y: usize, dose: Scalar);
}
```

#### 3.10.2 `circulatory_network.rs` — 1D 动脉网络血流

```rust
pub struct ArterialSegment {
    pub length: Scalar, pub radius: Scalar,
    pub wall_thickness: Scalar, pub young_modulus: Scalar,
    pub r_proximal: Scalar, pub r_distal: Scalar,
}

pub struct CirculatoryNetwork {
    pub segments: Vec<ArterialSegment>,
    pub heart: WindkesselModel,
}

impl CirculatoryNetwork {
    pub fn pressure_wave(&self, t_end: Scalar, dt: Scalar) -> Result<Vec<Vec<Scalar>>, String>;
    pub fn reflection_coefficient(&self, i: usize) -> Scalar;
}
```

#### 3.10.3 `cardiac_electromechanics.rs` — 心脏电-机械耦合

```rust
pub struct CardiacModel {
    pub hh: HodgkinHuxley,          // Electrical
    pub mechanics: TissueMechanics, // Mechanical
    pub ca_transient: Vec<Scalar>,
}

impl CardiacModel {
    pub fn excitation_contraction_coupling(&mut self, i_stim: Scalar, dt: Scalar);
    pub fn frank_starling(volume: Scalar, contractility: Scalar) -> Scalar;
}
```

---

### 3.11 化学 Phase 24 扩展（`src/domains/chemical/`）

#### 3.11.1 `distillation_column.rs` — 精馏塔逐板计算

```rust
pub struct DistillationColumn {
    pub n_stages: usize,
    pub feed_stage: usize,
    pub reflux_ratio: Scalar,
    pub boilup_ratio: Scalar,
    pub alpha: Vec<Scalar>,  // Relative volatilities
}

impl DistillationColumn {
    pub fn mesh_equations(&self, x_f: &[Scalar], q: Scalar) -> Result<(Vec<Scalar>, Vec<Scalar>), String>;
    pub fn mccabe_thiele(&self, x_d: Scalar, x_b: Scalar, x_f: Scalar) -> usize;
}
```

#### 3.11.2 `catalytic_reactor.rs` — 催化反应器（含失活）

```rust
pub struct CatalyticReactor {
    pub length: Scalar, pub diameter: Scalar,
    pub epsilon: Scalar,  // Void fraction
    pub rho_cat: Scalar,  // Catalyst density
    pub deactivation_rate: Scalar,
    pub activity: Vec<Scalar>,
}

impl CatalyticReactor {
    pub fn profile(&self, inlet: &[Scalar], kinetics: &ReactionKinetics, t: Scalar) -> Result<Vec<Vec<Scalar>>, String>;
}
```

#### 3.11.3 `safety_relief.rs` — 安全阀/爆破片泄放

```rust
pub fn relief_valve_flow_area(set_pressure: Scalar, mass_flow: Scalar, gas_props: &GasProperties) -> Scalar;
pub fn flare_network_backpressure(pipes: &[PipeSegment], relief_rate: Scalar) -> Scalar;
```

---

### 3.12 PCB Phase 20 扩展（`src/domains/pcb/`）

#### 3.12.1 `via_model.rs` — 通孔建模（残桩谐振）

```rust
pub struct ViaModel {
    pub drill_diameter: Scalar,
    pub pad_diameter: Scalar,
    pub stub_length: Scalar,
    pub er: Scalar,
}

impl ViaModel {
    pub fn resonant_frequency(&self) -> Scalar;
    pub fn insertion_loss(&self, freq: Scalar) -> Scalar;
    pub fn stub_equalization(&mut self, back_drill_depth: Scalar);
}
```

#### 3.12.2 `serdes_com.rs` — SerDes 通道合规性（COM）

```rust
pub struct ChannelOperatingMargin {
    pub tx_eq: Vec<Scalar>,      // Tx FFE taps
    pub rx_eq: Vec<Scalar>,      // Rx CTLE/DFE taps
    pub channel_s4p: Vec<Vec<ComplexScalar>>,
    pub baud_rate: Scalar,
    pub ber_target: Scalar,
}

impl ChannelOperatingMargin {
    pub fn compute_com(&self) -> Scalar;
    pub fn eye_height_at_ber(&self, target_ber: Scalar) -> Scalar;
}
```

---

### 3.13 电力电子 Phase 21 扩展（`src/domains/powerelec/`）

#### 3.13.1 `resonant_converter.rs` — LLC 谐振变换器

```rust
pub struct LlcConverter {
    pub vin: Scalar, pub vout: Scalar,
    pub lr: Scalar, pub cr: Scalar, pub lm: Scalar,
    pub fs: Scalar, pub fr: Scalar,
}

impl LlcConverter {
    pub fn gain_curve(&self, fnorm: Scalar, q: Scalar) -> Scalar;
    pub fn dc_gain(&self, load: Scalar) -> Scalar;
    pub fn zvs_region(&self) -> (Scalar, Scalar);
}
```

#### 3.13.2 `dab_converter.rs` — 双有源桥 DAB

```rust
pub struct DabConverter {
    pub vin: Scalar, pub vout: Scalar,
    pub L: Scalar, pub fs: Scalar,
    pub phase_shift: Scalar,
}

impl DabConverter {
    pub fn power_flow(&self) -> Scalar;
    pub fn zvs_condition(&self) -> bool;
}
```

---

### 3.14 数字逻辑 Phase 15 扩展（`src/domains/digital/`）

#### 3.14.1 `event_driven.rs` — 事件驱动仿真引擎

```rust
pub struct EventDrivenSimulator {
    pub events: BinaryHeap<EventItem>,
    pub time: Scalar,
    pub signal_values: HashMap<String, SignalValue>,
}

impl EventDrivenSimulator {
    pub fn schedule(&mut self, delay: Scalar, signal: String, value: SignalValue);
    pub fn run(&mut self, t_end: Scalar) -> Result<(), String>;
    pub fn vcd_dump(&self, filepath: &str) -> Result<(), String>;
}
```

---

### 3.15 模拟电路 Phase 14 扩展（`src/domains/analog/`）

#### 3.15.1 `harmonic_balance.rs` — 谐波平衡分析

```rust
pub struct HarmonicBalance {
    pub n_harmonics: usize,
    pub mna: MnaMatrix,
    pub freqs: Vec<Scalar>,
}

impl HarmonicBalance {
    pub fn solve(&self, excitation: &[(usize, Vec<ComplexScalar>)]) -> Result<Vec<Vec<ComplexScalar>>, String>;
}
```

---

### 3.16 TCAD Phase 13 扩展（`src/domains/tcad/`）

#### 3.16.1 `bsim.rs` — BSIM4 MOSFET 紧凑模型

```rust
pub struct Bsim4Model {
    pub vth0: Scalar, pub u0: Scalar, pub vsat: Scalar,
    pub rdsw: Scalar, pub alpha0: Scalar, pub beta0: Scalar,
    // ... ~20 BSIM4 parameters
}

impl Bsim4Model {
    pub fn drain_current(&self, vgs: Scalar, vds: Scalar, vbs: Scalar) -> Scalar;
    pub fn capacitances(&self, vgs: Scalar, vds: Scalar, vbs: Scalar) -> [Scalar; 4];  // Cgg,Cgd,Csg,Cds
}
```

#### 3.16.2 `reliability.rs` — NBTI/HCI 可靠性

```rust
pub fn nbti_degradation(vgs: Scalar, temp: Scalar, time: Scalar) -> Scalar;
pub fn hci_degradation(vds: Scalar, ids: Scalar, time: Scalar) -> Scalar;
```

---

### 3.17 分子动力学 Phase 16 扩展（`src/domains/molbio/`）

#### 3.17.1 `ewald.rs` — 粒子网格 Ewald (PME)

```rust
pub struct ParticleMeshEwald {
    pub nx: usize, pub ny: usize, pub nz: usize,
    pub alpha: Scalar,
    pub charges: Vec<Scalar>,
    pub positions: Vec<Coord3D>,
    pub box_size: Coord3D,
}

impl ParticleMeshEwald {
    pub fn coulomb_energy(&self) -> Scalar;
    pub fn forces(&self) -> Vec<[Scalar; 3]>;  // par_iter
}
```

#### 3.17.2 `shake_rattle.rs` — SHAKE 约束算法

```rust
pub struct ShakeConstraints {
    pub constraints: Vec<(usize, usize, Scalar)>,  // (i, j, target_distance)
}

impl ShakeConstraints {
    pub fn satisfy(&self, positions: &mut [Coord3D], tolerance: Scalar) -> Result<(), String>;
}
```

---

### 3.18 细胞生物学 Phase 17 扩展（`src/domains/cellbio/`）

#### 3.18.1 `angiogenesis.rs` — 血管新生模型

```rust
pub struct Angiogenesis {
    pub vegf_gradient: Vec<Vec<Scalar>>,
    pub tip_cells: Vec<(usize, usize)>,
    pub vessel_density: Vec<Vec<Scalar>>,
}

impl Angiogenesis {
    pub fn step(&mut self, dt: Scalar);
}
```

#### 3.18.2 `immune_model.rs` — 肿瘤-免疫交互

```rust
pub struct TumourImmuneModel {
    pub tumour_cells: CellPopulation,
    pub t_cells: CellPopulation,
    pub nk_cells: CellPopulation,
    pub cytokine_levels: HashMap<String, Scalar>,
}

impl TumourImmuneModel {
    pub fn step(&mut self, dt: Scalar);
}
```

---

### 3.19 航空航天 Phase 29 扩展（`src/domains/aerospace/`）

#### 3.19.1 `hypersonic.rs` — 高超音速流动

```rust
pub struct HypersonicFlow {
    pub mach: Scalar, pub altitude: Scalar,
    pub angle_of_attack: Scalar,
}

impl HypersonicFlow {
    pub fn stagnation_temperature(&self) -> Scalar;
    pub fn stagnation_pressure(&self) -> Scalar;
    pub fn convective_heating(&self, nose_radius: Scalar) -> Scalar;
}
```

#### 3.19.2 `reentry.rs` — 再入动力学

```rust
pub struct ReentryTrajectory {
    pub vehicle: SixDofAircraft,
    pub atmosphere: HighAltitudeAtmosphere,
    pub heat_shield: ThermalProtectionSystem,
}

impl ReentryTrajectory {
    pub fn propagate(&mut self, dt: Scalar, t_end: Scalar) -> Result<Vec<ReentryState>, String>;
    pub fn max_heat_flux(&self) -> Scalar;
    pub fn max_deceleration(&self) -> Scalar;
}
```

---

## 4. 统一计算平台要求

### 4.1 并行化规范

所有 BLUE12 新增的大规模求解器必须遵循：
串行/并行，需通过认真分析，根据应用场景决定。

```rust
// ✅ 必须使用并行
data.par_iter_mut().enumerate().for_each(|(i, row)| { ... });
```

### 4.2 类型统一

所有 BLUE12 新增代码必须使用：

| 类型 | 来源 | 用途 |
|------|------|------|
| `Scalar` | `core::types::Scalar` (= f64) | 全部物理量 |
| `Time` | `core::types::Time` (= f64) | 仿真时间 |
| `Coord3D` | `core::coord::Coord3D` | 3D 坐标 |
| `ComplexScalar` | `num_complex::Complex<Scalar>` | 复数 |
| `Vec<Vec<Scalar>>` | `core::compute::matrix` | 矩阵运算 |
| `OdeSolver` trait | `runtime::solver::traits` | ODE 求解 |

### 4.3 计算委托

禁止领域模块自己实现数学运算——全部委托给 `core::compute`：

```rust
// ❌ 禁止
fn my_mat_mul(a, b) { ... }

// ✅ 必须
use crate::core::compute::matrix::mat_mul;
```

### 4.4 模块文件结构

```
domains/<domain>/
  mod.rs          — 仅 pub mod 声明 + pub use 重导出
  physics.rs      — 常量（可选，已有则扩展）
  *.rs            — 核心功能文件（每文件≤2000 行）
  analysis.rs     — 后处理/分析函数（可选）
```

---

## 5. 测试要求

| 领域 | 现有测试数 | BLUE12 新增测试数 | 总目标 |
|------|-----------|-------------------|--------|
| fluid | ~15 | +20 | ~35 |
| thermal | ~78 | +15 | ~93 |
| structural | ~65 | +15 | ~80 |
| emag | ~20 | +15 | ~35 |
| quantum | ~76 | +15 | ~91 |
| astrophysics | ~25 | +15 | ~40 |
| multibody | ~64 | +10 | ~74 |
| acoustic | ~30 | +10 | ~40 |
| optical | ~35 | +10 | ~45 |
| bio_medical | ~25 | +10 | ~35 |
| chemical | ~25 | +10 | ~35 |
| pcb | ~20 | +10 | ~30 |
| powerelec | ~25 | +10 | ~35 |
| digital | ~57 | +10 | ~67 |
| analog | ~38 | +10 | ~48 |
| tcad | ~52 | +10 | ~62 |
| molbio | ~47 | +10 | ~57 |
| cellbio | ~33 | +10 | ~43 |
| aerospace | ~89 | +10 | ~99 |

新增测试总目标：**~220 个**，全项目总测试数达到 **~1788** 个。

---

## 6. 实现顺序

### Phase A — 核心基础设施（优先级 P0）

1. `core/compute` — 验证所有域已使用统一计算平台
2. `fluid/navier_stokes_3d.rs` — 3D NS 求解器（流体是最大需求)
3. `emag/fdtd3d.rs` — 3D FDTD（电磁第二大需求）

### Phase B — 领域深度扩展（优先级 P1）

4. `thermal/conduction_3d.rs` — 3D 热传导
5. `structural/nonlinear_fea.rs` — 非线性 FEA
6. `quantum/mps.rs` — 张量网络量子模拟
7. `astrophysics/magnetohydrodynamics.rs` — MHD
8. `multibody/flexible_body.rs` — 柔体

### Phase C — 高级物理模型（优先级 P2）

9. `fluid/compressible_ns.rs` — 可压缩 NS
10. `fluid/turbulence.rs` — 湍流模型
11. `fluid/multiphase.rs` — 多相流
12. `emag/dispersion.rs` — 色散材料
13. `emag/antenna_3d.rs` — 3D 天线

### Phase D — 交叉耦合与后处理（优先级 P3）

14. 耦合总线集成测试
15. 后处理扩展
16. 批量性能基准测试

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 3D 求解器内存爆炸 | OOM | 稀疏表示 + 流式写入 + 网格自适应 |
| 可压缩 NS 激波不稳定 | 数值振荡 | TVD/Roe 通量 + 限制器 |
| FDTD 3D CPML 复杂性 | 实现错误 | 逐层验证 vs 解析解 |
| 湍流模型刚性问题 | 发散 | BDF 求解器 + 自适应步长 |
| MPS 张量网络收敛 | 精度损失 | 动态 bond dimension 调整 |
| 多体柔体模态截断 | 精度不足 | 模态截断误差估计 |
| FVM 3D 网格数量大 | 计算慢 | par_iter + grid 分区 |
