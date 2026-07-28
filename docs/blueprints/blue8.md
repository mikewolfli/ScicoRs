# BLUE8 — 阶段 16 & 17：分子动力学与生物分子仿真 + 细胞培养与组织生长仿真

## 1. 概述

阶段 16~17 在已完成的内核（阶段 1~15）之上构建两个生物医学领域的仿真能力：

- **阶段 16**：分子动力学与生物分子仿真 — 蛋白质折叠、分子对接、DNA/RNA 结构、分子力场、动力学模拟
- **阶段 17**：细胞培养与组织生长仿真 — 细胞增殖/凋亡、营养物质扩散、pH/溶氧/温度影响、群落生长形态、生物反应器动态

两个阶段共享生物分子库（`LibraryCategory::Biomolecule`）和化学品库（`LibraryCategory::Chemical`），并与已有的坐标系统、量纲系统、求解器、调度引擎完全集成。

**状态目标：** 100% — 编译通过、零 clippy 警告、全部测试通过、无占位代码。

---

## 2. 模块架构

```
src/
  domains/
    mod.rs              — 更新：暴露 molbio + cellbio 子模块
    molbio/             — [新增] 阶段 16：分子动力学与生物分子仿真
      mod.rs            — 模块接口
      physics.rs        — 分子物理常量、原子量、键参数
      forcefield.rs     — 分子力场（键伸缩、键角弯曲、二面角扭转、范德华、静电）
      molecule.rs       — 分子结构（原子、键、残基）、PDB 数据模型
      dynamics.rs       — 分子动力学引擎（Verlet 积分、温度/压力控制、能量最小化）
      docking.rs        — 分子对接评分函数（简化）
      analysis.rs       — 分子轨迹分析（RMSD、回转半径、能量追踪）
    cellbio/            — [新增] 阶段 17：细胞培养与组织生长仿真
      mod.rs            — 模块接口
      physics.rs        — 生物物理常量（扩散系数、生长速率、代谢参数）
      cell_model.rs     — 细胞模型（细胞状态、分裂、凋亡、迁移、贴壁）
      media.rs          — 培养基模型（营养物质、pH、溶氧、温度、渗透压）
      growth.rs         — 群落/组织生长形态仿真（格子模型 + 反应-扩散耦合）
      bioreactor.rs     — 生物反应器动态模型（搅拌、补料、收获）
      analysis.rs       — 数据分析（细胞密度、活性、代谢速率）
```

---

## 3. 详细规格

### 3.1 阶段 16 — 分子动力学与生物分子仿真（`src/domains/molbio/`）

#### 3.1.1 `physics.rs` — 分子物理常量

```rust
/// 阿伏伽德罗常数 (mol⁻¹)
pub const AVOGADRO: Scalar = 6.02214076e23;

/// 玻尔兹曼常数 (J/K)
pub const KB: Scalar = 1.380649e-23;

/// 真空介电常数 (C²/(J·m))
pub const EPSILON_0: Scalar = 8.854187817e-12;

/// 基本电荷 (C)
pub const QE: Scalar = 1.602176634e-19;

/// 原子质量单位→kg 转换因子
pub const AMU_TO_KG: Scalar = 1.66053906660e-27;

/// 标准室温 (K)
pub const T_300K: Scalar = 300.0;

/// 标准大气压 (Pa)
pub const ATM_TO_PA: Scalar = 101325.0;

/// 1 kcal/mol → kJ/mol
pub const KCAL_TO_KJ: Scalar = 4.184;

/// 1 Å = 1e-10 m
pub const ANGSTROM: Scalar = 1e-10;
```

**ElementMass** — 常见原子质量表（amu）：
```rust
pub fn element_mass(symbol: &str) -> Option<Scalar>
// H=1.008, C=12.011, N=14.007, O=15.999, S=32.065, P=30.974, ...
```

**BondParameters** — 标准键参数查询（平衡键长、力常数）：
```rust
pub fn bond_parameters(type1: &str, type2: &str) -> Option<(Scalar, Scalar)>
// returns (b0 in Å, kb in kcal/(mol·Å²))
```

#### 3.1.2 `forcefield.rs` — 分子力场

**分子力场总能量**：`E_total = E_bond + E_angle + E_dihedral + E_vdw + E_elec`

**HarmonicBond** — 谐振子键伸缩势能：
```rust
pub struct HarmonicBond {
    pub k: Scalar,      // 力常数 (kcal/(mol·Å²))
    pub b0: Scalar,     // 平衡键长 (Å)
}

impl HarmonicBond {
    /// E = k * (r - b0)²
    pub fn energy(&self, r: Scalar) -> Scalar;
    /// dE/dr = 2 * k * (r - b0)
    pub fn force(&self, r: Scalar) -> Scalar;
}
```

**HarmonicAngle** — 谐振子键角弯曲势能：
```rust
pub struct HarmonicAngle {
    pub k: Scalar,      // 力常数 (kcal/(mol·rad²))
    pub theta0: Scalar, // 平衡键角 (rad)
}

impl HarmonicAngle {
    /// E = k * (θ - θ₀)²
    pub fn energy(&self, theta: Scalar) -> Scalar;
    pub fn force(&self, theta: Scalar) -> Scalar;
}
```

