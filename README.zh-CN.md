# SCIcoRS — 统一仿真内核

[English Documentation](README.md) | [更新日志](CHANGELOG.md) | [开发清单](docs/checklist/CHECKLIST.MD) | [路线图](docs/blueprints/roadmap.md) | [设计原则](docs/blueprints/principle.md)

> **SCI**entific **co**mputing & **R**eality **S**imulation —— 一个统一所有学科、尺度和领域的工程与科学仿真通用内核。

---

## 概述

SCIcoRS 提供**单一架构**用于建模、仿真和数据管理，实现从最小芯片到最大宇宙系统的无缝集成。

### 尺度覆盖

| 尺度 | 示例 |
|------|------|
| **纳米 (10⁻⁹ m)** | 分子、芯片晶体管、量子点 |
| **微米 (10⁻⁶ m)** | 细胞、MEMS 器件、微流控 |
| **毫米 (10⁻³ m)** | PCB 走线、生物器官、电子元件 |
| **米 (10⁰ m)** | 机械系统、人体、车辆 |
| **千米 (10³ m)** | 建筑物、地形、设备安装 |
| **光年 (10¹⁶ m)** | 恒星、星系、宇宙结构 |

### 统一架构

- **一套**坐标系（1D/2D/3D，笛卡尔/极坐标/柱坐标/球坐标）
- **一套**量纲/单位系统（7 个 SI 基本量纲，自动转换）
- **一套**求解器引擎（ODE/DAE，刚性/非刚性，稀疏，非线性）
- **一套**可扩展数据库（TOML 数据 + SQLite 索引）

---

## 架构（7 层）

```
┌────────────────────────────────────────────────────────────────┐
│  bindings/   — Python API、插件系统、C ABI、数据 I/O (STL/STEP/Mesh)  │
│  cli/        — 命令行工具链（check/run/status/results/validate）     │
│  validation/ — 基准、收敛研究、不变量、验证报告                       │
│  postproc/   — 数据记录、可视化、报告、数据集、批处理、HIL             │
│  analysis/   — 灵敏度、校准、不确定性、优化                          │
│  coupling/   — 多物理场耦合总线、跨尺度映射                            │
│  domains/    — 19 个领域专用仿真模块                                 │
│  blocks/     — 标准模块库（信号源、数学运算、逻辑门...）               │
│  runtime/    — 上下文、引擎、求解器、调度器、事件、状态、检查点、弹性执行 │
│  core/       — Block、Port、Link、Diagram、类型、坐标、单位、计算、网格   │
└────────────────────────────────────────────────────────────────┘
```

### 各层详情

#### `core/` — 数据模型层
仿真的"名词"层，提供构建所有仿真模型的基础组件。

- **Block** — 基本功能仿真单元，包含端口、参数和生命周期
- **Port / Link** — 类型化 I/O 接口和定向信号连接
- **Diagram** — 互联模块拓扑结构，支持序列化（JSON/TOML）和验证
- **Component** — 可复用的组件模板系统
- **Signal** — 连续、离散、事件和总线信号类型
- **Tensor** — N 维数组类型
- **State / IO / Dependency** — 状态变量、I/O 规格和模块间依赖声明
- **Coord** — 1D/2D/3D 坐标系（笛卡尔、极坐标、柱坐标、球坐标）+ `Transform4x4`
- **Units** — 7 个 SI 基本量纲、导出量纲、`Unit`/`Quantity` 自动转换
- **Compute** — 统一数学平台：矩阵运算、向量运算、FFT、数值积分、特征值求解（Jacobi、子空间迭代）

#### `runtime/` — 仿真执行层
仿真的"动词"层，驱动数据模型上的执行。

