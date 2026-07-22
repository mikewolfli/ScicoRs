# BLUE1 — Phase 1: Core Model Kernel Implementation Blueprint

## 1. Overview

Phase 1 establishes the **foundational data model** for the entire simulation kernel:
Block (functional module), Port (I/O interface), Link (signal connection), Diagram (topology).
All higher phases build on this layer.

**Status target:** 100% — compile, zero clippy warnings, all tests pass, no placeholder code.

---

## 2. Component Architecture

```
core/
  mod.rs          — Module interface (pub mod + pub use only)
  types.rs        — Core numeric types, SignalValue, enums (SignalType, PortDirection, etc.)
  tensor.rs       — [NEW] Multi-dimensional Tensor type
  signal.rs       — Signal, ContinuousSignal, DiscreteSignal, EventSignal, BusSignal
  port.rs         — Port struct, PortSet collection
  param.rs        — Parameter, ExpressionParameter, ParameterSet
  block.rs        — Block trait, BlockError, SimpleBlock
  io.rs           — [NEW] IO declaration: InputDecl, OutputDecl, IODeclaration
  state.rs        — [NEW] State declaration: ContinuousState, DiscreteState, StateDeclaration
  dependency.rs   — [NEW] Dependency declaration for block composition
  component.rs    — [NEW] Reusable component template system
  link.rs         — Link struct, LinkSet (topological sort)
  diagram.rs      — Diagram struct (blocks + links)
  diagram_ser.rs  — [NEW] Diagram serialization/deserialization (JSON)
  diagram_validate.rs — [NEW] Diagram validation rules
```

---

## 3. Detailed Specifications

### 3.1 `types.rs` (Update existing)

**Add missing types:**
- `Tensor` — generic N-dimensional array type with shape and flat storage
- `TensorDims` — shape descriptor for Tensor
- `Epsilon` — global comparison threshold constant

### 3.2 `tensor.rs` [NEW]

**Struct `TensorDims`** — N-dimensional shape descriptor:
```rust
pub struct TensorDims(Vec<usize>);
```
Methods: `new(shape)`, `dims()`, `len()`, `is_scalar()`, `is_vector()`, `is_matrix()`, `flat_size()`, `strides()`.

**Struct `Tensor`** — multi-dimensional array:
```rust
pub struct Tensor {
    dims: TensorDims,
    data: Vec<Scalar>,
}
```
Methods: `new(dims)`, `from_vec(dims, data)`, `fill(dims, value)`, `get(indices)`, `set(indices, value)`, `reshape(new_dims)`, `map(f)`, `add(&self, other)`, `mul(&self, other)`, `transpose()`, `dim_size(axis)`, `shape()`, `data()`, `flat_index(indices)`.

**Add `Tensor(Tensor)` variant to `SignalValue`** in types.rs.

### 3.3 `io.rs` [NEW]

**Declarative I/O specification** for blocks, decoupled from Port runtime:

```rust
pub struct InputDecl {
    pub name: String,
    pub signal_type: SignalType,
    pub extent: Extent,
    pub description: String,
    pub required: bool,
    pub default: Option<SignalValue>,
}

pub struct OutputDecl {
    pub name: String,
    pub signal_type: SignalType,
    pub extent: Extent,
    pub description: String,
}

pub struct IODeclaration {
    pub inputs: Vec<InputDecl>,
    pub outputs: Vec<OutputDecl>,
}
```

Methods: `find_input(name)`, `find_output(name)`, `has_input(name)`, `has_output(name)`, `to_port_set()` — converts declarations to actual Port instances.

### 3.4 `state.rs` [NEW]

**State declaration** for blocks' internal runtime state:

```rust
pub struct ContinuousStateVariable {
    pub name: String,
    pub description: String,
    pub initial_value: Scalar,
    pub min: Option<Scalar>,
    pub max: Option<Scalar>,
}

pub struct DiscreteStateVariable {
    pub name: String,
    pub description: String,
    pub initial_value: SignalValue,
}

pub struct StateDeclaration {
    pub continuous: Vec<ContinuousStateVariable>,
    pub discrete: Vec<DiscreteStateVariable>,
}
```

Methods: `continuous_count()`, `discrete_count()`, `get_continuous(name)`, `get_discrete(name)`.

### 3.5 `dependency.rs` [NEW]

**Dependency declaration** for block composition and automatic wiring:

```rust
pub struct DependencyDecl {
    pub provider_block: String,
    pub provider_port: String,
    pub consumer_port: String,
    pub description: String,
}

pub struct DependencySet {
    pub dependencies: Vec<DependencyDecl>,
}
```

### 3.6 `component.rs` [NEW]

**Reusable component template** — a pre-configured Diagram that can be instantiated as a block:

