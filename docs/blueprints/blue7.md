# BLUE7 — 阶段 12~15：资料库系统、TCAD、SPICE 与数字/RTL 仿真

## 1. 概述

阶段 12~15 在已完成的内核（阶段 1~11）之上构建三个主要领域仿真能力及统一的数据库系统：

- **阶段 12（已完成）**：统一资料库系统 — TOML + SQLite 混合存储，支持 15 个资料库分类。已在 `src/db/mod.rs` 中实现，包含完整 CRUD、FTS5 全文搜索、TOML 加载和 12 个内置示例条目。
- **阶段 13**：半导体器件物理仿真（TCAD）— 晶体管级物理模型、漂移-扩散、器件电学特性表征。
- **阶段 14**：模拟电路仿真（SPICE 级）— MNA 求解器、RCL 无源/有源器件模型、DC/AC/瞬态分析。
- **阶段 15**：数字逻辑与 RTL 仿真 — 门电路、触发器、组合逻辑、CPU 微架构仿真。

**状态目标：** 100% — 编译通过、零 clippy 警告、全部测试通过、无占位代码、与现有 Block/Diagram/Engine 系统完全集成。

---

## 2. 模块架构

```
src/
  domains/
    mod.rs              — 模块接口 + 重导出
    tcad/               — [新增] 阶段 13：半导体 TCAD
      mod.rs            — 模块接口
      physics.rs        — 物理常量、载流子输运模型
      mosfet.rs         — MOSFET DC/AC 模型（Shichman-Hodges Level 1）
      bjt.rs            — BJT Ebers-Moll 模型
      iv_curve.rs       — IV/CV 曲线计算工具
      process.rs        — 基本工艺仿真（扩散、注入、氧化）
    analog/             — [新增] 阶段 14：SPICE 级模拟电路
      mod.rs            — 模块接口
      mna.rs            — 改进节点分析（MNA）矩阵构建/求解
      devices.rs        — R、C、L、二极管、MOSFET、BJT、运放元件模板
      analysis.rs       — DC 工作点、AC 小信号、瞬态分析
      noise.rs          — 基本噪声分析（热噪声、散粒噪声、闪烁噪声）
    digital/            — [新增] 阶段 15：数字逻辑与 RTL
      mod.rs            — 模块接口
      gates.rs          — 扩展逻辑门模块（与非、或非、同或、缓冲器、三态）
      sequential.rs     — D 触发器、JK 触发器、T 触发器、锁存器、移位寄存器
      combinational.rs  — 加法器、减法器、乘法器、译码器、ALU
      cpu.rs            — 简单 CPU 模型：寄存器堆、控制单元、流水线
      timing.rs         — 时序分析：建立/保持时间、时钟抖动、传播延迟
  db/
    mod.rs              — [阶段 12 ✅] 已实现（CRUD、FTS5、TOML 加载器）
```

---

## 3. 详细规格

### 3.1 阶段 12 — 统一资料库系统 [已完成]

已在 `src/db/mod.rs` 中实现。关键能力：
- 15 个资料库分类 via `LibraryCategory` 枚举
- `LibraryDb` — SQLite 支持 WAL 模式、FTS5 全文搜索、CRUD 操作
- `TomlLoader` — 从 `resources/data/**/*.toml` 加载条目
- `LibraryManager` — 高层 API，结合 SQLite 索引 + TOML 数据
- 12 个内置示例条目（铜、硅、地球、太阳、电阻等）
- 12 个测试覆盖全部操作

**除非需要与阶段 13~15 集成，否则无需进一步工作。**

### 3.2 阶段 13 — 半导体 TCAD（`src/domains/tcad/`）

#### 3.2.1 `physics.rs` — 物理常量与载流子输运

```rust
pub const Q: Scalar = 1.602176634e-19;         // 基本电荷（C）
pub const K_B: Scalar = 1.380649e-23;          // 玻尔兹曼常数（J/K）
pub const EPSILON_0: Scalar = 8.854187817e-12; // 真空介电常数（F/m）
pub const T_300K: Scalar = 300.0;              // 室温（K）
pub const V_T_300K: Scalar = 0.02585;          // 300K 热电压（V）
```

**MobilityModel** — 载流子迁移率模型（含温度和场依赖）：
- `electron_mobility(temp, field)` — 有效电子迁移率
- `hole_mobility(temp, field)` — 有效空穴迁移率

