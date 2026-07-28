# BLUE6 — 阶段 8~11：代数环与数值稳定性、数学/信号/控制基础库、坐标系统、量纲与单位系统

## 1. 概述

阶段 8~11 在已完成阶段 1~7 内核之上构建四层关键基础设施：

- **阶段 8**：代数环自动检测与数值稳定性防护
- **阶段 9**：标准模块库 — 信号源、数学运算、逻辑门、连续/离散控制系统、观测模块
- **阶段 10**：统一坐标系统（1D/2D/3D、笛卡尔/极坐标/柱坐标/球坐标、变换）
- **阶段 11**：统一量纲与单位系统（7 个 SI 基本量纲、导出量纲、单位换算）

**状态目标：** 100% — 编译通过、零 clippy 警告、全部测试通过、无占位代码。

---

## 2. 组件架构

```
core/
  coord/
    mod.rs          — [Phase 10 ✅] Coord1D, Coord2D, Coord3D, Transform4x4
  units/
    mod.rs          — [Phase 11 ✅] Dimension, Unit, Quantity, si_units

runtime/
  algebraic/
    mod.rs          — [Phase 8 ✅] AlgebraicLoopDetector, FixedPointIteration 等

blocks/
  mod.rs            — [Phase 9 NEW] 模块接口
  sources.rs        — [Phase 9 NEW] 信号源（常量、正弦、方波、阶跃、脉冲、噪声）
  math.rs           — [Phase 9 NEW] 数学运算（加、减、乘、除、三角、矩阵）
  logic.rs          — [Phase 9 NEW] 逻辑运算（与、或、非、异或、比较器、多路选择）
  continuous.rs     — [Phase 9 NEW] 连续控制系统（积分器、PID、传递函数、状态空间）
  discrete_ctrl.rs  — [Phase 9 NEW] 离散控制系统（单位延迟、离散滤波、离散 PID）
  sinks.rs          — [Phase 9 NEW] 观测模块（示波器、数据记录器、显示）
```

---

## 3. 详细规格

### 3.1 阶段 8 — 代数环与数值稳定性 [已实现]

已在 `runtime/algebraic/mod.rs` 中实现：
- `AlgebraicLoopDetector` — 基于 Tarjan SCC 的环路检测
- `DirectFeedthroughPath` — DFS 路径识别
- `FixedPointIteration` — 可配置的不动点迭代求解器
- `RelaxationIteration` — 欠松弛变体
- `NumericalGuard` — NaN/Inf 检测、清理、奇异性检查

### 3.2 阶段 9 — 数学、信号与控制基础库 [待实现]

#### 3.2.1 `blocks/sources.rs` — 信号源

**ConstantSource** — 发射恒定信号值：
```rust
pub struct ConstantSource { value: SignalValue }
```

**SineSource** — 正弦波振荡器：
```rust
pub struct SineSource {
    pub amplitude: Scalar,      // 幅值
    pub frequency: Scalar,      // 频率 (Hz)
    pub phase: Scalar,          // 相位 (弧度)
    pub offset: Scalar,         // 直流偏置
}
// y(t) = offset + amplitude * sin(2π * frequency * t + phase)
```

**SquareSource** — 方波发生器：
```rust
pub struct SquareSource {
    pub amplitude: Scalar,
    pub frequency: Scalar,
    pub duty_cycle: Scalar,     // 0.0–1.0，高电平占空比
    pub offset: Scalar,
}
```

**StepSource** — 阶跃信号：
```rust
pub struct StepSource {
    pub initial: Scalar,        // 初始值
    pub final_val: Scalar,      // 阶跃后值
    pub step_time: Time,        // 阶跃发生时间
}
```

**PulseSource** — 单脉冲或周期脉冲：
```rust
pub struct PulseSource {
    pub amplitude: Scalar,
    pub width: Time,            // 脉冲宽度
    pub period: Option<Time>,   // None = 单脉冲
    pub delay: Time,            // 延迟时间
}
```

**NoiseSource** — 随机信号发生器：
```rust
pub struct NoiseSource {
    pub mean: Scalar,
    pub std_dev: Scalar,
    pub seed: Option<u64>,
    pub noise_type: NoiseType,  // Gaussian, Uniform, PseudoRandom
}
```

每个信号源都实现 `Block` trait。

#### 3.2.2 `blocks/math.rs` — 数学运算

**Adder** — `y = k1*u1 + k2*u2 + bias`（加法器，带系数和偏置）
**Subtractor** — `y = u1 - u2`（减法器）
**Multiplier** — `y = u1 * u2`（乘法器）
**Divider** — `y = u1 / u2`（除法器，带零保护）
**Gain** — `y = k * u`（增益放大器）
**TrigFunction** — 三角函数和超越函数：sin、cos、tan、asin、acos、atan、exp、log、log10
**MatrixMultiply** — 2×2 矩阵乘法 `y = A * u`

每个数学运算块从输入端口读取标量值，计算结果并写入输出端口。

