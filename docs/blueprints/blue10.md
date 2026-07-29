# BLUE10 — 阶段 23~29：生理系统、化学反应、结构 FEA、热传导、CFD、多体动力学与航空航天仿真

## 1. 概述

阶段 23~29 在已完成的内核（阶段 1~22）之上构建七个工程与科学领域的仿真能力，覆盖从生物医学到航空航天的中宏观尺度：

- **阶段 23**：生理系统与生物医学仿真 — 组织力学、血流动力学、心脏电生理、药物代谢 PK/PD、神经网络、肿瘤生长
- **阶段 24**：化学反应与化工流程仿真 — 反应动力学、燃烧/爆炸/聚合、混合/分离/蒸馏、反应器/换热器/塔器流程
- **阶段 25**：结构力学与有限元 FEA 仿真 — 应力/应变、梁/板壳/实体单元、静力/模态/屈曲/疲劳分析
- **阶段 26**：热力学与热传导仿真 — 热传导/对流/辐射、相变/潜热、多物理场热耦合
- **阶段 27**：流体动力学 CFD 仿真 — 连续性/动量/能量方程、层流/湍流/多相流、空气动力学/水力学
- **阶段 28**：多体动力学与机械系统仿真 — 刚体/柔体、运动约束、碰撞检测、3D 几何联动
- **阶段 29**：航空航天与气动仿真 — 飞行器气动布局、火箭推进、飞行控制、热防护

**状态目标：** 100% — 编译通过、零 clippy 警告、全部测试通过、无占位代码、与已有 Block/Diagram/Engine 系统完全集成。

---

## 2. 模块架构

```
src/
  domains/
    mod.rs              — 更新：暴露 bio_medical + chemical + structural + thermal + fluid + multibody + aerospace 子模块
    bio_medical/        — [新增] 阶段 23：生理系统与生物医学仿真
      mod.rs            — 模块接口
      physics.rs        — 生物医学物理常量（组织密度、血液粘度、扩散系数）
      tissue.rs         — 组织力学模型（肌肉、骨骼、软骨、血管）
      hemodynamics.rs   — 血流动力学、心脏电生理、脉搏传播
      pharmacokinetics.rs— 药物代谢 PK/PD、吸收/分布/代谢/排泄
      neural.rs         — 神经网络动作电位、信号传递（Hodgkin-Huxley）
      oncology.rs       — 肿瘤生长、侵袭、扩散、治疗响应
      analysis.rs       — 数据分析工具
    chemical/           — [新增] 阶段 24：化学反应与化工流程仿真
      mod.rs            — 模块接口
      physics.rs        — 化学物理常量（气体常数、活化能、平衡常数）
      kinetics.rs       — 反应动力学、速率方程、平衡常数计算
      reactor.rs        — 反应器模型（CSTR、PFR、Batch）
      separation.rs     — 混合/分离/蒸馏/吸收/萃取
      combustion.rs     — 燃烧、爆炸、聚合、氧化、还原
      flowsheet.rs      — 化工流程模拟（换热器、塔器、管道网络）
      analysis.rs       — 温度/压力/浓度影响分析
    structural/         — [新增] 阶段 25：结构力学与有限元 FEA
      mod.rs            — 模块接口
      physics.rs        — 材料力学常量（杨氏模量、泊松比、剪切模量）
      elements.rs       — 梁单元、板壳单元、实体单元、弹簧单元
      fem_solver.rs     — 有限元求解器（静力、模态、屈曲）
      dynamics.rs       — 振动分析、疲劳分析、响应谱
      contact.rs        — 接触、摩擦、连接、紧固、约束
      analysis.rs       — 变形/失效/强度/寿命预测
    thermal/            — [新增] 阶段 26：热力学与热传导仿真
      mod.rs            — 模块接口
      physics.rs        — 热物理常量（热导率、热容、对流系数）
      conduction.rs     — 热传导求解（1D/2D/3D 稳态和瞬态）
      convection.rs     — 热对流（自然/强制对流换热系数）
      radiation.rs      — 热辐射（Stefan-Boltzmann、视角因子）
      phase_change.rs   — 相变、潜热、凝固、熔化、蒸发
      coupling.rs       — 多物理场热耦合（电-热、机械-热、流体-热）
      analysis.rs       — 散热系统、热管、散热器、冷却系统分析
    fluid/              — [新增] 阶段 27：流体动力学 CFD
      mod.rs            — 模块接口
      physics.rs        — 流体物理常量（密度、粘度、声速）
      navier_stokes.rs  — 连续性/动量/能量方程求解
      flow_regimes.rs   — 层流、湍流、多相流、可压缩/不可压缩
      aerodynamics.rs   — 空气动力学、升力、阻力、压力场
      hydraulics.rs     — 水力学、管道流、边界层
      analysis.rs       — 流量/压降/速度场分析
    multibody/          — [新增] 阶段 28：多体动力学与机械系统
      mod.rs            — 模块接口
      physics.rs        — 力学常量、刚体/柔体参数
      body.rs           — 刚体、柔体、质量、惯量、位姿
      constraints.rs    — 运动约束（转动副、移动副、齿轮、连杆）
      collision.rs      — 碰撞检测、接触力、摩擦力、反弹
      dynamics.rs       — 多体动力学求解（拉格朗日/牛顿-欧拉）
      analysis.rs       — 机械系统运动/轨迹/速度/加速度分析
    aerospace/          — [新增] 阶段 29：航空航天与气动仿真
      mod.rs            — 模块接口
      physics.rs        — 航空航天物理常量（大气模型、声速、气体常数）
      aerodynamics.rs   — 飞行器气动布局、升阻比、激波、边界层分离
      propulsion.rs     — 火箭发动机、推进系统、燃料燃烧
      flight_ctrl.rs    — 飞行控制、姿态控制、轨道初始段
      environment.rs    — 高空大气、低温、高速环境模型
      thermal_protection.rs — 热防护、结构载荷、振动冲击
      analysis.rs       — 气动性能分析、轨迹优化
```

---

## 3. 详细规格

### 3.1 阶段 23 — 生理系统与生物医学仿真（`src/domains/bio_medical/`）

#### 3.1.1 `physics.rs` — 生物医学物理常量

```rust
/// 血液密度 (kg/m³)
pub const BLOOD_DENSITY: Scalar = 1060.0;

/// 血液粘度 (Pa·s) —— 全血在 37°C
pub const BLOOD_VISCOSITY: Scalar = 0.0035;

/// 心脏密度 (kg/m³)
pub const HEART_DENSITY: Scalar = 1050.0;

/// 骨骼杨氏模量 (Pa) —— 皮质骨
pub const BONE_YOUNG_MODULUS: Scalar = 18e9;

/// 骨骼泊松比
pub const BONE_POISSON_RATIO: Scalar = 0.3;

/// 骨骼密度 (kg/m³)
pub const BONE_DENSITY: Scalar = 1900.0;

/// 肌肉密度 (kg/m³)
pub const MUSCLE_DENSITY: Scalar = 1060.0;

/// 软骨杨氏模量 (Pa)
pub const CARTILAGE_YOUNG_MODULUS: Scalar = 0.79e9;

/// 血管杨氏模量 (Pa)
pub const VESSEL_YOUNG_MODULUS: Scalar = 1.3e6;

/// 组织导热系数 (W/(m·K))
pub const TISSUE_THERMAL_CONDUCTIVITY: Scalar = 0.5;

/// 组织比热容 (J/(kg·K))
pub const TISSUE_SPECIFIC_HEAT: Scalar = 3600.0;

/// 药物扩散系数默认值 (m²/s)
pub const DRUG_DIFFUSIVITY_DEFAULT: Scalar = 1e-10;
```

#### 3.1.2 `tissue.rs` — 组织力学模型

```rust
/// 组织材料特性
pub struct TissueMaterial {
    pub young_modulus: Scalar,     // 杨氏模量 (Pa)
    pub poisson_ratio: Scalar,     // 泊松比
    pub density: Scalar,           // 密度 (kg/m³)
    pub yield_stress: Scalar,      // 屈服应力 (Pa)
    pub is_hyperelastic: bool,     // 是否超弹性材料
}

/// 组织力学行为计算
pub struct TissueMechanics;

impl TissueMechanics {
    /// 计算组织在给定应变下的应力 (Pa)
    pub fn stress(strain: Scalar, material: &TissueMaterial) -> Scalar;
    /// 计算组织杨氏模量（线弹性）
    pub fn elastic_modulus(material: &TissueMaterial) -> Scalar;
    /// 超弹性 Neo-Hookean 模型应力
    pub fn neo_hookean_stress(stretch: Scalar, mu: Scalar, bulk_modulus: Scalar) -> Scalar;
}

/// 预定义组织材料
pub fn cortical_bone() -> TissueMaterial;
pub fn trabecular_bone() -> TissueMaterial;
pub fn skeletal_muscle() -> TissueMaterial;
pub fn articular_cartilage() -> TissueMaterial;
pub fn artery_wall() -> TissueMaterial;
```

#### 3.1.3 `hemodynamics.rs` — 血流动力学与心脏电生理

