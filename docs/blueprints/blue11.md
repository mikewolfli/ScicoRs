# BLUE11 — 阶段 30~34：量子物理、天体轨道、统一耦合总线、后处理可视化与生态扩展

## 1. 概述

阶段 30~34 在已完成的内核（阶段 1~29）之上构建五个横跨微观量子到宇宙尺度的仿真能力，以及全物理场统一耦合总线、工业级后处理可视化和平台生态扩展：

- **阶段 30**：量子物理与量子计算仿真 — 量子态、量子门、薛定谔方程、林德布拉德主方程、量子算法（VQE/QAOA/Grover/HHL）
- **阶段 31**：天体物理与宇宙轨道仿真 — 行星/恒星/黑洞/星系、万有引力/轨道力学、二体/多体问题、航天轨道设计
- **阶段 32**：全物理场统一耦合总线 — 多物理场接口标准、场量映射/插值/投影、跨尺度耦合（纳米→微米→米→宇宙）、统一收敛控制
- **阶段 33**：工业高级功能、后处理与可视化 — 数据记录/回放/离线分析、图表/曲线/云图/等值面、报告生成、硬件在环 HIL、批量任务
- **阶段 34**：跨平台、脚本生态与扩展系统 — Python 全脚本接口、插件系统、CAD/CAE 数据接口（STEP/STL/网格）、跨平台/云端/分布式部署

**状态目标：** 100% — 编译通过、零 clippy 警告、全部测试通过、无占位代码、与已有 Block/Diagram/Engine 系统完全集成。

---

## 2. 模块架构

```
src/
  domains/
    mod.rs              — 更新：暴露 quantum + astrophysics 子模块
    quantum/            — [新增] 阶段 30：量子物理与量子计算仿真
      mod.rs            — 模块接口
      physics.rs        — 量子物理常量（普朗克常数、玻尔兹曼常数、精细结构常数）
      state.rs          — 量子态表示（态向量、密度矩阵、量子比特）
      gates.rs          — 量子门（H/X/Y/Z/CNOT/CZ/SWAP/Toffoli/旋转门/相位门）
      schrodinger.rs    — 薛定谔方程求解（含时/不含时、有限差分/谱方法）
      lindblad.rs       — 林德布拉德主方程（开放系统、退相干、耗散）
      measurement.rs    — 量子测量与坍缩（投影测量、POVM、纠缠/叠加检测）
      algorithms.rs     — 量子算法实现（VQE、QAOA、Grover、HHL、Shor）
      analysis.rs       — 量子系统分析工具（保真度、纠缠熵、概率分布）
    astrophysics/       — [新增] 阶段 31：天体物理与宇宙轨道仿真
      mod.rs            — 模块接口
      physics.rs        — 天体物理常量（引力常数、光速、太阳质量/半径）
      celestial_body.rs — 天体模型（行星、恒星、卫星、黑洞、星系）
      orbital.rs        — 轨道力学（二体/多体问题、开普勒轨道、轨道摄动）
      gravity.rs        — 万有引力、潮汐力、N体引力求解器
      cosmology.rs      — 宇宙演化、膨胀、引力透镜、暗物质/暗能量
      spacecraft.rs     — 航天轨道设计（变轨、对接、逃逸速度、Lambert 问题）
      analysis.rs       — 轨道分析工具（周期、能量、角动量、稳定性）
  coupling/             — [更新] 阶段 32：全物理场统一耦合总线
    mod.rs              — 模块接口（更新：暴露总线结构）
    bus.rs              — [新增] 统一耦合总线（耦合接口注册、数据交互规范）
    field_mapping.rs    — [新增] 场量映射、插值、投影、数据传递
    cross_scale.rs      — [新增] 跨尺度耦合（纳米→微米→米→宇宙）
    convergence.rs      — [新增] 统一收敛控制、求解调度、时间同步
  postproc/             — [更新] 阶段 33：后处理与可视化
    mod.rs              — 模块接口（更新：暴露后处理结构）
    recorder.rs         — [新增] 数据记录/回放/离线分析
    visualization.rs    — [新增] 图表/曲线/云图/矢量图/等值面
    reporting.rs        — [新增] 报告生成、数据导出
    batch.rs            — [新增] 批量任务、自动化仿真
    hilsupport.rs       — [新增] 硬件在环 HIL 支持
  bindings/             — [更新] 阶段 34：脚本生态与扩展系统
    mod.rs              — 模块接口（更新：暴露绑定结构）
    python/             — [新增] Python 全脚本接口
      mod.rs            — Python 绑定接口模块
      py_simulation.rs  — Python 端仿真控制
      py_blocks.rs      — Python 端模块构建
      py_data.rs        — Python 端数据访问
    plugins/            — [新增] 插件系统
      mod.rs            — 插件管理器
    data_io/            — [新增] CAD/CAE/PLM 数据接口
      mod.rs            — 数据 IO 接口
      step_io.rs        — STEP 文件解析/导出
      stl_io.rs         — STL 文件解析/导出
      mesh_io.rs        — 网格文件格式
    platform/           — [新增] 跨平台与部署
      mod.rs            — 平台抽象层
      cloud.rs          — 云端部署与分布式计算
```

---

## 3. 详细规格

### 3.1 阶段 30 — 量子物理与量子计算仿真（`src/domains/quantum/`）

#### 3.1.1 `physics.rs` — 量子物理常量

```rust
/// 约化普朗克常数 ℏ (J·s)
pub const HBAR: Scalar = 1.054571817e-34;

/// 普朗克常数 h (J·s)
pub const PLANCK: Scalar = 6.62607015e-34;

/// 玻尔兹曼常数 k_B (J/K)
pub const BOLTZMANN: Scalar = 1.380649e-23;

/// 精细结构常数 α
pub const FINE_STRUCTURE: Scalar = 7.2973525693e-3;

/// 基本电荷 e (C)
pub const ELEMENTARY_CHARGE: Scalar = 1.602176634e-19;

/// 玻尔磁子 μ_B (J/T)
pub const BOHR_MAGNETON: Scalar = 9.2740100783e-24;

/// 电子质量 m_e (kg)
pub const ELECTRON_MASS: Scalar = 9.1093837e-31;

/// 质子质量 m_p (kg)
pub const PROTON_MASS: Scalar = 1.6726219e-27;

/// 真空光速 c (m/s)
pub const SPEED_OF_LIGHT: Scalar = 299792458.0;

/// 真空介电常数 ε₀ (F/m)
pub const VACUUM_PERMITTIVITY: Scalar = 8.854187817e-12;

/// 真空磁导率 μ₀ (N/A²)
pub const VACUUM_PERMEABILITY: Scalar = 1.2566370612e-6;

/// Rydberg 常数 R_∞ (1/m)
pub const RYDBERG: Scalar = 10973731.568157;
```

#### 3.1.2 `state.rs` — 量子态表示

```rust
/// 量子态表示
#[derive(Debug, Clone, PartialEq)]
pub struct QuantumState {
    /// 态向量（复数幅度）
    pub amplitudes: Vec<ComplexScalar>,
    /// 量子比特数
    pub num_qubits: usize,
}

impl QuantumState {
    /// 创建 |0⟩ 基态
    pub fn ground_state(num_qubits: usize) -> Self;
    /// 创建均匀叠加态
    pub fn uniform_superposition(num_qubits: usize) -> Self;
    /// 从计算基构造状态
    pub fn from_basis(basis_index: usize, num_qubits: usize) -> Self;
    /// 归一化
    pub fn normalize(&mut self) -> Result<(), String>;
    /// 计算内积 ⟨ψ|φ⟩
    pub fn inner_product(&self, other: &QuantumState) -> ComplexScalar;
    /// 计算概率幅分布
    pub fn probabilities(&self) -> Vec<Scalar>;
    /// 测量某量子比特得到 0 的概率
    pub fn measure_probability(&self, qubit: usize) -> Scalar;
    /// 部分迹（对指定量子比特求迹）
    pub fn partial_trace(&self, qubits: &[usize]) -> DensityMatrix;
    /// 保真度 F = |⟨ψ|φ⟩|²
    pub fn fidelity(&self, other: &QuantumState) -> Scalar;
}

/// 密度矩阵表示
#[derive(Debug, Clone, PartialEq)]
pub struct DensityMatrix {
    /// 密度矩阵元素（复数，行主序）
    pub data: Vec<ComplexScalar>,
    /// 维度（2^num_qubits）
    pub dim: usize,
}

impl DensityMatrix {
    /// 从纯态构造密度矩阵 ρ = |ψ⟩⟨ψ|
    pub fn from_pure_state(state: &QuantumState) -> Self;
    /// 创建最大混合态
    pub fn maximally_mixed(dim: usize) -> Self;
    /// 迹 Tr(ρ)
    pub fn trace(&self) -> ComplexScalar;
    /// 纯度 Tr(ρ²)
    pub fn purity(&self) -> Scalar;
    /// 冯·诺依曼熵 S = -Tr(ρ·log₂ρ)
    pub fn von_neumann_entropy(&self) -> Scalar;
    /// 应用 Kraus 算符: ρ' = Σᵢ Kᵢ·ρ·Kᵢ†
    pub fn apply_kraus(&mut self, kraus_ops: &[Vec<ComplexScalar>]) -> Result<(), String>;
}

/// 复标量类型
pub type ComplexScalar = num_complex::Complex<Scalar>;
// 或使用内置的 (re, im) 元组表示
```