#### 3.2.3 `blocks/logic.rs` — 逻辑运算

**LogicAnd/LogicOr/LogicNot/LogicXor** — 布尔逻辑门
**Comparator** — `y = u1 > u2 ? v_true : v_false`（比较器）
**Multiplexer** — `y = sel == 0 ? u0 : sel == 1 ? u1 : ...`（多路选择器）
**Saturation** — `y = clamp(u, min, max)`（限幅器）
**Switch** — `y = control > threshold ? u2 : u1`（开关）

#### 3.2.4 `blocks/continuous.rs` — 连续控制系统

**Integrator** — `y = ∫ u dt`，带初始条件、输出限幅、复位功能
**PIDController** — `y = Kp*e + Ki*∫e dt + Kd*de/dt`，带抗积分饱和
**TransferFunction** — 连续传递函数 `G(s) = num(s)/den(s)`（可控标准型状态空间实现）
**StateSpaceSystem** — 通用状态空间 `dx = Ax + Bu; y = Cx + Du`

#### 3.2.5 `blocks/discrete_ctrl.rs` — 离散控制系统

**UnitDelay** — `y[n] = u[n-1]`（单位延迟）
**DiscreteIntegratorBlock** — `y[n] = y[n-1] + dt * u[n]`（离散积分器）
**DiscreteFilter** — FIR/IIR 离散滤波器，封装 `runtime::discrete::FIRFilter`/`IIRFilter`
**DiscretePID** — 离散 PID，梯形积分 + 微分滤波

#### 3.2.6 `blocks/sinks.rs` — 观测模块

**Scope** — 存储指定采样数的信号历史（环形缓冲区）
**DataRecorder** — 将信号值和时间戳记录到内部 Vec
**NumericDisplay** — 将信号转换为格式化字符串输出
**ChartBuffer** — 累积 (time, value) 对用于外部图表

### 3.3 阶段 10 — 坐标系统 [已实现]

已在 `core/coord/mod.rs` 中实现：
- `CoordSystem` 枚举（6 种坐标类型）
- `Coord1D`、`Coord2D`、`Coord3D` 及坐标系间转换
- `Transform4x4` — 单位矩阵、平移、旋转、缩放、组合

### 3.4 阶段 11 — 量纲与单位系统 [已实现]

已在 `core/units/mod.rs` 中实现：
- `Dimension` — 7 个 SI 指数、mul/div/pow
- 10+ 导出量纲（速度、力、能量、电压等）
- `Unit` — 比例因子、偏移量、符号、单位换算
- `Quantity` — 数值+单位、转换、加法、减法、缩放
- 30+ 预定义单位常量

---

## 4. 实现顺序

1. `blocks/mod.rs` — 模块接口（已完成）
2. `blocks/sources.rs` — 信号源（6 种模块 + 测试，已完成）
3. `blocks/math.rs` — 数学运算（7 种模块 + 测试，已完成）
4. `blocks/logic.rs` — 逻辑运算（7 种模块 + 测试）
5. `blocks/continuous.rs` — 连续控制（4 种模块 + 测试）
6. `blocks/discrete_ctrl.rs` — 离散控制（4 种模块 + 测试）
7. `blocks/sinks.rs` — 观测模块（4 种模块 + 测试）
8. 审查并强化 Phase 8/10/11
9. 更新 `src/lib.rs` 重导出
10. 全面测试（60+ 新测试）

---

## 5. 测试要求

### 阶段 8 测试（已有 10 个）：
- 无环/有环/自环检测
- 不动点迭代收敛与发散
- 松弛迭代
- 数值防护（NaN/Inf/奇异）

### 阶段 9 测试（60+ 新增）：
- 每个信号源在已知时间点的输出准确性
- 数学块与已知值的数值正确性
- 逻辑块覆盖所有输入组合
- 连续控制阶跃响应特性
- 离散控制逐采样点正确性
- 观测块数据捕获正确性
- 边界情况：零输入、NaN 防护、极值

### 阶段 10 测试（已有 15 个）：
- 转换往返、旋转、叉积/点积、归一化、变换

### 阶段 11 测试（已有 14 个）：
- 量纲运算、单位兼容性、换算、Quantity 算术

---

## 6. 验收标准

- [ ] `cargo build` 成功
- [ ] `cargo test` — 全部通过（0 失败，0 忽略）
- [ ] `cargo clippy --all-targets -- -D warnings` — 零警告
- [ ] 无 `todo!()`、`unimplemented!()` 或空函数体
- [ ] 所有新增代码有英文注释
- [ ] `blocks/mod.rs` 仅暴露接口
- [ ] 每个模块至少 1 个创建测试 + 1 个输出测试
- [ ] 信号源在采样点产生正确的波形
- [ ] 数学块优雅处理 NaN/Inf
- [ ] PID 控制器产生正确的阶跃响应
- [ ] 观测块正确捕获信号历史
- [ ] 阶段 8/10/11 保持 100% 无回归