**PeriodicDihedral** — 周期性二面角扭转势能：
```rust
pub struct PeriodicDihedral {
    pub vn: Scalar,     // 势垒高度 (kcal/mol)
    pub n: i32,         // 周期性
    pub gamma: Scalar,  // 相位 (rad)
}

impl PeriodicDihedral {
    /// E = Vn * (1 + cos(n·φ - γ))
    pub fn energy(&self, phi: Scalar) -> Scalar;
    pub fn force(&self, phi: Scalar) -> Scalar;
}
```

**LennardJones** — Lennard-Jones 范德华势能：
```rust
pub struct LennardJones {
    pub sigma: Scalar,  // 零势能距离 (Å)
    pub epsilon: Scalar,// 势阱深度 (kcal/mol)
}

impl LennardJones {
    /// E = 4·ε·[(σ/r)¹² - (σ/r)⁶]
    pub fn energy(&self, r: Scalar) -> Scalar;
    /// dE/dr
    pub fn force(&self, r: Scalar) -> Scalar;
    /// 结合规则：ε_ij = sqrt(ε_i·ε_j), σ_ij = (σ_i + σ_j)/2
    pub fn combine_lorentz_berthelot(&self, other: &LennardJones) -> LennardJones;
}
```

**CoulombPotential** — 静电相互作用：
```rust
pub struct CoulombPotential {
    pub epsilon_r: Scalar, // 相对介电常数
}

impl CoulombPotential {
    /// E = (1/(4·π·ε₀·εr)) · (qi·qj/r)
    pub fn energy(&self, qi: Scalar, qj: Scalar, r: Scalar) -> Scalar;
    pub fn force(&self, qi: Scalar, qj: Scalar, r: Scalar) -> Scalar;
}
```

**ForceField** — 完整力场聚合：
```rust
pub struct ForceField {
    pub bonds: Vec<(usize, usize, HarmonicBond)>,   // (atom_i, atom_j, params)
    pub angles: Vec<(usize, usize, usize, HarmonicAngle)>, // (i, j, k, params)
    pub dihedrals: Vec<(usize, usize, usize, usize, PeriodicDihedral)>,
    pub lj_params: Vec<(usize, LennardJones)>,      // (atom_idx, params)
    pub charges: Vec<(usize, Scalar)>,               // (atom_idx, charge in e)
    pub coulomb: CoulombPotential,
}

impl ForceField {
    pub fn new() -> Self;
    pub fn add_bond(&mut self, i: usize, j: usize, bond: HarmonicBond);
    pub fn add_angle(&mut self, i: usize, j: usize, k: usize, angle: HarmonicAngle);
    pub fn add_dihedral(&mut self, i: usize, j: usize, k: usize, l: usize, dihedral: PeriodicDihedral);
    pub fn add_lj(&mut self, atom: usize, lj: LennardJones);
    pub fn add_charge(&mut self, atom: usize, charge: Scalar);

    /// 计算给定坐标下总能量
    pub fn total_energy(&self, coords: &[Vec3]) -> Scalar;

    /// 计算所有原子上的力 (-dE/dr)
    pub fn compute_forces(&self, coords: &[Vec3]) -> Vec<Vec3>;
}
```

**Vec3** — 辅助三维向量（与 `core/coord` 兼容）：
```rust
pub struct Vec3 {
    pub x: Scalar,
    pub y: Scalar,
    pub z: Scalar,
}

impl Vec3 {
    pub fn new(x: Scalar, y: Scalar, z: Scalar) -> Self;
    pub fn distance(&self, other: &Vec3) -> Scalar;
    pub fn dot(&self, other: &Vec3) -> Scalar;
    pub fn cross(&self, other: &Vec3) -> Vec3;
    pub fn norm(&self) -> Scalar;
    pub fn normalized(&self) -> Vec3;
    pub fn subtract(&self, other: &Vec3) -> Vec3;
}
```

#### 3.1.3 `molecule.rs` — 分子结构与 PDB 数据模型

**Atom** — 单个原子：
```rust
pub struct Atom {
    pub serial: u32,
    pub name: String,           // e.g., "CA"
    pub resname: String,        // e.g., "ALA"
    pub chain: char,
    pub resseq: u32,
    pub element: String,        // e.g., "C"
    pub position: Vec3,
    pub velocity: Vec3,         // (Å/ps)
    pub mass: Scalar,           // (amu)
    pub charge: Scalar,         // (e)
    pub lj: Option<LennardJones>,
}
```

**Bond** — 分子内共价键：
```rust
pub struct Bond {
    pub i: usize,
    pub j: usize,
    pub order: u8,              // 1=单键, 2=双键, 3=三键
    pub params: Option<HarmonicBond>,
}
```

**Residue** — 残基（氨基酸/核苷酸）：
```rust
pub struct Residue {
    pub name: String,           // 三字母代码
    pub chain: char,
    pub seqnum: u32,
    pub atoms: Vec<usize>,      // 原子索引
}
```