**漂移-扩散电流密度** — `drift_diffusion_current(q, n, p, μn, μp, ∇φ, ∇n, ∇p)`：
- 电子电流：Jn = q·(n·μn·∇φ + Dn·∇n)，Dn = μn·Vt（爱因斯坦关系）
- 空穴电流：Jp = q·(p·μp·∇φ - Dp·∇p)

**PN 结物理**：
- `built_in_potential(na, nd, ni, temp)` — 内建电势
- `depletion_width(na, nd, vbi, vr, ε)` — 耗尽层宽度
- `max_electric_field(na, nd, vbi, vr, ε)` — 最大电场
- `junction_capacitance(na, nd, vbi, vr, ε)` — 结电容

#### 3.2.2 `mosfet.rs` — MOSFET Shichman-Hodges Level 1 模型

**MosfetModel** 参数：
- `vto` — 阈值电压（V）
- `kp` — 跨导参数 KP = μ·Cox（A/V²）
- `lambda` — 沟道长度调制系数（1/V）
- `gamma` — 体效应系数（V^0.5）
- `phi` — 表面势（V）
- `w / l` — 沟道宽/长（m）
- `is_nmos` — true=NMOS, false=PMOS

**三个工作区**：
- **截止区**：Vgs ≤ Vth → Id = 0
- **线性区（三极管区）**：Vds < Vdsat → Id = β·(Veff - Vds/2)·Vds·(1 + λ·Vds)
- **饱和区**：Vds ≥ Vdsat → Id = β/2·Veff²·(1 + λ·Vds)

**小信号参数**：
- `mosfet_gm(model, vgs, vds, vbs)` — 跨导 gm = ∂Id/∂Vgs
- `mosfet_gds(model, vgs, vds, vbs)` — 输出电导 gds = ∂Id/∂Vds

**MosfetBlock** — 实现 Block trait，端口：g（栅极）、d（漏极）、s（源极）、b（衬底）输入，id（漏极电流）输出。

#### 3.2.3 `bjt.rs` — BJT Ebers-Moll 模型

**BjtModel** 参数：
- `is` — 饱和电流（A）
- `bf / br` — 正向/反向电流增益
- `vaf / var` — 正向/反向厄尔利电压（V）
- `nf / nr` — 正向/反向发射系数
- `is_npn` — true=NPN, false=PNP

**电流计算**：
- `bjt_collector_current(model, vbe, vbc)` — 集电极电流，含厄尔利效应
- `bjt_base_current(model, vbe, vbc)` — 基极电流
- `bjt_emitter_current(ic, ib)` — 发射极电流（KCL）

**小信号参数**：
- `bjt_gm(ic, vt)` — 跨导 gm = Ic/Vt
- `bjt_rpi(β, gm)` — 基极-发射极电阻 rπ = β/gm

**BjtBlock** — 实现 Block trait，端口：b、c、e（输入电压），ic、ib（输出电流）。

#### 3.2.4 `iv_curve.rs` — IV/CV 特性曲线

- `mosfet_iv_curve(model, vgs_range, vds_range, vbs)` — MOSFET 输出特性 Id-Vds 曲线族
- `mosfet_transfer_curve(model, vds, vgs_range, vbs)` — MOSFET 转移特性 Id-Vgs
- `mosfet_cv_curve(model, vgs_range)` — MOSFET 电容特性 Cgg-Vgs
- `bjt_iv_curve(model, vbe_list, vce_range)` — BJT 输出特性 Ic-Vce 曲线族
- `bjt_transfer_curve(model, vbe_range, vce)` — BJT 转移特性 Ic-Vbe

#### 3.2.5 `process.rs` — 基本工艺仿真

- `diffusion_profile(dose, diffusivity, time, x)` — 高斯扩散分布
- `diffusivity_arrhenius(D0, Ea, temp)` — 阿伦尼乌斯扩散系数
- `boron_diffusivity(temp)` / `phosphorus_diffusivity(temp)` / `arsenic_diffusivity(temp)` — 硅中常见掺杂剂扩散系数
- `implant_range(energy, m_ion, m_target)` — 离子注入投影射程（简化 LSS）
- `implant_straggle(energy, m_ion, m_target)` / `implant_profile(dose, rp, Δrp, x)` — 注入分布
- `oxide_thickness(time, temp, ambient)` — Deal-Grove 热氧化模型

