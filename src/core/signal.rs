//! Signal types and signal propagation primitives.
//!
//! Defines how signals flow between ports via links, including
//! continuous, discrete, event, and bus signal variants.

use crate::core::types::{Scalar, SignalType, SignalValue, Time};
use std::any::Any;
use std::sync::Arc;

/// A timestamped signal value.
#[derive(Debug, Clone)]
pub struct Signal {
    /// The signal classification.
    pub signal_type: SignalType,
    /// The data carried by this signal.
    pub value: SignalValue,
    /// Simulation time when this signal was produced.
    pub time: Time,
    /// Optional metadata attached to the signal.
    pub metadata: Option<Arc<dyn Any + Send + Sync>>,
}

impl Signal {
    pub fn new(signal_type: SignalType, value: SignalValue, time: Time) -> Self {
        Self { signal_type, value, time, metadata: None }
    }

    pub fn with_metadata(mut self, metadata: Arc<dyn Any + Send + Sync>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Extract a scalar value if present.
    pub fn as_scalar(&self) -> Option<Scalar> {
        match &self.value {
            SignalValue::Scalar(v) => Some(*v),
            _ => None,
        }
    }
}

/// A continuous-time signal: value is defined at all times between samples.
#[derive(Debug, Clone)]
pub struct ContinuousSignal {
    pub current: SignalValue,
    pub time: Time,
}

/// A discrete-time signal: value only defined at sample instants.
#[derive(Debug, Clone)]
pub struct DiscreteSignal {
    pub value: SignalValue,
    pub sample_time: Time,
    pub sample_index: u64,
}

/// An event signal: carries no continuous value, only triggers.
#[derive(Debug, Clone)]
pub struct EventSignal {
    pub event_id: String,
    pub payload: Option<SignalValue>,
}

/// A bus signal: groups multiple named sub-signals.
#[derive(Debug, Clone, Default)]
pub struct BusSignal {
    pub signals: Vec<(String, Signal)>,
}

impl BusSignal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: &str, signal: Signal) {
        self.signals.push((name.to_string(), signal));
    }

    pub fn get(&self, name: &str) -> Option<&Signal> {
        self.signals.iter().find(|(n, _)| n == name).map(|(_, s)| s)
    }
}