```rust
/// 血管分段模型
pub struct VesselSegment {
    pub length: Scalar,           // 长度 (m)
    pub radius: Scalar,           // 内半径 (m)
    pub wall_thickness: Scalar,   // 壁厚 (m)
    pub young_modulus: Scalar,    // 杨氏模量 (Pa)
}

/// 血流动力学计算
impl VesselSegment {
    /// Poiseuille 流阻 (Pa·s/m³)
    pub fn flow_resistance(&self, viscosity: Scalar) -> Scalar;
    /// 顺应性 (m³/Pa)
    pub fn compliance(&self) -> Scalar;
    /// 惯性 (Pa·s²/m³)
    pub fn inertance(&self, density: Scalar) -> Scalar;
    /// 给定压降下的体积流量 (m³/s)
    pub fn flow_rate(&self, pressure_drop: Scalar, viscosity: Scalar) -> Scalar;
}

/// 心脏电生理 — 简化 Hodgkin-Huxley 模型
pub struct HodgkinHuxley {
    pub v_rest: Scalar,          // 静息电位 (mV)
    pub v_threshold: Scalar,     // 阈值电位 (mV)
    pub g_na: Scalar,            // Na⁺ 最大电导 (mS/cm²)
    pub g_k: Scalar,             // K⁺ 最大电导 (mS/cm²)
    pub g_l: Scalar,             // 漏电导 (mS/cm²)
}

impl HodgkinHuxley {
    /// 计算膜电位导数 dV/dt
    pub fn membrane_potential_derivative(&self, v: Scalar, m: Scalar, n: Scalar, h: Scalar, i_stim: Scalar) -> Scalar;
    /// 门控变量导数
    pub fn gate_derivatives(&self, v: Scalar) -> (Scalar, Scalar, Scalar);
    /// 单步欧拉积分
    pub fn step(&mut self, dt: Scalar, i_stim: Scalar);
}

/// 脉搏波传播速度 (m/s) —— Moens-Korteweg 方程
pub fn pulse_wave_velocity(e: Scalar, h: Scalar, r: Scalar, rho: Scalar) -> Scalar;

/// Windkessel 模型 —— 动脉系统集总参数
pub struct WindkesselModel {
    pub r_proximal: Scalar,       // 近端阻力 (Pa·s/m³)
    pub compliance: Scalar,       // 动脉顺应性 (m³/Pa)
    pub r_peripheral: Scalar,     // 外周阻力 (Pa·s/m³)
}

impl WindkesselModel {
    /// 计算主动脉压
    pub fn aortic_pressure(&self, flow: Scalar, p_prev: Scalar, dt: Scalar) -> Scalar;
    /// 频域阻抗计算（返回 (real, imag) 实数-虚数对）
    pub fn impedance(&self, omega: Scalar) -> (Scalar, Scalar);
}
```

#### 3.1.4 `pharmacokinetics.rs` — 药物代谢 PK/PD

```rust
/// 房室模型
pub struct CompartmentModel {
    pub volumes: Vec<Scalar>,     // 各房室体积 (L)
    pub clearance: Vec<Vec<Scalar>>, // 清除率矩阵 (L/h)
}

impl CompartmentModel {
    /// 一室模型：C(t) = (Dose/Vd) * exp(-ke * t)
    pub fn one_compartment(dose: Scalar, vd: Scalar, ke: Scalar, t: Scalar) -> Scalar;
    /// 二室模型口服给药
    pub fn two_compartment_oral(ka: Scalar, ke: Scalar, vd: Scalar, dose: Scalar, t: Scalar, f: Scalar) -> Scalar;
    /// 静脉输注稳态浓度
    pub fn iv_infusion_steady_state(infusion_rate: Scalar, clearance: Scalar) -> Scalar;
    /// 多房室数值积分
    pub fn simulate(&self, doses: &[(Scalar, Scalar)], dt: Scalar, t_end: Scalar, n_comp: usize) -> Vec<Vec<Scalar>>;
}

/// PK/PD 参数
pub struct PkPdParams {
    pub bioavailability: Scalar,   // 生物利用度 F
    pub vd: Scalar,                // 分布容积 (L)
    pub clearance: Scalar,         // 清除率 (L/h)
    pub half_life: Scalar,         // 半衰期 (h)
    pub ec50: Scalar,              // 半最大效应浓度
    pub e_max: Scalar,             // 最大效应
    pub hill_coefficient: Scalar,  // Hill 系数
}

/// 药效学 Emax 模型
pub fn emax_model(concentration: Scalar, e_max: Scalar, ec50: Scalar, hill: Scalar) -> Scalar;
```

#### 3.1.5 `neural.rs` — 神经网络动作电位

```rust
/// 简化 Hodgkin-Huxley 神经元模型
pub struct NeuronModel {
    pub capacitance: Scalar,      // 膜电容 (μF/cm²)
    pub v_rest: Scalar,           // 静息电位 (mV)
    pub threshold: Scalar,        // 阈值 (mV)
    pub refractory_period: Scalar, // 不应期 (ms)
}

impl NeuronModel {
    /// 检测动作电位发放
    pub fn detect_spike(&self, v: Scalar, v_prev: Scalar) -> bool;
    /// Integrate-and-Fire 模型单步
    pub fn lif_step(&mut self, i_syn: Scalar, dt: Scalar) -> Option<Scalar>;
    /// 突触后电位
    pub fn psp_response(amplitude: Scalar, tau: Scalar, t: Scalar) -> Scalar;
}
```

#### 3.1.6 `oncology.rs` — 肿瘤生长模型

```rust
/// 肿瘤生长模型（Gompertz 生长）
pub struct TumorModel {
    pub growth_rate: Scalar,       // 生长速率 (1/day)
    pub carrying_capacity: Scalar, // 最大肿瘤体积 (mm³)
    pub initial_volume: Scalar,    // 初始体积 (mm³)
}

impl TumorModel {
    /// Gompertz 生长曲线: V(t) = V₀ * exp((α/β) * (1 - exp(-β*t)))
    pub fn gompertz_growth(&self, t: Scalar) -> Scalar;
    /// 药物治疗响应: dV/dt = α*V*ln(K/V) - k_drug*C(t)*V
    pub fn treatment_response(&self, drug_conc: Scalar, kill_rate: Scalar, dt: Scalar) -> Scalar;
    /// 肿瘤侵袭深度
    pub fn invasion_depth(&self, t: Scalar, diffusivity: Scalar) -> Scalar;
}
```

#### 3.1.7 `analysis.rs` — 生物医学分析工具

```rust
/// 心输出量 (L/min): CO = HR × SV
pub fn cardiac_output(heart_rate: Scalar, stroke_volume: Scalar) -> Scalar;

/// 体表面积 (m²) —— Mosteller 公式
pub fn body_surface_area(weight_kg: Scalar, height_cm: Scalar) -> Scalar;

/// 肾小球滤过率估计 (mL/min/1.73m²) —— CKD-EPI
pub fn egfr_ckd_epi(creatinine: Scalar, age: Scalar, is_male: bool, is_black: bool) -> Scalar;

/// 组织灌注压 (mmHg)
pub fn perfusion_pressure(map: Scalar, cvp: Scalar) -> Scalar;
```

---

### 3.2 阶段 24 — 化学反应与化工流程仿真（`src/domains/chemical/`）

#### 3.2.1 `physics.rs` — 化学物理常量

```rust
/// 通用气体常数 (J/(mol·K))
pub const R: Scalar = 8.314462618;

/// 标准大气压 (Pa)
pub const ATM: Scalar = 101325.0;

/// 阿伏伽德罗常数
pub const AVOGADRO: Scalar = 6.02214076e23;

/// 标准温度 (K)
pub const T_STP: Scalar = 273.15;

/// 理想气体在 STP 下的摩尔体积 (m³/mol)
pub const MOLAR_VOLUME_STP: Scalar = 0.022414;

/// 法拉第常数 (C/mol)
pub const FARADAY: Scalar = 96485.33212;

/// 水的三相点温度 (K)
pub const WATER_TRIPLE_POINT: Scalar = 273.16;
```

#### 3.2.2 `kinetics.rs` — 反应动力学

```rust
/// 反应速率常数 —— Arrhenius 方程 k = A * exp(-Ea/(R*T))
pub fn arrhenius_rate(a: Scalar, ea: Scalar, t: Scalar) -> Scalar;

/// 简单反应：r = k * C_A^α * C_B^β
pub fn reaction_rate(k: Scalar, c_a: Scalar, c_b: Scalar, alpha: Scalar, beta: Scalar) -> Scalar;

/// 可逆反应净速率：r = k_f * C_A - k_r * C_B
pub fn reversible_rate(k_f: Scalar, k_r: Scalar, c_a: Scalar, c_b: Scalar) -> Scalar;

/// 平衡常数：K_eq = exp(-ΔG/(R*T))
pub fn equilibrium_constant(delta_g: Scalar, t: Scalar) -> Scalar;

/// 半衰期（一级反应）
pub fn half_life_first_order(k: Scalar) -> Scalar;

/// 反应进度 ξ 随时间演化
pub struct ReactionKinetics {
    pub rate_constants: Vec<Scalar>,
    pub stoichiometry: Vec<Vec<Scalar>>,  // 化学计量系数矩阵
    pub species_count: usize,
    pub reaction_count: usize,
}

impl ReactionKinetics {
    /// 计算各物质浓度变化率 dC/dt
    pub fn concentration_derivatives(&self, concentrations: &[Scalar], t: Scalar) -> Vec<Scalar>;
    /// 数值积分一步
    pub fn step(&self, concentrations: &mut [Scalar], dt: Scalar, t: Scalar);
}
```

#### 3.2.3 `reactor.rs` — 反应器模型

