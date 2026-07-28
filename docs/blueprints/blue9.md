# BLUE9 — 阶段 18~22：光学、声学、PCB、电力电子、电磁场仿真

## 1. 概述

阶段 18~22 在已完成的内核（阶段 1~17）之上构建五个工程与物理领域的仿真能力：

- **阶段 18**：光学与光子学仿真 — 几何光学、波动光学、激光/光纤/波导、光谱响应、光电耦合
- **阶段 19**：声学与振动仿真 — 声场传播、振动/共振/阻尼、腔体声学、结构-声耦合
- **阶段 20**：板级电路与 PCB 系统仿真 — PCB 传输线、电源完整性 PI、信号完整性 SI、电-热耦合
- **阶段 21**：电力电子与电机驱动仿真 — 功率器件、变换器拓扑、电机模型、驱动控制
- **阶段 22**：电磁场与微波射频仿真 — 麦克斯韦方程组、静/瞬态/时谐场、天线/雷达/射频

**状态目标：** 100% — 编译通过、零 clippy 警告、全部测试通过、无占位代码、与已有系统完全集成。

---

## 2. 模块架构

```
src/
  domains/
    mod.rs              — 更新：暴露 optical + acoustic + pcb + powerelec + emag 子模块
    optical/            — [新增] 阶段 18：光学与光子学仿真
      mod.rs            — 模块接口
      physics.rs        — 光学物理常量、光谱范围、折射率模型
      ray.rs            — 几何光学：光线、反射、折射、聚焦、成像
      wave.rs           — 波动光学：干涉、衍射、偏振、高斯光束
      laser.rs          — 激光器件模型、光纤传输、波导、光栅
      photoelectric.rs  — 光电转换、光-电耦合
      analysis.rs       — 像差、透过率、效率分析
    acoustic/           — [新增] 阶段 19：声学与振动仿真
      mod.rs            — 模块接口
      physics.rs        — 声学物理常量、声速模型（空气/水/固体）
      wave_prop.rs      — 声场传播：平面波、球面波、衰减
      cavity.rs         — 腔体声学、谐振、模态分析
      transducer.rs     — 扬声器、麦克风、传感器建模
      vibro_acoustic.rs — 结构-声耦合、振动噪声传递
      analysis.rs       — 声压/声强/声功率、频响函数、阻尼分析
    pcb/                — [新增] 阶段 20：板级电路与 PCB 系统
      mod.rs            — 模块接口
      transmission.rs   — 传输线模型、阻抗控制、S 参数
      power_integrity.rs— 电源完整性：压降、纹波、去耦电容网络
      signal_integrity.rs— 信号完整性：反射、串扰、振铃、眼图
      thermal.rs        — 板级电-热耦合、芯片功耗→温度分布
      package.rs        — 封装、BGA、引线键合、寄生参数
    powerelec/          — [新增] 阶段 21：电力电子与电机驱动
      mod.rs            — 模块接口
      devices.rs        — 功率器件模型：二极管、MOSFET、IGBT、晶闸管
      converters.rs     — 变换器拓扑：整流、逆变、PWM、斩波、DC-DC、AC-DC
      motors.rs         — 电机模型：直流、步进、伺服、异步、永磁同步
      drive_ctrl.rs     — 驱动控制：闭环调速、力矩控制、效率分析
      thermal.rs        — 功率流向、能量转换、损耗、发热
    emag/               — [新增] 阶段 22：电磁场与微波射频
      mod.rs            — 模块接口
      physics.rs        — 电磁物理常量、麦克斯韦方程组基础
      static_fields.rs  — 静电场、静磁场求解
      transient_em.rs   — 瞬态电磁场、时谐场
      devices.rs        — 线圈、变压器、永磁体、天线、雷达
      rf_microwave.rs   — 射频电路、微波网络、谐振腔、传输线
      analysis.rs       — 涡流、磁滞、焦耳热、辐射效率
```

---

## 3. 详细规格

### 3.1 阶段 18 — 光学与光子学仿真（`src/domains/optical/`）

#### 3.1.1 `physics.rs` — 光学物理常量

```rust
/// 真空光速 (m/s)
pub const C: Scalar = 299792458.0;

/// 普朗克常数 (J·s)
pub const H_PLANCK: Scalar = 6.62607015e-34;

/// 真空介电常数 (F/m)
pub const EPSILON_0: Scalar = 8.854187817e-12;

/// 真空磁导率 (H/m)
pub const MU_0: Scalar = 1.25663706212e-6;

/// 光谱范围枚举
pub enum SpectralBand {
    Ultraviolet,   // 10-400 nm
    Visible,       // 400-700 nm
    NearInfrared,  // 700-2500 nm
    MidInfrared,   // 2.5-50 μm
    FarInfrared,   // 50-1000 μm
}

/// 波长 → 频率 (Hz)
pub fn wavelength_to_freq(lambda: Scalar) -> Scalar;

/// 频率 → 波长 (m)
pub fn freq_to_wavelength(freq: Scalar) -> Scalar;

/// 光子能量 (J)
pub fn photon_energy(lambda: Scalar) -> Scalar;

/// 折射率模型 trait
pub trait RefractiveIndex: Send + Sync {
    fn n(&self, wavelength: Scalar) -> Scalar;
}

/// 恒定折射率
pub struct ConstantRefractiveIndex { pub n: Scalar }

/// 塞耳迈耶色散模型: n²(λ) = 1 + Σ(Bᵢ·λ²)/(λ² - Cᵢ)
pub struct SellmeierModel {
    pub coefficients: Vec<(Scalar, Scalar)>, // (B_i, C_i)
}
impl RefractiveIndex for SellmeierModel { ... }

/// 常用材料折射率
pub fn fused_silica() -> SellmeierModel;
pub fn bk7_glass() -> SellmeierModel;
pub fn silicon_n() -> ConstantRefractiveIndex;
```

#### 3.1.2 `ray.rs` — 几何光学

**Ray** — 光线数据结构：
```rust
pub struct Ray {
    pub origin: Coord3D,
    pub direction: Coord3D,    // 单位方向向量
    pub wavelength: Scalar,    // m
    pub intensity: Scalar,     // W/m²
    pub phase: Scalar,         // 弧度
    pub optical_path: Scalar,  // 光程
}
```

**OpticalElement** — 光学元件 trait：
```rust
pub trait OpticalElement: Send + Sync {
    fn name(&self) -> &str;
    fn intersect(&self, ray: &Ray) -> Option<Coord3D>;           // 求交点
    fn transmit(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String>; // 透射
    fn reflect(&self, ray: &mut Ray, hit: &Coord3D) -> Result<(), String>; // 反射
}
```

**光学元件实现**：
- **FlatMirror** — 平面反射镜：Snell 反射定律，θ_out = θ_in
- **SphericalMirror** — 球面反射镜：焦距 f = R/2，球面交点计算
- **ThinLens** — 薄透镜：焦距 f，旁轴光线追踪，焦距由曲率半径和折射率计算
- **FlatInterface** — 平面介质界面：Snell 折射 n₁sinθ₁ = n₂sinθ₂
- **Aperture** — 孔径光阑：半径限制，判断光线是否通过