**Molecule** — 完整分子结构：
```rust
pub struct Molecule {
    pub name: String,
    pub atoms: Vec<Atom>,
    pub bonds: Vec<Bond>,
    pub residues: Vec<Residue>,
    pub forcefield: ForceField,
}

impl Molecule {
    pub fn new(name: &str) -> Self;
    pub fn add_atom(&mut self, atom: Atom) -> usize;
    pub fn add_bond(&mut self, i: usize, j: usize, order: u8);
    pub fn num_atoms(&self) -> usize;
    pub fn atom_positions(&self) -> Vec<Vec3>;
    pub fn center_of_mass(&self) -> Vec3;
    pub fn radius_of_gyration(&self) -> Scalar;
    pub fn distance_matrix(&self) -> Vec<Vec<Scalar>>;

    /// 从简化的 PDB 行数据构建分子
    pub fn from_pdb_lines(lines: &[String]) -> Result<Self, String>;

    /// 生成简化 PDB 格式输出
    pub fn to_pdb_string(&self) -> String;

    /// 生成标准氨基酸（丙氨酸 Ala）
    pub fn alanine() -> Self;

    /// 生成 DNA 短链（poly-AT 4bp）
    pub fn dna_at4() -> Self;
}
```

#### 3.1.4 `dynamics.rs` — 分子动力学引擎

**Integrator** — 分子动力学积分器：
```rust
pub enum Integrator {
    /// 速度 Verlet 算法
    VelocityVerlet,
    /// Langevin 动力学（隐式溶剂）
    Langevin { friction: Scalar, temperature: Scalar },
}

pub struct SimParams {
    pub dt: Scalar,             // 时间步长 (ps), 默认 0.002
    pub temperature: Scalar,    // 目标温度 (K)
    pub pressure: Option<Scalar>, // 目标压力 (bar), None=NVT
    pub friction: Scalar,       // Langevin 摩擦系数 (ps⁻¹), 默认 1.0
    pub steps: u64,             // 总步数
    pub report_interval: u64,   // 报告间隔（步数）
}
```

**EnergyMinimizer** — 能量最小化：
```rust
pub struct EnergyMinimizer {
    pub max_iter: usize,
    pub convergence: Scalar,    // 梯度收敛阈值 (kcal/(mol·Å))
    pub initial_step: Scalar,  // 初始步长 (Å)
}

impl EnergyMinimizer {
    /// 最速下降法能量最小化
    pub fn steepest_descent(
        &self,
        mol: &Molecule,
        coords: &mut [Vec3],
        ff: &ForceField,
    ) -> Result<MinimizationResult, String>;

    /// 共轭梯度法能量最小化
    pub fn conjugate_gradient(
        &self,
        mol: &Molecule,
        coords: &mut [Vec3],
        ff: &ForceField,
    ) -> Result<MinimizationResult, String>;
}

pub struct MinimizationResult {
    pub final_energy: Scalar,
    pub iterations: usize,
    pub converged: bool,
    pub energy_trace: Vec<Scalar>,
}
```

**MolecularDynamics** — MD 模拟引擎：
```rust
pub struct MolecularDynamics {
    pub molecule: Molecule,
    pub coords: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub forces: Vec<Vec3>,
    pub forcefield: ForceField,
    pub params: SimParams,
    pub integrator: Integrator,
    pub step_count: u64,
    pub current_time: Scalar,   // (ps)
    pub total_energy: Scalar,
    pub potential_energy: Scalar,
    pub kinetic_energy: Scalar,
    pub temperature: Scalar,
}

impl MolecularDynamics {
    pub fn new(molecule: Molecule, params: SimParams) -> Self;

    /// 初始化速度（从 Maxwell-Boltzmann 分布采样）
    pub fn initialize_velocities(&mut self, seed: u64);

    /// 执行一步 MD 模拟（速度 Verlet）
    /// 步骤：1) 更新坐标 r(t+dt) = r(t) + v(t)·dt + 0.5·a(t)·dt²
    ///       2) 计算新力 F(t+dt) = -∇E(t+dt)
    ///       3) 更新速度 v(t+dt) = v(t) + 0.5·(a(t) + a(t+dt))·dt
    pub fn step(&mut self) -> Result<(), String>;

    /// 执行完整 MD 模拟
    pub fn run(&mut self) -> Result<MdResult, String>;

    /// 计算瞬时温度
    pub fn compute_temperature(&self) -> Scalar;

    /// 应用 Langevin 热浴
    pub fn langevin_thermostat(&mut self);

    /// 速度标定（Berendsen 控温）
    pub fn berendsen_thermostat(&mut self, tau: Scalar);
}

pub struct MdResult {
    pub trajectory: Vec<Vec<Vec3>>,  // 每 report_interval 步保存一次坐标
    pub energy_trace: Vec<(Scalar, Scalar, Scalar)>, // (E_pot, E_kin, E_total)
    pub temperature_trace: Vec<Scalar>,
    pub final_coords: Vec<Vec3>,
    pub steps_completed: u64,
}
```

#### 3.1.5 `docking.rs` — 分子对接评分函数