### 3.3 阶段 14 — SPICE 模拟电路（`src/domains/analog/`）

#### 3.3.1 `mna.rs` — 改进节点分析

MNA 公式：`[G B; C D] * [v; i] = [s; 0]`

**MnaMatrix** — MNA 矩阵构建器：
- `stamp_resistor(ni, nj, R)` — 电阻模板
- `stamp_conductance(ni, nj, G)` — 电导模板
- `stamp_voltage_source(ni, nj, V, idx)` — 独立电压源模板
- `stamp_current_source(ni, nj, I)` — 独立电流源模板
- `stamp_vccs(ni, nj, nk, nl, gm)` — 压控电流源模板
- `solve()` — 高斯消元法求解节点电压和源电流

**MnaSolution** — 结果：`node_voltages`（节点电压）+ `source_currents`（源电流）

**solve_mna()** — 便捷函数：创建 MNA 矩阵、执行模板、求解一步到位。

#### 3.3.2 `devices.rs` — 元件模板与 Block 封装

**无源器件**：
- `ResistorBlock` — I = (Vp - Vn) / R
- `CapacitorBlock` — 后向欧拉伴随模型：Geq = C/dt, Ieq = -C/dt·V(t)
- `InductorBlock` — 后向欧拉伴随模型：Geq = dt/L

**有源器件**：
- `DiodeBlock` — Id = Is·(exp(Vd/(N·Vt)) - 1)，含线性化伴随模型
- `DiodeStamp` — 小信号电导 Gd = Is/(N·Vt)·exp(Vd/(N·Vt))
- `MosfetStamp` — 线性化小信号模型（Gds + gm·Vgs VCCS）
- `BjtStamp` — 线性化小信号模型（Gπ + gm·Vbe VCCS + Go）
- `OpAmpStamp` — 理想运放 VCVS 模型

**辅助模板**：
- `ResistorStamp` / `CapacitorStamp` / `InductorStamp` / `CurrentSourceStamp` / `VoltageSourceStamp`

#### 3.3.3 `analysis.rs` — 电路分析算法

**分析类型**：
- `DcOpPoint` — DC 工作点分析
- `DcSweep` — DC 扫描（源参数变化）
- `AcSweep` — AC 小信号频率扫描（线性/十倍频/八倍频）
- `Transient` — 瞬态时域分析（后向欧拉积分）
- `Noise` — 噪声分析

**分析函数**：
- `run_dc_op(num_nodes, num_vsources, stamp_fn)` — 单点 DC 分析
- `run_dc_sweep(num_nodes, num_vsources, config, stamp_fn)` — DC 扫描
- `run_ac_sweep(num_nodes, num_vsources, config, stamp_fn)` — AC 扫描
- `run_transient(num_nodes, num_vsources, config, stamp_fn)` — 瞬态分析

**结果类型**：`DcOpResult`、`AcResult`（含 `gain_db()` 和 `phase_deg()`）、`TransientResult`

#### 3.3.4 `noise.rs` — 噪声分析

- `thermal_noise_psd(R, T)` — 热噪声 PSD：4kT/R（V²/Hz）
- `thermal_noise_current_psd(G, T)` — 热噪声电流 PSD：4kT·G（A²/Hz）
- `shot_noise_psd(I)` — 散粒噪声 PSD：2qI（A²/Hz）
- `flicker_noise_psd(Kf, Af, I, f)` — 闪烁噪声 PSD：Kf·I^Af / f（A²/Hz）
- `rms_noise_voltage(psd, f1, f2)` — 带宽内 RMS 噪声电压
- `snr_db(Vsig, Vnoise)` — 信噪比（dB）
- `noise_figure_db(SNRin, SNRout)` — 噪声系数（dB）

### 3.4 阶段 15 — 数字逻辑与 RTL（`src/domains/digital/`）

#### 3.4.1 `gates.rs` — 扩展逻辑门

在 `blocks/logic.rs` 的基础上增加门类型：
- **LogicNand** — y = !(u1 && u2)
- **LogicNor** — y = !(u1 || u2)
- **LogicXnor** — y = (u1 == u2)
- **LogicBuffer** — y = u（缓冲器）
- **TriStateBuffer** — y = en ? u : 0.5（三态，高阻态输出 0.5）
- **LogicNotBlock** — y = !u（独立 NOT 门）

#### 3.4.2 `sequential.rs` — 时序元件