#### 3.1.3 `gates.rs` — 量子门

```rust
/// 单量子比特门
pub enum SingleQubitGate {
    /// Hadamard 门: H = (1/√2)[[1,1],[1,-1]]
    Hadamard,
    /// Pauli-X (NOT): X = [[0,1],[1,0]]
    PauliX,
    /// Pauli-Y: Y = [[0,-i],[i,0]]
    PauliY,
    /// Pauli-Z: Z = [[1,0],[0,-1]]
    PauliZ,
    /// 相位门 S = [[1,0],[0,i]]
    Phase,
    /// π/8 门 T = [[1,0],[0,e^(iπ/4)]]
    PiOver8,
    /// 任意旋转 Rx(θ)
    RotationX(Scalar),
    /// 任意旋转 Ry(θ)
    RotationY(Scalar),
    /// 任意旋转 Rz(θ)
    RotationZ(Scalar),
}

/// 多量子比特门
pub enum MultiQubitGate {
    /// CNOT (CX): 控制-NOT
    CNOT,
    /// CZ: 控制-Z
    CZ,
    /// SWAP
    SWAP,
    /// Toffoli (CCNOT)
    Toffoli,
    /// 任意控制-酉门
    ControlledU(Vec<ComplexScalar>),
}

/// 量子电路门操作
#[derive(Debug, Clone)]
pub struct GateOperation {
    /// 门类型
    pub gate: GateType,
    /// 目标量子比特索引
    pub target_qubits: Vec<usize>,
    /// 控制量子比特索引
    pub control_qubits: Vec<usize>,
}

#[derive(Debug, Clone)]
pub enum GateType {
    Single(SingleQubitGate),
    Multi(MultiQubitGate),
    /// 自定义酉矩阵门
    Custom(Vec<ComplexScalar>),
}

/// 量子门应用
impl GateOperation {
    /// 构建门操作的矩阵表示
    pub fn matrix(&self, num_qubits: usize) -> Vec<Vec<ComplexScalar>>;
    /// 对量子态应用此门
    pub fn apply(&self, state: &QuantumState) -> Result<QuantumState, String>;
    /// 对密度矩阵应用此门
    pub fn apply_to_density(&self, rho: &DensityMatrix) -> Result<DensityMatrix, String>;
}

/// 标准量子门矩阵
pub fn hadamard_matrix() -> Vec<Vec<ComplexScalar>>;
pub fn pauli_x_matrix() -> Vec<Vec<ComplexScalar>>;
pub fn pauli_y_matrix() -> Vec<Vec<ComplexScalar>>;
pub fn pauli_z_matrix() -> Vec<Vec<ComplexScalar>>;
pub fn cnot_matrix() -> Vec<Vec<ComplexScalar>>;
pub fn swap_matrix() -> Vec<Vec<ComplexScalar>>;
pub fn toffoli_matrix() -> Vec<Vec<ComplexScalar>>;

/// 旋转门矩阵
pub fn rotation_x(theta: Scalar) -> Vec<Vec<ComplexScalar>>;
pub fn rotation_y(theta: Scalar) -> Vec<Vec<ComplexScalar>>;
pub fn rotation_z(theta: Scalar) -> Vec<Vec<ComplexScalar>>;

/// 量子电路
#[derive(Debug, Clone)]
pub struct QuantumCircuit {
    pub num_qubits: usize,
    pub operations: Vec<GateOperation>,
}

impl QuantumCircuit {
    pub fn new(num_qubits: usize) -> Self;
    pub fn add_gate(&mut self, gate: GateOperation);
    /// 对整个电路进行矩阵乘积
    pub fn unitary(&self) -> Result<Vec<Vec<ComplexScalar>>, String>;
    /// 模拟执行电路
    pub fn simulate(&self, initial_state: &QuantumState) -> Result<QuantumState, String>;
}
```

#### 3.1.4 `schrodinger.rs` — 薛定谔方程求解

```rust
/// 含时薛定谔方程: iℏ·d|ψ⟩/dt = H·|ψ⟩
pub struct SchrodingerSolver {
    pub hamiltonian: Vec<Vec<ComplexScalar>>,  // 哈密顿量矩阵
    pub dt: Scalar,                             // 时间步长 (s)
}

impl SchrodingerSolver {
    /// 创建求解器
    pub fn new(hamiltonian: Vec<Vec<ComplexScalar>>, dt: Scalar) -> Self;
    /// Crank-Nicolson 一步演化: |ψ(t+dt)⟩ = (I + i·H·dt/2ℏ)·(I - i·H·dt/2ℏ)⁻¹·|ψ(t)⟩
    pub fn crank_nicolson_step(&self, state: &QuantumState) -> Result<QuantumState, String>;
    /// 指数传播子一步演化: |ψ(t+dt)⟩ = exp(-i·H·dt/ℏ)·|ψ(t)⟩
    pub fn propagator_step(&self, state: &QuantumState) -> Result<QuantumState, String>;
    /// 时间演化到 t_end（多步）
    pub fn evolve(&self, initial: &QuantumState, t_end: Scalar) -> Result<Vec<QuantumState>, String>;
}

/// 不含时薛定谔方程 H·|ψ⟩ = E·|ψ⟩
pub struct StationarySolver;

impl StationarySolver {
    /// 幂迭代法求基态能量
    pub fn power_iteration(h: &[Vec<ComplexScalar>], max_iter: usize, tol: Scalar) -> Option<(Scalar, Vec<ComplexScalar>)>;
    /// 雅可比法求多个本征值/本征态
    pub fn jacobi_method(h: &[Vec<ComplexScalar>], num_eigenvalues: usize) -> Result<(Vec<Scalar>, Vec<Vec<ComplexScalar>>), String>;
}

/// 一维无限深势阱本征态
pub fn infinite_well_state(n: usize, x: Scalar, l: Scalar) -> Scalar;

/// 谐振子本征态
pub fn harmonic_oscillator_state(n: usize, x: Scalar, m: Scalar, omega: Scalar) -> Scalar;
```

#### 3.1.5 `lindblad.rs` — 林德布拉德主方程

```rust
/// 林德布拉德主方程: dρ/dt = -i/ℏ[H,ρ] + Σⱼ(Lⱼ·ρ·Lⱼ† - ½{Lⱼ†·Lⱼ, ρ})
pub struct LindbladSolver {
    pub hamiltonian: Vec<Vec<ComplexScalar>>,  // 系统哈密顿量
    pub jump_operators: Vec<Vec<Vec<ComplexScalar>>>,  // 跳跃算符集合
    pub dt: Scalar,
}

impl LindbladSolver {
    pub fn new(
        hamiltonian: Vec<Vec<ComplexScalar>>,
        jump_operators: Vec<Vec<Vec<ComplexScalar>>>,
        dt: Scalar,
    ) -> Self;
    /// RK4 一步积分
    pub fn rk4_step(&self, rho: &DensityMatrix) -> Result<DensityMatrix, String>;
    /// 时间演化
    pub fn evolve(&self, initial: &DensityMatrix, t_end: Scalar) -> Result<Vec<DensityMatrix>, String>;
}

/// 常见的退相干通道
pub fn amplitude_damping(gamma: Scalar) -> Vec<Vec<ComplexScalar>>;
pub fn dephasing_channel(gamma: Scalar) -> Vec<Vec<ComplexScalar>>;
pub fn depolarizing_channel(p: Scalar) -> Vec<Vec<ComplexScalar>>;
pub fn phase_flip_channel(p: Scalar) -> Vec<Vec<ComplexScalar>>;

/// 自发辐射算符
pub fn spontaneous_emission(gamma: Scalar) -> Vec<Vec<ComplexScalar>>;

/// 热库 Lindblad 算符
pub fn thermal_bath(n_bar: Scalar, gamma: Scalar) -> Vec<Vec<ComplexScalar>>;
```

