//! Fundamental data types used throughout the simulation kernel.
//!
//! This module defines core numeric types, signal value representations,
//! and common enum/struct types that all other modules depend on.

/// Scalar type alias — f64 throughout the kernel for precision.
pub type Scalar = f64;

/// Integer index type.
pub type Index = usize;

/// Time type — seconds as f64.
pub type Time = f64;

/// Global comparison threshold for floating-point equality.
pub const EPSILON: Scalar = 1e-12;

/// A generic signal value that can be carried across ports and links.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SignalValue {
    /// Real scalar value.
    Scalar(Scalar),
    /// Vector of real values.
    Vector(Vec<Scalar>),
    /// Matrix stored in row-major order (rows, cols, data).
    Matrix(usize, usize, Vec<Scalar>),
    /// Complex value (real, imag).
    Complex(Scalar, Scalar),
    /// Boolean value.
    Boolean(bool),
    /// Integer value.
    Integer(i64),
    /// String value.
    String(String),
    /// N-dimensional tensor value.
    Tensor(crate::core::tensor::Tensor),
    /// No value / uninitialized.
    #[default]
    None,
}

/// Classification of a signal's temporal behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalType {
    /// Continuous-time signal.
    Continuous,
    /// Discrete-time (sampled) signal.
    Discrete,
    /// Event-triggered signal.
    Event,
    /// Composite bus signal.
    Bus,
}

/// The direction of data flow through a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    /// Input port (receives data).
    Input,
    /// Output port (sends data).
    Output,
    /// Bidirectional port.
    InOut,
}

/// The rate at which a block executes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rate {
    /// Continuous-time integration.
    Continuous,
    /// Fixed discrete sample time in seconds.
    Fixed(Time),
    /// Triggered by an event.
    Triggered,
}

/// Execution step within a block's lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// Initialization phase.
    Init,
    /// Output computation phase.
    Output,
    /// Derivative computation phase.
    Deriv,
    /// State update phase.
    Update,
    /// Event detection phase.
    Event,
    /// Termination phase.
    Terminate,
}

/// Status of a simulation component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentStatus {
    /// Component is inactive / uninitialized.
    Inactive,
    /// Component is ready for execution.
    Ready,
    /// Component is currently executing.
    Running,
    /// Component encountered an error.
    Error,
    /// Component has completed.
    Completed,
}

/// A 2D or 3D size specification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    /// Number of dimensions.
    pub dimensions: usize,
    /// Size along each dimension.
    pub sizes: [usize; 3],
}

impl Extent {
    pub const fn scalar() -> Self {
        Self {
            dimensions: 0,
            sizes: [0, 0, 0],
        }
    }

    pub const fn vector(n: usize) -> Self {
        Self {
            dimensions: 1,
            sizes: [n, 0, 0],
        }
    }

    pub const fn matrix(rows: usize, cols: usize) -> Self {
        Self {
            dimensions: 2,
            sizes: [rows, cols, 0],
        }
    }
}