```rust
/// 连续搅拌釜反应器 (CSTR)
pub struct Cstr {
    pub volume: Scalar,               // 反应器体积 (m³)
    pub flow_rate_in: Scalar,         // 进料流速 (m³/s)
    pub flow_rate_out: Scalar,        // 出料流速 (m³/s)
    pub inlet_concentrations: Vec<Scalar>, // 进料浓度 (mol/m³)
    pub heat_transfer_coeff: Scalar,  // 换热系数 (W/(m²·K))
    pub heat_transfer_area: Scalar,   // 换热面积 (m²)
    pub coolant_temperature: Scalar,  // 冷却剂温度 (K)
}

impl Cstr {
    /// CSTR 质量平衡微分方程 dC/dt = (C_in - C)/τ + r(C)
    pub fn mass_balance(&self, concentrations: &[Scalar], reaction: &ReactionKinetics) -> Vec<Scalar>;
    /// CSTR 能量平衡 dT/dt
    pub fn energy_balance(&self, t: Scalar, concentrations: &[Scalar], reaction: &ReactionKinetics, delta_h: Scalar) -> Scalar;
    /// 稳态求解
    pub fn steady_state(&self, reaction: &ReactionKinetics, t_guess: Scalar) -> Option<(Vec<Scalar>, Scalar)>;
}

/// 平推流反应器 (PFR)
pub struct Pfr {
    pub length: Scalar,               // 反应器长度 (m)
    pub diameter: Scalar,             // 直径 (m)
    pub flow_velocity: Scalar,        // 流速 (m/s)
}

impl Pfr {
    /// PFR 质量平衡 dC/dz = r(C)/u
    pub fn profile(&self, inlet: &[Scalar], reaction: &ReactionKinetics) -> Vec<Vec<Scalar>>;
}

/// 间歇反应器 (Batch)
pub struct BatchReactor {
    pub volume: Scalar,
}

impl BatchReactor {
    /// Batch 质量平衡 dC/dt = r(C)
    pub fn batch_profile(&self, initial: &[Scalar], reaction: &ReactionKinetics, t_end: Scalar, dt: Scalar) -> Vec<Vec<Scalar>>;
}
```

#### 3.2.4 `separation.rs` — 分离过程

```rust
/// 蒸馏：相对挥发度 α → 理论塔板数 N
pub fn fenske_equation(n: Scalar, alpha: Scalar, x_d: Scalar, x_b: Scalar) -> Scalar;

/// 最小回流比 —— Underwood 方程
pub fn minimum_reflux_ratio(alpha: Scalar, x_f: Scalar, x_d: Scalar, q: Scalar) -> Scalar;

/// 吸收因子法计算吸收率
pub fn absorption_factor(l: Scalar, g: Scalar, k: Scalar) -> Scalar;

/// 液液萃取分配系数
pub fn distribution_coefficient(c_org: Scalar, c_aq: Scalar) -> Scalar;

/// 闪蒸分离 —— Rachford-Rice 方程
pub fn rachford_rice(vapor_frac: Scalar, z_i: &[Scalar], k_i: &[Scalar]) -> Scalar;
```

#### 3.2.5 `combustion.rs` — 燃烧与反应

```rust
/// 绝热火焰温度计算（简化）
pub fn adiabatic_flame_temperature(fuel_hhv: Scalar, cp_products: Scalar, t_initial: Scalar, excess_air: Scalar) -> Scalar;

/// 层流火焰速度 (m/s)
pub fn laminar_flame_speed(unburned_temp: Scalar, pressure: Scalar, equivalence_ratio: Scalar) -> Scalar;

/// 爆炸下限/上限 (LEL/UEL)
pub fn explosive_limits(gas_name: &str) -> Option<(Scalar, Scalar)>;

/// 聚合反应转化率（自催化模型）
pub fn auto_catalytic_conversion(conversion: Scalar, k: Scalar, t: Scalar) -> Scalar;
```

#### 3.2.6 `flowsheet.rs` — 化工流程模拟

```rust
/// 流程单元连接
pub struct ProcessUnit {
    pub id: String,
    pub unit_type: String,           // "reactor", "distillation", "heat_exchanger", etc.
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub parameters: HashMap<String, Scalar>,
}

/// 流程网络
pub struct ProcessFlowsheet {
    pub units: Vec<ProcessUnit>,
    pub streams: Vec<(String, String, String)>, // (from_unit, to_unit, stream_name)
}

impl ProcessFlowsheet {
    /// 流程拓扑排序
    pub fn topological_order(&self) -> Result<Vec<usize>, String>;
    /// 顺序模块模拟
    pub fn sequential_simulate(&self) -> Result<HashMap<String, Vec<Scalar>>, String>;
    /// 流程收敛（含循环流 tears）
    pub fn converge(&mut self, max_iter: usize, tolerance: Scalar) -> Result<(), String>;
}

/// 换热器模型
pub fn heat_exchanger_ntu(c_hot: Scalar, c_cold: Scalar, ua: Scalar, t_hot_in: Scalar, t_cold_in: Scalar, flow_config: &str) -> (Scalar, Scalar);
```

#### 3.2.7 `analysis.rs` — 化工分析

```rust
/// 转化率
pub fn conversion(c_in: Scalar, c_out: Scalar) -> Scalar;

/// 收率
pub fn yield_ratio(product_moles: Scalar, reactant_moles: Scalar, stoichiometric_coeff: Scalar) -> Scalar;

/// 选择性
pub fn selectivity(desired_product: Scalar, total_products: Scalar) -> Scalar;

/// 反应热 ΔH_rxn (J/mol)
pub fn reaction_enthalpy(formation_enthalpies: &[Scalar], stoichiometry: &[Scalar]) -> Scalar;
```

---

### 3.3 阶段 25 — 结构力学与有限元 FEA（`src/domains/structural/`）

#### 3.3.1 `physics.rs` — 材料力学常量

```rust
/// 钢材标准属性
pub fn steel_structural() -> MaterialProperties;
/// 铝合金
pub fn aluminum_6061() -> MaterialProperties;
/// 混凝土
pub fn concrete_30mpa() -> MaterialProperties;
/// 钛合金
pub fn titanium_ti6al4v() -> MaterialProperties;

pub struct MaterialProperties {
    pub young_modulus: Scalar,    // 杨氏模量 (Pa)
    pub poisson_ratio: Scalar,    // 泊松比
    pub density: Scalar,          // 密度 (kg/m³)
    pub yield_strength: Scalar,   // 屈服强度 (Pa)
    pub ultimate_strength: Scalar,// 极限强度 (Pa)
    pub thermal_expansion: Scalar,// 热膨胀系数 (1/K)
}

/// 应力/应变关系
pub fn hookes_law_1d(strain: Scalar, e: Scalar) -> Scalar;
pub fn hookes_law_3d(strain: &[Scalar; 6], e: Scalar, nu: Scalar) -> [Scalar; 6];
pub fn von_mises_stress(sigma: &[Scalar; 6]) -> Scalar;
```

#### 3.3.2 `elements.rs` — 有限元单元

```rust
/// 梁单元
pub struct BeamElement {
    pub length: Scalar,
    pub area: Scalar,              // 横截面积 (m²)
    pub i_y: Scalar,               // 惯性矩 (m⁴)
    pub i_z: Scalar,
    pub j: Scalar,                 // 扭转常数 (m⁴)
    pub material: MaterialProperties,
}

impl BeamElement {
    /// 局部刚度矩阵 (12×12)
    pub fn stiffness_matrix(&self) -> Vec<Vec<Scalar>>;
    /// 质量矩阵 (12×12)
    pub fn mass_matrix(&self) -> Vec<Vec<Scalar>>;
}

/// 杆单元 (Truss)
pub struct TrussElement {
    pub length: Scalar,
    pub area: Scalar,
    pub material: MaterialProperties,
}

impl TrussElement {
    /// 局部刚度矩阵 (4×4)
    pub fn stiffness_matrix(&self) -> Vec<Vec<Scalar>>;
}

/// 弹簧单元
pub struct SpringElement {
    pub stiffness: Scalar,         // 弹簧刚度 (N/m)
}

/// 板壳单元（简化 Kirchhoff-Love 理论）
pub struct ShellElement {
    pub length: Scalar,
    pub width: Scalar,
    pub thickness: Scalar,
    pub e: Scalar,                 // 杨氏模量 (Pa)
    pub nu: Scalar,                // 泊松比
    pub rho: Scalar,               // 密度 (kg/m³)
}

impl ShellElement {
    /// 弯曲刚度 D = E·t³/(12·(1-ν²))
    pub fn bending_stiffness(&self) -> Scalar;
    /// 单元刚度矩阵（简化，24×24）
    pub fn stiffness_matrix(&self) -> Vec<Vec<Scalar>>;
}

/// 实体单元（简化 8 节点六面体）
pub struct SolidElement {
    pub dx: Scalar, pub dy: Scalar, pub dz: Scalar,  // 单元尺寸 (m)
    pub e: Scalar,                 // 杨氏模量 (Pa)
    pub nu: Scalar,                // 泊松比
    pub rho: Scalar,               // 密度 (kg/m³)
}

impl SolidElement {
    /// 单元刚度矩阵（24×24）
    pub fn stiffness_matrix(&self) -> Vec<Vec<Scalar>>;
    /// 单元质量矩阵（一致质量）
    pub fn mass_matrix(&self) -> Vec<Vec<Scalar>>;
}
```

#### 3.3.3 `fem_solver.rs` — 有限元求解器

```rust
/// 全局有限元系统
pub struct FemSystem {
    pub nodes: Vec<Coord3D>,
    pub elements: Vec<FemElement>,
    pub constraints: Vec<(usize, usize, Scalar)>,  // (node, dof, value)
    pub loads: Vec<(usize, usize, Scalar)>,         // (node, dof, force)
}

pub enum FemElement {
    Beam(BeamElement),
    Truss(TrussElement),
    Spring(SpringElement),
    Shell(ShellElement),
    Solid(SolidElement),
}

impl FemSystem {
    /// 组装全局刚度矩阵
    pub fn assemble_stiffness(&self) -> Vec<Vec<Scalar>>;
    /// 施加边界条件
    pub fn apply_bc(&mut self) -> Result<(), String>;
    /// 静力求解 K·u = F
    pub fn solve_static(&self) -> Result<Vec<Scalar>, String>;
    /// 模态分析 K·φ = λ·M·φ
    pub fn solve_modal(&self, n_modes: usize) -> Result<(Vec<Scalar>, Vec<Vec<Scalar>>), String>;
    /// 屈曲分析
    pub fn solve_buckling(&self, n_modes: usize) -> Result<(Vec<Scalar>, Vec<Vec<Scalar>>), String>;
}
```

#### 3.3.4 `dynamics.rs` — 结构动力学

