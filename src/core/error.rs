//! Unified error system for the simulation kernel.
//!
//! Provides a single `SimError` type with numeric error codes,
//! consistent formatting, and context tracking. All kernel modules
//! use this error type instead of separate error enums.

/// Numeric error codes for all simulation kernel errors.
///
/// Format: EABCD where
///   A = module group (0=core, 1=ser, 2=validate, 3=engine)
///   BC = specific error within group
///   D = severity (0=error, 1=warning)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // ── Core errors (E00xx) ──
    /// E0001: A required port connection is missing.
    MissingPort,
    /// E0002: A required parameter is missing.
    MissingParameter,
    /// E0003: Signal type mismatch between connected ports.
    SignalTypeMismatch,
    /// E0004: Numerical computation failed (division by zero, NaN, overflow).
    NumericalError,
    /// E0005: Generic runtime error.
    RuntimeError,
    /// E0006: Validation configuration failed.
    ValidationError,

    // ── Serialization errors (E01xx) ──
    /// E0101: Failed to parse serialized data.
    ParseError,
    /// E0102: Required field is missing from serialized data.
    MissingField,
    /// E0103: Block type in serialized data is unknown.
    InvalidBlockType,
    /// E0104: Port reference in serialized data is invalid.
    InvalidPortRef,
    /// E0105: I/O operation failed.
    IoError,

    // ── Validation errors (E02xx) ──
    /// E0201: Duplicate block ID in diagram.
    DuplicateBlockId,
    /// E0202: Duplicate link ID in diagram.
    DuplicateLinkId,
    /// E0203: Reference to a block that does not exist.
    MissingBlock,
    /// E0204: Cycle detected in diagram topology.
    CycleDetected,
    /// E0205: Input port is not connected.
    UnconnectedInput,
    /// E0206: Output port is not connected (dangling).
    DanglingOutput,
    /// E0207: Port direction mismatch in link.
    PortDirectionMismatch,
    /// E0208: Signal type mismatch between linked ports.
    SignalTypeMismatchLink,

    // ── Engine errors (E03xx) ──
    /// E0301: No execution order available (diagram not sorted).
    NoExecutionOrder,
    /// E0302: General simulation execution error.
    SimulationError,
    /// E0303: Component instantiation or execution error.
    ComponentError,
    /// E0304: Diagram contains algebraic cycles.
    AlgebraicCycle,
}

impl ErrorCode {
    /// Return the numeric string for this error code, e.g. `"E0001"`.
    pub fn code_str(&self) -> &'static str {
        match self {
            Self::MissingPort => "E0001",
            Self::MissingParameter => "E0002",
            Self::SignalTypeMismatch => "E0003",
            Self::NumericalError => "E0004",
            Self::RuntimeError => "E0005",
            Self::ValidationError => "E0006",
            Self::ParseError => "E0101",
            Self::MissingField => "E0102",
            Self::InvalidBlockType => "E0103",
            Self::InvalidPortRef => "E0104",
            Self::IoError => "E0105",
            Self::DuplicateBlockId => "E0201",
            Self::DuplicateLinkId => "E0202",
            Self::MissingBlock => "E0203",
            Self::CycleDetected => "E0204",
            Self::UnconnectedInput => "E0205",
            Self::DanglingOutput => "E0206",
            Self::PortDirectionMismatch => "E0207",
            Self::SignalTypeMismatchLink => "E0208",
            Self::NoExecutionOrder => "E0301",
            Self::SimulationError => "E0302",
            Self::ComponentError => "E0303",
            Self::AlgebraicCycle => "E0304",
        }
    }

    /// Short human-readable label for this error code.
    pub fn label(&self) -> &'static str {
        match self {
            Self::MissingPort => "missing-port",
            Self::MissingParameter => "missing-parameter",
            Self::SignalTypeMismatch => "signal-type-mismatch",
            Self::NumericalError => "numerical-error",
            Self::RuntimeError => "runtime-error",
            Self::ValidationError => "validation-error",
            Self::ParseError => "parse-error",
            Self::MissingField => "missing-field",
            Self::InvalidBlockType => "invalid-block-type",
            Self::InvalidPortRef => "invalid-port-ref",
            Self::IoError => "io-error",
            Self::DuplicateBlockId => "duplicate-block-id",
            Self::DuplicateLinkId => "duplicate-link-id",
            Self::MissingBlock => "missing-block",
            Self::CycleDetected => "cycle-detected",
            Self::UnconnectedInput => "unconnected-input",
            Self::DanglingOutput => "dangling-output",
            Self::PortDirectionMismatch => "port-direction-mismatch",
            Self::SignalTypeMismatchLink => "signal-type-mismatch-link",
            Self::NoExecutionOrder => "no-execution-order",
            Self::SimulationError => "simulation-error",
            Self::ComponentError => "component-error",
            Self::AlgebraicCycle => "algebraic-cycle",
        }
    }
}