```rust
/// 简化分子对接评分函数。
/// 使用形状互补 + 静电 + 去溶剂化三项加权和。

pub struct DockingScore {
    pub shape_weight: Scalar,
    pub electrostatic_weight: Scalar,
    pub desolvation_weight: Scalar,
}

impl DockingScore {
    pub fn new() -> Self;

    /// 计算配体-受体对接评分
    /// ligand_coords: 配体原子坐标
    /// ligand_charges: 配体原子电荷
    /// receptor_coords: 受体原子坐标
    /// receptor_charges: 受体原子电荷
    pub fn score(
        &self,
        ligand_coords: &[Vec3],
        ligand_charges: &[Scalar],
        receptor_coords: &[Vec3],
        receptor_charges: &[Scalar],
    ) -> Scalar;

    /// 形状互补评分（基于 Lennard-Jones 吸引力）
    pub fn shape_score(&self, ligand_coords: &[Vec3], receptor_coords: &[Vec3]) -> Scalar;

    /// 静电评分（基于 Coulomb 定律）
    pub fn electrostatic_score(
        &self,
        ligand_coords: &[Vec3],
        ligand_charges: &[Scalar],
        receptor_coords: &[Vec3],
        receptor_charges: &[Scalar],
    ) -> Scalar;
}
```

#### 3.1.6 `analysis.rs` — 分子轨迹分析

```rust
/// RMSD 计算 — 相对于参考结构的均方根偏差（Å）
pub fn compute_rmsd(coords: &[Vec3], reference: &[Vec3]) -> Scalar;

/// 回转半径 Rg（Å）
pub fn radius_of_gyration(coords: &[Vec3], masses: &[Scalar]) -> Scalar;

/// 均方位移 MSD（Å²）
pub fn mean_squared_displacement(traj: &[Vec<Vec3>], start_frame: usize, interval: usize) -> Vec<Scalar>;

/// 二面角计算（四个原子坐标）
pub fn compute_dihedral_angle(a: &Vec3, b: &Vec3, c: &Vec3, d: &Vec3) -> Scalar;

/// 氢键检测（距离 + 角度判据）
pub fn detect_hydrogen_bonds(
    coords: &[Vec3],
    elements: &[String],
    donor_cutoff: Scalar,     // 默认 3.5 Å
    angle_cutoff: Scalar,     // 默认 120° (弧度)
) -> Vec<(usize, usize)>;

/// 溶剂可及表面积 SAS（Å²）
/// 使用 Shrake-Rupley 算法简化版
pub fn solvent_accessible_surface(coords: &[Vec3], radii: &[Scalar], n_points: usize) -> Scalar;
```

---

### 3.2 阶段 17 — 细胞培养与组织生长仿真（`src/domains/cellbio/`）

#### 3.2.1 `physics.rs` — 生物物理常量

```rust
/// 水在 37°C 的扩散系数 (m²/s) — 小分子
pub const DIFFUSION_WATER_37C: Scalar = 2.5e-9;

/// 典型哺乳动物细胞直径 (m)
pub const TYPICAL_CELL_DIAMETER: Scalar = 15e-6;

/// 典型哺乳动物细胞体积 (m³)
pub const TYPICAL_CELL_VOLUME: Scalar = 1.0e-15;

/// 典型细胞倍增时间 (s) — 22 小时
pub const TYPICAL_DOUBLING_TIME: Scalar = 79200.0;

/// 氧气在水中的扩散系数 (m²/s)
pub const O2_DIFFUSION_COEFFICIENT: Scalar = 2.1e-9;

/// 葡萄糖在水中的扩散系数 (m²/s)
pub const GLUCOSE_DIFFUSION_COEFFICIENT: Scalar = 6.7e-10;

/// CO₂在水中的扩散系数 (m²/s)
pub const CO2_DIFFUSION_COEFFICIENT: Scalar = 1.9e-9;

/// 典型接种密度 (cells/mL)
pub const TYPICAL_SEEDING_DENSITY: Scalar = 1e5;

/// 最大细胞密度 (cells/mL) — 接触抑制上限
pub const MAX_CELL_DENSITY: Scalar = 2e6;

/// Avogadro constant
pub const AVOGADRO_CELL: Scalar = 6.02214076e23;
```

**MediumProperty** — 培养基物理属性查询：
```rust
pub fn water_density(temp: Scalar) -> Scalar;          // kg/m³
pub fn water_viscosity(temp: Scalar) -> Scalar;        // Pa·s
pub fn o2_solubility(temp: Scalar) -> Scalar;          // mol/(L·atm)
pub fn co2_solubility(temp: Scalar) -> Scalar;         // mol/(L·atm)
pub fn diffusion_coefficient(molecule: &str, temp: Scalar) -> Option<Scalar>;
```

#### 3.2.2 `cell_model.rs` — 细胞模型