- **DFlipFlopBlock** — 上升沿触发 D 触发器，端口：d, clk, rst → q, qn
- **JKFlipFlopBlock** — 上升沿触发 JK 触发器：J=0,K=0 保持；J=0,K=1 复位；J=1,K=0 置位；J=1,K=1 翻转
- **TFlipFlopBlock** — 上升沿触发 T 触发器（en=1 翻转）
- **LatchBlock** — 电平敏感锁存器（en=1 透明）
- **ShiftRegisterBlock** — 串入并出移位寄存器

#### 3.4.3 `combinational.rs` — 组合逻辑

- **AdderBlock** — 宽度可配置的加法器，端口：a, b, cin → sum, cout
- **MultiplierBlock** — 无符号整数乘法器
- **DecoderBlock** — n 输入 → 2ⁿ 输出译码器
- **ALUBlock** — 算术逻辑单元：ADD、SUB、AND、OR、XOR、NOT、SHL、SHR 八种操作，输出 result + zero 标志 + carry 标志

#### 3.4.4 `cpu.rs` — 简单 CPU 模型

**SimpleCpu** — 最小但功能完整的 RISC 风格 CPU：
- 8 个 32 位通用寄存器
- 64KB 字节寻址内存
- 5 级流水线：取指（IF）、译码（ID）、执行（EX）、访存（MEM）、写回（WB）
- 16 位指令字

**指令集（8 条）**：

| 指令 | 操作码 | 格式 | 功能 |
|------|--------|------|------|
| ADD | 0000 | rd, rs1, rs2 | rd = rs1 + rs2 |
| SUB | 0001 | rd, rs1, rs2 | rd = rs1 - rs2 |
| AND | 0010 | rd, rs1, rs2 | rd = rs1 & rs2 |
| OR | 0011 | rd, rs1, rs2 | rd = rs1 \| rs2 |
| LW | 0100 | rd, [rs1+imm] | rd = mem[rs1+imm] |
| SW | 0101 | rs2, [rs1+imm] | mem[rs1+imm] = rs2 |
| BEQ | 0110 | rs1, rs2, offset | if rs1==rs2: PC += offset |
| ADDI | 0111 | rd, rs1, imm | rd = rs1 + imm |

**CpuInstruction** — 指令编解码：`opcode()`、`rd()`、`rs1()`、`rs2()`、`imm()`、工厂方法（`add()`、`sub()`、`and()`、`or()`、`lw()`、`sw()`、`beq()`、`addi()`）

**CpuProgram** — 程序容器 + `example_add()` 示例

**exec_one()** — 直接执行单条指令（绕过流水线，用于测试）

#### 3.4.5 `timing.rs` — 数字时序分析

**TimingAnalyzer** — 数字逻辑时序分析器：
- `gate_delays` — 门类型 → 传播延迟映射（默认值：INV=10ps, NAND2=15ps, DFF=50ps 等）
- `wire_delays` — 节点对 → 线延迟
- `clock_period`、`setup_time`、`hold_time`、`clock_jitter`

**关键方法**：
- `critical_path_delay(netlist, inputs, outputs)` — 最长路径延迟（拓扑排序 + 前向传播）
- `check_setup(path_delay)` — 建立时间检查：Tclk - Tpath - Tjitter ≥ Tsetup
- `check_hold(path_delay)` — 保持时间检查：Tpath - Tjitter ≥ Thold
- `setup_slack(path_delay)` / `hold_slack(path_delay)` — 时序裕量
- `max_frequency(path_delay)` — 最大工作频率
- `slack_report(path_delays)` — 多路径时序报告

---

## 4. 实现顺序

### 第一轮：蓝图 + 领域基础设施
1. 创建 `docs/blueprints/blue7.md`（本文档）
2. 创建 `src/domains/tcad/mod.rs` + `physics.rs` — 物理常量与输运模型
3. 创建 `src/domains/analog/mod.rs` + `mna.rs` — MNA 矩阵构建/求解
4. 创建 `src/domains/digital/mod.rs` + `gates.rs` — 扩展逻辑门
5. 更新 `src/domains/mod.rs` — 暴露新模块
6. 构建、测试、clippy 验证

### 第二轮：TCAD 器件模型
7. `domains/tcad/mosfet.rs` — MOSFET Shichman-Hodges 模型
8. `domains/tcad/bjt.rs` — BJT Ebers-Moll 模型
9. `domains/tcad/iv_curve.rs` — IV/CV 曲线计算
10. `domains/tcad/process.rs` — 基本工艺仿真

