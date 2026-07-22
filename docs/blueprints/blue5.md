# BLUE5 — 阶段 5/6/7：计算工作流、事件触发系统与离散多速率系统

## 1. 概述

本蓝图覆盖 Roadmap 阶段 5~7，在阶段 1~4 核心模型、仿真上下文、数值求解器、调度引擎之上构建三层能力：

- **阶段 5：计算工作流与任务编排系统** — DAG 任务建模、并行/串行混合调度、流水线工作流、动态重调度与容错
- **阶段 6：事件与触发系统** — 过零检测完整实现、外部/中断/条件触发、事件驱动求解器重启、事件日志与统计
- **阶段 7：离散与多速率系统** — 数字滤波器、离散积分、采样保持、嵌入式逻辑（计数器/定时器）、PLC 逻辑、数字逻辑时序

**状态目标：** 100% — 编译通过、零 clippy 警告、所有测试通过、无占位代码、与阶段 1~4 完全集成。

---

## 2. 组件架构

```
runtime/
  mod.rs                  — 更新：暴露 workflow 和 discrete 模块
  workflow/               — [NEW] 阶段 5：计算工作流与任务编排
    mod.rs                — 模块接口 + 重导出
    dag.rs                — [NEW] WorkflowDAG、WorkflowTask、WorkflowEdge
    stage.rs              — [NEW] 工作流阶段分解（pipeline stages）
    parallel.rs           — [NEW] 并行调度器 + barrier 同步
    engine.rs             — [NEW] 工作流引擎（编排 + 动态重调度 + 容错）
  event.rs                — [增强] 过零检测完整实现 + 触发系统
  discrete/               — [NEW] 阶段 7：离散与多速率系统
    mod.rs                — 模块接口 + 重导出
    digital_filter.rs     — [NEW] 数字滤波器（FIR、IIR、移动平均）
    discrete_integrator.rs — [NEW] 离散积分器（前向/后向/梯形）
    sample_hold.rs        — [NEW] 采样保持与重采样
    counter.rs            — [NEW] 计数器、定时器、PWM
    plc_logic.rs          — [NEW] PLC 逻辑（梯形图原语：与/或/非/RS触发器/边沿检测）
    digital_timing.rs     — [NEW] 数字逻辑时序、竞争与冒险处理
  scheduler/
    hybrid.rs             — [增强] 完整过零检测 + 事件触发集成
```

---

## 3. 详细规格

### 3.1 `workflow/dag.rs` — 工作流 DAG

**WorkflowTask** — 计算工作流中的节点：
```rust
pub struct WorkflowTask {
    pub id: String,
    pub name: String,
    pub block_id: Option<BlockId>,         // 关联的仿真 Block
    pub sub_diagram: Option<Diagram>,      // 子框图（子图粒度）
    pub priority: u32,
    pub estimated_cost: f64,               // 预估计算开销（秒）
}
```

**WorkflowEdge** — 任务间依赖边：
```rust
pub struct WorkflowEdge {
    pub source: String,
    pub destination: String,
    pub data_type: EdgeDataType,
    pub delay: Option<Time>,
}

pub enum EdgeDataType {
    Data,       // 数据依赖
    Signal,     // 信号依赖
    Event,      // 事件依赖
    Control,    // 控制流（if-then-else）
}
```

**WorkflowDAG** — 完整的 DAG 结构：
- `add_task(task)`, `add_edge(edge)`, `remove_task(id)`, `remove_edge(source, dest)`
- `topological_sort()` — Kahn 拓扑排序
- `parallel_stages()` — 将任务按依赖深度划分为阶段
- `critical_path()` — 关键路径分析
- `validate()` — 校验 DAG 合法性（无环、所有节点可达）

### 3.2 `workflow/stage.rs` — 工作流阶段分解

**PipelineStage** — 流水线阶段：
```rust
pub enum PipelineStage {
    PreProcess,
    Solve,          // 多物理场并行求解
    CouplingSync,   // 耦合同步
    PostProcess,
}

pub struct WorkflowStage {
    pub stage_type: PipelineStage,
    pub tasks: Vec<String>,           // 阶段内任务 ID 列表
    pub parallel: bool,              // 是否可并行
    pub barrier_required: bool,      // 是否需要 barrier 同步
}
```

