# BLUE2 — 阶段 2：仿真上下文与时间系统

## 1. 概述

阶段 2 在阶段 1 核心模型之上构建**仿真执行基础设施**。引入集中式时间管理、统一状态处理、仿真生命周期控制和多模式执行引擎 —— 驱动 Block 随时间推进的运行时层。

**状态目标：** 100% — 编译通过、零 clippy 警告、所有测试通过、无占位代码、与阶段 1 完全集成。

---

## 2. 组件架构

```
core/
  mod.rs              — 模块接口（更新：添加 pub mod context, sim_state, engine）
  context.rs          — [NEW] 集中式仿真上下文（时间、模式、生命周期、共享数据）
  sim_state.rs        — [NEW] 统一连续 + 离散状态管理
  engine.rs           — [NEW] 仿真执行引擎编排器
  ... 阶段 1 模块（不变）
```

---

## 3. 详细规格

### 3.1 `context.rs` [NEW] — 仿真上下文

**枚举 `SimRunMode`** — 仿真执行模式：
```rust
pub enum SimRunMode {
    /// 正常连续执行，固定/可变步长。
    Normal,
    /// 挂钟同步执行（实时因子）。
    RealTime { time_scale: f64 },
    /// 执行一步，然后暂停。
    SingleStep,
    /// 暂停 — 不推进时间。
    Paused,
    /// 运行直到断点条件满足。
    Breakpoint { condition: Box<dyn Fn(&SimContext) -> bool + Send + Sync> },
}
```

**枚举 `SimLifecycle`** — 仿真生命周期有限状态机：
```rust
pub enum SimLifecycle {
    Constructed,
    Initialized,
    Running,
    Paused,
    Completed,
    Error(String),
}
```

**结构体 `TimeConfig`** — 时间配置参数：
```rust
pub struct TimeConfig {
    pub start_time: Time,
    pub end_time: Time,
    pub max_step: Time,
    pub min_step: Time,
    pub initial_step: Time,
}
```

**结构体 `SimContext`** — 中央仿真上下文：
- 时间管理：当前时间 `t`、步长 `dt`、步数计数
- 模式与生命周期状态
- 共享数据映射（模块间数据共享）
- 日志缓冲区
- 错误跟踪

### 3.2 `sim_state.rs` [NEW] — 统一状态管理

**结构体 `ContinuousState`** — 带导数的连续状态向量：
- 命名变量 `x` 向量
- 导数 `dx` 向量
- 按名称/索引访问、重置、快照/恢复

**结构体 `DiscreteState`** — 离散状态向量：
- 命名变量 `z` 向量
- 按名称/索引访问、重置、快照/恢复

**结构体 `StateSnapshot`** — 可冻结状态捕获

**结构体 `SimStateManager`** — 统一访问两种状态类型

### 3.3 `engine.rs` [NEW] — 仿真执行引擎

**结构体 `SimEngine`** — 顶层仿真编排器：
- `context: SimContext` — 仿真上下文
- `state: SimStateManager` — 状态管理器
- `diagram: Diagram` — 阶段 1 框图
- `execution_order: Vec<BlockId>` — 拓扑执行顺序

**生命周期方法：** `new`、`init`、`start`、`step`、`run`、`pause`、`resume`、`stop`、`reset`

**每步执行阶段：**
1. 设置所有 Block 的上下文时间
2. 按拓扑序调用所有 Block 的 `output()`
3. 计算导数（调用所有 Block 的 `derivative()`）
4. 积分连续状态：`x += dt * dx`
5. 调用所有 Block 的 `update()`
6. 检测过零事件
7. 推进上下文时间
8. 检查断点/停止条件

**结果类型：** `SimStepResult`（枚举）、`SimSummary`（结构体）

---

## 4. 实现顺序

1. `context.rs` — SimRunMode、SimLifecycle、TimeConfig、SimContext（基础）
2. `sim_state.rs` — ContinuousState、DiscreteState、StateSnapshot、SimStateManager
3. `engine.rs` — 完整生命周期、逐步执行、多模式支持的 SimEngine
4. 更新 `core/mod.rs` — 导出所有新公共类型
5. 全面测试所有模块（15+ 个测试）

---

## 5. 测试要求

### context.rs 测试：
- TimeConfig 默认值和验证
- SimContext 时间推进
- SimContext 进度计算
- SimContext 共享数据 set/get
- SimContext 日志记录
- SimContext 完成检测
- 运行模式创建变体

### sim_state.rs 测试：
- ContinuousState 创建和访问
- ContinuousState 导数
- ContinuousState 重置
- DiscreteState 创建和访问
- StateManager 统一操作
- 快照/恢复往返

### engine.rs 测试：
- 引擎创建和初始化
- 使用简单 Block 单步执行
- 完整运行到完成
- 暂停/恢复循环
- 生命周期状态转换
- Block 错误传播
- 多 Block 框图执行
- 实时模式创建
- 单步模式验证

---

## 6. 验收标准

- [ ] `cargo build` 成功
- [ ] `cargo test` — 全部 56+ 测试通过（新增 15+）
- [ ] `cargo clippy --all-targets -- -D warnings` — 零警告
- [ ] 无 `todo!()`、`unimplemented!()` 或空函数体
- [ ] 所有新增代码有英文注释
- [ ] 引擎能驱动 Diagram 完成完整生命周期
- [ ] 时间推进单调且精确
- [ ] 状态快照能捕获和恢复完整系统状态
- [ ] 多种运行模式正常工作
- [ ] 与阶段 1 Block trait 无缝集成

---

## 7. 与阶段 1 集成

阶段 2 消费阶段 1 类型：
- `Block` trait — 执行回调
- `Diagram` — 拓扑和 Block 集合
- `SimError` — 错误传播
- `SignalValue`、`Scalar`、`Time` — 数据表示
- `ComponentStatus` — Block 状态跟踪
- `ExecutionPhase` — 每 Block 生命周期
- `StateDeclaration` — 状态管理器初始化
- `BlockId` — 执行排序

阶段 1 模块无需修改 — 阶段 2 是纯新增。