#### 3.1.6 `measurement.rs` — 量子测量

```rust
/// 投影测量结果
pub struct MeasurementResult {
    pub outcome: usize,              // 测量结果（计算基索引）
    pub probability: Scalar,         // 概率
    pub collapsed_state: QuantumState, // 坍缩后的状态
}

/// 投影测量
pub fn projective_measurement(state: &QuantumState, qubit: usize) -> MeasurementResult;

/// 计算基全测量
pub fn computational_basis_measurement(state: &QuantumState) -> (usize, Scalar);

/// POVM 测量
pub struct PovmMeasurement {
    pub operators: Vec<Vec<Vec<ComplexScalar>>>,  // POVM 元 {E_i}
}

impl PovmMeasurement {
    pub fn new(operators: Vec<Vec<Vec<ComplexScalar>>>) -> Self;
    /// 检查完备性 Σᵢ Eᵢ = I
    pub fn check_completeness(&self, dim: usize) -> bool;
    /// 对量子态执行 POVM
    pub fn measure(&self, state: &QuantumState) -> Result<(usize, Scalar), String>;
}

/// 纠缠检测 —— 计算并发度 Concurrence
pub fn concurrence(state: &QuantumState, qubit_a: usize, qubit_b: usize) -> Scalar;

/// Bell 不等式违背检测
pub fn bell_inequality_violation(state: &QuantumState) -> Option<Scalar>;

/// 量子态层析（简化：从测量结果重建密度矩阵）
pub fn quantum_state_tomography(measurement_data: &[(usize, Scalar)], dim: usize) -> DensityMatrix;
```

#### 3.1.7 `algorithms.rs` — 量子算法

```rust
/// 变分量子本征求解器 VQE
pub struct VqeSolver {
    pub hamiltonian: Vec<Vec<ComplexScalar>>,
    pub ansatz_circuit: QuantumCircuit,
    pub optimizer: VqeOptimizer,
}

pub enum VqeOptimizer {
    /// 梯度下降
    GradientDescent { learning_rate: Scalar, max_iter: usize },
    /// COBYLA
    Cobyla { max_iter: usize, tolerance: Scalar },
    /// SPSA（同时扰动随机逼近）
    Spsa { max_iter: usize, alpha: Scalar, c: Scalar },
}

impl VqeSolver {
    pub fn new(hamiltonian: Vec<Vec<ComplexScalar>>, ansatz: QuantumCircuit, optimizer: VqeOptimizer) -> Self;
    /// 计算能量期望值 ⟨ψ(θ)|H|ψ(θ)⟩
    pub fn energy_expectation(&self, params: &[Scalar]) -> Result<Scalar, String>;
    /// 运行优化
    pub fn optimize(&mut self) -> Result<(Scalar, Vec<Scalar>), String>;
}

/// QAOA（量子近似优化算法）
pub struct QaoaSolver {
    pub cost_hamiltonian: Vec<Vec<ComplexScalar>>,
    pub mixer_hamiltonian: Vec<Vec<ComplexScalar>>,
    pub p_layers: usize,
}

impl QaoaSolver {
    pub fn new(cost_h: Vec<Vec<ComplexScalar>>, mixer_h: Vec<Vec<ComplexScalar>>, p: usize) -> Self;
    /// QAOA 电路构建
    pub fn build_circuit(&self, gamma: &[Scalar], beta: &[Scalar]) -> QuantumCircuit;
    /// 运行 QAOA
    pub fn optimize(&mut self) -> Result<(Scalar, Vec<Scalar>, Vec<Scalar>), String>;
}

/// Grover 搜索算法
pub fn grover_search(oracle: Box<dyn Fn(usize) -> bool>, num_qubits: usize, num_solutions: usize) -> Option<usize>;

/// HHL 线性系统求解算法（简化接口）
pub fn hhl_solver(a: &[Vec<ComplexScalar>], b: &[ComplexScalar], num_qubits: usize) -> Result<Vec<ComplexScalar>, String>;

/// 量子傅里叶变换 QFT
pub fn quantum_fourier_transform(circuit: &mut QuantumCircuit, qubits: &[usize]);
```

#### 3.1.8 `analysis.rs` — 量子分析

```rust
/// 保真度 F(ρ, σ) = Tr(√(√ρ·σ·√ρ))
pub fn fidelity_density(rho: &DensityMatrix, sigma: &DensityMatrix) -> Scalar;

/// 迹距离 D(ρ, σ) = ½·Tr(|ρ - σ|)
pub fn trace_distance(rho: &DensityMatrix, sigma: &DensityMatrix) -> Scalar;

/// 纠缠熵（子系统约化密度矩阵的冯·诺依曼熵）
pub fn entanglement_entropy(state: &QuantumState, subsystem: &[usize]) -> Scalar;

/// 量子互信息 I(A:B) = S(A) + S(B) - S(AB)
pub fn quantum_mutual_information(state: &QuantumState, system_a: &[usize], system_b: &[usize]) -> Scalar;

/// 概率分布从测量结果统计
pub fn measurement_statistics(counts: &[usize], num_shots: usize) -> Vec<(usize, Scalar)>;
```

---

### 3.2 阶段 31 — 天体物理与宇宙轨道仿真（`src/domains/astrophysics/`）

#### 3.2.1 `physics.rs` — 天体物理常量

```rust
/// 引力常数 G (m³/(kg·s²))
pub const GRAVITATIONAL: Scalar = 6.67430e-11;

/// 光速 c (m/s)
pub const C: Scalar = 299792458.0;

/// 太阳质量 M☉ (kg)
pub const SOLAR_MASS: Scalar = 1.98847e30;

/// 太阳半径 R☉ (m)
pub const SOLAR_RADIUS: Scalar = 6.957e8;

/// 地球质量 M⊕ (kg)
pub const EARTH_MASS: Scalar = 5.9722e24;

/// 地球半径 R⊕ (m)
pub const EARTH_RADIUS: Scalar = 6371000.0;

/// 地球轨道半径 1 AU (m)
pub const AU: Scalar = 1.495978707e11;

/// 秒差距 pc (m)
pub const PARSEC: Scalar = 3.085677581e16;

/// 光年 ly (m)
pub const LIGHT_YEAR: Scalar = 9.460730472e15;

/// 哈勃常数 H₀ (km/s/Mpc) — 近似值
pub const HUBBLE_CONSTANT: Scalar = 70.0;

/// 太阳光度 L☉ (W)
pub const SOLAR_LUMINOSITY: Scalar = 3.828e26;

/// 太阳表面温度 (K)
pub const SOLAR_TEMPERATURE: Scalar = 5772.0;

/// 标准重力参数 GM⊕ (m³/s²)
pub const EARTH_GM: Scalar = 3.986004418e14;

/// 太阳标准重力参数 GM☉ (m³/s²)
pub const SOLAR_GM: Scalar = 1.32712442099e20;
```

#### 3.2.2 `celestial_body.rs` — 天体模型

```rust
/// 天体类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CelestialBodyType {
    Star,
    Planet,
    Moon,
    DwarfPlanet,
    Asteroid,
    Comet,
    BlackHole,
    NeutronStar,
    Galaxy,
}

/// 天体物理属性
#[derive(Debug, Clone, PartialEq)]
pub struct CelestialBody {
    pub id: String,
    pub name: String,
    pub body_type: CelestialBodyType,
    pub mass: Scalar,                // 质量 (kg)
    pub radius: Scalar,              // 半径 (m)
    pub position: Coord3D,           // 位置 (m)
    pub velocity: [Scalar; 3],       // 速度 (m/s)
    pub rotation_rate: Scalar,       // 自转角速度 (rad/s)
    pub axial_tilt: Scalar,          // 转轴倾角 (rad)
    pub gravitational_parameter: Scalar,  // GM (m³/s²)
}

impl CelestialBody {
    pub fn new(id: &str, name: &str, mass: Scalar, radius: Scalar) -> Self;
    /// 表面重力加速度 (m/s²)
    pub fn surface_gravity(&self) -> Scalar;
    /// 逃逸速度 (m/s)
    pub fn escape_velocity(&self, altitude: Scalar) -> Scalar;
    /// 施瓦西半径（黑洞）(m)
    pub fn schwarzschild_radius(&self) -> Scalar;
    /// 开普勒第三定律轨道周期（绕母体）
    pub fn orbital_period(&self, semi_major_axis: Scalar, parent_mass: Scalar) -> Scalar;
}

/// 太阳系行星预设
pub fn mercury() -> CelestialBody;
pub fn venus() -> CelestialBody;
pub fn earth() -> CelestialBody;
pub fn mars() -> CelestialBody;
pub fn jupiter() -> CelestialBody;
pub fn saturn() -> CelestialBody;
pub fn uranus() -> CelestialBody;
pub fn neptune() -> CelestialBody;

/// 太阳预设
pub fn sun() -> CelestialBody;

/// 月球预设
pub fn moon() -> CelestialBody;
```