### 第三轮：SPICE 模拟电路
11. `domains/analog/devices.rs` — RCL/二极管/MOSFET/BJT 元件模板
12. `domains/analog/analysis.rs` — DC/AC/瞬态分析
13. `domains/analog/noise.rs` — 噪声分析

### 第四轮：数字逻辑与 RTL
14. `domains/digital/sequential.rs` — 触发器、锁存器、移位寄存器
15. `domains/digital/combinational.rs` — 加法器、乘法器、译码器、ALU
16. `domains/digital/cpu.rs` — 简单 CPU 模型
17. `domains/digital/timing.rs` — 时序分析

### 第五轮：集成、测试与验证
18. 所有新模块的综合测试
19. 跨模块集成（TCAD 模型 → SPICE 模板）
20. CPU 程序执行测试
21. 完整构建/测试/clippy 验证

---

## 5. 测试要求

### 阶段 13 TCAD 测试（15+）：
- 物理常量正确性（验证已知值）
- 迁移率模型温度依赖（高温迁移率降低）
- 迁移率场依赖（速度饱和效应）
- 漂移-扩散电流守恒（零梯度下零电流）
- PN 结内建电势（~0.7V @ 300K Si）
- MOSFET 各工作区电流（截止/线性/饱和）
- MOSFET 小信号参数（gm, gds）
- BJT 集电极电流指数依赖 Ic-Vbe
- BJT 电流增益 β = Ic/Ib
- PNP 电流方向
- 扩散剂量守恒（数值积分）
- 注入射程与能量关系
- 氧化厚度温度依赖

### 阶段 14 模拟电路测试（15+）：
- MNA 简单分压器
- MNA 电压源
- MNA VCCS
- MNA 奇异矩阵检测
- MNA 矩阵重置
- 方便函数 solve_mna
- 电阻/电容/二极管 Block 创建
- 电容/电感伴随模型
- DC 工作点分析
- DC 扫描（电压随源线性变化）
- AC 频率点生成（线性/十倍频）
- 瞬态分析
- AC 结果增益计算

### 阶段 15 数字逻辑测试（20+）：
- 与非门真值表（4 种输入组合）
- 或非门真值表
- 同或门真值表
- 缓冲器直通
- 三态使能/高阻
- NOT 门
- D 触发器边沿触发
- D 触发器复位
- JK 触发器翻转模式
- T 触发器翻转
- 锁存器透明
- 移位寄存器
- 加法器（含进位）
- 乘法器
- 译码器
- ALU 全部 8 种操作
- CPU 指令编解码
- CPU 程序执行（exec_one 绕过流水线）
- CPU 内存加载/存储
- CPU 分支跳转/不跳转
- 时序分析关键路径
- 建立/保持时间检查
- 最大频率计算

---

## 6. 验收标准

- [ ] `cargo build` 成功
- [ ] `cargo test` — 全部测试通过（0 失败，0 忽略）
- [ ] `cargo clippy --all-targets -- -D warnings` — 零警告
- [ ] 无 `todo!()`、`unimplemented!()` 或空函数体
- [ ] 所有新增代码有英文注释
- [ ] `domains/mod.rs` 仅暴露接口
- [ ] 每个模块至少 2 个测试（创建 + 行为验证）
- [ ] 阶段 12 数据库系统保持 100% 可用
- [ ] TCAD 模型产生物理合理的 IV 曲线
- [ ] MNA 求解器对基准电路产生正确节点电压
- [ ] 数字门电路实现正确真值表
- [ ] CPU 模型能正确执行简单程序
- [ ] 所有现有测试保持通过（无回归）

---

## 7. 与现有系统的集成