| 子模块 | 说明 |
|--------|------|
| **Context** | 集中式时间、模式（`Normal`/`RealTime`/`SingleStep`/`Breakpoint`）、生命周期和共享数据 |
| **Engine** | 顶层编排器：生命周期管理、时间推进、模块执行排序 |
| **State** | 统一的连续 + 离散状态管理，支持快照 |
| **Solvers** | 定步长（Euler/RK4/Heun/Midpoint）、自适应（RK45/RK23/CashKarp）、刚性（BackwardEuler/Trapezoidal/BDF2）、DAE（index-1）、非线性（Newton-Raphson）、线性（稠密 LU、稀疏 CSR）|
| **Scheduler** | 拓扑排序、信号流分析、混合连续/离散/事件/多速率调度、时钟域隔离 |
| **Workflow** | 基于 DAG 的任务编排，支持并行/串行阶段、屏障同步、流水线 |
| **Event** | 时间排序事件队列、过零检测、外部/条件触发器 |
| **Discrete** | 数字滤波器（FIR/IIR）、积分器、计数器、定时器、PLC 逻辑（AND/OR/NAND/NOR/XOR/NOT 门）|
| **Algebraic** | 代数环检测、不动点/松弛迭代、数值防护 |

#### `blocks/` — 标准模块库
内置仿真模块，用于快速构建模型。

- **sources** — 常量、正弦波、方波、阶跃、脉冲、噪声
- **math** — 加法器、减法器、乘法器、除法器、增益、三角函数、矩阵乘法
- **logic** — AND/OR/NOT/XOR 门、比较器、多路选择器、开关、限幅器
- **continuous** — 积分器、PID 控制器、传递函数、状态空间
- **discrete_ctrl** — 单位延迟、离散滤波器、离散 PID
- **sinks** — 示波器、图表缓冲区、数据记录器、数字显示

#### `domains/` — 19 个领域专用仿真模块

| # | 领域 | 模块 | 覆盖范围 |
|---|------|------|----------|
| 13 | **TCAD** | `tcad/` | MOSFET/BJT 模型、漂移扩散、掺杂分布、CV/IV 曲线、迁移率模型、氧化 |
| 14 | **模拟电路** | `analog/` | MNA 矩阵、R/L/C/D/Diode/OpAmp/MOSFET 模板、DC 工作点/扫描、AC 扫描、瞬态、噪声分析 |
| 15 | **数字电路** | `digital/` | 逻辑门、触发器、ALU、解码器、乘法器、移位寄存器、CPU 流水线、时序分析 |
| 16 | **分子动力学** | `molbio/` | 力场（LJ、Harmonic）、积分器、能量最小化、RMSD、氢键、二面角 |
| 17 | **细胞/组织** | `cellbio/` | 细胞模型、种群动态、生物反应器（分批/补料/连续）、生长动力学 |
| 18 | **光学** | `optical/` | 光线追踪、高斯光束、Jones/Mueller 矩阵、光栅、光纤、波导、太阳能电池 |
| 19 | **声学** | `acoustic/` | 声压级、RT60、房间模式、传输损耗、BEM、扬声器、麦克风、加速度计 |
| 20 | **PCB** | `pcb/` | 传输线（微带/带状线/共面波导）、PDN 阻抗、眼图、热分析、S2P/T 参数 |
| 21 | **电力电子** | `powerelec/` | Buck/Boost/单相/三相变换器、电机（直流/异步/PMSM/步进）、FOC、IGBT |
| 22 | **电磁场/RF** | `emag/` | 1D/3D FDTD（Yee 算法）、静电、天线（偶极子、阵列）、RCS、Smith 圆图、趋肤深度 |
| 23 | **生物医学** | `bio_medical/` | Hodgkin-Huxley、Windkessel、房室 PK/PD、组织力学、扩散、肿瘤模型 |
| 24 | **化工** | `chemical/` | 间歇/CSTR/PFR 反应器、反应动力学、化学平衡、精馏、换热器 NTU |
| 25 | **结构力学** | `structural/` | 有限元（桁架/梁/壳/实体）、非线性有限元（Newton-Raphson）、SDOF、疲劳、显式动力学 |
| 26 | **热力学** | `thermal/` | 1D/2D/3D 热传导（ADI/SOR）、对流、辐射、相变、热管 |
| 27 | **流体 (CFD)** | `fluid/` | 2D/3D Navier-Stokes（投影法）、2D 可压缩 NS（Roe 格式）、湍流（k-ε RANS、Smagorinsky LES）、多相流 VOF |
| 28 | **多体动力学** | `multibody/` | 刚体、约束、碰撞检测/响应、四元数、AABB |
| 29 | **航空航天** | `aerospace/` | 六自由度飞行器、ISA/高空大气、气动力学、热防护、火箭推力 |
| 30 | **量子计算** | `quantum/` | 态矢量、密度矩阵、MPS（张量网络）、VQE/QAOA/Grover/HHL/QFT、Lindblad 主方程 |
| 31 | **天体物理** | `astrophysics/` | N 体、ΛCDM 宇宙学、2D MHD（HLL Riemann 求解器）|