#### 3.2.3 `orbital.rs` — 轨道力学

```rust
/// 开普勒轨道根数
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeplerianElements {
    pub semi_major_axis: Scalar,     // 半长轴 a (m)
    pub eccentricity: Scalar,        // 离心率 e
    pub inclination: Scalar,         // 轨道倾角 i (rad)
    pub raan: Scalar,                // 升交点赤经 Ω (rad)
    pub argument_of_periapsis: Scalar, // 近心点幅角 ω (rad)
    pub true_anomaly: Scalar,        // 真近点角 ν (rad)
}

impl KeplerianElements {
    /// 轨道能量 (J/kg)
    pub fn specific_energy(&self, gm: Scalar) -> Scalar;
    /// 轨道角动量 (m²/s)
    pub fn specific_angular_momentum(&self, gm: Scalar) -> Scalar;
    /// 轨道周期 (s)
    pub fn period(&self, gm: Scalar) -> Scalar;
    /// 近心点距离 (m)
    pub fn periapsis_distance(&self) -> Scalar;
    /// 远心点距离 (m)
    pub fn apoapsis_distance(&self) -> Scalar;
    /// 转换为位置/速度向量
    pub fn to_cartesian(&self, gm: Scalar) -> (Coord3D, [Scalar; 3]);
    /// 从位置/速度向量转换
    pub fn from_cartesian(pos: &Coord3D, vel: &[Scalar; 3], gm: Scalar) -> Self;
    /// 开普勒方程求解（偏近点角 → 真近点角）
    pub fn solve_kepler(&self, mean_anomaly: Scalar) -> Scalar;
}

/// 二体轨道传播
pub struct TwoBodyPropagator {
    pub gm: Scalar,                   // 中心天体引力参数
}

impl TwoBodyPropagator {
    pub fn new(gm: Scalar) -> Self;
    /// 解析传播一个时间步
    pub fn propagate(&self, elements: &KeplerianElements, dt: Scalar) -> KeplerianElements;
    /// 数值传播（考虑 J2 摄动）
    pub fn propagate_with_perturbation(&self, elements: &KeplerianElements, dt: Scalar, j2: Scalar) -> KeplerianElements;
}

/// N 体引力求解器
pub struct NBodySolver {
    pub bodies: Vec<CelestialBody>,
    pub softening: Scalar,            // 软化参数（防止奇点）
}

impl NBodySolver {
    pub fn new(bodies: Vec<CelestialBody>, softening: Scalar) -> Self;
    /// 计算每个天体上的加速度
    pub fn accelerations(&self) -> Vec<[Scalar; 3]>;
    /// 蛙跳积分一步
    pub fn leapfrog_step(&mut self, dt: Scalar);
    /// Hermite 积分一步（4阶）
    pub fn hermite_step(&mut self, dt: Scalar);
    /// 总能量（验证守恒）
    pub fn total_energy(&self) -> Scalar;
    /// 总角动量（验证守恒）
    pub fn total_angular_momentum(&self) -> [Scalar; 3];
}

/// 轨道摄动（J2 引起的进动）
pub fn j2_precession_rate(semi_major: Scalar, eccentricity: Scalar, inclination: Scalar, j2: Scalar, radius: Scalar, gm: Scalar) -> Scalar;
```

#### 3.2.4 `gravity.rs` — 万有引力

```rust
/// 两点间万有引力: F = G·m₁·m₂/r²
pub fn gravitational_force(m1: Scalar, m2: Scalar, distance: Scalar) -> Scalar;

/// 两点间引力加速度
pub fn gravitational_acceleration(gm: Scalar, position: &Coord3D) -> [Scalar; 3];

/// 多体引力加速度
pub fn nbody_accelerations(positions: &[Coord3D], masses: &[Scalar], softening: Scalar) -> Vec<[Scalar; 3]>;

/// 潮汐力计算
pub fn tidal_force(primary_mass: Scalar, primary_pos: &Coord3D, secondary_pos: &Coord3D, secondary_mass: Scalar) -> [Scalar; 3];

/// 希尔球半径（稳定轨道最大距离）
pub fn hill_sphere_radius(semi_major: Scalar, mass: Scalar, parent_mass: Scalar) -> Scalar;

/// 拉格朗日点 L1 距离
pub fn lagrange_l1_distance(semi_major: Scalar, mass_ratio: Scalar) -> Scalar;

/// 势能（N 体系统）
pub fn gravitational_potential_energy(positions: &[Coord3D], masses: &[Scalar]) -> Scalar;
```

#### 3.2.5 `cosmology.rs` — 宇宙学

```rust
/// 红移 z 对应的尺度因子 a = 1/(1+z)
pub fn scale_factor(redshift: Scalar) -> Scalar;

/// 哈勃参数 H(z)（ΛCDM 模型）
pub fn hubble_parameter(redshift: Scalar, h0: Scalar, omega_m: Scalar, omega_l: Scalar) -> Scalar;

/// 共动距离 (Mpc)
pub fn comoving_distance(redshift: Scalar, h0: Scalar, omega_m: Scalar, omega_l: Scalar) -> Scalar;

/// 光度距离 (Mpc)
pub fn luminosity_distance(redshift: Scalar, h0: Scalar, omega_m: Scalar, omega_l: Scalar) -> Scalar;

/// 宇宙年龄 (Gyr)
pub fn universe_age(h0: Scalar, omega_m: Scalar, omega_l: Scalar) -> Scalar;

/// 引力透镜——爱因斯坦角半径
pub fn einstein_radius(lens_mass: Scalar, d_l: Scalar, d_s: Scalar, d_ls: Scalar) -> Scalar;

/// 暗物质晕密度分布（NFW 轮廓）
pub fn nfw_profile(radius: Scalar, scale_radius: Scalar, rho0: Scalar) -> Scalar;
```

#### 3.2.6 `spacecraft.rs` — 航天轨道设计

```rust
/// 霍曼转移轨道 Δv 计算
pub fn hohmann_transfer_delta_v(
    r1: Scalar, r2: Scalar, gm: Scalar,
) -> (Scalar, Scalar);  // (Δv₁, Δv₂)

/// 双曲超速（行星借力）
pub fn gravity_assist_delta_v(
    v_inf_in: [Scalar; 3],
    planet_velocity: [Scalar; 3],
    turning_angle: Scalar,
) -> [Scalar; 3];

/// Lambert 问题求解（两点边界值）
pub fn lambert_solver(
    r1: &Coord3D, r2: &Coord3D,
    dt: Scalar, gm: Scalar, prograde: bool,
) -> Result<([Scalar; 3], [Scalar; 3]), String>;  // (v1, v2)

/// 发射窗口计算
pub fn launch_window(
    target_orbital_elements: &KeplerianElements,
    launch_latitude: Scalar,
    launch_longitude: Scalar,
    time_range: (Scalar, Scalar),
) -> Vec<Scalar>;

/// 航天器轨道保持 Δv 预算
pub fn station_keeping_budget(
    semi_major: Scalar, drag_perturbation: Scalar, duration: Scalar,
) -> Scalar;

/// 会合轨道
pub fn rendezvous_maneuver(
    chaser_oe: &KeplerianElements, target_oe: &KeplerianElements, gm: Scalar,
) -> Result<Vec<(Scalar, [Scalar; 3])>, String>;  // (time, Δv) 序列
```

#### 3.2.7 `analysis.rs` — 轨道分析

```rust
/// 轨道能量
pub fn orbital_energy(position: &Coord3D, velocity: &[Scalar; 3], gm: Scalar) -> Scalar;

/// 轨道角动量
pub fn orbital_angular_momentum(position: &Coord3D, velocity: &[Scalar; 3]) -> [Scalar; 3];

/// 轨道离心率向量
pub fn eccentricity_vector(position: &Coord3D, velocity: &[Scalar; 3], gm: Scalar) -> [Scalar; 3];

/// 碰撞概率（两物体轨道最近距离）
pub fn collision_probability(
    body1_pos: &Coord3D, body1_vel: &[Scalar; 3],
    body2_pos: &Coord3D, body2_vel: &[Scalar; 3],
    body1_radius: Scalar, body2_radius: Scalar,
) -> Scalar;

/// TLE 轨道寿命预测
pub fn orbital_lifetime(semi_major: Scalar, eccentricity: Scalar, area_mass_ratio: Scalar, solar_activity: Scalar) -> Scalar;

/// 可见窗口计算
pub fn visibility_window(
    observer_pos: &Coord3D, target_oe: &KeplerianElements,
    gm: Scalar, min_elevation: Scalar, time_range: (Scalar, Scalar),
) -> Vec<(Scalar, Scalar)>;
```