阶段 12~15 使用以下现有模块：
- `Block` trait — 所有仿真模块实现（阶段 1）
- `SignalValue`、`Scalar`、`Time` — 数据类型（阶段 1）
- `SimEngine`、`SimContext` — 执行引擎（阶段 2）
- `OdeSolver` — 瞬态分析积分（阶段 3）
- `Scheduler` — 模块执行排序（阶段 4）
- `Dimension`、`Unit`、`Quantity` — 单位一致性（阶段 11）
- `LibraryDb`、`LibraryManager` — 资料库查找（阶段 12）
- `AlgebraicLoopDetector` — 反馈环路检测（阶段 8）
- `SignalCache`、`EventQueue` — 信号/事件处理（阶段 4/6）
- `FIRFilter`、`IIRFilter` — 数字信号处理（阶段 7）
- 坐标类型 — 器件几何（阶段 10）
    pub fn stamp_voltage_source(&mut self, ni: usize, nj: usize, v: Scalar);
    /// Stamp a current source between nodes ni, nj.
    pub fn stamp_current_source(&mut self, ni: usize, nj: usize, i: Scalar);
    /// Stamp a VCCS (Voltage Controlled Current Source).
    pub fn stamp_vccs(&mut self, ni: usize, nj: usize, nk: usize, nl: usize, gm: Scalar);
    /// Solve the MNA system: compute node voltages and source currents.
    pub fn solve(&self) -> Result<(Vec<Scalar>, Vec<Scalar>), SimError>;
}
```

#### 3.3.2 `devices.rs` — Device Element Stamps

**ResistorBlock** — Block wrapping a resistor MNA stamp:
```rust
pub struct ResistorBlock {
    pub resistance: Scalar,
    // Ports: p, n (positive/negative nodes)
    // Outputs: v (voltage across), i (current through)
}
```

**CapacitorBlock** — Uses companion model for transient analysis:
- Backward Euler companion: `I(t+dt) = C/dt * V(t+dt) - C/dt * V(t)`
- Equivalent conductance `G_eq = C/dt` and current source `I_eq = -C/dt * V(t)`

**InductorBlock** — Companion model for transient analysis:
- Backward Euler companion: `V(t+dt) = L/dt * I(t+dt) - L/dt * I(t)`

**DiodeBlock** — Non-linear element solved via Newton iteration:
- `Id = Is * (exp(Vd/(n*Vt)) - 1)`
- Linearized companion: `I = I_eq + G_eq * V`

**MosfetBlock** — Transistor stamp (connects to TCAD model from Phase 13):
- Uses MnaMatrix::stamp_vccs for transconductance
- Adds drain-source conductance

#### 3.3.3 `analysis.rs` — Circuit Analysis Types

```rust
pub enum AnalysisType {
    /// DC operating point (steady-state).
    DcOpPoint,
    /// DC sweep (vary a source).
    DcSweep { source: String, start: Scalar, stop: Scalar, steps: usize },
    /// AC small-signal analysis (frequency sweep).
    AcSweep { start_freq: Scalar, stop_freq: Scalar, points: usize, scale: FreqScale },
    /// Transient analysis (time-domain).
    Transient { t_start: Scalar, t_stop: Scalar, t_step: Scalar },
    /// Noise analysis.
    Noise { input_source: String, output_node: usize, freq_sweep: AcSweep },
}

pub enum FreqScale { Linear, Decade, Octave }

pub struct DcOpResult {
    pub node_voltages: Vec<Scalar>,
    pub source_currents: Vec<Scalar>,
    pub power: Vec<Scalar>,
}

pub struct AcResult {
    pub freq: Vec<Scalar>,
    pub node_voltages: Vec<Vec<num_complex::Complex<Scalar>>>,
    pub gain_db: Vec<Vec<Scalar>>,
    pub phase_deg: Vec<Vec<Scalar>>,
}

pub struct TransientResult {
    pub time: Vec<Scalar>,
    pub node_voltages: Vec<Vec<Scalar>>,
}
```

#### 3.3.4 `noise.rs` — Noise Analysis

```rust
/// Thermal noise PSD: 4kT/R (V²/Hz) for resistors.
pub fn thermal_noise_psd(resistance: Scalar, temp: Scalar) -> Scalar

/// Shot noise PSD: 2qI (A²/Hz) for diodes/BJTs.
pub fn shot_noise_psd(current: Scalar) -> Scalar

/// Flicker noise PSD: Kf * I^Af / f (A²/Hz).
pub fn flicker_noise_psd(kf: Scalar, af: Scalar, current: Scalar, freq: Scalar) -> Scalar
```

### 3.4 Phase 15 — Digital Logic & RTL (`src/domains/digital/`)

#### 3.4.1 `gates.rs` — Extended Logic Gate Blocks

Extends `blocks/logic.rs` with additional gate types:

```rust
pub struct LogicNand { /* y = !(u1 && u2) */ }
pub struct LogicNor { /* y = !(u1 || u2) */ }
pub struct LogicXnor { /* y = !(u1 ^ u2) */ }
pub struct LogicBuffer { /* y = u (with drive strength) */ }
pub struct TriStateBuffer { /* y = en ? u : Z (high-impedance) */ }
pub struct LogicNotBlock { /* y = !u (single input NOT) */ }
```

Each implements `Block` trait with appropriate ports.

#### 3.4.2 `sequential.rs` — Sequential Elements

```rust
pub struct DFlipFlopBlock {
    pub initial_q: bool;
    // Ports: d, clk, rst -> q, qn
    // Rising-edge triggered D flip-flop
}