**成像分析**：
```rust
pub struct ImagingSystem {
    pub elements: Vec<Box<dyn OpticalElement>>,
}

impl ImagingSystem {
    pub fn trace_ray(&self, ray: &mut Ray) -> Result<Vec<TracePoint>, String>;
    pub fn trace_fan(&self, origin: Coord3D, angles: &[Scalar]) -> Vec<Vec<TracePoint>>;
    pub fn paraxial_matrix(&self, wavelength: Scalar) -> [[Scalar; 2]; 2]; // ABCD 矩阵
    pub defocus_spot(&self, source: Coord3D, n_rays: usize) -> Scalar;    // 散焦光斑半径
}

pub struct TracePoint {
    pub element_name: String,
    pub position: Coord3D,
    pub path_length: Scalar,
}
```

**ABCD 矩阵光学**（旁轴近似）：
- 自由传播：[[1, d], [0, 1]]
- 薄透镜：[[1, 0], [-1/f, 1]]
- 球面折射：[[1, 0], [(n₂-n₁)/(R·n₂), n₁/n₂]]
- 系统矩阵：M = Mₙ · ... · M₁
- 成像条件：B = 0

#### 3.1.3 `wave.rs` — 波动光学

**Wavefront** — 波前表示：
```rust
pub struct Wavefront {
    pub wavelength: Scalar,
    pub amplitude: Vec<Vec<Scalar>>,    // 2D 振幅分布
    pub phase: Vec<Vec<Scalar>>,        // 2D 相位分布
    pub grid_size: (usize, usize),      // 网格尺寸
    pub spacing: Scalar,                // 网格间距 (m)
}
```

**干涉**：
```rust
/// 双缝干涉强度分布
/// I(x) = I₀ · cos²(π·d·x/(λ·L))，d=缝距，L=屏距
pub fn double_slit_intensity(x: Scalar, d: Scalar, lambda: Scalar, L: Scalar, i0: Scalar) -> Scalar;

/// 薄膜干涉（等倾/等厚）
pub fn thin_film_interference(n_film: Scalar, thickness: Scalar, lambda: Scalar, n_incident: Scalar) -> Scalar;

/// 迈克尔逊干涉仪输出强度
pub fn michelson_intensity(lambda: Scalar, path_diff: Scalar, i0: Scalar) -> Scalar;
```

**衍射**：
```rust
/// 单缝夫琅禾费衍射
/// I(θ) = I₀ · [sin(π·a·sinθ/λ) / (π·a·sinθ/λ)]²
pub fn single_slit_diffraction(theta: Scalar, slit_width: Scalar, lambda: Scalar, i0: Scalar) -> Scalar;

/// 圆孔夫琅禾费衍射（艾里斑）
pub fn circular_aperture_diffraction(theta: Scalar, diameter: Scalar, lambda: Scalar, i0: Scalar) -> Scalar;

/// 光栅衍射（N 缝）
pub fn grating_diffraction(theta: Scalar, d: Scalar, n_slits: usize, lambda: Scalar, i0: Scalar) -> Scalar;
```

**偏振**：
```rust
pub enum PolarizationState {
    Linear { angle: Scalar },    // 线偏振角度（弧度）
    Circular { handedness: CircularPolarization },
    Elliptical { psi: Scalar, chi: Scalar },
    Unpolarized,
}

/// 马吕斯定律：I = I₀ · cos²θ
pub fn malus_law(intensity: Scalar, angle: Scalar) -> Scalar;

/// 布鲁斯特角：θ_B = arctan(n₂/n₁)
pub fn brewster_angle(n1: Scalar, n2: Scalar) -> Scalar;

/// 菲涅耳方程：s 偏振和 p 偏振反射/透射系数
pub fn fresnel_reflection(n1: Scalar, n2: Scalar, theta_i: Scalar, polarization: &PolarizationState) -> Scalar;
pub fn fresnel_transmission(n1: Scalar, n2: Scalar, theta_i: Scalar, polarization: &PolarizationState) -> Scalar;
```

**高斯光束**：
```rust
pub struct GaussianBeam {
    pub wavelength: Scalar,
    pub w0: Scalar,           // 束腰半径 (m)
    pub z0: Scalar,           // 束腰位置 (m)
}

impl GaussianBeam {
    pub fn rayleigh_range(&self) -> Scalar;             // z_R = π·w₀²/λ
    pub fn beam_radius(&self, z: Scalar) -> Scalar;     // w(z) = w₀·√(1+(z/z_R)²)
    pub fn curvature_radius(&self, z: Scalar) -> Scalar;// R(z) = z·(1+(z_R/z)²)
    pub fn gouy_phase(&self, z: Scalar) -> Scalar;      // ζ(z) = arctan(z/z_R)
    pub fn intensity(&self, r: Scalar, z: Scalar) -> Scalar; // I(r,z)
}
```

#### 3.1.4 `laser.rs` — 激光与波导

**LaserSource** — 激光源模型：
```rust
pub struct LaserSource {
    pub wavelength: Scalar,
    pub power: Scalar,           // W
    pub beam: GaussianBeam,
    pub linewidth: Scalar,       // 线宽 (m)
    pub coherence_length: Scalar,// 相干长度 (m)
}
```

**Fiber** — 光纤传输模型：
```rust
pub struct Fiber {
    pub core_n: Scalar,          // 纤芯折射率
    pub cladding_n: Scalar,      // 包层折射率
    pub core_diameter: Scalar,   // m
    pub length: Scalar,          // m
    pub attenuation: Scalar,     // dB/km
}

impl Fiber {
    pub fn numerical_aperture(&self) -> Scalar;    // NA = √(n₁²-n₂²)
    pub fn v_number(&self, lambda: Scalar) -> Scalar;  // V = 2π·a·NA/λ
    pub fn is_single_mode(&self, lambda: Scalar) -> bool; // V < 2.405
    pub fn mode_field_diameter(&self, lambda: Scalar) -> Scalar;
    pub fn transmission(&self, lambda: Scalar, length: Scalar) -> Scalar; // 透过率
    pub fn dispersion(&self, lambda: Scalar) -> Scalar;  // 色散 (ps/(nm·km))
}
```

**Waveguide** — 平板波导：
```rust
pub struct Waveguide {
    pub n_core: Scalar,
    pub n_cladding: Scalar,
    pub thickness: Scalar,  // m
}

impl Waveguide {
    pub fn te_modes(&self, lambda: Scalar) -> Vec<Scalar>; // 有效折射率列表
    pub fn tm_modes(&self, lambda: Scalar) -> Vec<Scalar>;
    pub fn mode_count(&self, lambda: Scalar) -> usize;
}
```

**Grating** — 衍射光栅：
```rust
pub struct Grating {
    pub lines_per_mm: Scalar,
    pub blaze_angle: Option<Scalar>,
}

impl Grating {
    /// 光栅方程：m·λ = d·(sinθ_m - sinθ_i)
    pub fn diffraction_angles(&self, lambda: Scalar, order: i32) -> Vec<Scalar>;
    pub fn angular_dispersion(&self, lambda: Scalar, order: i32) -> Scalar;
    pub fn resolving_power(&self, order: i32, n_lines: usize) -> Scalar;
    pub fn free_spectral_range(&self, order: i32) -> Scalar;
}
```

#### 3.1.5 `photoelectric.rs` — 光电转换