#### `coupling/` — 多物理场耦合总线
实现跨域和跨尺度联合仿真的统一耦合总线。

- 物理场类型注册、场映射/网格间插值（RBF）
- 跨尺度耦合（纳米 → 微米 → 米 → 宇宙）含 RVE 均匀化
- 收敛控制（不动点、松弛、Aitken）
- 时间同步和耦合迭代调度

#### `postproc/` — 后处理与可视化
- **数据记录** — 流式数据记录/回放、3D 场快照、离线分析（RMS、FFT）
- **可视化** — 图表、等高线、等值面、矢量场、体积切片
- **报告** — 带章节和表格的仿真报告、数据导出（CSV、JSON、HDF5、VTK、XLSX）
- **批处理** — 参数扫描、优化循环、求解器基准测试
- **HIL 支持** — 硬件在环 I/O 通道和运行器

#### `bindings/` — 跨平台与扩展系统
- **Python** 脚本桩 — 运行仿真、读取信号、注册自定义模块、查询库
- **插件系统** — 模块、求解器和后处理注册表，支持清单加载、API 版本兼容校验与分阶段隔离加载
- **C ABI** — 稳定的 `extern "C"` 外观层，不透明句柄、显式 `scico_free`、稳定错误码
- **数据 I/O** — STEP/STL 网格导入/导出、通用网格格式
- **平台** — OS 检测、路径规范化、云/分布式运行器

#### `core::mesh/` — 网格、区域与场工作流
- 拓扑契约（维度、单位、ID、连通性、边界面、区域标签）
- 命名材料/边界区域，含结构性与质量校验
- 节点/单元/面上的标量、向量、张量场，含守恒校验的跨网格映射
- 误差指标 / 标记 / 细化接口，含具体三角形细化器
- 与既有 STEP/STL/通用网格 I/O 的无损桥接（保留区域名与单位）

#### `analysis/` — 灵敏度、校准、不确定性与优化
- 数据契约：`ParameterSpec`、`ObservationSet`、`ObjectiveSpec`、`AnalysisResult`
- 有限差分局部灵敏度与梯度排序
- Levenberg–Marquardt 参数估计，含拟合诊断与逐次运行溯源
- Monte-Carlo / 拉丁超立方不确定性，含可复现种子 RNG
- 有界 Nelder–Mead 优化与全因子实验设计

#### `validation/` — 数值验证与可信度基线
- 带来源、精度标注与**具理由容差**的版本化基准
- 收敛研究估计观测阶（数据不足时拒绝捏造）
- 可注册物理不变量，含适用性与源汇项声明
- 结果比较，将插值对齐误差与求解误差分开报告
- 机器可读报告，分别统计 通过/失败/跳过/不适用

#### `runtime/` — 检查点与弹性执行
- 版本化、崩溃安全的原子检查点，含状态/事件/RNG/记录器快照
- 模型/求解器/插件兼容性检查，不兼容检查点被拒绝并列出差异
- 六种运行结束态（正常/取消/资源限制/数值失败/IO 失败/超时）
- 协作式取消、资源预算、失败分类重试与退避

---

## 计算平台

所有领域模块将数学运算委托给统一的 `core::compute` 模块：