**CellState** — 细胞生命周期状态：
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellState {
    /// 存活且可增殖
    Viable,
    /// G1 期（DNA 合成前期）
    G1,
    /// S 期（DNA 合成期）
    S,
    /// G2 期（DNA 合成后期）
    G2,
    /// M 期（有丝分裂期）
    M,
    /// 静止期（营养缺乏时进入）
    Quiescent,
    /// 凋亡（程序性死亡）
    Apoptotic,
    /// 坏死
    Necrotic,
    /// 贴壁/迁移态
    Adherent,
    /// 迁移中
    Migrating,
}
```

**Cell** — 单个细胞个体：
```rust
pub struct Cell {
    pub id: u64,
    pub state: CellState,
    pub position: Vec3,              // 空间位置 (m)
    pub velocity: Vec3,              // 迁移速度 (m/s)
    pub radius: Scalar,              // 细胞半径 (m)
    pub mass: Scalar,                // 细胞质量 (kg)
    pub age: Scalar,                 // 细胞年龄 (s)
    pub cycle_progress: Scalar,      // 细胞周期进度 [0, 1)
    pub cycle_duration: Scalar,      // 细胞周期时长 (s)
    pub nutrient_uptake_rate: Scalar,// 营养摄取速率 (mol/s)
    pub o2_consumption: Scalar,      // 耗氧速率 (mol/s)
    pub lactate_production: Scalar,  // 乳酸产率 (mol/s)
    pub atp_level: Scalar,           // ATP 水平 [0, 1]
    pub dna_content: Scalar,         // 相对 DNA 含量
    pub lineage: Vec<u64>,           // 谱系（父细胞 ID 链）
}
```

**CellPopulation** — 细胞群体管理：
```rust
pub struct CellPopulation {
    pub cells: Vec<Cell>,
    pub next_id: u64,
    pub max_cells: usize,            // 最大细胞数
    pub total_doublings: u64,
}

impl CellPopulation {
    pub fn new() -> Self;

    /// 接种指定数量细胞
    pub fn seed(&mut self, n: usize, position: Vec3, radius: Scalar);

    /// 更新所有细胞状态（一个时间步）
    /// dt: 时间步长 (s)
    /// nutrient: 局部营养浓度 (mol/m³)
    /// o2: 局部溶氧浓度 (mol/m³)
    /// ph: 局部 pH 值
    /// temp: 温度 (K)
    pub fn update(&mut self, dt: Scalar, nutrient: Scalar, o2: Scalar, ph: Scalar, temp: Scalar) -> CellUpdateResult;

    /// 细胞增殖（有丝分裂）
    fn divide(&mut self, idx: usize) -> Option<usize>;

    /// 细胞凋亡
    fn apoptose(&mut self, idx: usize);

    /// 细胞迁移（随机游走 + 趋化性）
    fn migrate(&mut self, idx: usize, chemo_gradient: Vec3, dt: Scalar);

    /// 活细胞数量
    pub fn viable_count(&self) -> usize;

    /// 细胞密度 (cells/m³)
    pub fn density(&self) -> Scalar;

    /// 细胞活性 = 活细胞 / 总细胞
    pub fn viability(&self) -> Scalar;
}

pub struct CellUpdateResult {
    pub divisions: usize,
    pub apoptoses: usize,
    pub migrations: usize,
    pub total_nutrient_consumed: Scalar,
    pub total_o2_consumed: Scalar,
    pub total_lactate_produced: Scalar,
}
```

#### 3.2.3 `media.rs` — 培养基模型

**MediumComponent** — 培养基组分：
```rust
pub struct MediumComponent {
    pub name: String,               // "Glucose", "Glutamine", "O2", "CO2", "Lactate", etc.
    pub concentration: Scalar,      // 浓度 (mol/m³)
    pub diffusion_coeff: Scalar,    // 扩散系数 (m²/s)
    pub consumption_rate: Scalar,   // 细胞消耗速率 (mol/(cell·s))
    pub production_rate: Scalar,    // 细胞产生速率 (mol/(cell·s))
}

impl MediumComponent {
    pub fn new(name: &str, concentration: Scalar) -> Self;

    /// 默认葡萄糖组分
    pub fn glucose(conc: Scalar) -> Self;

    /// 默认氧气组分
    pub fn oxygen(conc: Scalar) -> Self;

    /// 默认乳酸组分
    pub fn lactate() -> Self;
}
```

**CultureMedia** — 完整培养基配方：
```rust
pub struct CultureMedia {
    pub components: Vec<MediumComponent>,
    pub ph: Scalar,                  // pH
    pub temperature: Scalar,         // 温度 (K)
    pub osmolarity: Scalar,          // 渗透压 (mOsm/L)
    pub volume: Scalar,              // 体积 (m³)
    pub gas_o2_fraction: Scalar,    // 气相 O₂ 分压分数
    pub gas_co2_fraction: Scalar,   // 气相 CO₂ 分压分数
}

impl CultureMedia {
    pub fn new() -> Self;

    /// 默认 DMEM（高糖）配方
    pub fn dmem_high_glucose() -> Self;

    /// 默认 RPMI-1640 配方
    pub fn rpmi_1640() -> Self;

    /// 获取指定组分浓度
    pub fn get_concentration(&self, name: &str) -> Option<Scalar>;

    /// 设置组分浓度
    pub fn set_concentration(&mut self, name: &str, conc: Scalar) -> Result<(), String>;

    /// 更新 pH（基于 CO₂ 分压和碳酸氢盐缓冲）
    pub fn update_ph(&mut self);

    /// 计算溶氧浓度（基于温度和 O₂ 分压）
    pub fn dissolved_o2(&self) -> Scalar;

    /// 渗透压检查（是否在细胞可耐受范围）
    pub fn is_osmolarity_valid(&self) -> bool;
}