```rust
/// 单自由度振动系统
pub struct SdofSystem {
    pub m: Scalar,     // 质量 (kg)
    pub c: Scalar,     // 阻尼 (N·s/m)
    pub k: Scalar,     // 刚度 (N/m)
}

impl SdofSystem {
    /// 固有频率 (Hz)
    pub fn natural_frequency(&self) -> Scalar;
    /// 阻尼比 ζ
    pub fn damping_ratio(&self) -> Scalar;
    /// 频率响应函数 H(ω)
    pub fn frf_magnitude(&self, omega: Scalar) -> Scalar;
    /// Newmark-β 时域积分
    pub fn newmark_beta(&self, force_history: &[(Scalar, Scalar)], dt: Scalar, beta: Scalar, gamma: Scalar) -> Vec<(Scalar, Scalar)>;
}

/// 疲劳分析 —— S-N 曲线
pub fn sn_curve(stress_amplitude: Scalar, uts: Scalar, endurance_limit: Scalar) -> Scalar;

/// Miner 线性累积损伤
pub fn miner_damage(cycles: &[Scalar], cycles_to_failure: &[Scalar]) -> Scalar;
```

#### 3.3.5 `contact.rs` — 接触与约束

```rust
/// 接触检测
pub fn point_to_point_distance(p1: &Coord3D, p2: &Coord3D) -> Scalar;

/// Hertz 接触应力 (球-球)
pub fn hertz_contact_stress(f: Scalar, r1: Scalar, r2: Scalar, e1: Scalar, e2: Scalar, nu1: Scalar, nu2: Scalar) -> Scalar;

/// 库仑摩擦力
pub fn coulomb_friction(normal_force: Scalar, mu: Scalar) -> Scalar;

/// 螺栓连接预紧力
pub fn bolt_preload(torque: Scalar, diameter: Scalar, k_factor: Scalar) -> Scalar;
```

#### 3.3.6 `analysis.rs` — 结构分析

```rust
/// 安全系数
pub fn safety_factor(ultimate_stress: Scalar, allowable_stress: Scalar) -> Scalar;

/// 欧拉屈曲临界载荷
pub fn euler_buckling_load(e: Scalar, i: Scalar, l: Scalar, k: Scalar) -> Scalar;

/// 梁挠度（简支梁，集中力）
pub fn beam_deflection_simple(f: Scalar, l: Scalar, e: Scalar, i: Scalar, x: Scalar) -> Scalar;
```

---

### 3.4 阶段 26 — 热力学与热传导仿真（`src/domains/thermal/`）

#### 3.4.1 `physics.rs` — 热物理常量

```rust
/// Stefan-Boltzmann 常数 (W/(m²·K⁴))
pub const SIGMA_SB: Scalar = 5.670374419e-8;

/// 标准重力加速度 (m/s²)
pub const G: Scalar = 9.80665;

/// 空气导热系数 (W/(m·K)) 在 300K
pub const AIR_THERMAL_CONDUCTIVITY: Scalar = 0.026;

/// 空气动力粘度 (Pa·s) 在 300K
pub const AIR_DYNAMIC_VISCOSITY: Scalar = 1.85e-5;

/// 水导热系数 (W/(m·K)) 在 300K
pub const WATER_THERMAL_CONDUCTIVITY: Scalar = 0.6;

/// 铜导热系数 (W/(m·K))
pub const COPPER_THERMAL_CONDUCTIVITY: Scalar = 401.0;

/// 铝导热系数 (W/(m·K))
pub const ALUMINUM_THERMAL_CONDUCTIVITY: Scalar = 237.0;

/// 水凝固潜热 (J/kg)
pub const WATER_FUSION_LATENT_HEAT: Scalar = 334000.0;

/// 水汽化潜热 (J/kg)
pub const WATER_VAPORIZATION_LATENT_HEAT: Scalar = 2260000.0;
```

#### 3.4.2 `conduction.rs` — 热传导

```rust
/// 一维稳态热传导: q = -k·A·dT/dx
pub fn fourier_law_1d(k: Scalar, a: Scalar, t_hot: Scalar, t_cold: Scalar, dx: Scalar) -> Scalar;

/// 热阻网络
pub struct ThermalResistance {
    pub resistances: Vec<(Scalar, Scalar, Scalar)>, // (k, A, L) 串联
    pub parallel: bool,
}

impl ThermalResistance {
    /// 串联热阻: R_total = Σ(L/(k·A))
    pub fn series_resistance(&self) -> Scalar;
    /// 并联热阻: 1/R_total = Σ(1/R_i)
    pub fn parallel_resistance(&self) -> Scalar;
    /// 热流量: Q = ΔT / R
    pub fn heat_flow(&self, delta_t: Scalar) -> Scalar;
}

/// 一维瞬态热传导: dT/dt = α·d²T/dx²
pub struct HeatConduction1D {
    pub alpha: Scalar,            // 热扩散率 (m²/s)
    pub length: Scalar,           // 长度 (m)
    pub n_cells: usize,           // 网格数
    pub temperature: Vec<Scalar>, // 温度分布 (K)
}

impl HeatConduction1D {
    /// 初始化温度场
    pub fn new(alpha: Scalar, length: Scalar, n_cells: usize, initial_temp: Scalar) -> Self;
    /// FTCS 显式差分一步
    pub fn ftcs_step(&mut self, dt: Scalar) -> Result<(), String>;
    /// Crank-Nicolson 隐式一步
    pub fn crank_nicolson_step(&mut self, dt: Scalar) -> Result<(), String>;
    /// 稳态温度分布
    pub fn steady_state(&self, t_left: Scalar, t_right: Scalar) -> Vec<Scalar>;
}

/// 二维稳态热传导
pub struct HeatConduction2D {
    pub alpha: Scalar;
    pub nx: usize;
    pub ny: usize;
    pub dx: Scalar;
    pub dy: Scalar;
    pub temperature: Vec<Vec<Scalar>>;
}

impl HeatConduction2D {
    /// Gauss-Seidel 迭代求解
    pub fn gauss_seidel_step(&mut self, boundary: &[BoundaryCondition]) -> Result<(), String>;
    /// 收敛检查
    pub fn check_convergence(&self, tolerance: Scalar) -> bool;
}

pub enum BoundaryCondition {
    FixedTemp(Scalar),       // 固定温度 T
    FixedHeatFlux(Scalar),   // 固定热通量 q'' (W/m²)
    Convection(Scalar, Scalar), // 对流 (h, T_inf)
    Adiabatic,               // 绝热
}
```

#### 3.4.3 `convection.rs` — 热对流

```rust
/// 自然对流 —— 竖直平板 Nusselt 数
pub fn natural_convection_nu(gr: Scalar, pr: Scalar, laminar: bool) -> Scalar;

/// 强制对流 —— 管内充分发展湍流 (Dittus-Boelter)
pub fn forced_convection_nu_turbulent(re: Scalar, pr: Scalar, heating: bool) -> Scalar;

/// 强制对流 —— 层流 (恒壁温)
pub fn forced_convection_nu_laminar(re: Scalar, pr: Scalar, d_l_ratio: Scalar) -> Scalar;

/// 格拉晓夫数 Gr = g·β·ΔT·L³/ν²
pub fn grashof_number(g: Scalar, beta: Scalar, delta_t: Scalar, l: Scalar, nu: Scalar) -> Scalar;

/// 对流换热系数 h = Nu·k/L
pub fn convection_coefficient(nu: Scalar, k: Scalar, l: Scalar) -> Scalar;

/// 沸腾换热 —— Rohsenow 关联式
pub fn nucleate_boiling_h(delta_t_sat: Scalar, fluid: &str) -> Scalar;
```

#### 3.4.4 `radiation.rs` — 热辐射

```rust
/// Stefan-Boltzmann 定律: E = ε·σ·T⁴
pub fn stefan_boltzmann(emissivity: Scalar, temperature: Scalar) -> Scalar;

/// 两灰体间辐射换热: Q = A₁·σ·(T₁⁴ - T₂⁴) / (1/ε₁ + 1/ε₂ - 1)
pub fn radiation_exchange(a1: Scalar, eps1: Scalar, eps2: Scalar, t1: Scalar, t2: Scalar) -> Scalar;

/// 视角因子（两平行同轴圆盘）
pub fn view_factor_parallel_disks(r1: Scalar, r2: Scalar, distance: Scalar) -> Scalar;

/// 视角因子（两垂直矩形）
pub fn view_factor_perpendicular_rectangles(l: Scalar, w: Scalar, h: Scalar) -> Scalar;
```

#### 3.4.5 `phase_change.rs` — 相变

```rust
/// 凝固/熔化 —— 等效热容法
pub struct PhaseChange1D {
    pub k_solid: Scalar,       // 固相导热系数 (W/(m·K))
    pub k_liquid: Scalar,      // 液相导热系数 (W/(m·K))
    pub latent_heat: Scalar,   // 潜热 (J/kg)
    pub melt_temp: Scalar,     // 相变温度 (K)
    pub cp: Scalar,            // 比热容 (J/(kg·K))
    pub rho: Scalar,           // 密度 (kg/m³)
    pub temperature: Vec<Scalar>,
    pub liquid_fraction: Vec<Scalar>,  // 液相分数 0~1
}

impl PhaseChange1D {
    /// 等效比热容（Gauss 展宽）
    pub fn effective_cp(&self, t: Scalar, delta_t: Scalar) -> Scalar;
    /// 时间步进
    pub fn step(&mut self, dt: Scalar, t_left: Scalar, t_right: Scalar) -> Result<(), String>;
}

/// 蒸发速率 (kg/s) —— 基于传质
pub fn evaporation_rate(area: Scalar, pressure_sat: Scalar, pressure_ambient: Scalar, mass_transfer_coeff: Scalar) -> Scalar;
```

#### 3.4.6 `coupling.rs` — 多物理场热耦合