| 运算 | 实现 |
|------|------|
| **矩阵** | 乘法、转置、行列式、求逆、LU/Cholesky 分解 |
| **稀疏** | COO/CSR/CSC、SpMV、CG/MINRES/GMRES(m)/BiCGSTAB、Jacobi/ILU(0) 预条件 |
| **最小二乘** | Householder QR、截断 SVD、最小范数解、秩/条件数诊断 |
| **向量** | 点积、叉积、范数、归一化、线性/样条插值 |
| **FFT** | 基-2 Cooley-Tukey FFT，用于频谱分析 |
| **积分** | 梯形、Simpson、Gauss-Legendre 数值积分 |
| **特征值** | Jacobi 方法、子空间迭代 |
| **并行** | 基于 `rayon` 的计算密集型循环并行化 |
| **GPU** | wgpu 30.0.1 计算着色器（可选 `gpu` 特性）|

这消除了跨领域模块存在的 5 份 Gaussian 消元副本。

---

## 科学计算工具链（BLUE13，阶段 35–41）

除领域模型外，SCIcoRS 还提供真实科研工作流所需的通用数值能力，均已实现、
测试且可观测，无任何桩代码：

| 能力 | 模块 | 提供内容 |
|------|------|----------|
| **稀疏线性代数** | `core::compute::sparse` | COO/CSR/CSC、SpMV、CG/MINRES/GMRES(m)/BiCGSTAB（带显式停止原因）、预条件器、矩阵自由算子 |
| **最小二乘** | `core::compute::least_squares` | Householder QR、单边 Jacobi SVD、最小范数解、秩/条件数诊断 |
| **分析与不确定性** | `analysis` | 有限差分灵敏度、LM 校准、Monte-Carlo/拉丁超立方、有界优化、因子设计 |
| **网格与场** | `core::mesh` | 拓扑、命名区域、质量校验、场映射（含守恒误差）、自适应细化、STEP/STL 桥接 |
| **检查点与弹性** | `runtime::checkpoint`、`runtime::execution` | 崩溃安全原子检查点、兼容性检查、六态结束、取消、资源预算、重试 |
| **自描述数据集** | `postproc::dataset` | 版本化清单、真实单位换算、分块原子写入、CRC-32、schema 迁移、CSV/JSON 适配 |
| **数值验证** | `validation` | 带理由容差的基准、收敛阶研究、可注册不变量、对齐感知比较、四态报告 |
| **工具链与绑定** | `cli`、`bindings` | 稳定退出码的 `scico` CLI、稳定 C ABI、插件兼容与诊断 |

### 稳健数值求解

迭代 Krylov 求解器绝不把失败报成成功：达到迭代上限返回
`StopReason::MaxIterationsReached`，崩溃或停滞也会如实报告。算法/输入不匹配
（如在非对称矩阵上用 CG）返回具体错误，而非默默切换方法。

### 可信由构造保证

验证工具链拒绝捏造信心：仅当数据支持时才报告收敛阶（数据不足或非单调时
明确返回“非渐近”），基准容差必须附带理由，未执行项与已通过项分开统计。
参见 `examples/validation_study.rs`（真实求解器贯穿完整验证流程）。

---

## 项目统计

| 指标 | 数值 |
|------|------|
| Rust 源文件数 | 300+ |
| 代码行数 | ~90,000 |
| 测试数 | **2529 通过**（默认）/ **2563**（`--features gpu`）✅ |
| 测试失败 | **0** ✅ |
| 忽略测试 | **0** ✅ |
| Clippy 警告 | **0**（`-D warnings`），含/不含 `gpu` 均通过 ✅ |
| 构建配置 | Release 模式，LTO fat，codegen-units=1 |
| 默认依赖图 | **不含 `wgpu`**（GPU 完全可选）✅ |
| 文档文件数 | 34（蓝图、清单、日志） |

---

## 依赖项