```rust
/// 光电流（简化光电二极管模型）
/// I_photo = R·P，R=响应度 (A/W)
pub fn photocurrent(responsivity: Scalar, optical_power: Scalar) -> Scalar;

/// 量子效率：QE = (I_photo/q) / (P·λ/(h·c))
pub fn quantum_efficiency(photocurrent: Scalar, optical_power: Scalar, wavelength: Scalar) -> Scalar;

/// 太阳能电池简化 I-V 特性
/// I = I_ph - I₀·(exp(qV/(nkT)) - 1)
pub fn solar_cell_iv(photocurrent: Scalar, saturation_current: Scalar,
                     voltage: Scalar, n: Scalar, temp: Scalar) -> Scalar;

/// 光-电耦合 Block：输入光功率，输出电流/电压
pub struct PhotodetectorBlock { ... }  // 实现 Block trait
```

#### 3.1.6 `analysis.rs` — 光学分析

```rust
/// 瑞利判据分辨率 (rad)
pub fn rayleigh_criterion(diameter: Scalar, lambda: Scalar) -> Scalar;

/// 透镜像差简化模型（球差、彗差、像散）
pub struct AberrationEstimator {
    pub spherical: Scalar,   // 球差系数
    pub coma: Scalar,        // 彗差系数
    pub astigmatism: Scalar, // 像散系数
}

/// MTF 调制传递函数（简化）
pub fn modulation_transfer_function(spatial_freq: Scalar, aperture: Scalar, lambda: Scalar) -> Scalar;

/// 系统透过率（串联光学元件）
pub fn system_transmittance(elements: &[Scalar]) -> Scalar;

/// 光学效率
pub fn optical_efficiency(transmitted_power: Scalar, incident_power: Scalar) -> Scalar;
```

---

### 3.2 阶段 19 — 声学与振动仿真（`src/domains/acoustic/`）

#### 3.2.1 `physics.rs` — 声学物理常量

```rust
/// 空气中声速 (m/s, 20°C)
pub const SPEED_OF_SOUND_AIR: Scalar = 343.0;

/// 水中声速 (m/s)
pub const SPEED_OF_SOUND_WATER: Scalar = 1482.0;

/// 钢中纵波声速 (m/s)
pub const SPEED_OF_SOUND_STEEL: Scalar = 5900.0;

/// 空气特性阻抗 (rayl, 20°C)
pub const Z0_AIR: Scalar = 413.0;

/// 水特性阻抗 (rayl)
pub const Z0_WATER: Scalar = 1.48e6;

/// 参考声压 (Pa) — 空气中
pub const P_REF_AIR: Scalar = 20e-6;

/// 参考声压 (Pa) — 水中
pub const P_REF_WATER: Scalar = 1e-6;

/// 声速的温度依赖（空气）：c(T) = 331.3·√(1 + T/273.15)
pub fn speed_of_sound_air(temperature_c: Scalar) -> Scalar;

/// 声速的盐度/温度/深度依赖（水）
pub fn speed_of_sound_water(temperature_c: Scalar, salinity_ppt: Scalar, depth_m: Scalar) -> Scalar;

/// 特性阻抗: Z = ρ·c
pub fn characteristic_impedance(density: Scalar, speed: Scalar) -> Scalar;
```

#### 3.2.2 `wave_prop.rs` — 声场传播

**SoundField** — 声场类型：
```rust
pub enum SoundField {
    PlaneWave { amplitude: Scalar, frequency: Scalar, direction: Coord3D },
    SphericalWave { amplitude: Scalar, frequency: Scalar, source: Coord3D },
    FarField { amplitude: Scalar, frequency: Scalar, distance: Scalar },
}

pub fn sound_pressure_level(p_rms: Scalar, p_ref: Scalar) -> Scalar; // dB SPL

pub fn spherical_spreading(r: Scalar, r_ref: Scalar) -> Scalar; // 球面扩展衰减

/// 空气吸收衰减系数 (dB/m)，ISO 9613-1
pub fn air_attenuation_coefficient(freq: Scalar, temp_c: Scalar, humidity_pct: Scalar) -> Scalar;

/// 声压级随距离衰减
pub fn spl_at_distance(spl_ref: Scalar, r_ref: Scalar, r: Scalar, absorption: Scalar) -> Scalar;

/// 声强 I = p²/(ρ·c)
pub fn sound_intensity(p_rms: Scalar, impedance: Scalar) -> Scalar;

/// 声功率 W = I·A
pub fn sound_power(intensity: Scalar, area: Scalar) -> Scalar;
```

#### 3.2.3 `cavity.rs` — 腔体声学

```rust
pub struct Cavity {
    pub dimensions: Coord3D,     // Lx, Ly, Lz (m)
    pub wall_material: String,
}

/// 矩形腔体固有频率: f_{nx,ny,nz} = c/2·√((nx/Lx)²+(ny/Ly)²+(nz/Lz)²)
pub fn rectangular_room_modes(dims: &Coord3D, c: Scalar, max_freq: Scalar) -> Vec<(i32, i32, i32, Scalar)>;

/// 亥姆霍兹共振频率: f₀ = (c/2π)·√(A/(V·L))
pub fn helmholtz_resonance(c: Scalar, neck_area: Scalar, neck_length: Scalar, volume: Scalar) -> Scalar;

/// 混响时间 RT60 (Sabine 公式): T₆₀ = 0.161·V/(Σαᵢ·Sᵢ)
pub fn rt60_sabine(volume: Scalar, areas: &[Scalar], absorption_coeffs: &[Scalar]) -> Scalar;

/// 临界距离: r_c = 0.057·√(V/RT60)
pub fn critical_distance(volume: Scalar, rt60: Scalar) -> Scalar;
```

#### 3.2.4 `transducer.rs` — 换能器建模

```rust
/// 扬声器简化模型（活塞模型）
pub struct Loudspeaker {
    pub sd: Scalar,             // 有效辐射面积 (m²)
    pub mms: Scalar,            // 振动质量 (kg)
    pub cms: Scalar,            // 顺性 (m/N)
    pub rms: Scalar,            // 机械阻尼 (N·s/m)
    pub bl: Scalar,             // 力因子 (N/A)
    pub re: Scalar,             // 音圈直流电阻 (Ω)
    pub le: Scalar,             // 音圈电感 (H)
}

impl Loudspeaker {
    pub fn fundamental_resonance(&self) -> Scalar;          // fₛ = 1/(2π·√(Mms·Cms))
    pub fn electrical_impedance(&self, freq: Scalar) -> num_complex::Complex<Scalar>;
    pub fn sound_pressure(&self, freq: Scalar, voltage: Scalar, distance: Scalar) -> Scalar; // dB SPL @ 1m
    pub fn efficiency(&self) -> Scalar;                     // η = ρ·(Bl)²·Sd²/(2π·c·Re·Mms²)
}

/// 麦克风简化模型（电容式）
pub struct Microphone {
    pub sensitivity: Scalar,    // mV/Pa
    pub frequency_response: Vec<(Scalar, Scalar)>, // (freq, relative_gain_db)
}

impl Microphone {
    pub fn output_voltage(&self, sound_pressure_pa: Scalar) -> Scalar;
    pub fn frequency_correction(&self, freq: Scalar) -> Scalar;
}

/// 加速度传感器模型
pub struct Accelerometer {
    pub sensitivity: Scalar,    // mV/g
    pub resonant_freq: Scalar,  // Hz
    pub damping_ratio: Scalar,
}
```

#### 3.2.5 `vibro_acoustic.rs` — 结构-声耦合