```rust
/// 电-热耦合：焦耳热 Q = I²·R
pub fn joule_heating(current: Scalar, resistance: Scalar) -> Scalar;

/// 机械-热耦合：摩擦生热
pub fn friction_heating(friction_force: Scalar, velocity: Scalar) -> Scalar;

/// 流体-热耦合：对流换热量 Q = h·A·(T_surface - T_fluid)
pub fn convective_heat_transfer(h: Scalar, area: Scalar, t_surface: Scalar, t_fluid: Scalar) -> Scalar;

/// 热-结构耦合：热应变 ε = α·ΔT
pub fn thermal_strain(alpha: Scalar, delta_t: Scalar) -> Scalar;
```

#### 3.4.7 `analysis.rs` — 热系统分析

```rust
/// 散热器热阻
pub fn heatsink_thermal_resistance(air_flow: Scalar, fin_area: Scalar, fin_efficiency: Scalar, h: Scalar) -> Scalar;

/// 热管有效导热系数
pub fn heat_pipe_effective_k(heat_input: Scalar, delta_t: Scalar, length: Scalar, cross_section: Scalar) -> Scalar;

/// 冷却系统 COP
pub fn cooling_cop(cooling_power: Scalar, electrical_power: Scalar) -> Scalar;

/// 温度梯度驱动力
pub fn temperature_gradient(t1: Scalar, t2: Scalar, distance: Scalar) -> Scalar;
```

---

### 3.5 阶段 27 — 流体动力学 CFD（`src/domains/fluid/`）

#### 3.5.1 `physics.rs` — 流体物理常量

```rust
/// 空气气体常数 (J/(kg·K))
pub const AIR_GAS_CONSTANT: Scalar = 287.058;

/// 空气比热比 γ = Cp/Cv
pub const AIR_GAMMA: Scalar = 1.4;

/// 水密度 (kg/m³) 在 4°C
pub const WATER_DENSITY: Scalar = 1000.0;

/// 水动力粘度 (Pa·s) 在 20°C
pub const WATER_VISCOSITY: Scalar = 1.002e-3;

/// 空气密度 (kg/m³) 在 STP
pub const AIR_DENSITY_STP: Scalar = 1.225;

/// 标准重力加速度 (m/s²)
pub const G: Scalar = 9.80665;

/// 运动粘度与动力粘度关系: ν = μ/ρ
pub fn kinematic_viscosity(dynamic: Scalar, density: Scalar) -> Scalar;

/// 等离子体频率 (rad/s): ωp = √(n·e²/(ε₀·m))
pub fn plasma_frequency(electron_density: Scalar) -> Scalar;

/// 高温空气物性（简化 2 温度模型 T_t, T_v）
pub fn high_temp_air_properties(t_translational: Scalar, t_vibrational: Scalar) -> (Scalar, Scalar);
```

#### 3.5.2 `navier_stokes.rs` — Navier-Stokes 求解

```rust
/// 二维不可压缩 Navier-Stokes 求解（有限差分，投影法）
pub struct NavierStokes2D {
    pub nx: usize,              // x 方向网格数
    pub ny: usize,              // y 方向网格数
    pub dx: Scalar,             // x 方向网格间距
    pub dy: Scalar,             // y 方向网格间距
    pub dt: Scalar,             // 时间步长
    pub re: Scalar,             // 雷诺数
    pub u: Vec<Vec<Scalar>>,   // x 方向速度场
    pub v: Vec<Vec<Scalar>>,   // y 方向速度场
    pub p: Vec<Vec<Scalar>>,   // 压力场
}

impl NavierStokes2D {
    /// 投影法一步
    pub fn projection_step(&mut self) -> Result<(), String>;
    /// 计算中间速度（对流+扩散）
    pub fn compute_intermediate_velocity(&self) -> (Vec<Vec<Scalar>>, Vec<Vec<Scalar>>);
    /// 求解压力 Poisson 方程
    pub fn solve_pressure_poisson(&mut self, u_star: &[Vec<Scalar>], v_star: &[Vec<Scalar>]) -> Result<(), String>;
    /// 速度修正
    pub fn velocity_correction(&mut self, u_star: &[Vec<Scalar>], v_star: &[Vec<Scalar>]);
    /// 设置边界条件
    pub fn set_bc(&mut self, boundary: &[WallCondition]);
}

pub enum WallCondition {
    NoSlip,           // 无滑移壁面 u=0, v=0
    FreeSlip,         // 自由滑移 ∂u/∂n=0
    Inlet(Scalar, Scalar), // 入口速度 (u, v)
    Outflow,          // 出口 ∂u/∂n=0, p=0
    MovingWall(Scalar, Scalar), // 移动壁面
}

/// 雷诺数 Re = ρ·u·L/μ
pub fn reynolds_number(density: Scalar, velocity: Scalar, length: Scalar, viscosity: Scalar) -> Scalar;

/// 马赫数 Ma = v/c
pub fn mach_number(velocity: Scalar, speed_of_sound: Scalar) -> Scalar;
```

#### 3.5.3 `flow_regimes.rs` — 流态与多相流

```rust
/// 层流/湍流判断
pub fn flow_regime(re: Scalar) -> FlowRegime;

pub enum FlowRegime {
    Laminar,        // Re < 2300
    Transitional,   // 2300 ≤ Re ≤ 4000
    Turbulent,      // Re > 4000
}

/// 湍流模型 —— 混合长度
pub fn mixing_length_turbulent_viscosity(velocity_gradient: Scalar, wall_distance: Scalar, kappa: Scalar) -> Scalar;

/// 管道摩擦因子 —— Darcy-Weisbach
pub fn darcy_friction_factor(re: Scalar, roughness: Scalar, diameter: Scalar) -> Scalar;

/// 管流压降: Δp = f·(L/D)·(ρ·v²/2)
pub fn pipe_pressure_drop(f: Scalar, length: Scalar, diameter: Scalar, density: Scalar, velocity: Scalar) -> Scalar;

/// 多相流 —— 均相模型密度
pub fn homogeneous_density(alpha_gas: Scalar, rho_gas: Scalar, rho_liquid: Scalar) -> Scalar;

/// 气泡上升终端速度
pub fn bubble_terminal_velocity(bubble_diameter: Scalar, rho_l: Scalar, rho_g: Scalar, mu_l: Scalar) -> Scalar;
```

#### 3.5.4 `aerodynamics.rs` — 空气动力学

```rust
/// 升力系数
pub fn lift_coefficient(cl_alpha: Scalar, alpha_rad: Scalar) -> Scalar;

/// 阻力系数（包含诱导阻力）
pub fn drag_coefficient(cd0: Scalar, cl: Scalar, aspect_ratio: Scalar, oswald: Scalar) -> Scalar;

/// 升力 L = 0.5·ρ·v²·S·CL
pub fn lift_force(density: Scalar, velocity: Scalar, area: Scalar, cl: Scalar) -> Scalar;

/// 阻力 D = 0.5·ρ·v²·S·CD
pub fn drag_force(density: Scalar, velocity: Scalar, area: Scalar, cd: Scalar) -> Scalar;

/// 动压 q = 0.5·ρ·v²
pub fn dynamic_pressure(density: Scalar, velocity: Scalar) -> Scalar;

/// 边界层厚度（平板湍流）
pub fn turbulent_boundary_layer_thickness(x: Scalar, re_x: Scalar) -> Scalar;
```

#### 3.5.5 `hydraulics.rs` — 水力学

```rust
/// 明渠均匀流 —— Manning 公式
pub fn manning_flow(area: Scalar, hydraulic_radius: Scalar, slope: Scalar, n: Scalar) -> Scalar;

/// 孔口流量: Q = Cd·A·√(2·g·h)
pub fn orifice_flow(cd: Scalar, area: Scalar, head: Scalar) -> Scalar;

/// 堰流: Q = Cd·L·H^(3/2)
pub fn weir_flow(cd: Scalar, crest_length: Scalar, head: Scalar) -> Scalar;

/// 水击压力: Δp = ρ·c·Δv
pub fn water_hammer_pressure(density: Scalar, wave_speed: Scalar, velocity_change: Scalar) -> Scalar;
```

#### 3.5.6 `analysis.rs` — 流体分析

```rust
/// 体积流量 Q = v·A
pub fn volumetric_flow(velocity: Scalar, area: Scalar) -> Scalar;

/// 质量流量 ṁ = ρ·v·A
pub fn mass_flow(density: Scalar, velocity: Scalar, area: Scalar) -> Scalar;

/// 水力直径 Dh = 4A/P
pub fn hydraulic_diameter(area: Scalar, wetted_perimeter: Scalar) -> Scalar;

/// 压力系数 Cp = (p - p_inf)/q
pub fn pressure_coefficient(p: Scalar, p_inf: Scalar, q: Scalar) -> Scalar;
```

---

### 3.6 阶段 28 — 多体动力学与机械系统（`src/domains/multibody/`）

#### 3.6.1 `physics.rs` — 多体力学常量

```rust
/// 标准重力加速度
pub const G: [Scalar; 3] = [0.0, 0.0, -9.80665];

/// 刚体质量特性
pub struct RigidBodyProperties {
    pub mass: Scalar,             // 质量 (kg)
    pub com: Coord3D,             // 质心位置 (m)
    pub inertia_tensor: [[Scalar; 3]; 3], // 惯性张量 (kg·m²)
}
```

#### 3.6.2 `body.rs` — 刚体与柔体