| Crate | 版本 | 用途 |
|-------|------|------|
| `serde` / `serde_json` | 1.x | 序列化 |
| `toml` | 1.1 | 人类可读数据存储 |
| `rusqlite` | 0.40 | SQLite 索引与查询 |
| `num-complex` | 0.4 | 复数支持 |
| `rayon` | 1.x | 数据并行 |
| `matrixmultiply` | 0.3 | 纯 Rust SIMD GEMM 内核 |
| `wgpu`（可选） | 30.0.1 | GPU 计算后端（`gpu` 特性）|

---

## 开发阶段（清单）

详见[完整开发清单](docs/checklist/CHECKLIST.MD)了解全部 33 个阶段的详细进度。

**阶段 1-7（核心框架）：** ✅ 100% 完成
- 核心模型内核（Block/Port/Link/Diagram）
- 仿真上下文与时间系统
- 通用数值求解器系统（ODE/DAE/非线性）
- 调度与执行引擎
- 工作流编排（DAG）
- 事件与触发系统
- 离散与多速率系统

**领域阶段（13-31）：** 全部 19 个领域模块完整实现，含计算逻辑、测试和零警告。

**集成阶段（32-34）：** 耦合总线、后处理、绑定 —— 全部完成。

**科学计算阶段（35-41，BLUE13）：** 稀疏线性代数，灵敏度/校准/不确定性/优化，
网格与场工作流，检查点与弹性执行，自描述数据集，数值验证，以及 CLI/C ABI/插件
工具链 —— 全部完成。详见[更新日志](CHANGELOG.md)与[开发日志](docs/log/log20261007-1.md)。

---

## 命令行工具

`scico` 二进制驱动 CLI 库（成功返回 0，出错返回稳定的非零退出码）：

```sh
cargo build --release --bin scico

scico check   project.json      # 校验项目配置、模型与输出路径
scico run     project.json      # 一次性有界运行 → 结果数据集
scico status  out/              # 查询运行状态 / 恢复点
scico results out/              # 检视结果数据集清单
scico cancel  out/ 0.5          # 取消运行并记录恢复点
scico resume  project.json      # 恢复被取消的运行
scico validate model diagram.json
scico version
```

## 示例

```sh
cargo run --example validation_study   # 真实求解器贯穿基准 + 收敛 + 不变量
cargo run --example gpu_bench          # CPU vs GPU GEMM 基准
cargo run --example compute_bench      # 计算原语微基准
```

---

## 数据与可扩展性

- **数据库：** TOML 用于人类可读数据，SQLite 用于快速索引和搜索
- **库：** 材料、天体、流体、截面、电气、逻辑门、芯片、板级、光学、声学、化学品、生物分子、细胞、培养基、半导体工艺
- **可扩展：** 公开/私有库、自定义数据、导入/导出、版本控制
- **LibraryManager** — 完整 CRUD 操作、TOML 批量导入、分类列表、关键词搜索

---

## 快速开始

```rust
use scico_rs::*;

// 创建一个框图
let mut diagram = Diagram::new("my_simulation");

// 添加模块
let src = SineSource::new("src", 1.0, 60.0);  // 60 Hz 正弦波
let gain = Gain::new("gain", 2.0);
let scope = Scope::new("scope", 1024);

diagram.add_block(Box::new(src));
diagram.add_block(Box::new(gain));
diagram.add_block(Box::new(scope));

// 连接模块
diagram.connect("src:output", "gain:input").unwrap();
diagram.connect("gain:output", "scope:input").unwrap();

// 创建仿真上下文并运行
let ctx = SimContext::new(TimeConfig::new(0.0, 1.0, 1e-4));
let mut engine = SimEngine::new(diagram, ctx);
let summary = engine.run().unwrap();
println!("完成 {} 步，仿真时间 {}", summary.total_steps, summary.final_time);
```

---

## 许可证

双许可证，任选其一：

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

SPDX 表达式为 `MIT OR Apache-2.0`。

---

[English Documentation](README.md) | [更新日志](CHANGELOG.md) | [开发清单](docs/checklist/CHECKLIST.MD) | [路线图](docs/blueprints/roadmap.md) | [设计原则](docs/blueprints/principle.md) | [开发日志](docs/log/)