pub struct JKFlipFlopBlock {
    pub initial_q: bool;
    // Ports: j, k, clk, rst -> q, qn
}

pub struct LatchBlock {
    // Ports: d, en -> q, qn
    // Level-sensitive transparent latch
}

pub struct ShiftRegisterBlock {
    pub width: usize;
    pub initial: Vec<bool>;
    // Ports: din, clk -> dout, parallel_out[width]
}
```

#### 3.4.3 `combinational.rs` — Combinational Logic

```rust
pub struct AdderBlock {
    pub width: usize;
    // Ports: a[width], b[width], cin -> sum[width], cout
}

pub struct MultiplierBlock {
    pub width: usize;
    // Ports: a[width], b[width] -> product[2*width]
}

pub struct DecoderBlock {
    pub input_width: usize;
    // Ports: in[input_width] -> out[2^input_width]
}

pub struct ALUBlock {
    pub width: usize;
    // Ports: a[width], b[width], opcode -> result[width], flags
    // Operations: ADD, SUB, AND, OR, XOR, NOT, SHL, SHR
}
```

#### 3.4.4 `cpu.rs` — Simple CPU Model

A minimal but functional CPU model demonstrating the digital simulation system:

```rust
pub struct SimpleCpu {
    pub reg_file: [u32; 8],        // 8 general-purpose registers
    pub pc: u32,                    // Program counter
    pub memory: Vec<u8>,           // Instruction/data memory
    pub pipeline: PipelineStages,  // Fetch, Decode, Execute, Memory, WriteBack
}

pub struct PipelineStages {
    pub if_reg: IFRegister,   // Instruction fetch
    pub id_reg: IDRegister,   // Instruction decode
    pub ex_reg: EXRegister,   // Execute
    pub mem_reg: MEMRegister, // Memory access
    pub wb_reg: WBRegister,   // Write back
}
```

**Instruction set** (RISC-like, 16-bit):
- `ADD rd, rs1, rs2` (opcode 0000)
- `SUB rd, rs1, rs2` (opcode 0001)
- `AND rd, rs1, rs2` (opcode 0010)
- `OR rd, rs1, rs2` (opcode 0011)
- `LW rd, [rs1+imm]` (opcode 0100)
- `SW rs2, [rs1+imm]` (opcode 0101)
- `BEQ rs1, rs2, offset` (opcode 0110)
- `ADDI rd, rs1, imm` (opcode 0111)

#### 3.4.5 `timing.rs` — Digital Timing Analysis

```rust
pub struct TimingAnalyzer {
    pub gate_delays: HashMap<String, Scalar>,     // Gate type -> delay (s)
    pub wire_delays: HashMap<(usize, usize), Scalar>, // Node pair -> delay
    pub clock_period: Scalar,
    pub setup_time: Scalar,
    pub hold_time: Scalar,
}