---

### 3.3 阶段 32 — 全物理场统一耦合总线（`src/coupling/`）

#### 3.3.1 `bus.rs` — 统一耦合总线

```rust
/// 物理场类型标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicsField {
    Structural,      // 结构力学场
    Thermal,         // 温度场
    Fluid,           // 流场
    Electromagnetic, // 电磁场
    Acoustic,        // 声场
    Optical,         // 光场
    Chemical,        // 浓度场
    Biological,      // 生物场
    Quantum,         // 量子场
    Gravitational,   // 引力场
    Custom(String),  // 自定义场
}

/// 耦合接口注册
#[derive(Debug, Clone)]
pub struct CouplingInterface {
    pub source_field: PhysicsField,
    pub target_field: PhysicsField,
    pub quantity_type: QuantityType,
    pub mapping: FieldMappingMethod,
    pub time_sync: TimeSyncMode,
}

/// 场量数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityType {
    Scalar,           // 标量场（温度、压力、浓度）
    Vector3,          // 三维向量场（位移、速度、力）
    Tensor6,          // 对称张量（应力、应变）
    Tensor3x3,        // 全张量
}

/// 时间同步模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSyncMode {
    LockStep,          // 锁步同步（所有场同一时间步）
    SubCycling,        // 子循环（细网格多步、粗网格一步）
    Interpolated,      // 插值同步（异步步长间插值）
    EventDriven,       // 事件驱动（耦合点在事件触发时同步）
}

/// 统一耦合总线
pub struct CouplingBus {
    pub interfaces: Vec<CouplingInterface>,
}

impl CouplingBus {
    pub fn new() -> Self;
    /// 注册耦合接口
    pub fn register_interface(&mut self, interface: CouplingInterface);
    /// 查找支持的耦合对
    pub fn find_interface(&self, source: PhysicsField, target: PhysicsField) -> Option<&CouplingInterface>;
    /// 执行一次耦合数据交换
    pub fn exchange(&self, source_data: &FieldData, interface: &CouplingInterface) -> Result<FieldData, String>;
}

/// 场数据容器
#[derive(Debug, Clone)]
pub struct FieldData {
    pub field_type: PhysicsField,
    pub quantity: QuantityType,
    pub points: Vec<Coord3D>,           // 数据点坐标
    pub values: Vec<Scalar>,            // 场量值（展平存储）
    pub time: Scalar,                   // 数据对应时间戳
    pub metadata: HashMap<String, Scalar>,
}
```

#### 3.3.2 `field_mapping.rs` — 场量映射

```rust
/// 场映射方法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMappingMethod {
    NearestNeighbor,    // 最近邻插值
    LinearInterp,       // 线性插值
    RadialBasis,        // 径向基函数插值
    InverseDistance,    // 反距离加权
    FiniteElementInterp, // 有限元形函数插值
    Conservative,       // 守恒映射（通量守恒）
}

/// 场映射器
pub struct FieldMapper {
    pub method: FieldMappingMethod,
}

impl FieldMapper {
    pub fn new(method: FieldMappingMethod) -> Self;
    /// 将源网格上的场数据映射到目标点
    pub fn map(
        &self,
        source_points: &[Coord3D],
        source_values: &[Scalar],
        target_points: &[Coord3D],
    ) -> Result<Vec<Scalar>, String>;
    /// 3D 反距离加权插值
    pub fn inverse_distance_weighted(
        source: &[Coord3D], values: &[Scalar], target: &[Coord3D], power: Scalar,
    ) -> Vec<Scalar>;
    /// 径向基函数插值
    pub fn radial_basis_interpolation(
        source: &[Coord3D], values: &[Scalar], target: &[Coord3D], rbf_type: RbfType,
    ) -> Result<Vec<Scalar>, String>;
}

pub enum RbfType {
    Gaussian(Scalar),        // exp(-(εr)²)
    Multiquadric(Scalar),    // √(1 + (εr)²)
    ThinPlateSpline,         // r²·ln(r)
}
```

#### 3.3.3 `cross_scale.rs` — 跨尺度耦合

```rust
/// 尺度层次
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScaleLevel {
    Nano,        // 10⁻⁹~10⁻⁶ m (分子/芯片)
    Micro,       // 10⁻⁶~10⁻³ m (细胞/器件)
    Milli,       // 10⁻³~1 m (PCB/器官)
    Meter,       // 1~10³ m (机械/人体)
    Kilo,        // 10³~10⁶ m (装备/建筑)
    Mega,        // 10⁶~10⁹ m (行星/地球)
    Giga,        // 10⁹~10¹² m (恒星/太阳系)
    Tera,        // >10¹² m (星系/宇宙)
}

/// 跨尺度耦合配置
pub struct CrossScaleCoupling {
    pub source_scale: ScaleLevel,
    pub target_scale: ScaleLevel,
    pub homogenization: bool,          // 是否均质化（细尺度→粗尺度）
    pub localization: bool,            // 是否局部化（粗尺度→细尺度边界条件）
}

/// 跨尺度数据桥接
pub struct ScaleBridge {
    pub couplings: Vec<CrossScaleCoupling>,
}

impl ScaleBridge {
    pub fn new() -> Self;
    /// 添加跨尺度耦合
    pub fn add_coupling(&mut self, coupling: CrossScaleCoupling);
    /// 细尺度→粗尺度：均质化/平均化
    pub fn upscale(&self, fine_data: &FieldData, target_points: &[Coord3D], scale_ratio: Scalar) -> Result<FieldData, String>;
    /// 粗尺度→细尺度：局部化/插值
    pub fn downscale(&self, coarse_data: &FieldData, target_points: &[Coord3D], scale_ratio: Scalar) -> Result<FieldData, String>;
}

/// 代表性体积单元 RVE（微观→宏观桥接）
pub struct RveHomogenization {
    pub rve_size: Scalar,              // RVE 尺寸 (m)
    pub micro_fields: Vec<FieldData>,  // 细观场数据
}

impl RveHomogenization {
    /// 体积平均
    pub fn volume_average(&self) -> FieldData;
    /// 计算等效宏观属性
    pub fn effective_properties(&self) -> HashMap<String, Scalar>;
}
```

#### 3.3.4 `convergence.rs` — 统一收敛控制

```rust
/// 耦合收敛准则
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConvergenceCriteria {
    pub absolute_tolerance: Scalar,    // 绝对容差
    pub relative_tolerance: Scalar,    // 相对容差
    pub max_iterations: usize,         // 最大耦合迭代次数
    pub relaxation_factor: Scalar,     // 松弛因子 (0~1)
}

impl Default for ConvergenceCriteria {
    fn default() -> Self {
        Self {
            absolute_tolerance: 1e-8,
            relative_tolerance: 1e-6,
            max_iterations: 50,
            relaxation_factor: 0.5,
        }
    }
}

/// 耦合求解调度器
pub struct CouplingScheduler {
    pub criteria: ConvergenceCriteria,
    pub interfaces: Vec<CouplingInterface>,
}

impl CouplingScheduler {
    pub fn new(criteria: ConvergenceCriteria) -> Self;
    /// 固定点迭代求解耦合系统
    pub fn fixed_point_iteration(
        &self,
        initial_data: &[FieldData],
        compute_field: &dyn Fn(&[FieldData], PhysicsField) -> Result<FieldData, String>,
    ) -> Result<Vec<FieldData>, String>;
    /// 高斯-赛德尔耦合迭代
    pub fn gauss_seidel_coupling(
        &self,
        fields: &mut [FieldData],
        compute_fn: &dyn Fn(&mut FieldData) -> Result<(), String>,
    ) -> Result<(), String>;
    /// 雅可比耦合迭代（可并行）
    pub fn jacobi_coupling(
        &self,
        fields: &[FieldData],
        compute_fn: &dyn Fn(&FieldData) -> Result<FieldData, String>,
    ) -> Result<Vec<FieldData>, String>;
    /// 收敛检测
    pub fn check_convergence(&self, delta: &[Scalar]) -> bool;
}

/// 时间同步管理器
pub struct TimeSyncManager {
    pub time_step: Scalar,
    pub sync_points: Vec<Scalar>,        // 同步时间点
    pub current_index: usize,
}

impl TimeSyncManager {
    pub fn new(time_step: Scalar, sync_interval: usize, total_time: Scalar) -> Self;
    /// 当前时间
    pub fn current_time(&self) -> Scalar;
    /// 是否需要同步
    pub fn need_sync(&self) -> bool;
    /// 推进到下一步
    pub fn advance(&mut self) -> bool;
}
```

