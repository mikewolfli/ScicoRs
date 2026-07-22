# BLUE4 — 阶段 4：调度与执行引擎

## 1. 概述

阶段 4 在阶段 1~3 基础模型、仿真上下文、数值求解器之上构建**统一的调度与执行引擎**。将仿真系统从"单步按序执行"升级为支持拓扑排序、信号流分析、混合调度（连续/离散/事件/多速率）、端口信号传播与缓存、多速率任务隔离的完整调度层。

**当前 `SimEngine`** 已有基础生命周期管理，但：
- 未使用真正的拓扑排序（依赖符号排序，而非图论算法）
- 无信号传播（port→link→port 数据流动）
- 无混合调度（连续/离散/事件统一调度）
- 无多速率任务隔离

**状态目标：** 100% — 编译通过、零 clippy 警告、所有测试通过、无占位代码、与阶段 1~3 完全集成。

---

## 2. 组件架构

```
runtime/
  mod.rs                — 更新：暴露 scheduler 模块所有公共类型
  scheduler/
    mod.rs              — 模块接口 + 全局重导出
    traits.rs           — [增强] Scheduler trait + ScheduleConfig
    topo.rs             — [NEW] 拓扑排序算法（Kahn + DFS 环路检测）
    signal_flow.rs      — [NEW] 信号流方向分析、端口传播图
    hybrid.rs           — [NEW] 混合调度器（连续/离散/事件统一调度）
    multirate.rs        — [NEW] 多速率调度、时钟域隔离、任务队列
    signal_prop.rs      — [NEW] 端口信号更新、传播、缓存、同步
  engine.rs             — [集成] 替换内部调度逻辑为 Scheduler trait

新增 runtime/ 模块：
  event.rs              — [NEW] 事件队列与触发系统（阶段 6 支持，基础框架）
```

---

## 3. 详细规格

### 3.1 `scheduler/topo.rs` — 拓扑排序与环路检测

**核心算法（Kahn 算法）：**
```rust
pub fn topological_sort(diagram: &Diagram) -> Result<Vec<BlockId>, Vec<BlockId>>
```
- 基于 Block 间 Link 连接关系构建有向图
- 边方向：source (output) → destination (input)
- 返回拓扑排序后的 BlockId 列表
- 若存在环路，返回参与环路的 BlockId 列表

**DFS 环路检测：**
```rust
pub fn detect_cycles(diagram: &Diagram) -> Vec<Vec<BlockId>>
```
- 深度优先搜索检测所有强连通分量（SCC）
- 用于代数环检测（阶段 8 的基础）

**数据结构：**
```rust
pub struct DiGraph {
    pub nodes: Vec<BlockId>,
    pub adjacency: HashMap<BlockId, Vec<BlockId>>, // node -> successors
}
```

**测试要求：**
- 线性链 DAG 的正确排序
- 分支 DAG 的正确排序
- 环路检测（简单环、复杂环、自环）
- 空 diagram
- 单节点 diagram

### 3.2 `scheduler/signal_flow.rs` — 信号流方向分析

**分析信号传播路径：**
```rust
pub struct SignalFlowGraph {
    pub sources: Vec<BlockId>,       // 只有 Output 口的源 Block（无输入依赖）
    pub sinks: Vec<BlockId>,         // 只有 Input 口的汇 Block（无输出传播）
    pub propagation_order: Vec<BlockId>, // 信号传播顺序（按层）
}
```

**功能：**
```rust
pub fn analyze_signal_flow(diagram: &Diagram) -> SignalFlowGraph
pub fn compute_propagation_layers(diagram: &Diagram) -> Vec<Vec<BlockId>>
pub fn find_implicit_connections(diagram: &Diagram) -> Vec<(BlockId, BlockId)>
```

- `propagation_order` = 按入度排序的分层传播顺序
- 每层内的 Block 无相互信号依赖，可并行传播

### 3.3 `scheduler/hybrid.rs` — 混合调度器

**统一调度 trait（替代 engine 中的硬编码顺序调度）：**

```rust
pub enum BlockTaskType {
    Continuous,   // 连续系统块（需要 ODE 求解）
    Discrete,     // 离散系统块（固定步长更新）
    EventDriven,  // 事件驱动块（响应事件触发）
    MultiRate,    // 多速率块（不同步长）
}

pub struct ScheduleConfig {
    pub continuous_solver: Option<Box<dyn OdeSolver>>,
    pub discrete_step: Option<Scalar>,
    pub event_queue_capacity: usize,
}

pub struct ScheduleContext<'a> {
    pub diagram: &'a Diagram,
    pub execution_order: &'a [BlockId],
    pub current_time: Time,
    pub dt: Scalar,
    pub event_queue: &'a mut EventQueue,
    pub signal_cache: &'a mut SignalCache,
}

pub fn classify_blocks(diagram: &Diagram) -> HashMap<BlockId, BlockTaskType>
pub fn build_schedule(diagram: &Diagram, config: &ScheduleConfig) -> Vec<SchedulePhase>
```