```rust
/// 刚体位姿与运动状态
pub struct RigidBody {
    pub id: String,
    pub mass: Scalar,
    pub inertia: [[Scalar; 3]; 3],  // 惯性张量 (局部坐标系)
    pub position: Coord3D,           // 位置 (m)
    pub orientation: Quaternion,     // 姿态（四元数）
    pub linear_velocity: [Scalar; 3],    // 线速度 (m/s)
    pub angular_velocity: [Scalar; 3],   // 角速度 (rad/s)
}

impl RigidBody {
    /// 计算动能: KE = 0.5*m*v² + 0.5*ωᵀ·I·ω
    pub fn kinetic_energy(&self) -> Scalar;
    /// 计算动量: p = m·v
    pub fn linear_momentum(&self) -> [Scalar; 3];
    /// 计算角动量: L = I·ω
    pub fn angular_momentum(&self) -> [Scalar; 3];
    /// 应用力和力矩（牛顿-欧拉方程）
    pub fn apply_force_and_torque(&mut self, force: [Scalar; 3], torque: [Scalar; 3], dt: Scalar);
}

/// 四元数
pub struct Quaternion {
    pub w: Scalar, pub x: Scalar, pub y: Scalar, pub z: Scalar,
}

impl Quaternion {
    pub fn identity() -> Self;
    pub fn from_axis_angle(axis: [Scalar; 3], angle: Scalar) -> Self;
    pub fn rotate_vector(&self, v: [Scalar; 3]) -> [Scalar; 3];
    pub fn conjugate(&self) -> Self;
    pub fn normalize(&self) -> Self;
}
```

#### 3.6.3 `constraints.rs` — 运动约束

```rust
/// 约束类型
pub enum ConstraintType {
    Revolute,          // 转动副（1 DOF）
    Prismatic,         // 移动副（1 DOF）
    Fixed,             // 固定副（0 DOF）
    Spherical,         // 球副（3 DOF）
    Cylindrical,       // 圆柱副（2 DOF）
    Planar,            // 平面副（3 DOF）
    Screw,             // 螺旋副（1 DOF）
    Gear,              // 齿轮副
    Belt,              // 带传动
    RackPinion,        // 齿条-齿轮
}

/// 约束
pub struct Constraint {
    pub id: String,
    pub constraint_type: ConstraintType,
    pub body_a: String,
    pub body_b: String,
    pub anchor_a: Coord3D,        // 约束点在 body A 局部坐标
    pub anchor_b: Coord3D,        // 约束点在 body B 局部坐标
    pub axis: [Scalar; 3],        // 约束轴方向
}

/// 约束雅可比矩阵
pub struct ConstraintJacobian {
    pub j_rows: Vec<[Scalar; 6]>,  // 每个约束行对应 6 个元素 (body_a 3 trans + 3 rot)
}

impl Constraint {
    /// 位置约束误差
    pub fn position_error(&self, body_a: &RigidBody, body_b: &RigidBody) -> Vec<Scalar>;
    /// 速度约束误差
    pub fn velocity_error(&self, body_a: &RigidBody, body_b: &RigidBody) -> Vec<Scalar>;
    /// 约束雅可比矩阵
    pub fn jacobian(&self, body_a: &RigidBody, body_b: &RigidBody) -> ConstraintJacobian;
}

/// 多体系统约束求解
pub struct ConstraintSolver {
    pub constraints: Vec<Constraint>,
}

impl ConstraintSolver {
    /// Baumgarte 稳定化: Φ̈ + 2α·Φ̇ + β²·Φ = 0
    pub fn baumgarte_stabilization(&self, bodies: &[RigidBody], alpha: Scalar, beta: Scalar) -> Vec<Scalar>;
    /// 拉格朗日乘子求解
    pub fn solve_lagrange_multipliers(&self, bodies: &mut [RigidBody], dt: Scalar) -> Result<Vec<Scalar>, String>;
}
```

#### 3.6.4 `collision.rs` — 碰撞检测与接触

```rust
/// 碰撞检测结果
pub struct CollisionResult {
    pub body_a: String,
    pub body_b: String,
    pub contact_point_a: Coord3D,
    pub contact_point_b: Coord3D,
    pub contact_normal: [Scalar; 3],
    pub penetration_depth: Scalar,
    pub has_collision: bool,
}

/// 包围盒
pub struct Aabb {
    pub min: Coord3D,
    pub max: Coord3D,
}

impl Aabb {
    pub fn from_points(points: &[Coord3D]) -> Self;
    pub fn overlaps(&self, other: &Aabb) -> bool;
}

/// 基本碰撞几何体
pub enum CollisionShape {
    Sphere { radius: Scalar },
    Box { half_extents: Coord3D },
    Plane { normal: [Scalar; 3], d: Scalar },
    Mesh { vertices: Vec<Coord3D>, triangles: Vec<(usize, usize, usize)> },
}

/// 碰撞检测
pub fn sphere_sphere_collision(
    pos_a: Coord3D, radius_a: Scalar,
    pos_b: Coord3D, radius_b: Scalar,
) -> CollisionResult;

/// 接触力模型 —— 弹簧-阻尼模型
pub fn contact_force_spring_damper(
    penetration: Scalar, penetration_velocity: Scalar,
    stiffness: Scalar, damping: Scalar,
) -> Scalar;

/// 摩擦力 —— Coulomb + 粘滞摩擦
pub fn friction_force(normal_force: Scalar, mu_static: Scalar, mu_kinetic: Scalar, relative_velocity: Scalar) -> Scalar;

/// 碰撞冲量
pub fn collision_impulse(
    relative_velocity: [Scalar; 3], normal: [Scalar; 3],
    restitution: Scalar, mass_a: Scalar, mass_b: Scalar,
) -> Scalar;
```

#### 3.6.5 `dynamics.rs` — 多体动力学求解

```rust
/// 多体系统
pub struct MultibodySystem {
    pub bodies: Vec<RigidBody>,
    pub constraints: Vec<Constraint>,
    pub forces: Vec<ExternalForce>,
    pub gravity: [Scalar; 3],
}

pub struct ExternalForce {
    pub body_id: String,
    pub force: [Scalar; 3],
    pub application_point: Coord3D,  // 施力点（局部坐标）
}

impl MultibodySystem {
    /// 牛顿-欧拉方程组装
    pub fn assemble_eom(&self) -> (Vec<[Scalar; 3]>, Vec<[Scalar; 3]>);
    /// 拉格朗日方程
    pub fn lagrangian_dynamics(&self) -> Vec<Scalar>;
    /// 半隐式欧拉积分
    pub fn semi_implicit_euler_step(&mut self, dt: Scalar) -> Result<(), String>;
    /// 约束力计算
    pub fn constraint_forces(&self) -> Vec<[Scalar; 3]>;
    /// 系统总能量
    pub fn total_energy(&self) -> Scalar;
}
```

#### 3.6.6 `analysis.rs` — 多体系统分析

```rust
/// 质心位置
pub fn center_of_mass(bodies: &[RigidBody]) -> Coord3D;

/// 系统总动量
pub fn total_momentum(bodies: &[RigidBody]) -> [Scalar; 3];

/// 系统总角动量
pub fn total_angular_momentum(bodies: &[RigidBody]) -> [Scalar; 3];

/// 轨迹距离
pub fn trajectory_length(positions: &[Coord3D]) -> Scalar;

/// 连杆传动比
pub fn linkage_ratio(input_angle: Scalar, output_angle: Scalar) -> Scalar;
```

---

### 3.7 阶段 29 — 航空航天与气动仿真（`src/domains/aerospace/`）

#### 3.7.1 `physics.rs` — 航空航天物理常量

```rust
/// 国际标准大气 (ISA) 海平面参数
pub const ISA_SL_TEMP: Scalar = 288.15;        // 海平面温度 (K)
pub const ISA_SL_PRESSURE: Scalar = 101325.0;  // 海平面气压 (Pa)
pub const ISA_SL_DENSITY: Scalar = 1.225;      // 海平面密度 (kg/m³)
pub const ISA_LAPSE_RATE: Scalar = 0.0065;     // 温度递减率 (K/m)

/// 气体常数
pub const R_AIR: Scalar = 287.058;             // 空气气体常数 (J/(kg·K))
pub const GAMMA_AIR: Scalar = 1.4;             // 空气比热比

/// 标准重力加速度
pub const G0: Scalar = 9.80665;

/// 地球参数
pub const EARTH_RADIUS: Scalar = 6371000.0;    // 地球半径 (m)
pub const EARTH_MASS: Scalar = 5.9722e24;      // 地球质量 (kg)
pub const EARTH_GRAVITATIONAL_PARAMETER: Scalar = 3.986004418e14; // GM (m³/s²)
pub const EARTH_ROTATION_RATE: Scalar = 7.2921150e-5; // 地球自转角速度 (rad/s)

/// 国际标准大气模型
pub struct IsaAtmosphere;

impl IsaAtmosphere {
    /// 海拔高度 → 温度 (K)
    pub fn temperature(altitude: Scalar) -> Scalar;
    /// 海拔高度 → 气压 (Pa)
    pub fn pressure(altitude: Scalar) -> Scalar;
    /// 海拔高度 → 密度 (kg/m³)
    pub fn density(altitude: Scalar) -> Scalar;
    /// 海拔高度 → 声速 (m/s)
    pub fn speed_of_sound(altitude: Scalar) -> Scalar;
    /// 海拔高度 → 动力粘度 (Pa·s)
    pub fn dynamic_viscosity(altitude: Scalar) -> Scalar;
}
```

#### 3.7.2 `aerodynamics.rs` — 飞行器气动

```rust
/// 翼型升力系数（薄翼型理论）
pub fn thin_airfoil_cl(alpha_rad: Scalar) -> Scalar;

/// 翼型阻力系数（基于零升阻力和诱导阻力）
pub fn airfoil_cd(cl: Scalar, cd0: Scalar, ar: Scalar, e: Scalar) -> Scalar;

/// 翼型俯仰力矩系数
pub fn airfoil_cm(alpha_rad: Scalar, camber: Scalar) -> Scalar;

/// 整机气动系数
pub struct AircraftAerodynamics {
    pub wing_area: Scalar,       // 机翼面积 (m²)
    pub aspect_ratio: Scalar,    // 展弦比
    pub cd0: Scalar,             // 零升阻力系数
    pub oswald: Scalar,          // Oswald 效率因子
    pub cl_alpha: Scalar,        // 升力线斜率 (1/rad)
    pub alpha_stall: Scalar,     // 失速攻角 (rad)
}

impl AircraftAerodynamics {
    pub fn cl(&self, alpha: Scalar) -> Scalar;
    pub fn cd(&self, alpha: Scalar) -> Scalar;
    pub fn lift_to_drag(&self, alpha: Scalar) -> Scalar;
    pub fn stall_speed(&self, weight: Scalar, density: Scalar) -> Scalar;
}

/// 激波角（斜激波）—— θ-β-M 关系
pub fn oblique_shock_angle(mach: Scalar, deflection_angle: Scalar, gamma: Scalar) -> Option<Scalar>;

/// 激波前后的压力比（正激波）
pub fn normal_shock_pressure_ratio(mach: Scalar, gamma: Scalar) -> Scalar;

/// Prandtl-Meyer 膨胀角
pub fn prandtl_meyer_angle(mach: Scalar, gamma: Scalar) -> Scalar;
```