---

### 3.4 阶段 33 — 工业高级功能与后处理可视化（`src/postproc/`）

#### 3.4.1 `recorder.rs` — 数据记录与回放

```rust
/// 数据记录配置
pub struct RecorderConfig {
    pub max_samples: usize,            // 最大采样点数
    pub sampling_interval: Scalar,     // 采样间隔 (s)
    pub record_signals: Vec<String>,   // 要记录的信号名称
    pub enable_streaming: bool,        // 是否启用流式写入磁盘
    pub output_path: Option<String>,   // 磁盘输出路径
}

/// 数据记录器
pub struct DataRecorder {
    pub config: RecorderConfig,
    pub time_stamps: Vec<Scalar>,
    pub recorded_data: HashMap<String, Vec<Scalar>>,
    pub current_count: usize,
}

impl DataRecorder {
    pub fn new(config: RecorderConfig) -> Self;
    /// 记录一个时间点的数据
    pub fn record(&mut self, time: Scalar, signals: &HashMap<String, Scalar>);
    /// 将数据写入磁盘
    pub fn flush_to_disk(&self) -> Result<(), String>;
    /// 获取记录的时间序列
    pub fn get_timeseries(&self, signal_name: &str) -> Option<&[Scalar]>;
    /// 获取所有记录信号名
    pub fn signal_names(&self) -> Vec<&String>;
    /// 清空内存缓存
    pub fn clear(&mut self);
    /// 导出为 CSV
    pub fn export_csv(&self, filepath: &str) -> Result<(), String>;
}

/// 数据回放
pub struct DataReplayer {
    pub data: HashMap<String, Vec<Scalar>>,
    pub time: Vec<Scalar>,
    pub current_index: usize,
}

impl DataReplayer {
    pub fn new(data: HashMap<String, Vec<Scalar>>, time: Vec<Scalar>) -> Self;
    /// 从文件加载
    pub fn from_csv(filepath: &str) -> Result<Self, String>;
    /// 获取当前时间点所有信号值
    pub fn current_values(&self) -> HashMap<String, Scalar>;
    /// 推进到下一个时间点
    pub fn advance(&mut self) -> bool;
    /// 重置到起始
    pub fn reset(&mut self);
}

/// 离线分析上下文
pub struct OfflineAnalysis {
    pub recorder: DataRecorder,
}

impl OfflineAnalysis {
    pub fn new(recorder: DataRecorder) -> Self;
    /// 计算均方根 RMS
    pub fn rms(&self, signal: &str) -> Option<Scalar>;
    /// 计算均值
    pub fn mean(&self, signal: &str) -> Option<Scalar>;
    /// 计算最大值/最小值
    pub fn min_max(&self, signal: &str) -> Option<(Scalar, Scalar)>;
    /// FFT 频谱分析
    pub fn fft_analysis(&self, signal: &str) -> Option<(Vec<Scalar>, Vec<Scalar>)>; // (freq, magnitude)
}
```

#### 3.4.2 `visualization.rs` — 图表与可视化

```rust
/// 曲线数据
#[derive(Debug, Clone)]
pub struct CurveData {
    pub x_values: Vec<Scalar>,
    pub y_values: Vec<Scalar>,
    pub label: String,
    pub color: Option<String>,
}

/// 图表类型
pub enum ChartType {
    Line,              // 折线图
    Scatter,           // 散点图
    Bar,               // 柱状图
    Histogram,         // 直方图
    Contour,           // 等值线图
    Surface3D,         // 3D 曲面图
    VectorField,       // 矢量场图
}

/// 图表生成器
pub struct ChartGenerator {
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub curves: Vec<CurveData>,
    pub chart_type: ChartType,
}

impl ChartGenerator {
    pub fn new(title: &str, x_label: &str, y_label: &str) -> Self;
    pub fn add_curve(&mut self, curve: CurveData);
    /// 生成 SVG 格式图表
    pub fn to_svg(&self) -> Result<String, String>;
    /// 生成 JSON 格式图表数据（供前端渲染）
    pub fn to_json(&self) -> Result<String, String>;
}

/// 云图生成
pub struct ContourGenerator {
    pub x_grid: Vec<Scalar>,
    pub y_grid: Vec<Scalar>,
    pub z_values: Vec<Vec<Scalar>>,
    pub levels: usize,
}

impl ContourGenerator {
    pub fn new(x: Vec<Scalar>, y: Vec<Scalar>, z: Vec<Vec<Scalar>>) -> Self;
    /// 生成等值线 (contour levels)
    pub fn contours(&self, num_levels: usize) -> Vec<(Scalar, Vec<[Scalar; 2]>)>;
    /// 生成彩色云图数据
    pub fn color_map(&self) -> Vec<Vec<(Scalar, Scalar, Scalar)>>;
}

/// 3D 矢量场可视化
pub struct VectorFieldVisualization {
    pub positions: Vec<Coord3D>,
    pub vectors: Vec<[Scalar; 3]>,
    pub scale: Scalar,
}

impl VectorFieldVisualization {
    pub fn new(positions: Vec<Coord3D>, vectors: Vec<[Scalar; 3]>) -> Self;
    /// 生成箭头数据（供外部渲染）
    pub fn arrow_data(&self) -> Vec<([Scalar; 3], [Scalar; 3])>;
}
```

#### 3.4.3 `reporting.rs` — 报告生成

```rust
/// 报告章节
pub struct ReportSection {
    pub title: String,
    pub content: String,
    pub tables: Vec<ReportTable>,
    pub charts: Vec<ChartGenerator>,
}

/// 表格数据
pub struct ReportTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub caption: String,
}

/// 仿真报告
pub struct SimulationReport {
    pub title: String,
    pub description: String,
    pub sections: Vec<ReportSection>,
    pub generated_at: String,
}

impl SimulationReport {
    pub fn new(title: &str, description: &str) -> Self;
    pub fn add_section(&mut self, section: ReportSection);
    /// 导出为 Markdown
    pub fn to_markdown(&self) -> String;
    /// 导出为 HTML
    pub fn to_html(&self) -> String;
    /// 导出为 JSON
    pub fn to_json(&self) -> String;
}

/// 数据导出格式
pub enum ExportFormat {
    Csv,
    Json,
    Toml,
    Hdf5,
    Vtk,  // ParaView
}

/// 数据导出器
pub struct DataExporter;

impl DataExporter {
    pub fn export(recorder: &DataRecorder, format: ExportFormat, path: &str) -> Result<(), String>;
}
```

#### 3.4.4 `batch.rs` — 批量任务

```rust
/// 参数扫描任务
pub struct ParameterSweep {
    pub parameter_name: String,
    pub values: Vec<Scalar>,
    pub diagram_template: String,       // 框图模板路径
    pub output_dir: String,
}

impl ParameterSweep {
    pub fn new(name: &str, values: Vec<Scalar>, template: &str, output: &str) -> Self;
    /// 执行参数扫描
    pub fn run(&self) -> Result<Vec<String>, String>;
}

/// 批量仿真管理器
pub struct BatchSimManager {
    pub tasks: Vec<BatchTask>,
    pub max_parallel: usize,
}

pub struct BatchTask {
    pub id: String,
    pub config_path: String,
    pub diagram_path: String,
    pub output_path: String,
    pub status: BatchTaskStatus,
}

pub enum BatchTaskStatus {
    Pending,
    Running,
    Completed(Vec<String>),
    Failed(String),
}

impl BatchSimManager {
    pub fn new(max_parallel: usize) -> Self;
    pub fn add_task(&mut self, task: BatchTask);
    /// 并行执行所有任务
    pub fn run_all(&mut self) -> Result<(), String>;
    /// 获取所有结果
    pub fn results(&self) -> Vec<(&str, &BatchTaskStatus)>;
}

/// 自动化优化迭代
pub struct OptimizationLoop {
    pub objective_fn: String,            // 目标函数信号
    pub design_params: Vec<DesignParam>,
    pub max_iterations: usize,
}

pub struct DesignParam {
    pub name: String,
    pub min: Scalar,
    pub max: Scalar,
}

impl OptimizationLoop {
    pub fn new(objective: &str, max_iter: usize) -> Self;
    pub fn add_param(&mut self, param: DesignParam);
    /// 执行优化（网格搜索/随机搜索）
    pub fn optimize_grid(&self) -> Result<(Vec<Scalar>, Scalar), String>;
}
```