**阶段分解算法：**
- 输入：WorkflowDAG
- 输出：Vec<WorkflowStage>
- 策略：按依赖深度 + 任务类型分组
- 前处理 → 求解（并行）→ 耦合同步 → 后处理

### 3.3 `workflow/parallel.rs` — 并行调度

**BarrierSync** — 同步屏障：
```rust
pub struct BarrierSync {
    pub stage_id: String,
    pub expected_count: usize,
    pub timeout: Option<Duration>,
}
```

**ParallelScheduler** — 并行任务调度器：
- `schedule(stages)` — 为每个阶段分配线程
- `execute_parallel(tasks)` — 并行执行无依赖任务
- `barrier_wait()` — 阶段间同步屏障
- 负载均衡：按 estimated_cost 分配任务

### 3.4 `workflow/engine.rs` — 工作流引擎

**WorkflowEngine** — 工作流编排器：
```rust
pub struct WorkflowEngine {
    pub dag: WorkflowDAG,
    pub context: SimContext,
    pub diagram: Diagram,
    pub scheduler: Box<dyn Scheduler>,
    pub event_queue: EventQueue,
    pub status: WorkflowStatus,
}
```

**功能：**
- `run()` — 按阶段顺序执行完整工作流
- `step()` — 单步执行一个阶段
- `reschedule(diagram)` — 动态重调度（新增/删除模块时重建 DAG）
- `retry(task_id)` — 局部重试（失败任务回滚重试）
- `pause() / resume() / stop()` — 工作流生命周期控制

**容错机制：**
- 单任务失败时仅回滚该任务及其下游依赖
- 重试上限配置
- 错误传播到 WorkflowStatus

### 3.5 `event.rs` — [增强] 过零检测与触发系统

**增强 ZeroCrossingDetector：**
```rust
pub struct ZeroCrossingDetector {
    pub prev_signals: HashMap<(BlockId, String), Scalar>,
    pub curr_signals: HashMap<(BlockId, String), Scalar>,
    pub threshold: Scalar,
}

impl ZeroCrossingDetector {
    pub fn detect_rising(&self, block_id: &str, port: &str) -> bool;
    pub fn detect_falling(&self, block_id: &str, port: &str) -> bool;
    pub fn detect_any(&self, block_id: &str, port: &str) -> bool;
    pub fn update(&mut self, block_id: &str, port: &str, value: Scalar);
    pub fn advance(&mut self);
}
```

**Enhanced EventTrigger：**
```rust
pub enum TriggerCondition {
    TimeTrigger(Time),
    ZeroCrossing { block: BlockId, port: String, edge: EdgeType },
    External { source: String },
    Conditional { condition_fn: String }, // 表达式字符串
    Interrupt { irq: u32 },
}

pub enum EdgeType {
    Rising,
    Falling,
    Both,
}
```

**EventStatistics：**
```rust
pub struct EventStatistics {
    pub total_events: u64,
    pub events_by_type: HashMap<EventType, u64>,
    pub peak_queue_size: u64,
    pub last_event_time: Option<Time>,
}
```

### 3.6 `discrete/digital_filter.rs` — 数字滤波器

**FIR 滤波器：**
```rust
pub struct FIRFilter {
    pub coefficients: Vec<Scalar>,
    buffer: VecDeque<Scalar>,
}
impl FIRFilter {
    pub fn new(coefficients: &[Scalar]) -> Self;
    pub fn step(&mut self, input: Scalar) -> Scalar;
    pub fn reset(&mut self);
}
```

**IIR 滤波器：**
```rust
pub struct IIRFilter {
    pub b: Vec<Scalar>,           // 前向系数
    pub a: Vec<Scalar>,           // 反馈系数
    x_buffer: VecDeque<Scalar>,
    y_buffer: VecDeque<Scalar>,
}
impl IIRFilter {
    pub fn new(b: &[Scalar], a: &[Scalar]) -> Self;
    pub fn step(&mut self, input: Scalar) -> Scalar;
    pub fn reset(&mut self);
}
```