```rust
/// 振动速度 → 声压辐射
pub fn radiation_efficiency(plate_area: Scalar, freq: Scalar, critical_freq: Scalar, c: Scalar) -> Scalar;

/// 板的临界频率: f_c = c²/(1.8·t·c_L)，t=板厚，c_L=纵波速度
pub fn critical_frequency(c_fluid: Scalar, thickness: Scalar, c_longitudinal: Scalar) -> Scalar;

/// 结构传递损失 TL (mass law): TL = 20·log₁₀(f·m) - 47 dB
pub fn transmission_loss_mass_law(freq: Scalar, surface_density: Scalar) -> Scalar;

/// 隔声量（考虑吻合效应）
pub fn sound_transmission_loss(freq: Scalar, surface_density: Scalar, critical_freq: Scalar) -> Scalar;

/// 振动噪声传递函数
pub fn vibration_transfer_function(mass: Scalar, stiffness: Scalar, damping: Scalar, freq: Scalar)
    -> num_complex::Complex<Scalar>;
```

#### 3.2.6 `analysis.rs` — 声学分析

```rust
/// 1/n 倍频程滤波
pub fn octave_band_center_frequencies(base_freq: Scalar, n_bands: usize, n: u32) -> Vec<Scalar>;

/// A 计权修正（IEC 61672）
pub fn a_weighting(freq: Scalar) -> Scalar;  // dB 修正值

/// 等效连续声级 Leq
pub fn equivalent_sound_level(spl_trace: &[Scalar], duration: Scalar) -> Scalar;

/// 频响函数 FRF: H(f) = Gxy(f)/Gxx(f)
pub fn frequency_response_function(input_fft: &[num_complex::Complex<Scalar>],
                                    output_fft: &[num_complex::Complex<Scalar>]) -> Vec<num_complex::Complex<Scalar>>;

/// 阻尼比估计（半功率带宽法）
pub fn damping_ratio_from_peak(peak_freq: Scalar, bandwidth_3db: Scalar) -> Scalar;
```

---

### 3.3 阶段 20 — 板级电路与 PCB 系统（`src/domains/pcb/`）

#### 3.3.1 `transmission.rs` — 传输线

```rust
/// 微带线特性阻抗 (IPC-2141)
pub fn microstrip_z0(w: Scalar, h: Scalar, t: Scalar, er: Scalar) -> Scalar;

/// 带状线特性阻抗
pub fn stripline_z0(w: Scalar, h: Scalar, t: Scalar, er: Scalar, b: Scalar) -> Scalar;

/// 共面波导特性阻抗（简化）
pub fn cpw_z0(w: Scalar, gap: Scalar, h: Scalar, er: Scalar) -> Scalar;

/// 传播延迟: t_pd = √(εr)/c (s/m)
pub fn propagation_delay(er: Scalar) -> Scalar;

/// 传输线模型（集总 LC 分段）
pub struct TransmissionLine {
    pub z0: Scalar,             // 特性阻抗 (Ω)
    pub length: Scalar,         // 长度 (m)
    pub er: Scalar,             // 介电常数
    pub attenuation: Scalar,    // 衰减 (dB/m)
    pub segments: usize,        // LC 分段数
}

impl TransmissionLine {
    pub fn propagation_delay(&self) -> Scalar;
    pub fn electrical_length(&self, freq: Scalar) -> Scalar;  // 波长倍数
    pub fn input_impedance(&self, freq: Scalar, zl: num_complex::Complex<Scalar>)
        -> num_complex::Complex<Scalar>;
    pub fn s11(&self, freq: Scalar, z0: Scalar, zl: num_complex::Complex<Scalar>)
        -> num_complex::Complex<Scalar>;
    pub fn s21(&self, freq: Scalar, z0: Scalar) -> num_complex::Complex<Scalar>;
}

/// S 参数矩阵（2 端口）
pub fn s2p_to_t_params(s11: num_complex::Complex<Scalar>, s12: num_complex::Complex<Scalar>,
                        s21: num_complex::Complex<Scalar>, s22: num_complex::Complex<Scalar>,
                        z0: Scalar) -> [[num_complex::Complex<Scalar>; 2]; 2];
```

#### 3.3.2 `power_integrity.rs` — 电源完整性

```rust
/// DC 压降 IR Drop: V_drop = I·R
pub fn ir_drop(current: Scalar, resistance: Scalar) -> Scalar;

/// 电源纹波（Buck 变换器简化）
pub fn buck_ripple_voltage(vin: Scalar, vout: Scalar, l: Scalar, c: Scalar,
                           freq: Scalar, esr: Scalar) -> Scalar;

/// 去耦电容网络阻抗
pub struct DecapNetwork {
    pub capacitors: Vec<Decap>,  // 并联去耦电容
}

pub struct Decap {
    pub capacitance: Scalar,
    pub esr: Scalar,
    pub esl: Scalar,
    pub count: usize,
}

impl DecapNetwork {
    pub fn impedance(&self, freq: Scalar) -> num_complex::Complex<Scalar>;
    pub fn self_resonant_freq(&self, idx: usize) -> Scalar;  // SRF = 1/(2π·√(L·C))
    pub fn target_impedance(voltage: Scalar, ripple_pct: Scalar, max_current: Scalar) -> Scalar;
    pub fn parallel_resonance_peaks(&self) -> Vec<Scalar>;
}

/// 电源分配网络 PDN 阻抗
pub fn pdn_impedance(vrm_output: Scalar, decap_network: &DecapNetwork,
                     plane_cap: Scalar, freq: Scalar) -> num_complex::Complex<Scalar>;
```

#### 3.3.3 `signal_integrity.rs` — 信号完整性

```rust
/// 反射系数: Γ = (Z_L - Z₀)/(Z_L + Z₀)
pub fn reflection_coefficient(zl: Scalar, z0: Scalar) -> Scalar;

/// 回波损耗: RL = -20·log₁₀(|Γ|) (dB)
pub fn return_loss(gamma: Scalar) -> Scalar;

/// 插入损耗: IL = -20·log₁₀(|S₂₁|) (dB)
pub fn insertion_loss(s21: num_complex::Complex<Scalar>) -> Scalar;

/// 串扰模型（三线微带简化）
pub fn crosstalk_peak(aggressor_swing: Scalar, coupling_length: Scalar,
                      rise_time: Scalar, z0: Scalar, zcouple: Scalar) -> Scalar;

/// 振铃幅度（欠阻尼二阶系统阶跃响应过冲）
pub fn ringing_overshoot(damping_ratio: Scalar) -> Scalar;

/// 眼图参数
pub struct EyeDiagram {
    pub eye_height: Scalar,     // V
    pub eye_width: Scalar,      // s
    pub jitter: Scalar,         // s
    pub bit_rate: Scalar,       // bits/s
}

/// 简化眼图分析
pub fn eye_diagram_analysis(waveform: &[Scalar], time: &[Scalar],
                             bit_period: Scalar, n_bits: usize) -> EyeDiagram;

/// 时域反射计 TDR 仿真
pub fn tdr_waveform(source_impedance: Scalar, line_z0: Scalar,
                    load_impedance: Scalar, rise_time: Scalar,
                    length: Scalar, time: &[Scalar]) -> Vec<Scalar>;
```

#### 3.3.4 `thermal.rs` — 板级电-热耦合