#### 3.4.5 `hilsupport.rs` — 硬件在环

```rust
/// HIL 配置
pub struct HilConfig {
    pub hardware_interface: String,      // 硬件接口类型
    pub sample_rate: Scalar,             // 采样率 (Hz)
    pub io_channels: HilIoChannels,
    pub real_time_priority: bool,        // 实时优先级
}

pub struct HilIoChannels {
    pub analog_inputs: Vec<String>,
    pub analog_outputs: Vec<String>,
    pub digital_inputs: Vec<String>,
    pub digital_outputs: Vec<String>,
}

/// HIL 运行器
pub struct HilRunner {
    pub config: HilConfig,
    pub engine: Option<crate::runtime::engine::SimEngine>,
    pub is_running: bool,
}

impl HilRunner {
    pub fn new(config: HilConfig) -> Self;
    /// 初始化硬件连接
    pub fn initialize(&mut self) -> Result<(), String>;
    /// 启动 HIL 仿真
    pub fn start(&mut self, engine: crate::runtime::engine::SimEngine) -> Result<(), String>;
    /// HIL 单步（读硬件输入→仿真一步→写硬件输出）
    pub fn step(&mut self) -> Result<(), String>;
    /// 停止 HIL
    pub fn stop(&mut self);
}
```

---

### 3.5 阶段 34 — 跨平台、脚本生态与扩展系统（`src/bindings/`）

#### 3.5.1 Python 绑定（`bindings/python/`）

```rust
/// Python 绑定接口 —— 仿真控制
pub mod py_simulation {
    /// 创建并运行仿真
    pub fn run_simulation(diagram_json: &str, config_json: &str) -> Result<String, String>;
    /// 设置模块参数
    pub fn set_block_parameter(block_id: &str, param_name: &str, value: f64) -> Result<(), String>;
    /// 读取信号值
    pub fn read_signal(block_id: &str, port_name: &str) -> Result<f64, String>;
    /// 暂停/继续仿真
    pub fn pause_simulation() -> Result<(), String>;
    pub fn resume_simulation() -> Result<(), String>;
    /// 获取仿真状态
    pub fn get_simulation_status() -> Result<String, String>;
}

/// Python 绑定接口 —— 模块构建
pub mod py_blocks {
    /// 从 Python 创建自定义 Block
    pub fn register_custom_block(block_type: &str, block_json: &str) -> Result<(), String>;
    /// 连接模块
    pub fn connect_blocks(src_block: &str, src_port: &str, dst_block: &str, dst_port: &str) -> Result<(), String>;
}

/// Python 绑定接口 —— 数据访问
pub mod py_data {
    /// 查询资料库
    pub fn query_library(category: &str, query: &str) -> Result<String, String>;
    /// 读取仿真结果
    pub fn get_result_data(signal_name: &str) -> Result<String, String>;
}
```

#### 3.5.2 插件系统（`bindings/plugins/`）

```rust
/// 插件特性
#[derive(Debug, Clone)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub api_version: String,
    pub entry_point: String,            // 插件入口文件
}

/// 插件实例
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &PluginManifest;
    /// 初始化插件
    fn initialize(&mut self) -> Result<(), String>;
    /// 注册自定义 Block 类型
    fn register_blocks(&self, registry: &mut BlockRegistry) -> Result<(), String>;
    /// 注册自定义求解器
    fn register_solvers(&self, registry: &mut SolverRegistry) -> Result<(), String>;
    /// 注册自定义后处理
    fn register_postprocessors(&self, registry: &mut PostProcessorRegistry) -> Result<(), String>;
    /// 清理
    fn shutdown(&mut self) -> Result<(), String>;
}

/// 插件管理器
pub struct PluginManager {
    pub plugins: Vec<Box<dyn Plugin>>,
    pub block_registry: BlockRegistry,
    pub solver_registry: SolverRegistry,
    pub postprocessor_registry: PostProcessorRegistry,
}

impl PluginManager {
    pub fn new() -> Self;
    /// 从目录加载所有插件
    pub fn load_from_directory(&mut self, path: &str) -> Result<Vec<String>, String>;
    /// 加载单个插件
    pub fn load_plugin(&mut self, manifest_path: &str) -> Result<(), String>;
    /// 初始化所有插件
    pub fn initialize_all(&mut self) -> Result<(), String>;
    /// 获取插件列表
    pub fn list_plugins(&self) -> Vec<&PluginManifest>;
}

/// 模块注册器
pub struct BlockRegistry {
    pub block_types: HashMap<String, Box<dyn Fn() -> Box<dyn Block>>>,
}

impl BlockRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, type_name: &str, factory: Box<dyn Fn() -> Box<dyn Block>>);
    pub fn create_block(&self, type_name: &str) -> Option<Box<dyn Block>>;
}

/// 求解器注册器
pub struct SolverRegistry {
    pub solvers: HashMap<String, Box<dyn Fn() -> Box<dyn OdeSolver>>>,
}

/// 后处理注册器
pub struct PostProcessorRegistry {
    pub processors: HashMap<String, Box<dyn Fn() -> Box<dyn PostProcessor>>>,
}

pub trait PostProcessor: Send + Sync {
    fn name(&self) -> &str;
    fn process(&self, data: &DataRecorder) -> Result<SimulationReport, String>;
}
```

#### 3.5.3 CAD/CAE 数据接口（`bindings/data_io/`）

```rust
/// STEP 文件解析/导出
pub mod step_io {
    /// 从 STEP 文件导入几何
    pub fn import_step(filepath: &str) -> Result<StepModel, String>;
    /// 导出为 STEP 文件
    pub fn export_step(model: &StepModel, filepath: &str) -> Result<(), String>;

    /// STEP 模型数据
    pub struct StepModel {
        pub entities: Vec<StepEntity>,
        pub unit: String,
    }

    pub enum StepEntity {
        Point(Coord3D),
        Line(Coord3D, Coord3D),
        Circle(Coord3D, Scalar, Coord3D),
        BSplineCurve { control_points: Vec<Coord3D>, degree: usize },
        Face { outer_bound: Vec<Coord3D>, inner_bounds: Vec<Vec<Coord3D>> },
        Shell { faces: Vec<usize> },
    }
}

/// STL 文件解析/导出
pub mod stl_io {
    /// 从 STL 文件导入网格
    pub fn import_stl(filepath: &str) -> Result<StlMesh, String>;
    /// 导出为 STL 文件
    pub fn export_stl(mesh: &StlMesh, filepath: &str) -> Result<(), String>;

    /// STL 网格
    pub struct StlMesh {
        pub triangles: Vec<StlTriangle>,
        pub unit: String,
    }

    pub struct StlTriangle {
        pub normal: [Scalar; 3],
        pub v1: Coord3D,
        pub v2: Coord3D,
        pub v3: Coord3D,
    }
}

/// 网格文件格式
pub mod mesh_io {
    pub enum MeshFormat {
        Vtk,          // VTK Legacy
        Vtu,          // VTK XML Unstructured
        Gmsh,         // Gmsh MSH
        Abaqus,       // Abaqus INP
        Ansys,        // Ansys CDB
    }

    pub struct MeshData {
        pub nodes: Vec<Coord3D>,
        pub elements: Vec<MeshElement>,
        pub node_sets: HashMap<String, Vec<usize>>,
        pub element_sets: HashMap<String, Vec<usize>>,
    }

    pub enum MeshElement {
        Line { connectivity: [usize; 2] },
        Triangle { connectivity: [usize; 3] },
        Quadrilateral { connectivity: [usize; 4] },
        Tetrahedron { connectivity: [usize; 4] },
        Hexahedron { connectivity: [usize; 8] },
        Prism { connectivity: [usize; 6] },
    }

    pub fn import_mesh(filepath: &str, format: MeshFormat) -> Result<MeshData, String>;
    pub fn export_mesh(mesh: &MeshData, format: MeshFormat, filepath: &str) -> Result<(), String>;
}
```

#### 3.5.4 跨平台与部署（`bindings/platform/`）