**移动平均滤波器：**
```rust
pub struct MovingAverage {
    window_size: usize,
    buffer: VecDeque<Scalar>,
    sum: Scalar,
}
impl MovingAverage { pub fn step(&mut self, input: Scalar) -> Scalar; }
```

### 3.7 `discrete/discrete_integrator.rs` — 离散积分器

```rust
pub enum IntegrationMethod {
    ForwardEuler,     // x[n+1] = x[n] + dt * u[n]
    BackwardEuler,    // x[n+1] = x[n] + dt * u[n+1]
    Trapezoidal,      // x[n+1] = x[n] + dt/2 * (u[n] + u[n+1])
}

pub struct DiscreteIntegrator {
    pub method: IntegrationMethod,
    pub state: Scalar,
    pub dt: Scalar,
    pub initial: Scalar,
    pub reset_on: Option<TriggerCondition>,
}

impl DiscreteIntegrator {
    pub fn step(&mut self, input: Scalar, input_next: Option<Scalar>) -> Scalar;
    pub fn reset(&mut self);
    pub fn output(&self) -> Scalar;
}
```

### 3.8 `discrete/sample_hold.rs` — 采样保持

```rust
pub struct SampleHold {
    pub sample_rate: Scalar,
    pub phase: Scalar,
    pub held_value: Scalar,
    pub last_sample_time: Time,
}

impl SampleHold {
    pub fn update(&mut self, input: Scalar, time: Time) -> Scalar;
    pub fn resample(&self, input: &[Scalar], from_rate: Scalar, to_rate: Scalar) -> Vec<Scalar>;
    pub fn reset(&mut self);
}
```

### 3.9 `discrete/counter.rs` — 计数器与定时器

```rust
pub enum CounterDirection { Up, Down, UpDown }

pub struct Counter {
    pub direction: CounterDirection,
    pub preset: u64,
    pub current: u64,
    pub output: bool,
    pub reset_on: Option<TriggerCondition>,
}

impl Counter {
    pub fn clock(&mut self) -> bool;      // 时钟上升沿触发
    pub fn reset(&mut self);
    pub fn load(&mut self, value: u64);
}

pub struct Timer {
    pub period: Scalar,
    pub pulse_width: Scalar,
    pub elapsed: Scalar,
    pub output: bool,
}

impl Timer {
    pub fn update(&mut self, dt: Scalar) -> bool;
    pub fn reset(&mut self);
}
```

### 3.10 `discrete/plc_logic.rs` — PLC 逻辑

```rust
pub struct PLCBlock {
    pub inputs: Vec<(String, bool)>,
    pub outputs: Vec<(String, bool)>,
    pub logic_fn: Box<dyn Fn(&[bool]) -> Vec<bool> + Send + Sync>,
}

impl PLCBlock {
    pub fn evaluate(&mut self) -> Vec<bool>;
    pub fn set_input(&mut self, name: &str, value: bool);
    pub fn get_output(&self, name: &str) -> Option<bool>;
}

pub fn and_gate(inputs: &[bool]) -> Vec<bool>;
pub fn or_gate(inputs: &[bool]) -> Vec<bool>;
pub fn not_gate(inputs: &[bool]) -> Vec<bool>;

pub struct RSFlipFlop {
    pub set: bool, pub reset: bool, pub output: bool,
}
impl RSFlipFlop { pub fn clock(&mut self); }

pub struct EdgeDetector {
    pub last: bool, pub rising: bool, pub falling: bool,
}
impl EdgeDetector { pub fn update(&mut self, input: bool); }
```

### 3.11 `discrete/digital_timing.rs` — 数字逻辑时序