```rust
/// 芯片结温: T_j = T_a + θ_ja · P
pub fn junction_temperature(ambient_temp: Scalar, theta_ja: Scalar, power: Scalar) -> Scalar;

/// PCB 铜箔温升 (IPC-2151 简化)
pub fn pcb_trace_temperature_rise(current: Scalar, width: Scalar, thickness: Scalar,
                                   ambient_temp: Scalar) -> Scalar;

/// 热阻网络（芯片→封装→PCB→环境）
pub struct ThermalNetwork {
    pub theta_jc: Scalar,   // 结-壳热阻 (°C/W)
    pub theta_cb: Scalar,   // 壳-板热阻
    pub theta_ba: Scalar,   // 板-环境热阻
}

impl ThermalNetwork {
    pub fn total_theta_ja(&self) -> Scalar;  // θja = θjc + θcb + θba
    pub fn steady_state_temp(&self, power: Scalar, ambient: Scalar) -> Scalar;
    pub fn thermal_time_constant(&self, thermal_capacitance: Scalar) -> Scalar;
}

/// PCB 热仿真 Block：输入功耗阵列，输出温度分布
pub struct PcbThermalBlock { ... }  // 实现 Block trait

/// 热点分析
pub fn hot_spot_temperature(power_map: &[Vec<Scalar>], via_count: usize,
                             board_thickness: Scalar, copper_coverage: Scalar) -> Scalar;
```

#### 3.3.5 `package.rs` — 封装与寄生参数

```rust
/// 键合线寄生电感: L ≈ 2·l·[ln(4·l/d) - 0.75] (nH)，l=长度(mm), d=直径(mm)
pub fn bond_wire_inductance(length_mm: Scalar, diameter_mm: Scalar) -> Scalar;

/// BGA 焊球寄生电容
pub fn bga_ball_capacitance(ball_diameter: Scalar, ball_pitch: Scalar,
                             dielectric_er: Scalar, height: Scalar) -> Scalar;

/// 封装寄生参数模型
pub struct PackageParasitics {
    pub r_bond: Vec<Scalar>,   // 键合线电阻 (Ω)
    pub l_bond: Vec<Scalar>,   // 键合线电感 (H)
    pub c_pad: Vec<Scalar>,    // 焊盘电容 (F)
    pub c_coupling: Vec<(usize, usize, Scalar)>, // 耦合电容 (pin_i, pin_j, C)
}

impl PackageParasitics {
    pub fn total_pin_c(pin: usize) -> Scalar;
    pub fn mutual_inductance(pin_i: usize, pin_j: usize, k: Scalar) -> Scalar;
}
```

---

### 3.4 阶段 21 — 电力电子与电机驱动（`src/domains/powerelec/`）

#### 3.4.1 `devices.rs` — 功率器件模型

```rust
/// 功率二极管简化模型
pub struct PowerDiode {
    pub vf: Scalar,             // 正向压降 (V)
    pub r_on: Scalar,           // 导通电阻 (Ω)
    pub trr: Scalar,            // 反向恢复时间 (s)
    pub v_br: Scalar,           // 击穿电压 (V)
    pub i_max: Scalar,          // 最大电流 (A)
}

impl PowerDiode {
    pub fn forward_voltage(&self, current: Scalar) -> Scalar;  // V = Vf + I·Ron
    pub fn conduction_loss(&self, current: Scalar, duty: Scalar) -> Scalar;  // W
    pub fn switching_loss(&self, current: Scalar, v_dc: Scalar, freq: Scalar) -> Scalar;
}

/// 功率 MOSFET 模型（含导通和开关特性）
pub struct PowerMosfet {
    pub r_ds_on: Scalar,        // 导通电阻 (Ω)
    pub v_th: Scalar,           // 阈值电压 (V)
    pub q_g: Scalar,            // 栅极总电荷 (C)
    pub c_iss: Scalar,          // 输入电容 (F)
    pub c_rss: Scalar,          // 反向传输电容 (F)
    pub v_dss: Scalar,          // 漏源击穿电压 (V)
    pub i_d_max: Scalar,        // 最大漏极电流 (A)
}

impl PowerMosfet {
    pub fn conduction_loss(&self, i_d: Scalar, rds_on_temp: Scalar, duty: Scalar) -> Scalar;
    pub fn switching_loss(&self, i_d: Scalar, v_dc: Scalar, freq: Scalar) -> Scalar;
    pub fn gate_drive_power(&self, v_gs: Scalar, freq: Scalar) -> Scalar;  // P = Qg·Vgs·f
    pub fn rds_on_temp(&self, temp_c: Scalar) -> Scalar;  // 温度修正
}

/// IGBT 简化模型
pub struct Igbt {
    pub v_ce_sat: Scalar,       // 饱和压降 (V)
    pub r_on: Scalar,           // 导通电阻 (Ω)
    pub e_on: Scalar,           // 开通能量 (J)
    pub e_off: Scalar,          // 关断能量 (J)
    pub v_ces: Scalar,          // 集射极击穿电压 (V)
    pub i_c_max: Scalar,        // 最大集电极电流 (A)
}

impl Igbt {
    pub fn conduction_loss(&self, i_c: Scalar, duty: Scalar) -> Scalar;
    pub fn switching_loss(&self, i_c: Scalar, v_dc: Scalar, freq: Scalar) -> Scalar;
    pub fn total_loss(&self, i_c: Scalar, v_dc: Scalar, freq: Scalar, duty: Scalar) -> Scalar;
}

/// 晶闸管简化模型
pub struct Thyristor {
    pub v_ak_on: Scalar,        // 导通压降 (V)
    pub i_l: Scalar,            // 擎住电流 (A)
    pub i_h: Scalar,            // 维持电流 (A)
    pub v_rrm: Scalar,          // 反向重复峰值电压 (V)
    pub t_q: Scalar,            // 关断时间 (s)
}
```

#### 3.4.2 `converters.rs` — 变换器拓扑

```rust
/// PWM 信号生成
pub fn pwm_signal(control_voltage: Scalar, carrier_amplitude: Scalar,
                   carrier_freq: Scalar, time: Scalar) -> Scalar;  // 0 或 1

/// 单相整流桥
pub fn single_phase_rectifier(v_ac_peak: Scalar, r_load: Scalar, diode_vf: Scalar)
    -> (Scalar, Scalar);  // (V_dc_avg, I_dc)

/// 三相整流桥（简化）
pub fn three_phase_rectifier(v_ac_line_rms: Scalar, r_load: Scalar)
    -> (Scalar, Scalar);

/// Buck 变换器 DC-DC
pub struct BuckConverter {
    pub vin: Scalar,    pub vout: Scalar,
    pub l: Scalar,      pub c: Scalar,
    pub fs: Scalar,     pub esr: Scalar,
}

impl BuckConverter {
    pub fn duty_cycle(&self) -> Scalar;           // D = Vout/Vin
    pub fn ripple_current(&self) -> Scalar;       // ΔIL
    pub fn ripple_voltage(&self) -> Scalar;       // ΔVout
    pub fn efficiency(&self, i_out: Scalar) -> Scalar;
}

/// Boost 变换器 DC-DC
pub struct BoostConverter {
    pub vin: Scalar,    pub vout: Scalar,
    pub l: Scalar,      pub c: Scalar,
    pub fs: Scalar,     pub esr: Scalar,
}

impl BoostConverter {
    pub fn duty_cycle(&self) -> Scalar;           // D = 1 - Vin/Vout
    pub fn ripple_current(&self) -> Scalar;
    pub fn ripple_voltage(&self) -> Scalar;
    pub fn efficiency(&self, i_out: Scalar) -> Scalar;
}

/// 全桥逆变器（SPWM）
pub struct FullBridgeInverter {
    pub v_dc: Scalar,
    pub modulation_index: Scalar,
    pub carrier_freq: Scalar,
    pub output_freq: Scalar,
}

impl FullBridgeInverter {
    pub fn fundamental_output(&self) -> Scalar;     // V₁ = m·Vdc/√2 (rms)
    pub fn thd_estimate(&self) -> Scalar;           // 总谐波失真估计
    pub fn switching_losses(&self, i_out: Scalar, device: &PowerMosfet) -> Scalar;
    pub fn conduction_losses(&self, i_out: Scalar, device: &PowerMosfet) -> Scalar;
}

/// 斩波器（Buck 和 Boost 模式）
pub enum ChopperMode { Buck, Boost, BuckBoost }
pub struct Chopper { ... }
```