impl TimingAnalyzer {
    /// Compute critical path delay through a logic network.
    pub fn critical_path_delay(&self, netlist: &[GateConnection]) -> Scalar;
    /// Check setup time violation.
    pub fn check_setup(&self, path_delay: Scalar) -> bool;
    /// Check hold time violation.
    pub fn check_hold(&self, path_delay: Scalar) -> bool;
    /// Report slack for all timing paths.
    pub fn slack_report(&self, path_delays: &[Scalar]) -> Vec<(usize, Scalar)>;
}
```

---

## 4. Implementation Order

### Round 1: Blueprint + Basic Domain Infrastructure
1. Create `docs/blueprints/blue7.md` (this file)
2. Create `src/domains/tcad/mod.rs` + `physics.rs` — constants and transport models
3. Create `src/domains/analog/mod.rs` + `mna.rs` — MNA matrix builder/solver
4. Create `src/domains/digital/mod.rs` + `gates.rs` — extended logic gates
5. Update `src/domains/mod.rs` — expose all new modules
6. Update `src/lib.rs` — update re-exports

### Round 2: TCAD Device Models
7. `domains/tcad/mosfet.rs` — MOSFET Shichman-Hodges model
8. `domains/tcad/bjt.rs` — BJT Ebers-Moll model
9. `domains/tcad/iv_curve.rs` — IV/CV curve computation
10. `domains/tcad/process.rs` — Basic process simulation

### Round 3: SPICE Analog Circuit
11. `domains/analog/devices.rs` — RCL/diode/MOSFET/BJT element stamps
12. `domains/analog/analysis.rs` — DC/AC/transient analysis
13. `domains/analog/noise.rs` — Noise analysis

### Round 4: Digital Logic & RTL
14. `domains/digital/sequential.rs` — Flip-flops, latches, shift registers
15. `domains/digital/combinational.rs` — Adder, multiplier, decoder, ALU
16. `domains/digital/cpu.rs` — Simple CPU model
17. `domains/digital/timing.rs` — Timing analysis

### Round 5: Integration, Tests, and Verification
18. Integration tests for all new modules
19. Cross-module integration (TCAD models → SPICE stamps)
20. CPU program execution test
21. Full build/test/clippy verification

---

## 5. Testing Requirements

### Phase 13 TCAD tests (15+):
- Physical constants correctness (check known values)
- Mobility model temperature dependence
- Drift-diffusion current conservation
- PN junction built-in potential at 300K
- Depletion width calculation
- MOSFET drain current in triode/saturation regions
- MOSFET transconductance computation
- BJT collector current vs Vbe (exponential)
- BJT beta measurement
- IV curve data point correctness
- Diffusion profile integral (dose conservation)
- Implant range vs energy

### Phase 14 Analog tests (15+):
- MNA matrix construction for simple resistor divider
- MNA solve for voltage divider (verify Vout)
- MNA solve for RCL circuit
- ResistorBlock port I/O
- Capacitor companion model (transient step)
- Diode IV characteristic
- DC operating point (voltage divider)
- Transient analysis (RC charging)
- AC sweep magnitude/phase

### Phase 15 Digital tests (20+):
- LogicNand truth table (all 4 input combinations)
- LogicNor truth table
- TriStateBuffer high-impedance output
- DFlipFlop edge-triggered behavior
- JKFlipFlop toggle mode
- ShiftRegister shift operation
- AdderBlock addition (multiple bit widths)
- MultiplierBlock multiplication
- DecoderBlock output selection
- ALUBlock all operations
- SimpleCpu fetch-execute cycle
- SimpleCpu program: add two numbers
- TimingAnalyzer critical path
- Setup/hold violation detection

---

## 6. Acceptance Criteria

- [ ] `cargo build` succeeds
- [ ] `cargo test` — all tests pass (0 failed, 0 ignored)
- [ ] `cargo clippy --all-targets -- -D warnings` — zero warnings
- [ ] No `todo!()`, `unimplemented!()`, or empty function bodies
- [ ] All new code has English comments
- [ ] `domains/mod.rs` only exposes interfaces
- [ ] Each module has at least 2 tests (creation + behavior verification)
- [ ] Phase 12 database system remains 100% operational
- [ ] TCAD models produce physically reasonable IV curves
- [ ] MNA solver produces correct node voltages for benchmark circuits
- [ ] Digital gates implement correct truth tables
- [ ] CPU model executes a simple program correctly
- [ ] All existing tests remain passing (no regression)

---

## 7. Integration with Existing System

Phase 12~15 consume from existing phases:
- `Block` trait — all simulation blocks implement this (Phase 1)
- `SignalValue`, `Scalar`, `Time` — data types (Phase 1)
- `SimEngine`, `SimContext` — execution engine (Phase 2)
- `OdeSolver` — for transient analysis integration (Phase 3)
- `Scheduler` — for block execution ordering (Phase 4)
- `Dimension`, `Unit`, `Quantity` — unit consistency (Phase 11)
- `LibraryDb`, `LibraryManager` — for library lookup (Phase 12)
- `AlgebraicLoopDetector` — for feedback loop detection (Phase 8)
- `SignalCache`, `EventQueue` — for signal/event handling (Phase 4/6)
- `FIRFilter`, `IIRFilter` — for digital signal processing (Phase 7)
- Coordinate types — for device geometry (Phase 10)