/// 标准培养基配方（常量定义）
pub mod standard_media {
    /// DMEM 高糖：葡萄糖 25 mM
    pub fn dmem_high_glucose() -> Vec<MediumComponent>;
    /// RPMI-1640：葡萄糖 11 mM
    pub fn rpmi_1640() -> Vec<MediumComponent>;
    /// 默认细胞培养条件：37°C, 5% CO₂, pH 7.4
    pub fn standard_conditions() -> (Scalar, Scalar, Scalar); // (pH, temp_K, osmolarity)
}
```

#### 3.2.4 `growth.rs` — 群落/组织生长形态仿真

**GridModel** — 3D 格子模型（反应-扩散耦合）：
```rust
pub struct GridCell {
    pub cell_occupant: Option<u64>,     // 占据该格子的细胞 ID
    pub nutrients: HashMap<String, Scalar>, // 组分名→浓度
    pub o2_conc: Scalar,
    pub ph: Scalar,
    pub ecm_density: Scalar,            // 细胞外基质密度
    pub growth_factor: Scalar,          // 生长因子浓度
}

pub struct GridModel {
    pub grid: Vec<Vec<Vec<GridCell>>>,
    pub dx: Scalar,          // 格子尺寸 (m)
    pub dimensions: (usize, usize, usize), // (nx, ny, nz)
    pub media: CultureMedia,
}

impl GridModel {
    pub fn new(nx: usize, ny: usize, nz: usize, dx: Scalar, media: CultureMedia) -> Self;

    /// 在指定位置接种细胞
    pub fn seed_cells(&mut self, population: &mut CellPopulation, x: usize, y: usize, z: usize, n: usize);

    /// 扩散更新（有限差分法求解扩散方程）
    /// ∂c/∂t = D·∇²c — 使用显式欧拉 + 中心差分
    pub fn diffuse(&mut self, dt: Scalar);

    /// 反应项更新（细胞消耗/产生物质）
    pub fn react(&mut self, population: &CellPopulation, dt: Scalar);

    /// 完整生长步：扩散 → 反应 → 细胞更新
    pub fn step(&mut self, population: &mut CellPopulation, dt: Scalar) -> Result<(), String>;

    /// 获取指定位置的营养浓度
    pub fn nutrient_at(&self, x: usize, y: usize, z: usize, name: &str) -> Option<Scalar>;

    /// 检测接触抑制（周围格子是否已满）
    pub fn contact_inhibition(&self, x: usize, y: usize, z: usize) -> bool;
}
```

**TissueMorphology** — 组织形态分析：
```rust
pub struct TissueMorphology {
    pub total_volume: Scalar,            // 组织总体积 (m³)
    pub cell_count: usize,
    pub viable_count: usize,
    pub avg_radius: Scalar,
    pub necrotic_core_radius: Option<Scalar>, // 坏死核心半径（如有）
    pub surface_area: Scalar,            // 组织表面积 (m²)
    pub compactness: Scalar,             // 紧密度
}

/// 计算组织形态参数
pub fn analyze_tissue_morphology(population: &CellPopulation, grid: &GridModel) -> TissueMorphology;

/// 检测坏死核心（中心区域氧浓度低于阈值）
pub fn detect_necrotic_core(grid: &GridModel, o2_threshold: Scalar) -> Option<Scalar>;
```

#### 3.2.5 `bioreactor.rs` — 生物反应器动态模型

**BioreactorMode** — 反应器操作模式：
```rust
pub enum BioreactorMode {
    Batch,                    // 批式
    FedBatch { feed_rate: Scalar },  // 补料批式 (m³/s)
    Continuous { dilution_rate: Scalar }, // 连续培养 (s⁻¹)
    Perfusion { perfusion_rate: Scalar }, // 灌流 (m³/s)
}
```

**Bioreactor** — 生物反应器模型：
```rust
pub struct Bioreactor {
    pub mode: BioreactorMode,
    pub working_volume: Scalar,          // 工作体积 (m³)
    pub media: CultureMedia,
    pub population: CellPopulation,
    pub agitation_speed: Scalar,         // 搅拌速度 (rpm)
    pub aeration_rate: Scalar,           // 通气速率 (vvm: volume per volume per minute)
    pub temperature_setpoint: Scalar,    // 温度设定 (K)
    pub ph_setpoint: Scalar,             // pH 设定
    pub o2_setpoint: Scalar,             // 溶氧设定 (% saturation)
    pub harvest_interval: Option<Scalar>,// 收获间隔 (s)
    pub feed_concentration: Scalar,      // 补料营养浓度 (mol/m³)
}

impl Bioreactor {
    pub fn new(mode: BioreactorMode, volume: Scalar) -> Self;

    /// 接种
    pub fn inoculate(&mut self, cells: &mut CellPopulation, density: Scalar);

    /// 执行一个时间步的仿真
    pub fn step(&mut self, dt: Scalar) -> Result<(), String>;

    /// 补料操作
    pub fn feed(&mut self, dt: Scalar);

    /// 收获操作
    pub fn harvest(&mut self, volume: Scalar) -> CellPopulation;

    /// 控制 pH（通过 CO₂/碱液添加）
    pub fn control_ph(&mut self);

    /// 控制溶氧（通过搅拌速度/通气量）
    pub fn control_o2(&mut self);

    /// 计算比生长速率 μ (h⁻¹)
    pub fn specific_growth_rate(&self) -> Scalar;