```rust
pub enum HazardType {
    Static1,    // 应保持 1 但出现 0 毛刺
    Static0,    // 应保持 0 但出现 1 毛刺
    Dynamic,    // 过渡态毛刺
}

pub struct TimingAnalysis {
    pub critical_path_delay: Scalar,
    pub setup_time: Scalar,
    pub hold_time: Scalar,
    pub hazards: Vec<(String, HazardType)>,
}
```

---

## 4. 实现顺序

### 第一轮：阶段 5 — 工作流系统
1. `workflow/mod.rs` — 模块接口
2. `workflow/dag.rs` — WorkflowDAG 核心
3. `workflow/stage.rs` — 流水线阶段分解
4. `workflow/parallel.rs` — 并行调度 + barrier
5. `workflow/engine.rs` — 工作流引擎

### 第二轮：阶段 6 — 事件系统增强
6. `event.rs` — 增强过零检测、触发条件、事件统计

### 第三轮：阶段 7 — 离散系统
7. `discrete/mod.rs` — 模块接口
8. `discrete/digital_filter.rs` — 数字滤波器
9. `discrete/discrete_integrator.rs` — 离散积分器
10. `discrete/sample_hold.rs` — 采样保持
11. `discrete/counter.rs` — 计数器与定时器
12. `discrete/plc_logic.rs` — PLC 逻辑
13. `discrete/digital_timing.rs` — 数字逻辑时序

### 第四轮：集成与测试
14. 更新 `runtime/mod.rs` — 暴露 workflow + discrete 模块
15. 更新 `scheduler/hybrid.rs` — 集成事件触发
16. 全面测试（50+ 新增测试）

---

## 5. 测试要求

### workflow 测试（15+）：
- DAG 创建、添加/删除任务和边
- 拓扑排序（线性、分支、环路检测）
- 并行阶段分解
- 关键路径分析
- 流水线阶段创建
- Barrier 同步
- 完整工作流执行
- 动态重调度
- 任务失败重试
- 空工作流

### event 增强测试（5+）：
- 过零检测（上升沿、下降沿、双向）
- 触发条件匹配
- 事件统计收集
- 多事件时间排序

### discrete 测试（30+）：
- FIR 滤波器：创建、脉冲响应、稳态
- IIR 滤波器：低通滤波、稳定性
- 移动平均：窗口滑动
- 离散积分器：前向/后向/梯形
- 采样保持：保持值不变、重采样
- 计数器：向上/向下/预置
- 定时器：周期、脉冲宽度
- PLC 逻辑：与/或/非门
- RS 触发器：置位/复位
- 边沿检测：上升/下降
- 时序分析：关键路径

---

## 6. 验收标准

- [ ] `cargo build` 成功
- [ ] `cargo test` — 全部 220+ 测试通过（新增 50+）
- [ ] `cargo clippy --all-targets -- -D warnings` — 零警告
- [ ] 无 `todo!()`, `unimplemented!()`, 或空函数体
- [ ] 所有新增代码有英文注释
- [ ] workflow 模块支持 DAG 建模、阶段分解、并行调度、动态重调度
- [ ] 事件系统支持完整过零检测和多种触发条件
- [ ] 离散系统支持数字滤波器、积分器、采样保持、计数器、PLC 逻辑
- [ ] 与阶段 1~4 无缝集成，无回归
- [ ] 模块化设计符合 principle.md 要求

---

## 7. 与阶段 1~4 集成

阶段 5~7 消费以下类型：
- `Diagram`, `Block`, `BlockId`, `Link` — 拓扑结构（阶段 1）
- `SignalValue`, `SignalType`, `Scalar`, `Time` — 数据类型（阶段 1）
- `SimContext`, `SimEngine`, `SimStepResult` — 执行上下文（阶段 2）
- `OdeSolver` — 求解器（阶段 3）
- `Scheduler`, `SequentialScheduler` — 调度器（阶段 4）
- `EventQueue`, `Event`, `EventType` — 事件队列（阶段 4/6）
- `ClockDomain`, `MultiRateScheduler` — 多速率（阶段 4/7）

**向后兼容：** 所有阶段 1~4 现有 API 不变，阶段 5~7 是纯新增 + 内部增强。