#### 3.7.3 `propulsion.rs` — 推进系统

```rust
/// 火箭发动机 —— 推力 F = ṁ·ve + (pe - pa)·Ae
pub fn rocket_thrust(mass_flow: Scalar, exit_velocity: Scalar, exit_pressure: Scalar, ambient_pressure: Scalar, exit_area: Scalar) -> Scalar;

/// 特征速度 c* = p_c·A_t / ṁ
pub fn characteristic_velocity(chamber_pressure: Scalar, throat_area: Scalar, mass_flow: Scalar) -> Scalar;

/// 比冲 Isp = F/(ṁ·g₀)
pub fn specific_impulse(thrust: Scalar, mass_flow: Scalar) -> Scalar;

/// 喷管面积比（给定出口马赫数）
pub fn nozzle_area_ratio(mach: Scalar, gamma: Scalar) -> Scalar;

/// 等熵流动（喷管）
pub fn isentropic_flow(mach: Scalar, gamma: Scalar) -> (Scalar, Scalar, Scalar); // (T/T0, p/p0, rho/rho0)

/// 涡轮喷气发动机推力
pub fn turbojet_thrust(mass_flow_air: Scalar, mass_flow_fuel: Scalar, exhaust_velocity: Scalar, flight_velocity: Scalar) -> Scalar;

/// 燃料消耗率 TSFC
pub fn thrust_specific_fuel_consumption(fuel_flow: Scalar, thrust: Scalar) -> Scalar;
```

#### 3.7.4 `flight_ctrl.rs` — 飞行控制

```rust
/// 飞行器六自由度运动方程
pub struct SixDofAircraft {
    pub mass: Scalar,
    pub inertia: [[Scalar; 3]; 3],
    pub position: Coord3D,           // 地心惯性系位置
    pub velocity: [Scalar; 3],       // 速度 (m/s)
    pub attitude: Quaternion,        // 姿态四元数
    pub angular_velocity: [Scalar; 3], // 角速度 (rad/s)
    pub aerodynamics: AircraftAerodynamics,
    pub wing_area: Scalar,
    pub chord: Scalar,               // 平均气动弦长 (m)
    pub span: Scalar,                // 翼展 (m)
}

impl SixDofAircraft {
    /// 气动力/力矩计算
    pub fn aerodynamic_forces(&self, density: Scalar, speed: Scalar) -> ([Scalar; 3], [Scalar; 3]);
    /// 运动方程求导
    pub fn derivatives(&self, controls: &[Scalar; 4]) -> ([Scalar; 3], [Scalar; 3], [Scalar; 3], [Scalar; 3]);
    /// RK4 积分一步
    pub fn rk4_step(&mut self, controls: &[Scalar; 4], dt: Scalar);
    /// 平衡（配平）条件
    pub fn trim(&self, speed: Scalar, altitude: Scalar) -> Result<[Scalar; 4], String>;
}

/// PID 自动驾驶仪
pub struct Autopilot {
    pub kp: Scalar, pub ki: Scalar, pub kd: Scalar,
    pub setpoint: Scalar,
    pub integral: Scalar,
    pub prev_error: Scalar,
}

impl Autopilot {
    pub fn compute(&mut self, measured: Scalar, dt: Scalar) -> Scalar;
    pub fn reset(&mut self);
}

/// 姿态角 → 四元数
pub fn euler_to_quaternion(roll: Scalar, pitch: Scalar, yaw: Scalar) -> Quaternion;

/// 四元数 → 姿态角
pub fn quaternion_to_euler(q: &Quaternion) -> (Scalar, Scalar, Scalar);
```

#### 3.7.5 `environment.rs` — 高空/高速环境

```rust
/// 高空大气模型（延伸至 100km+）
pub struct HighAltitudeAtmosphere;

impl HighAltitudeAtmosphere {
    pub fn temperature(altitude: Scalar) -> Scalar;
    pub fn pressure(altitude: Scalar) -> Scalar;
    pub fn density(altitude: Scalar) -> Scalar;
    pub fn speed_of_sound(altitude: Scalar) -> Scalar;
}

/// 重力加速度随高度变化: g(h) = g₀·(R²/(R+h)²)
pub fn gravity_at_altitude(altitude: Scalar) -> Scalar;

/// 空气动力加热: q'' = 0.5·ρ·v³·St
pub fn aerodynamic_heating(density: Scalar, velocity: Scalar, stanton: Scalar) -> Scalar;

/// 高空低温环境温度
pub fn ambient_temperature(altitude: Scalar) -> Scalar;
```

#### 3.7.6 `thermal_protection.rs` — 热防护

```rust
/// 热防护系统一维热响应
pub struct ThermalProtectionSystem {
    pub layers: Vec<TpsLayer>,
}

pub struct TpsLayer {
    pub thickness: Scalar,
    pub k: Scalar,                 // 导热系数 (W/(m·K))
    pub cp: Scalar,                // 比热容 (J/(kg·K))
    pub rho: Scalar,               // 密度 (kg/m³)
    pub max_temp: Scalar,          // 最高工作温度 (K)
    pub emissivity: Scalar,        // 表面发射率
}

impl ThermalProtectionSystem {
    /// 一维瞬态热响应
    pub fn thermal_response(&self, heat_flux: Scalar, t_initial: Scalar, t_end: Scalar, dt: Scalar) -> Vec<Vec<Scalar>>;
    /// 背面温度
    pub fn back_face_temperature(&self, heat_flux: Scalar, duration: Scalar) -> Scalar;
    /// 总热容
    pub fn total_heat_capacity(&self) -> Scalar;
}

/// 结构载荷因子: n = F/mg
pub fn load_factor(total_force: Scalar, mass: Scalar) -> Scalar;

/// 振动冲击响应谱（简化 SRS）
pub fn shock_response_sweep(natural_freqs: &[Scalar], base_acceleration: &[(Scalar, Scalar)], damping: Scalar) -> Vec<Scalar>;
```

#### 3.7.7 `analysis.rs` — 航空航天分析

```rust
/// 升阻比
pub fn lift_to_drag_ratio(lift: Scalar, drag: Scalar) -> Scalar;

/// 航程 —— Breguet 航程方程
pub fn breguet_range(velocity: Scalar, ld_ratio: Scalar, sfc: Scalar, w_initial: Scalar, w_final: Scalar) -> Scalar;

/// 爬升率 ROC = (T - D)·v / W
pub fn rate_of_climb(thrust: Scalar, drag: Scalar, velocity: Scalar, weight: Scalar) -> Scalar;

/// 翼载荷 (N/m²)
pub fn wing_loading(weight: Scalar, wing_area: Scalar) -> Scalar;
```

---

## 3.8 与现有类型系统的兼容性说明

本蓝图各阶段使用的数值类型全部基于已有系统：

| 类型 | 来源 | 说明 |
|------|------|------|
| `Scalar` | `core::types::Scalar` (即 `f64`) | 所有物理量、矩阵元素统一使用 |
| `Time` | `core::types::Time` (即 `f64`) | 仿真时间统一类型 |
| `EPSILON` | `core::types::EPSILON` (即 `1e-12`) | 浮点比较阈值 |
| `Coord3D` | `core::coord::Coord3D` | 三维坐标点，含 `x/y/z` 字段及 `distance/normalize/dot/cross` 等方法 |
| `Transform4x4` | `core::coord::Transform4x4` | 4×4 齐次变换矩阵 |
| `OdeSolver` | `runtime::solver::OdeSolver` trait | ODE 系统求解接口 |
| `NewtonRaphson` | `runtime::solver::NewtonRaphson` | 非线性方程组求解 |

**注意事项：**
- multibody 模块定义了自己的 `Quaternion` 结构体（不在 `core/coord` 中），用于刚体姿态表示
- fluid 模块的 `NavierStokes2D` 使用独立的二维网格数据（`Vec<Vec<Scalar>>`），与 core 中的 Tensor 类型物理含义不同（Tensor 为通用 N 维数组，CFD 场数据为专用物理场）
- 所有模块的 mod.rs 遵循已有领域模块模式：`pub mod` 声明子模块 + `pub use` 重导出关键类型
- **常量命名避免冲突**：多个模块定义了 `G`（重力加速度），引用时需使用全路径（如 `thermal::physics::G` 或 `fluid::physics::G`），禁止通配符 `use` 导入可能冲突的常量
- **OCCT 几何接口（阶段 28）**：多体模块的碰撞几何 `CollisionShape::Mesh` 设计为未来与 OCCT 内核联动的预留接口；当前阶段使用简单三角面片表示，后续可通过 OCCT 的 `BRepMesh` 生成精确网格

---

## 4. 实现顺序

阶段 23~29 的实现按以下顺序推进，每个阶段完成后经编译+clippy+test 验证：