**标准执行阶段（每个时间步）：**
```
Phase 1: ComputeOutputs    — 对所有 Block 调用 output()
Phase 2: PropagateSignals  — 端口信号传播（input ← link ← output）
Phase 3: ComputeDerivs     — 对所有连续 Block 调用 derivative()
Phase 4: IntegrateStates   — 使用 OdeSolver 集成连续状态
Phase 5: UpdateDiscrete    — 对所有离散 Block 调用 update()
Phase 6: DetectEvents      — 零交叉检测、事件触发
Phase 7: HandleEvents      — 事件队列消费
Phase 8: AdvanceTime       — 推进仿真时间
```

**将 `SchedulePhase` 加入枚举：**
```rust
pub enum SchedulePhase {
    ComputeOutputs,
    PropagateSignals,
    ComputeDerivs,
    IntegrateStates,
    UpdateDiscrete,
    DetectEvents,
    HandleEvents,
    AdvanceTime,
}
```

### 3.4 `scheduler/signal_prop.rs` — 端口信号传播与缓存

**信号缓存机制：**
```rust
pub struct SignalCache {
    // block_id.port_name → current signal value
    cache: HashMap<(BlockId, String), SignalValue>,
    // block_id.port_name → previous signal value (for edge detection)
    prev_cache: HashMap<(BlockId, String), SignalValue>,
}
```

**信号传播引擎：**
```rust
pub fn propagate_signals(diagram: &Diagram, cache: &mut SignalCache) -> Result<(), SimError>
pub fn update_inputs(diagram: &Diagram, cache: &SignalCache) -> Result<(), SimError>
pub fn extract_outputs(diagram: &Diagram, cache: &mut SignalCache) -> Result<(), SimError>
```

**传播逻辑：**
1. 遍历所有 Link，对每个 Link：`dest_port_value = source_port_value`
2. 将值写入目标 Block 的输入端口
3. 支持缓存：保留上一时间步的值用于边沿检测
4. 同步点：所有端口传播完成后才进入下一阶段

### 3.5 `scheduler/multirate.rs` — 多速率调度

**时钟域定义：**
```rust
pub struct ClockDomain {
    pub id: String,
    pub base_rate: Scalar,      // 基准步长（秒）
    pub offset: Scalar,         // 相位偏移（0~base_rate）
}

pub struct MultiRateScheduler {
    pub domains: Vec<ClockDomain>,
    pub block_assignments: HashMap<BlockId, String>, // block_id -> domain_id
    pub domain_rates: HashMap<String, Scalar>,
}
```

**多速率同步机制：**
- 每个时钟域有自己的任务队列
- 快域执行多次后，慢域执行一次
- 在公共倍周期点对齐同步
- 跨域数据传递通过 SignalCache 的采样保持

```rust
pub fn build_multirate_schedule(diagram: &Diagram) -> MultiRateScheduler
pub fn domain_next_trigger(domain: &ClockDomain, current_time: Time) -> Time
pub fn synchronize_domains(scheduler: &MultiRateScheduler, time: Time) -> Vec<String>
```

### 3.6 `scheduler/traits.rs` — [增强] Scheduler trait

**增强现有 Scheduler trait，增加配置和执行上下文：**
```rust
pub trait Scheduler: Send + Sync {
    /// 名称
    fn name(&self) -> &str;

    /// 初始化调度器（拓扑排序、信号流分析）
    fn initialize(&mut self, diagram: &Diagram) -> Result<(), SimError>;

    /// 执行一个完整的调度步
    fn step(&mut self, ctx: &mut ScheduleContext) -> Result<ScheduleStepResult, SimError>;

    /// 重新调度（diagram 变更后）
    fn reschedule(&mut self, diagram: &Diagram) -> Result<(), SimError>;

    /// 获取执行顺序
    fn execution_order(&self) -> &[BlockId];
}

pub struct SequentialScheduler {
    order: Vec<BlockId>,
    config: ScheduleConfig,
    signal_cache: SignalCache,
    event_queue: EventQueue,
}
```

### 3.7 `engine.rs` — [集成] 引擎集成调度器