#### 3.4.3 `motors.rs` — 电机模型

```rust
/// 直流电机模型
pub struct DcMotor {
    pub ra: Scalar,             // 电枢电阻 (Ω)
    pub la: Scalar,             // 电枢电感 (H)
    pub ke: Scalar,             // 反电动势常数 (V/(rad/s))
    pub kt: Scalar,             // 转矩常数 (N·m/A)
    pub j: Scalar,              // 转动惯量 (kg·m²)
    pub b: Scalar,              // 粘滞摩擦系数 (N·m·s/rad)
}

impl DcMotor {
    pub fn back_emf(&self, omega: Scalar) -> Scalar;        // E = Ke·ω
    pub fn torque(&self, i_a: Scalar) -> Scalar;            // T = Kt·Ia
    pub fn electrical_eq(&self, v_a: Scalar, i_a: Scalar, omega: Scalar) -> Scalar;  // di/dt
    pub fn mechanical_eq(&self, t_em: Scalar, t_load: Scalar, omega: Scalar) -> Scalar;  // dω/dt
    pub fn steady_state_speed(&self, v_a: Scalar, t_load: Scalar) -> Scalar;
}

/// 步进电机简化模型
pub struct StepperMotor {
    pub steps_per_rev: u32,      // 每转步数
    pub phase_resistance: Scalar,
    pub phase_inductance: Scalar,
    pub holding_torque: Scalar,  // N·m
}

impl StepperMotor {
    pub fn step_angle(&self) -> Scalar;              // deg
    pub fn pull_out_torque(&self, speed_rps: Scalar) -> Scalar;
}

/// 永磁同步电机 PMSM（dq 模型）
pub struct Pmsm {
    pub rs: Scalar,             // 定子电阻 (Ω)
    pub ld: Scalar,             // d 轴电感 (H)
    pub lq: Scalar,             // q 轴电感 (H)
    pub flux_pm: Scalar,        // 永磁体磁链 (Wb)
    pub pole_pairs: u32,
    pub j: Scalar,              // 转动惯量 (kg·m²)
}

impl Pmsm {
    pub fn electrical_eq_d(&self, i_d: Scalar, i_q: Scalar, omega_e: Scalar, v_d: Scalar) -> Scalar;
    pub fn electrical_eq_q(&self, i_d: Scalar, i_q: Scalar, omega_e: Scalar, v_q: Scalar) -> Scalar;
    pub fn torque(&self, i_d: Scalar, i_q: Scalar) -> Scalar;  // T = 1.5·p·(λpm·iq + (ld-lq)·id·iq)
    pub fn mechanical_eq(&self, t_e: Scalar, t_load: Scalar, omega_m: Scalar) -> Scalar;
}

/// 异步电机简化模型（稳态等效电路）
pub struct InductionMotor {
    pub rs: Scalar,             // 定子电阻 (Ω)
    pub rr: Scalar,             // 转子电阻 (Ω)
    pub ls: Scalar,             // 定子漏感 (H)
    pub lr: Scalar,             // 转子漏感 (H)
    pub lm: Scalar,             // 励磁电感 (H)
    pub pole_pairs: u32,
}

impl InductionMotor {
    pub fn slip(&self, sync_speed: Scalar, rotor_speed: Scalar) -> Scalar;
    pub fn torque_slip(&self, v_phase: Scalar, slip: Scalar, freq: Scalar) -> Scalar;
    pub fn breakdown_torque(&self, v_phase: Scalar, freq: Scalar) -> Scalar;
}
```

#### 3.4.4 `drive_ctrl.rs` — 驱动控制

```rust
/// PI 控制器（通用）
pub struct PiController {
    pub kp: Scalar,
    pub ki: Scalar,
    pub integral: Scalar,
    pub output_min: Scalar,
    pub output_max: Scalar,
}

impl PiController {
    pub fn update(&mut self, error: Scalar, dt: Scalar) -> Scalar;  // 含抗积分饱和
    pub fn reset(&mut self);
}

/// 矢量控制 FOC（面向 PMSM）
pub struct FocController {
    pub asr: PiController,      // 速度环
    pub acr_d: PiController,    // d 轴电流环
    pub acr_q: PiController,    // q 轴电流环
}

impl FocController {
    pub fn update(&mut self, omega_ref: Scalar, omega: Scalar, i_d: Scalar,
                  i_q: Scalar, theta_e: Scalar, dt: Scalar) -> (Scalar, Scalar);  // (vd, vq)
    pub fn inv_park_transform(v_d: Scalar, v_q: Scalar, theta: Scalar) -> (Scalar, Scalar);  // (vα, vβ)
    pub fn svpwm(v_alpha: Scalar, v_beta: Scalar, v_dc: Scalar) -> [Scalar; 3];  // 三相占空比
}

/// 闭环调速系统效率分析
pub fn drive_efficiency(input_power: Scalar, output_power: Scalar, motor_loss: Scalar,
                         converter_loss: Scalar) -> Scalar;

/// 转矩-速度特性曲线
pub fn torque_speed_curve(motor_type: &str, params: &[Scalar], v_dc: Scalar) -> Vec<(Scalar, Scalar)>;
```

#### 3.4.5 `thermal_power.rs` — 热分析

```rust
/// 功率器件结温计算（P=N 并联）
pub fn device_junction_temp(total_loss: Scalar, n_devices: usize,
                             rth_jc: Scalar, rth_ch: Scalar, rth_ha: Scalar,
                             ambient: Scalar) -> Scalar;

/// 散热器热阻（自然对流）
pub fn heatsink_thermal_resistance(volume: Scalar, fin_area: Scalar, airflow: Scalar) -> Scalar;

/// 功率损耗分解
pub struct PowerLossBreakdown {
    pub conduction_loss: Scalar,
    pub switching_loss: Scalar,
    pub core_loss: Scalar,       // 磁芯损耗
    pub copper_loss: Scalar,     // 铜损
    pub mechanical_loss: Scalar, // 机械损耗（电机）
}
```

---

### 3.5 阶段 22 — 电磁场与微波射频（`src/domains/emag/`）

#### 3.5.1 `physics.rs` — 电磁物理常量

```rust
/// 真空光速 (m/s)
pub const C: Scalar = 299792458.0;

/// 真空介电常数 (F/m)
pub const EPSILON_0: Scalar = 8.854187817e-12;

/// 真空磁导率 (H/m)
pub const MU_0: Scalar = 1.25663706212e-6;

/// 真空阻抗 (Ω)
pub const Z0: Scalar = 376.730313668;

/// 波数: k = 2π/λ
pub fn wave_number(lambda: Scalar) -> Scalar;

/// 波长: λ = c/f
pub fn wavelength(freq: Scalar) -> Scalar;

/// 趋肤深度: δ = √(2/(ω·μ·σ))
pub fn skin_depth(freq: Scalar, mu: Scalar, sigma: Scalar) -> Scalar;

/// 波阻抗（媒质中）: η = √(μ/ε)
pub fn wave_impedance(mu: Scalar, epsilon: Scalar) -> Scalar;
```