1. **`domains/mod.rs`** — 在已有模块列表中添加以下声明，并补充阶段编号注释：
   ```rust
   pub mod bio_medical;   // Phase 23: Physiological/Biomedical
   pub mod chemical;      // Phase 24: Chemical & Process Engineering
   pub mod structural;    // Phase 25: Structural Mechanics & FEA
   pub mod thermal;       // Phase 26: Thermodynamics & Heat Transfer
   pub mod fluid;         // Phase 27: Fluid Dynamics & CFD
   pub mod multibody;     // Phase 28: Multibody Dynamics
   pub mod aerospace;     // Phase 29: Aerospace & Aerodynamics
   ```
2. **阶段 23：bio_medical** — 生理系统与生物医学（7 个文件 + ~40 个测试）
3. **阶段 24：chemical** — 化学反应与化工流程（7 个文件 + ~35 个测试）
4. **阶段 25：structural** — 结构力学与 FEA（6 个文件 + ~30 个测试）
5. **阶段 26：thermal** — 热传导与辐射（7 个文件 + ~35 个测试）
6. **阶段 27：fluid** — 流体动力学 CFD（6 个文件 + ~30 个测试）
7. **阶段 28：multibody** — 多体动力学（6 个文件 + ~30 个测试）
8. **阶段 29：aerospace** — 航空航天（7 个文件 + ~35 个测试）
9. **全局集成测试** — 跨阶段耦合验证（~20 个集成测试）
10. **全项目清理** — clippy、dead code、覆盖率

---

## 5. 测试要求

### 阶段 23（~40 个测试）：
- 组织材料属性验证（cortical_bone、skeletal_muscle 等）
- 血管流阻/顺应性/惯性计算
- Windkessel 模型压力量值和时间常数
- PK/PD 一室/二室模型浓度衰减曲线
- Hodgkin-Huxley 动作电位发放检测
- Gompertz 肿瘤生长曲线验证
- 边缘情况：零流量、零浓度、边界条件

### 阶段 24（~35 个测试）：
- Arrhenius 方程温度依赖性
- CSTR 稳态与瞬态质量平衡
- PFR 轴向浓度分布
- Batch 反应器转化率随时间演化
- Fenske 方程理论塔板数
- 一级/二级反应半衰期
- 边缘情况：零温度、零浓度、无限稀释

### 阶段 25（~30 个测试）：
- Hooke 定律应力-应变关系
- von Mises 等效应力计算
- 梁单元刚度矩阵对称性验证
- 简支梁集中力挠度（与解析解对比）
- 单自由度系统固有频率
- 欧拉屈曲载荷
- 边缘情况：零刚度、零面积、奇异矩阵检测

### 阶段 26（~35 个测试）：
- Fourier 定律热流量计算
- 热阻网络串联/并联
- 1D 瞬态热传导 FTCS 稳定性
- Stefan-Boltzmann 辐射换热
- 相变等效热容法潜热吸收
- 对流换热系数 Reynolds/Pr 相关性
- 边缘情况：绝热边界、零温差、稳态

### 阶段 27（~30 个测试）：
- Reynolds/Mach 数计算
- 管流压降 Darcy-Weisbach
- 2D Navier-Stokes 投影法单步
- 升力/阻力系数及力
- 曼宁公式明渠流量
- 孔口/堰流量
- 边缘情况：零速度、零密度、不可压缩极限

### 阶段 28（~30 个测试）：
- 刚体动能/动量计算
- 四元数旋转/归一化
- 转动副/移动副约束误差
- 球-球碰撞检测几何
- 接触力弹簧-阻尼模型
- 多体系统总能量守恒验证
- 边缘情况：零质量、零惯量、无约束

### 阶段 29（~35 个测试）：
- ISA 大气模型各高度层
- 翼型升力/阻力系数
- 正激波压力比
- 火箭推力/比冲
- 六自由度飞机运动方程配平
- Breguet 航程
- 边缘情况：真空、海平面静止、高超音速

### 全局集成测试（~20 个）：
- 热-结构耦合：温度场 → 热应力 → 变形
- 流体-热耦合：流动 → 对流换热 → 温度场
- 多体-结构耦合：刚体运动 → 结构柔性变形
- 反应-热耦合：反应放热 → 温度变化 → 反应速率
- 所有测试覆盖 phase 8 代数环和 phase 6 事件触发集成

---

## 6. 与已有系统的集成

### 6.1 核心系统集成

各阶段模块通过以下机制与已有系统集成：

| 机制 | 已有模块 | 集成方式 |
|------|---------|---------|
| Block trait | `core::block::Block` | 领域核心求解器可选择实现 Block 接口供 Diagram 使用；纯计算函数（如 FEA 矩阵组装、CFD 投影法）保持独立函数调用 |
| 坐标系 | `core::coord::Coord3D` | 结构 FemSystem、多体 RigidBody、航空航天 SixDofAircraft 使用 Coord3D 表示节点/位置坐标 |
| 量纲系统 | `core::units::Quantity` | 物理量计算可使用 Quantity 类型保证单位一致性；热物理常量建议标注单位注释 |
| 数值求解器 | `runtime::solver::OdeSolver` | 各领域 ODE 系统（PK/PD 房室模型、化学反应动力学、飞行动力学）通过 `OdeSolver` trait 集成；域状态需展平为 `&[Scalar]` 切片 |
| 非线性求解 | `runtime::solver::NewtonRaphson` | FEA 几何非线性、CSTR 稳态求解使用 NewtonRaphson |
| 线性代数 | `runtime::solver::SparseMatrix` | FEA 全局刚度矩阵可选用 SparseMatrix 存储；`solve_linear_dense` 用于小型稠密系统 |
| 事件系统 | `runtime::event::ZeroCrossingDetector` | 碰撞检测（多体）、阈值触发（PK/PD、肿瘤治疗响应）使用过零检测 |
| 调度引擎 | `runtime::scheduler` | 多物理场耦合（热-流-固）调度使用 scheduler |
| 代数环 | `runtime::algebraic` | 化工流程循环流 tears 收敛可复用 AlgebraicLoopDetector |
| 工作流 | `runtime::workflow::WorkflowDAG` | 多阶段计算（航空航天气动-热-结构顺序耦合）使用 WorkflowDAG 编排 |

### 6.2 数据依赖关系

```
阶段 23 (bio_medical) ─── 依赖 ─── 阶段 17 (cellbio) — 细胞/组织模型
                               ─── 阶段 16 (molbio) — 分子/蛋白质
                               
阶段 24 (chemical) ─────── 依赖 ─── 阶段 26 (thermal) — 反应热
                               ─── db — 化学品库

阶段 25 (structural) ──── 依赖 ─── 阶段 26 (thermal) — 热应力
                               ─── 阶段 28 (multibody) — 柔体

阶段 26 (thermal) ─────── 依赖 ─── 阶段 27 (fluid) — 对流换热
                               ─── db — 材料库

阶段 27 (fluid) ───────── 依赖 ─── 阶段 26 (thermal) — 热流体耦合

阶段 28 (multibody) ───── 依赖 ─── 阶段 25 (structural) — 柔体变形

阶段 29 (aerospace) ───── 依赖 ─── 阶段 27 (fluid) — 气动
                               ─── 阶段 26 (thermal) — 热防护
                               ─── 阶段 28 (multibody) — 飞行力学
```

### 6.3 `src/lib.rs` 重导出更新

需要在 `lib.rs` 中添加以下重导出：

```rust
// Re-export bio_medical key types
pub use domains::bio_medical::{
    TissueMaterial, TissueMechanics,
    VesselSegment, WindkesselModel, HodgkinHuxley,
    CompartmentModel, PkPdParams, NeuronModel, TumorModel,
};
pub use domains::bio_medical::analysis::{
    cardiac_output, body_surface_area, egfr_ckd_epi, perfusion_pressure,
};

// Re-export chemical key types
pub use domains::chemical::{
    ReactionKinetics, Cstr, Pfr, BatchReactor, ProcessFlowsheet,
};
pub use domains::chemical::analysis::{
    conversion, yield_ratio, selectivity, reaction_enthalpy,
};

// Re-export structural key types
pub use domains::structural::{
    MaterialProperties, BeamElement, TrussElement, FemSystem, SdofSystem,
};
pub use domains::structural::analysis::{
    safety_factor, euler_buckling_load, beam_deflection_simple,
};

// Re-export thermal key types
pub use domains::thermal::{
    ThermalResistance, HeatConduction1D, HeatConduction2D,
    PhaseChange1D, BoundaryCondition,
};
pub use domains::thermal::analysis::{
    heatsink_thermal_resistance, heat_pipe_effective_k, cooling_cop,
};

// Re-export fluid key types
pub use domains::fluid::{
    NavierStokes2D,
};
pub use domains::fluid::analysis::{
    volumetric_flow, mass_flow, hydraulic_diameter, pressure_coefficient,
};

// Re-export multibody key types
pub use domains::multibody::{
    RigidBody, Quaternion, Constraint, MultibodySystem, CollisionShape,
};
pub use domains::multibody::analysis::{
    center_of_mass, total_momentum, total_angular_momentum,
};

// Re-export aerospace key types
pub use domains::aerospace::{
    IsaAtmosphere, AircraftAerodynamics, SixDofAircraft, Autopilot,
};
pub use domains::aerospace::analysis::{
    lift_to_drag_ratio, breguet_range, rate_of_climb, wing_loading,
};
```

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| **跨尺度耦合数值不稳定** | 微米-米-千米尺度跨越导致数值溢出 | 统一归一化处理，各领域内部归一化后在边界换算 |
| **FEA 矩阵奇异** | 静力求解失败 | 约束检查 + 伪逆/SVD 回退 |
| **CFD 投影法发散** | 压力 Poisson 不收敛 | CFL 条件检查 + 自适应 dt 缩减 |
| **多体碰撞穿透** | 刚体重叠 | 子步迭代 + 位置修正 Baumgarte 稳定化 |
| **大气模型不连续** | 跨层温度/密度跳跃 | 线性插值 + 平滑过渡函数 |
| **与 blueprint 陷阱冲突** (trap.md) | 浮点精度、代数环、内存暴涨 | 所有模块遵守 trap.md 避坑规则 |