    /// 计算产物产率
    pub fn productivity(&self, component: &str) -> Scalar;
}
```

#### 3.2.6 `analysis.rs` — 细胞培养数据分析

```rust
/// 生长曲线 — 活细胞密度随时间变化
pub fn growth_curve(population_history: &[CellPopulation]) -> Vec<(Scalar, Scalar)>;

/// 比生长速率拟合（指数生长期）
pub fn specific_growth_rate(times: &[Scalar], densities: &[Scalar]) -> Result<Scalar, String>;

/// 倍增时间计算
pub fn doubling_time(mu: Scalar) -> Scalar;

/// 细胞活性曲线
pub fn viability_curve(population_history: &[CellPopulation]) -> Vec<(Scalar, Scalar)>;

/// 代谢速率分析
pub fn metabolic_rates(media_history: &[CultureMedia], dt: Scalar) -> MetabolicAnalysis;

pub struct MetabolicAnalysis {
    pub glucose_consumption_rate: Scalar,    // mmol/(cell·h)
    pub lactate_production_rate: Scalar,     // mmol/(cell·h)
    pub o2_uptake_rate: Scalar,              // mmol/(cell·h)
    pub co2_production_rate: Scalar,         // mmol/(cell·h)
    pub yield_lactate_glucose: Scalar,       // Y_lac/gluc (mol/mol)
    pub respiratory_quotient: Scalar,        // RQ = CO₂ prod / O₂ cons
}

/// Monod 生长动力学 — μ = μmax · S/(Ks + S)
pub fn monod_growth_rate(mu_max: Scalar, substrate: Scalar, ks: Scalar) -> Scalar;

/// Michaelis-Menten 摄取速率
pub fn michaelis_menten_uptake(vmax: Scalar, substrate: Scalar, km: Scalar) -> Scalar;