```rust
pub struct ComponentTemplate {
    pub name: String,
    pub io: IODeclaration,
    pub internal_diagram: Diagram,
    pub parameter_mappings: HashMap<String, String>, // external param -> internal block.param
    pub port_mappings: HashMap<String, (String, String)>, // external port -> (internal_block, internal_port)
}
```

Methods: `instantiate(id, param_overrides) -> Result<Box<dyn Block>>`, `validate()`, `export_io()`.

**`ComponentInstance`** — a wrapper that presents a ComponentTemplate as a Block:

```rust
pub struct ComponentInstance {
    id: BlockId,
    template: Arc<ComponentTemplate>,
    instance_diagram: Diagram,
    external_ports: PortSet,
    params: ParameterSet,
    status: ComponentStatus,
    current_time: Time,
}
```

Implements `Block` trait by delegating to the internal diagram's execution.

### 3.7 `block.rs` (Update existing)

**Add to `Block` trait:**
- `fn io_declaration(&self) -> IODeclaration` — returns planned I/O (optional, default empty)
- `fn state_declaration(&self) -> StateDeclaration` — returns planned state (optional, default empty)
- `fn dependencies(&self) -> DependencySet` — returns dependencies (optional, default empty)
- `fn validate_configuration(&self) -> Result<(), Vec<String>>` — self-validation of all declarations

**Add to `SimpleBlock`:**
- Fields: `io_decl`, `state_decl`, `dep_set`
- Methods: `with_io(decl)`, `with_state(decl)`, `with_dependencies(deps)`

### 3.8 `diagram_ser.rs` [NEW]

**Serialization** — JSON-based diagram persistence:

```rust
pub fn diagram_to_json(diagram: &Diagram) -> Result<String, SerError>
pub fn json_to_diagram(json: &str) -> Result<Diagram, SerError>
pub fn diagram_to_toml(diagram: &Diagram) -> Result<String, SerError>
pub fn toml_to_diagram(toml: &str) -> Result<Diagram, SerError>
```

Serialized format includes:
- Diagram name, description
- Blocks: id, type, parameters (name + value + mutability)
- Links: id, source (block.port), destination (block.port), delay
- Metadata: version, schema

**Error types:**
```rust
pub enum SerError {
    ParseError(String),
    MissingField(String),
    InvalidBlockType(String),
    InvalidPort(String),
    IoError(String),
}
```

### 3.9 `diagram_validate.rs` [NEW]

**Validation rules:**

```rust
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

pub enum ValidationError {
    DuplicateBlockId(String),
    DuplicateLinkId(String),
    MissingBlock(String),
    MissingPort { block: String, port: String },
    PortDirectionMismatch { link: String, detail: String },
    SignalTypeMismatch { link: String, detail: String },
    CycleDetected(Vec<String>),
    UnconnectedInput { block: String, port: String },
    DanglingOutput { block: String, port: String },
    ParameterError(String),
}

pub fn validate_diagram(diagram: &Diagram) -> ValidationResult
pub fn validate_block_config(block: &dyn Block) -> Result<(), Vec<String>>
```

---

## 4. Implementation Order

1. `tensor.rs` + update `types.rs` (SignalValue::Tensor variant)
2. `io.rs` — I/O declaration types
3. `state.rs` — State declaration types
4. `dependency.rs` — Dependency declaration types
5. Update `block.rs` — add declaration methods, validate_configuration
6. `component.rs` — ComponentTemplate, ComponentInstance
7. `diagram_ser.rs` — JSON/TOML serialization
8. `diagram_validate.rs` — Validation logic
9. Update `core/mod.rs` — expose all new modules
10. Comprehensive tests for every module

---

## 5. Testing Requirements

- Every public API has at least one test
- Serialization round-trip tests (JSON -> Diagram -> JSON, verify equivalence)
- Validation tests: valid diagram, duplicate IDs, missing blocks, cycles, unconnected ports
- Component instantiation and execution test
- I/O declaration to Port conversion test
- Tensor operations test (creation, indexing, reshape, arithmetic)
- Edge cases: empty diagram, single block, deeply nested components, circular dependencies
- `cargo clippy --all-targets -- -D warnings` — zero warnings

---

## 6. Acceptance Criteria

- [ ] `cargo build` succeeds
- [ ] `cargo test` — all tests pass
- [ ] `cargo clippy --all-targets -- -D warnings` — zero warnings
- [ ] No `todo!()`, `unimplemented!()`, or empty function bodies
- [ ] All new code has English comments
- [ ] `mod.rs` only exposes interfaces, implementations in separate files
- [ ] Serialization round-trip verified
- [ ] Component instantiation verified
- [ ] Diagram validation catches all error types