**修改 `SimEngine`:**
- 新增 `scheduler: Box<dyn Scheduler>` 字段
- `new()` 默认使用 `SequentialScheduler`
- `with_scheduler()` 方法设置自定义调度器
- `init()` 调用 `scheduler.initialize(diagram)`
- `step()` 委托给 `scheduler.step(ctx)`
- `reschedule()` 方法支持动态重调度

### 3.8 `event.rs` — [NEW] 事件队列基础

```rust
pub struct Event {
    pub id: String,
    pub time: Time,
    pub event_type: EventType,
    pub data: SignalValue,
}

pub enum EventType {
    TimeEvent,     // 定时事件
    ZeroCrossing,  // 过零检测事件
    External,      // 外部触发
    Condition,     // 条件触发
}

pub struct EventQueue {
    events: BinaryHeap<Event>,  // 按时间排序的最小堆
    pub processed: u64,
    pub pending: u64,
}
```

---

## 4. 实现顺序

1. `scheduler/topo.rs` — Kahn 拓扑排序 + DFS 环路检测
2. `scheduler/signal_flow.rs` — 信号流分析、传播层计算
3. `scheduler/signal_prop.rs` — 端口信号缓存、传播引擎
4. `scheduler/traits.rs` — [增强] Scheduler trait + SequentialScheduler
5. `scheduler/hybrid.rs` — 混合调度器、Block 分类、调度阶段
6. `scheduler/multirate.rs` — 多速率任务调度、时钟域
7. `event.rs` — 事件队列基础框架
8. 更新 `scheduler/mod.rs` — 暴露所有新类型
9. 更新 `runtime/mod.rs` — 暴露事件模块
10. 集成到 `engine.rs` — 替换硬编码逻辑为 Scheduler trait
11. 全面测试（20+ 测试覆盖所有路径）

---

## 5. 测试要求

### topo.rs 测试（6+）：
- 线性链：A→B→C 排序正确
- 分支 DAG：多个并行路径排序正确
- 简单环路检测：A→B→A 识别
- 复杂环路检测：A→B→C→A 识别
- 自环检测：A→A
- 空 diagram / 单节点

### signal_flow.rs 测试（4+）：
- 源/汇/传播顺序识别
- 分层传播计算
- 隐式连接查找
- 无连接 diagram

### signal_prop.rs 测试（4+）：
- 信号通过 Link 传播
- 多级传播（A→B→C）
- 缓存读写
- 断开连接的处理

### hybrid.rs 测试（4+）：
- Block 分类正确
- 调度阶段顺序执行
- SequentialScheduler 完整步执行
- 混合系统调度

### multirate.rs 测试（4+）：
- 时钟域创建
- 触发时间计算
- 域间同步点识别
- 多速率调度

### event.rs 测试（3+）：
- 事件队列创建
- 事件入队/出队
- 事件按时间排序

### engine.rs 集成测试（5+）：
- 引擎使用 SequentialScheduler
- 信号传播验证
- 事件队列集成
- 重调度
- 拓扑排序错误处理

---

## 6. 验收标准

- [ ] `cargo build` 成功
- [ ] `cargo test` — 所有测试通过（0 failed, 0 ignored）
- [ ] `cargo clippy --all-targets -- -D warnings` — 零警告
- [ ] 无 `todo!()`, `unimplemented!()`, 或空函数体
- [ ] 所有新增代码有英文注释
- [ ] scheduler 模块仅通过 mod.rs 暴露接口
- [ ] Kahn 拓扑排序正确处理 DAG 和检测环路
- [ ] 信号传播引擎完成 port→link→port 数据流动
- [ ] 混合调度器正确分类连续/离散/事件块
- [ ] 事件队列支持按时间排序和消费
- [ ] 多速率调度支持时钟域隔离和同步
- [ ] engine.rs 通过 Scheduler trait 集成
- [ ] 与阶段 1~3 无缝集成，无回归

---

## 7. 与阶段 1~3 集成

阶段 4 消费阶段 1~3 的以下类型：
- `Diagram`, `Block`, `Link`, `Port` — 拓扑结构（阶段 1）
- `BlockId`, `SignalValue`, `SignalType` — 数据表示（阶段 1）
- `SimContext`, `SimEngine`, `SimStepResult` — 执行上下文（阶段 2）
- `OdeSolver` — 连续块集成（阶段 3）
- `SimError` — 错误传播（阶段 1）

**向后兼容：** 所有阶段 1~3 现有 API 不变，阶段 4 是纯新增 + 引擎内部增强。