/// 细胞存活率模型（Arrhenius 温度依赖）
pub fn cell_viability_factor(temp: Scalar, t_opt: Scalar, t_min: Scalar, t_max: Scalar) -> Scalar;
```

---

## 4. 与资料库系统集成

阶段 16~17 使用 `LibraryCategory::Biomolecule`、`LibraryCategory::Chemical`、`LibraryCategory::Cell`、`LibraryCategory::CultureMedia` 四个资料库类别。

### 4.1 分子动力学资料库查询（示例）

```rust
// 从资料库加载蛋白质参数
fn load_protein_parameters(db: &LibraryManager, protein_name: &str) -> Result<ForceField, String> {
    let entry = db.load_entry(&format!("biomolecule/{}", protein_name))?;
    // 解析 TOML 中的力场参数
    // ...
}
```

### 4.2 细胞培养资料库查询（示例）

```rust
// 从资料库加载细胞系参数
fn load_cell_line_params(db: &LibraryManager, cell_line: &str) -> Result<CellLineParams, String> {
    let entry = db.load_entry(&format!("cell/{}", cell_line))?;
    // 解析 TOML 中的细胞参数
    // ...
}
```

---

## 5. 与已有系统集成

阶段 16~17 使用以下已有模块：

| 已有模块 | 集成方式 |
|---------|---------|
| `Block` trait | 分子动力学 + 细胞培养模块将实现 Block trait，可接入 Diagram/SimEngine |
| `SignalValue`, `Scalar`, `Time` | 数据类型 |
| `SimEngine`, `SimContext` | 动力学/培养引擎可被 SimEngine 编排 |
| `OdeSolver` | 反应-扩散方程求解使用已有 ODE 求解器 |
| `Scheduler` | 多模块耦合时使用调度器 |
| `Dimension`, `Unit`, `Quantity` | 生物物理量纲一致性保证 |
| `LibraryDb`, `LibraryManager` | 分子/细胞/培养基参数查询 |
| `Coord1D`, `Coord2D`, `Coord3D` | 分子坐标、细胞空间位置 |
| `Vec3` | 阶段 16 内部 Vec3 与 core::coord 三维坐标可互转 |
| `NumericalGuard` | NaN/Inf 防护 |
| `SignalCache` | 场量数据传递 |

### 5.1 Vec3 与 core::coord 的兼容性

`src/domains/molbio/forcefield.rs` 中的 `Vec3` 是阶段 16 内部的三维向量类型。
与 `core::coord::Coord3D` 的互转方法：

```rust
impl From<Coord3D> for Vec3 { /* ... */ }
impl From<Vec3> for Coord3D { /* ... */ }
```

---

## 6. 实现顺序

### 第一轮：阶段 16 分子动力学基础设施
1. `domains/molbio/mod.rs` — 模块接口
2. `domains/molbio/physics.rs` — 物理常量、元素质量表、键参数
3. `domains/molbio/molecule.rs` — 分子结构、Atom、Bond、Residue、Molecule
4. `domains/molbio/forcefield.rs` — 力场组件（HarmonicBond, HarmonicAngle, PeriodicDihedral, LennardJones, CoulombPotential, ForceField）+ Vec3

### 第二轮：阶段 16 动力学引擎
5. `domains/molbio/dynamics.rs` — 分子动力学引擎（Velocity Verlet, Langevin, Energy Minimizer）
6. `domains/molbio/docking.rs` — 分子对接评分
7. `domains/molbio/analysis.rs` — 轨迹分析（RMSD, Rg, MSD, 氢键, SAS）

### 第三轮：阶段 17 细胞培养基础设施
8. `domains/cellbio/mod.rs` — 模块接口
9. `domains/cellbio/physics.rs` — 生物物理常量、培养基属性
10. `domains/cellbio/cell_model.rs` — 细胞模型（Cell, CellPopulation, CellState）

### 第四轮：阶段 17 组织与反应器
11. `domains/cellbio/media.rs` — 培养基模型（CultureMedia, MediumComponent）
12. `domains/cellbio/growth.rs` — 组织生长格子模型（GridModel, 反应-扩散）
13. `domains/cellbio/bioreactor.rs` — 生物反应器模型
14. `domains/cellbio/analysis.rs` — 细胞培养数据分析

### 第五轮：集成与测试
15. 更新 `src/domains/mod.rs` — 暴露 molbio + cellbio 模块
16. 更新 `src/lib.rs` — 重导出关键类型
17. 全面测试（70+ 新增测试）
18. 完整构建/测试/clippy 验证

---

## 7. 测试要求

### 阶段 16 分子动力学测试（40+）：

**physics.rs 测试（6+）：**
- 物理常量正确性（验证已知值）
- 元素质量查询（C=12.011, H=1.008, O=15.999）
- 未知元素返回 None
- 键参数查询

**molecule.rs 测试（10+）：**
- Molecule 创建和原子添加
- Bond 添加和查询
- 质心计算
- 回转半径计算
- 距离矩阵
- 丙氨酸分子构建
- DNA 短链构建
- PDB 行解析
- PDB 输出
- 原子数量跟踪

**forcefield.rs 测试（12+）：**
- HarmonicBond 能量和力
- HarmonicAngle 能量和力
- PeriodicDihedral 能量（相位依赖）
- LennardJones 能量（排斥/吸引区域）
- LennardJones 力
- LJ 组合规则
- Coulomb 能量
- Coulomb 力
- ForceField 总能量
- ForceField 力计算
- Vec3 操作（距离、点积、叉积、归一化）
- 多原子力场能量

**dynamics.rs 测试（8+）：**
- MD 引擎创建
- 速度初始化（总动量为零校验）
- 单步速度 Verlet（能量守恒检查）
- 完整 MD 运行（短轨迹）
- Langevin 动力学
- 能量最小化（最速下降）
- 能量最小化（共轭梯度）
- 温度计算

**docking.rs 测试（3+）：**
- 形状评分
- 静电评分
- 总评分函数

**analysis.rs 测试（5+）：**
- RMSD 计算（相同结构→0）
- 回转半径
- 二面角计算
- 氢键检测
- MSD 计算

### 阶段 17 细胞培养测试（30+）：

**physics.rs 测试（4+）：**
- 物理常量正确性
- 水密度温度依赖
- 扩散系数查询
- 溶氧度计算

**cell_model.rs 测试（8+）：**
- 细胞创建和状态初始化
- CellPopulation 创建
- 接种细胞
- 细胞增殖（分裂）
- 细胞凋亡
- 群体更新（一个时间步）
- 活细胞计数
- 细胞活性计算

**media.rs 测试（6+）：**
- 培养基创建
- DMEM 高糖配方验证
- 组分浓度查询
- pH 更新
- 溶氧计算
- 渗透压检查

**growth.rs 测试（6+）：**
- GridModel 创建
- 细胞接种到格子
- 扩散更新（浓度守恒检查）
- 反应项更新
- 完整生长步
- 接触抑制检测

**bioreactor.rs 测试（5+）：**
- 反应器创建（批式模式）
- 接种
- 单步更新
- 比生长速率计算
- 补料批式模式

**analysis.rs 测试（4+）：**
- 生长曲线
- Monod 生长速率
- Michaelis-Menten 摄取
- 细胞存活率温度因子

---

## 8. 验收标准

- [ ] `cargo build` 成功
- [ ] `cargo test` — 全部测试通过（0 失败，0 忽略）
- [ ] `cargo clippy --all-targets -- -D warnings` — 零警告
- [ ] 无 `todo!()`、`unimplemented!()` 或空函数体
- [ ] 所有新增代码有英文注释
- [ ] `domains/mod.rs` 仅暴露接口
- [ ] 每个模块至少 2 个测试（创建 + 行为验证）
- [ ] 阶段 16 力场产生物理合理的能量和力
- [ ] 阶段 16 MD 引擎速度 Verlet 保持能量守恒
- [ ] 阶段 16 分子对接评分产生区分性分数
- [ ] 阶段 17 细胞模型正确模拟分裂/凋亡
- [ ] 阶段 17 扩散求解器浓度守恒
- [ ] 阶段 17 生物反应器正确跟踪细胞密度变化
- [ ] 与已有库系统（LibraryDb, Block trait, OdeSolver, Coord, Units）完全集成
- [ ] 所有已有测试保持通过（无回归）

---

## 9. 与阶段 1~15 的向后兼容性

阶段 16~17 是纯新增模块，不影响已有代码：
- 不修改 `core/`、`runtime/`、`blocks/`、`db/` 中的任何现有文件
- 仅添加新的领域模块到 `src/domains/`
- 仅更新 `src/domains/mod.rs` 暴露新模块
- 可选择更新 `src/lib.rs` 增加重导出