#### 3.5.2 `static_fields.rs` — 静电场/静磁场

```rust
/// 点电荷电场: E = Q/(4πεr²)
pub fn point_charge_field(q: Scalar, r: Scalar) -> Scalar;

/// 平行板电容: C = ε·A/d
pub fn parallel_plate_capacitance(area: Scalar, distance: Scalar, epsilon: Scalar) -> Scalar;

/// 无限长导线磁场: B = μ₀·I/(2πr)
pub fn wire_magnetic_field(current: Scalar, r: Scalar) -> Scalar;

/// 螺线管磁场: B = μ₀·n·I
pub fn solenoid_field(turns_per_meter: Scalar, current: Scalar) -> Scalar;

/// 静电场求解（有限差分简化版，1D）
pub struct ElectrostaticSolver1D {
    pub n_points: usize,
    pub boundary_conditions: Vec<(usize, Scalar)>,  // (index, potential)
}

impl ElectrostaticSolver1D {
    pub fn solve(&self) -> Vec<Scalar>;  // 高斯-赛德尔迭代
    pub fn electric_field(&self, potential: &[Scalar]) -> Vec<Scalar>;  // -dV/dx
}
```

#### 3.5.3 `transient_em.rs` — 瞬态/时谐电磁场

```rust
/// 时谐场复数表示
pub struct Phasor {
    pub magnitude: Scalar,
    pub phase: Scalar,       // 弧度
}

/// 平面波
pub struct PlaneWave {
    pub e0: Phasor,          // 电场相量
    pub h0: Phasor,          // 磁场相量 (E/η)
    pub direction: Coord3D,  // 传播方向
    pub freq: Scalar,
}

impl PlaneWave {
    pub fn poynting_vector(&self) -> Scalar;      // S = 0.5·Re(E×H*)
    pub fn power_density(&self) -> Scalar;        // W/m²
}

/// FDTD 1D 仿真内核（简化）
pub struct Fdtd1D {
    pub ez: Vec<Scalar>,      // 电场 (V/m)
    pub hy: Vec<Scalar>,      // 磁场 (A/m)
    pub dx: Scalar,           // 空间步长 (m)
    pub dt: Scalar,           // 时间步长 (s), Courant 稳定条件: dt ≤ dx/c
    pub n_steps: usize,
    pub boundary: BoundaryType,
}

pub enum BoundaryType {
    PEC,         // 理想电壁
    PMC,         // 理想磁壁
    Absorbing,   // 吸收边界（Mur 一阶）
}

impl Fdtd1D {
    pub fn new(n_cells: usize, dx: Scalar) -> Self;
    pub fn update_h(&mut self);  // H 场更新
    pub fn update_e(&mut self);  // E 场更新
    pub fn step(&mut self);      // 一个时间步
    pub fn run(&mut self);       // 完整运行
    pub fn inject_source(&mut self, position: usize, value: Scalar);
    pub fn probe(&self, position: usize) -> (Scalar, Scalar);  // (Ez, Hy)
}
```

#### 3.5.4 `devices.rs` — 电磁器件

```rust
/// 线圈电感: 多层螺线管近似
pub fn coil_inductance(n_turns: Scalar, radius: Scalar, length: Scalar, layers: u32) -> Scalar;

/// 互感: M = k·√(L₁·L₂)
pub fn mutual_inductance(l1: Scalar, l2: Scalar, k: Scalar) -> Scalar;

/// 变压器的简化模型
pub struct Transformer {
    pub n1: Scalar,             // 初级匝数
    pub n2: Scalar,             // 次级匝数
    pub lm: Scalar,             // 励磁电感 (H)
    pub ll: Scalar,             // 漏感 (H)
    pub r1: Scalar,             // 初级电阻 (Ω)
    pub r2: Scalar,             // 次级电阻 (Ω)
}

impl Transformer {
    pub fn turns_ratio(&self) -> Scalar;     // n = N₁/N₂
    pub fn open_circuit_test(&self, v1: Scalar, freq: Scalar) -> (Scalar, Scalar, Scalar);
    pub fn short_circuit_test(&self, v1: Scalar, freq: Scalar) -> (Scalar, Scalar, Scalar);
}

/// 偶极天线简化模型
pub struct DipoleAntenna {
    pub length: Scalar,         // m
    pub freq: Scalar,           // Hz
}

impl DipoleAntenna {
    pub fn radiation_resistance(&self) -> Scalar;   // Rr ≈ 80π²(L/λ)² for short dipole
    pub fn directivity(&self) -> Scalar;            // D ≈ 1.5 for half-wave dipole
    pub fn gain(&self, efficiency: Scalar) -> Scalar;  // G = η·D
    pub fn radiation_pattern(&self, theta: Scalar) -> Scalar;  // 归一化方向图
    pub fn bandwidth(&self, swr_max: Scalar) -> Scalar;
}

/// 永磁体简化模型
pub struct PermanentMagnet {
    pub br: Scalar,             // 剩磁 (T)
    pub hc: Scalar,             // 矫顽力 (A/m)
    pub volume: Scalar,         // 体积 (m³)
    pub shape: MagnetShape,
}

pub enum MagnetShape { Cylindrical { radius: Scalar, height: Scalar }, Block { dims: Coord3D } }
```

#### 3.5.5 `rf_microwave.rs` — 射频与微波

```rust
/// 史密斯圆图工具
pub fn smith_chart_impedance(z: num_complex::Complex<Scalar>, z0: Scalar) -> num_complex::Complex<Scalar>;  // Γ
pub fn gamma_to_z(gamma: num_complex::Complex<Scalar>, z0: Scalar) -> num_complex::Complex<Scalar>;

/// 谐振腔
pub struct ResonantCavity {
    pub shape: CavityShape,
    pub dimensions: Coord3D,
    pub wall_conductivity: Scalar,
}

pub enum CavityShape { Rectangular, Cylindrical }

impl ResonantCavity {
    pub fn resonant_freq(&self, mode: &str) -> Scalar;   // e.g., TE101, TM010
    pub fn quality_factor(&self, mode: &str) -> Scalar;   // Q
    pub fn bandwidth(&self, q: Scalar) -> Scalar;         // BW = f₀/Q
}

/// 微波网络 S 参数级联
pub fn cascade_s2p(s1: [[num_complex::Complex<Scalar>; 2]; 2],
                    s2: [[num_complex::Complex<Scalar>; 2]; 2]) -> [[num_complex::Complex<Scalar>; 2]; 2];

/// 微波放大器简化模型
pub struct RfAmplifier {
    pub gain_db: Scalar,        // 小信号增益 (dB)
    pub nf_db: Scalar,          // 噪声系数 (dB)
    pub p1db: Scalar,           // 1dB 压缩点 (dBm)
    pub oip3: Scalar,          // 输出三阶截点 (dBm)
}

impl RfAmplifier {
    pub fn linear_gain(&self) -> Scalar;           // 线性增益
    pub fn noise_temp(&self) -> Scalar;            // 等效噪声温度 (K)
    pub fn spurious_free_dr(&self, bw: Scalar) -> Scalar;  // SFDR (dB)
}

/// 传输线谐振器
pub fn transmission_line_resonator(length: Scalar, z0: Scalar, er: Scalar, n: u32, open_ended: bool) -> Scalar;
```