```rust
/// 平台抽象
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Linux,
    MacOS,
}

/// 当前平台检测
pub fn current_platform() -> Platform;

/// 文件系统路径标准化
pub fn normalize_path(path: &str) -> String;

/// 动态库加载
pub struct DynamicLibrary {
    pub path: String,
    handle: Option<*mut std::ffi::c_void>,
}

impl DynamicLibrary {
    pub fn new(path: &str) -> Result<Self, String>;
    pub fn load_symbol<T>(&self, name: &str) -> Result<*mut T, String>;
}

/// 云端部署配置
pub struct CloudConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub max_concurrent_jobs: usize,
    pub storage_path: String,
}

/// 分布式计算任务
pub struct DistributedTask {
    pub task_id: String,
    pub diagram_json: String,
    pub config_json: String,
    pub partition: TaskPartition,
}

pub enum TaskPartition {
    /// 参数扫描分片
    ParameterRange { param: String, values: Vec<Scalar> },
    /// 空间域分解
    SpatialDomain { x_range: (Scalar, Scalar), y_range: (Scalar, Scalar) },
    /// 时间域分解
    TimeDomain { t_start: Scalar, t_end: Scalar },
}

/// 分布式运行器
pub struct DistributedRunner {
    pub config: CloudConfig,
    pub tasks: Vec<DistributedTask>,
}

impl DistributedRunner {
    pub fn new(config: CloudConfig) -> Self;
    /// 将仿真分解为分布式任务
    pub fn decompose_simulation(&mut self, diagram_json: &str, config_json: &str, strategy: TaskPartition);
    /// 提交所有任务到云端
    pub fn submit_all(&self) -> Result<Vec<String>, String>;
    /// 收集结果
    pub fn collect_results(&self, task_ids: &[String]) -> Result<Vec<String>, String>;
}
```

---

## 4. 与已有系统的集成

### 4.1 核心系统集成

| 机制 | 已有模块 | 集成方式 |
|------|---------|---------|
| Block trait | `core::block::Block` | 量子电路、天体求解器可选择实现 Block 接口；纯计算函数保持独立 |
| 坐标系 | `core::coord::Coord3D` | 天体位置、轨道、后处理可视化点坐标使用 Coord3D |
| 数值求解器 | `runtime::solver::OdeSolver` | 薛定谔方程求解、林德布拉德方程、N体求解集成 OdeSolver |
| 事件系统 | `runtime::event::ZeroCrossingDetector` | 量子测量触发、轨道交会检测使用过零检测 |
| 工作流 | `runtime::workflow::WorkflowDAG` | 多物理场耦合、批量任务编排使用 WorkflowDAG |
| 调度引擎 | `runtime::scheduler` | 耦合总线时间同步使用 scheduler |
| 代数环 | `runtime::algebraic` | 耦合迭代收敛使用 AlgebraicLoopDetector |
| 资料库 | `db` | 天体数据、量子参数通过 db 系统加载 |

### 4.2 数据依赖关系

```
阶段 30 (quantum) ──── 依赖 ─── 阶段 13 (tcad) — 半导体量子效应
                           ─── 阶段 22 (emag) — 量子-电磁耦合

阶段 31 (astrophysics) ─ 依赖 ─── 阶段 27 (fluid) — 天体等离子体
                           ─── 阶段 26 (thermal) — 恒星热力学
                           ─── db — 天体库

阶段 32 (coupling) ──── 依赖 ─── 所有领域模块 — 耦合接口
                           ─── 阶段 10 (coord) — 坐标映射
                           ─── 阶段 3 (solver) — 耦合求解

阶段 33 (postproc) ──── 依赖 ─── runtime::engine — 仿真结果
                           ─── runtime::state — 状态快照

阶段 34 (bindings) ──── 依赖 ─── 所有模块 — 外部接口暴露
                           ─── core::block — 插件 Block 注册
```

### 4.3 `src/domains/mod.rs` 更新

```rust
// 在已有模块列表末尾添加：
pub mod quantum;        // Phase 30: Quantum Physics & Quantum Computing
pub mod astrophysics;   // Phase 31: Astrophysics & Orbital Simulation
```

### 4.4 `src/lib.rs` 重导出更新

```rust
// Re-export quantum key types
pub use domains::quantum::{
    QuantumState, DensityMatrix, QuantumCircuit, GateOperation,
    GateType, SingleQubitGate, MultiQubitGate,
    SchrodingerSolver, LindbladSolver,
    VqeSolver, QaoaSolver,
};
pub use domains::quantum::analysis::{
    fidelity_density, trace_distance, entanglement_entropy,
};

// Re-export astrophysics key types
pub use domains::astrophysics::{
    CelestialBody, KeplerianElements, TwoBodyPropagator, NBodySolver,
};
pub use domains::astrophysics::analysis::{
    orbital_energy, orbital_angular_momentum, collision_probability,
};

// Re-export coupling bus key types
pub use coupling::{
    CouplingBus, CouplingInterface, FieldData, PhysicsField,
    FieldMapper, ScaleBridge, CouplingScheduler,
};

// Re-export postproc key types
pub use postproc::{
    DataRecorder, DataReplayer, OfflineAnalysis,
    ChartGenerator, ContourGenerator, SimulationReport,
    BatchSimManager, HilRunner,
};

// Re-export bindings key types
pub use bindings::plugins::{
    Plugin, PluginManifest, PluginManager, BlockRegistry,
};
```

---

## 5. 测试要求

### 阶段 30 量子计算（~40 个测试）
- 量子态创建与归一化
- 单/多量子比特门矩阵正确性
- 量子电路模拟（Bell 态、GHZ 态）
- 薛定谔方程 Crank-Nicolson 演化保真度
- 林德布拉德方程退相干
- VQE 基态能量计算（H₂ 分子）
- 边缘情况：单量子比特、零哈密顿量、完全退相干

### 阶段 31 天体物理（~35 个测试）
- 天体预设参数（太阳、地球、月球）
- 开普勒轨道根数与笛卡尔坐标互转
- 二体轨道周期与能量守恒
- N体蛙跳积分能量/角动量守恒
- 霍曼转移 Δv 计算
- 引力透镜爱因斯坦半径
- 边缘情况：圆轨道、零质量、高离心率

### 阶段 32 耦合总线（~25 个测试）
- 耦合接口注册与查找
- 场数据反距离加权插值
- 跨尺度上下采样
- 固定点耦合迭代收敛
- 松弛因子对收敛速度影响
- 边缘情况：空场数据、零容差、单场耦合

### 阶段 33 后处理（~30 个测试）
- 数据记录与时间序列
- CSV 导入/导出
- 离线分析（RMS、均值、FFT）
- 图表生成（SVG/JSON 格式）
- 参数扫描批量执行
- 边缘情况：空记录、单点记录、零信号

### 阶段 34 脚本生态（~20 个测试）
- Python 绑定 API 调用
- 插件清单解析与加载
- STL 文件三角面片解析
- STEP 实体导入
- 平台检测
- 边缘情况：空插件目录、无效文件格式

### 全局集成测试（~15 个）
- 量子-电磁耦合（量子点 EM 场）
- 天体-流体耦合（行星大气 CFD）
- 热-结构-流体全耦合
- 跨尺度耦合（纳米材料→宏观结构）
- 批量参数扫描 → 后处理报告全流程
- Python 脚本驱动仿真全流程

---

## 6. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| **量子态维度指数增长** | 超过 20+ qubit 状态向量内存爆炸 | 支持稀疏表示、张量网络（MPS）备用方案 |
| **N体计算 O(n²) 复杂度** | 大规模天体模拟性能差 | Barnes-Hut 树状近似、GPU 加速预留接口 |
| **耦合迭代发散** | 多物理场求解不收敛 | Aitken 加速、Anderson 加速、自适应松弛 |
| **后处理内存爆炸** | 长时间仿真数据量过大 | 流式写入磁盘、自动降采样、环形缓冲区 |
| **Python 绑定性能开销** | 高频调用帧率下降 | 批量 API、零拷贝数据视图、异步调用 |
| **插件 ABI 兼容性** | 跨版本插件崩溃 | 严格的 API 版本检查、沙箱隔离 |
| **跨平台路径差异** | Windows/Linux 路径错误 | 标准化路径处理、全面 CI 矩阵 |
| **与 blueprint 陷阱冲突** (trap.md) | 浮点精度、内存溢出、代数环 | 所有模块遵守 trap.md 避坑规则 |

---

## 7. 实现顺序

阶段 30~34 按以下顺序推进，每个阶段完成后经 cargo build + clippy + test 验证：

1. **`domains/quantum/`** — 量子物理与量子计算（8 个文件 + ~40 个测试）
2. **`domains/astrophysics/`** — 天体物理与轨道仿真（7 个文件 + ~35 个测试）
3. **`coupling/`** — 更新耦合总线（4 个新增文件 + ~25 个测试）
4. **`postproc/`** — 更新后处理（5 个新增文件 + ~30 个测试）
5. **`bindings/`** — 更新绑定扩展（多个子模块 + ~20 个测试）
6. **`domains/mod.rs` + `lib.rs`** — 更新模块注册和重导出
7. **全局集成测试** — 跨阶段耦合验证（~15 个测试）
8. **全项目清理** — clippy、dead code、覆盖率