/// A structured simulation error with a numeric code and descriptive message.
#[derive(Debug, Clone, PartialEq)]
pub struct SimError {
    /// The error code identifying the category.
    pub code: ErrorCode,
    /// Human-readable error message.
    pub message: String,
    /// Optional context values (e.g. block name, port name) for diagnostics.
    pub context: Vec<String>,
}

impl SimError {
    /// Create a new `SimError` with the given code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: Vec::new(),
        }
    }

    /// Create a `SimError` with additional context.
    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context.push(ctx.into());
        self
    }

    /// Create an `E0001` (missing port) error.
    pub fn missing_port(port: impl Into<String>) -> Self {
        Self::new(ErrorCode::MissingPort, format!("missing port: {}", port.into()))
    }

    /// Create an `E0002` (missing parameter) error.
    pub fn missing_param(param: impl Into<String>) -> Self {
        Self::new(ErrorCode::MissingParameter, format!("missing parameter: {}", param.into()))
    }

    /// Create an `E0003` (signal type mismatch) error.
    pub fn signal_mismatch(port: impl Into<String>, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::SignalTypeMismatch,
            format!("signal type mismatch on port '{}': expected {}, got {}", port.into(), expected.into(), actual.into()),
        )
    }

    /// Create an `E0004` (numerical error) error.
    pub fn numerical(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::NumericalError, msg.into())
    }

    /// Create an `E0005` (runtime error) error.
    pub fn runtime(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::RuntimeError, msg.into())
    }

    /// Create an `E0101` (parse error) error.
    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::ParseError, msg.into())
    }

    /// Create an `E0105` (I/O error) error.
    pub fn io_error(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::IoError, msg.into())
    }

    /// Create an `E0301` (no execution order) error.
    pub fn no_execution_order() -> Self {
        Self::new(ErrorCode::NoExecutionOrder, "no execution order available; call compute_execution_order first")
    }

    /// Create an `E0304` (algebraic cycle) error.
    pub fn algebraic_cycle() -> Self {
        Self::new(ErrorCode::AlgebraicCycle, "cycle detected in diagram topology")
    }
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code.code_str(), self.message)?;
        if !self.context.is_empty() {
            write!(f, " (context: {})", self.context.join(", "))?;
        }
        Ok(())
    }
}

impl std::error::Error for SimError {}

/// Helper trait for converting results into `SimError` results.
pub trait IntoSimError<T> {
    /// Convert into a `Result<T, SimError>`.
    fn into_sim_err(self, code: ErrorCode) -> Result<T, SimError>;
}

impl<T, E: std::fmt::Display> IntoSimError<T> for Result<T, E> {
    fn into_sim_err(self, code: ErrorCode) -> Result<T, SimError> {
        self.map_err(|e| SimError::new(code, e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_format() {
        assert_eq!(ErrorCode::MissingPort.code_str(), "E0001");
        assert_eq!(ErrorCode::ParseError.code_str(), "E0101");
        assert_eq!(ErrorCode::NoExecutionOrder.code_str(), "E0301");
    }

    #[test]
    fn test_sim_error_display() {
        let err = SimError::missing_port("in1");
        let msg = err.to_string();
        assert!(msg.contains("E0001"));
        assert!(msg.contains("missing port: in1"));
    }

    #[test]
    fn test_sim_error_with_context() {
        let err = SimError::new(ErrorCode::RuntimeError, "something failed")
            .with_context("block=amp1")
            .with_context("phase=output");
        assert_eq!(err.context.len(), 2);
    }

    #[test]
    fn test_convenience_constructors() {
        let e1 = SimError::missing_port("out");
        assert_eq!(e1.code, ErrorCode::MissingPort);

        let e2 = SimError::runtime("unexpected error");
        assert_eq!(e2.code, ErrorCode::RuntimeError);

        let e3 = SimError::no_execution_order();
        assert_eq!(e3.code, ErrorCode::NoExecutionOrder);
    }
}