#### 3.5.6 `analysis.rs` — 电磁分析

```rust
/// 涡流损耗（薄板近似）
pub fn eddy_current_loss(freq: Scalar, b_peak: Scalar, thickness: Scalar,
                          conductivity: Scalar, volume: Scalar) -> Scalar;

/// 磁滞损耗（Steinmetz 方程）：P_h = k·f^α·B^β
pub fn hysteresis_loss(k: Scalar, freq: Scalar, b_peak: Scalar, alpha: Scalar, beta: Scalar) -> Scalar;

/// 焦耳热: P = I²·R
pub fn joule_heating(current: Scalar, resistance: Scalar) -> Scalar;

/// 辐射效率: η = Rr/(Rr + Rloss)
pub fn radiation_efficiency(r_rad: Scalar, r_loss: Scalar) -> Scalar;

/// 天线增益（dBi）
pub fn antenna_gain_dbi(directivity: Scalar, efficiency: Scalar) -> Scalar;

/// 雷达方程（简化）
pub fn radar_range_eq(pt: Scalar, gt: Scalar, gr: Scalar, sigma: Scalar,
                       lambda: Scalar, snr_min: Scalar, losses: Scalar) -> Scalar;

/// 电磁屏蔽效能 SE(dB) = R + A + M
pub fn shielding_effectiveness(freq: Scalar, material: &str, thickness: Scalar,
                                conductivity: Scalar, mu_r: Scalar) -> Scalar;
```

---

## 4. 实现顺序

### 第一轮：蓝图 + 光学（阶段 18）
1. 创建 `docs/blueprints/blue9.md`（本文档）
2. 创建 `src/domains/optical/mod.rs` + `physics.rs` — 光学物理常量
3. 创建 `src/domains/optical/ray.rs` — 几何光学
4. 创建 `src/domains/optical/wave.rs` — 波动光学
5. 创建 `src/domains/optical/laser.rs` — 激光/光纤/波导
6. 创建 `src/domains/optical/photoelectric.rs` — 光电转换
7. 创建 `src/domains/optical/analysis.rs` — 光学分析
8. 更新 `src/domains/mod.rs`

### 第二轮：声学（阶段 19）
9. 创建 `src/domains/acoustic/mod.rs` + `physics.rs`
10. 创建 `src/domains/acoustic/wave_prop.rs`
11. 创建 `src/domains/acoustic/cavity.rs`
12. 创建 `src/domains/acoustic/transducer.rs`
13. 创建 `src/domains/acoustic/vibro_acoustic.rs`
14. 创建 `src/domains/acoustic/analysis.rs`

### 第三轮：PCB（阶段 20）
15. 创建 `src/domains/pcb/mod.rs` + `transmission.rs`
16. 创建 `src/domains/pcb/power_integrity.rs`
17. 创建 `src/domains/pcb/signal_integrity.rs`
18. 创建 `src/domains/pcb/thermal.rs`
19. 创建 `src/domains/pcb/package.rs`

### 第四轮：电力电子（阶段 21）
20. 创建 `src/domains/powerelec/mod.rs` + `devices.rs`
21. 创建 `src/domains/powerelec/converters.rs`
22. 创建 `src/domains/powerelec/motors.rs`
23. 创建 `src/domains/powerelec/drive_ctrl.rs`
24. 创建 `src/domains/powerelec/thermal_power.rs`

### 第五轮：电磁场（阶段 22）
25. 创建 `src/domains/emag/mod.rs` + `physics.rs`
26. 创建 `src/domains/emag/static_fields.rs`
27. 创建 `src/domains/emag/transient_em.rs`
28. 创建 `src/domains/emag/devices.rs`
29. 创建 `src/domains/emag/rf_microwave.rs`
30. 创建 `src/domains/emag/analysis.rs`

### 第六轮：集成与修复
31. 更新 `src/domains/mod.rs` — 暴露所有新模块
32. 更新 `src/lib.rs` — 重导出关键类型
33. 全面测试 + clippy 修复

---

## 5. 测试要求

### 阶段 18 光学测试（25+）：
- 折射率模型：恒定折射率 n=1.5，Sellmeier 在校准波长处正确
- 几何光学：平面镜反射角等于入射角
- 薄透镜成像：1/f = 1/u + 1/v
- ABCD 矩阵：自由传播 + 透镜组合
- 双缝干涉：暗纹位置正确
- 单缝衍射：零级极大位置
- 光栅衍射：主极大位置 m·λ = d·sinθ
- 高斯光束：束腰半径 -> 远场发散角
- 光纤：V 数单模/多模判断
- 偏振：马吕斯定律
- 菲涅耳方程：垂直入射反射率

### 阶段 19 声学测试（20+）：
- 声速温度依赖
- 声压级计算（94 dB SPL = 1 Pa）
- 球面扩展 -6dB/倍距离
- 亥姆霍兹共振频率
- RT60 Sabine 公式
- 扬声器谐振频率
- 质量定律 TL
- A 计权修正
- 阻尼比估计

### 阶段 20 PCB 测试（20+）：
- 微带线阻抗计算
- 传输线传播延迟
- IR Drop 计算
- Buck 变换器纹波
- 去耦网络 SRF
- 反射系数
- 回波损耗
- 串扰峰值
- 结温计算
- 键合线电感

### 阶段 21 电力电子测试（20+）：
- 功率二极管导通压降
- MOSFET 导通损耗
- Buck 变换器占空比
- Boost 变换器纹波
- PWM 信号产生
- 直流电机稳态速度
- PMSM dq 方程
- PI 控制器
- 器件结温

### 阶段 22 电磁场测试（20+）：
- 点电荷电场
- 平行板电容
- 螺线管磁场
- 趋肤深度
- 平面波功率密度
- FDTD 1D 稳定运行
- 线圈电感
- 变压器匝比
- 偶极天线辐射电阻
- 史密斯圆图转换
- 涡流损耗
- 屏蔽效能

---

## 6. 验收标准

- [ ] `cargo build` 成功
- [ ] `cargo test` — 全部测试通过（0 失败，0 忽略）
- [ ] `cargo clippy --all-targets -- -D warnings` — 零警告
- [ ] 无 `todo!()`、`unimplemented!()` 或空函数体
- [ ] 所有新增代码有英文注释
- [ ] `domains/mod.rs` 仅暴露接口
- [ ] 每个模块至少 2 个测试（创建 + 行为验证）
- [ ] 所有已有测试保持通过（无回归）
- [ ] 光学模块产生物理合理的射线/波前行为
- [ ] 声学模块频率响应分析物理合理
- [ ] PCB 传输线阻抗与理论值一致
- [ ] 电力电子变换器稳态分析与经典公式一致
- [ ] 电磁 FDTD 内核 Courant 稳定条件满足

---

## 7. 与现有系统的集成

阶段 18~22 使用以下现有模块：
- `Block` trait — 所有仿真模块实现（阶段 1）
- `SignalValue`、`Scalar`、`Time` — 数据类型（阶段 1）
- `Coord3D`、`Transform4x4` — 坐标系统（阶段 10）
- `Dimension`、`Unit`、`Quantity` — 单位一致性（阶段 11）
- `LibraryDb`、`LibraryManager` — 资料库查找（阶段 12）
- `SimEngine`、`SimContext` — 执行引擎（阶段 2）
- `num_complex::Complex` — 复数支持（阶段 14）
- `Scheduler` — 模块执行排序（阶段 4）
