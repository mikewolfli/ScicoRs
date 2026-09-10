//! State declaration types for simulation blocks.
//!
//! Defines the structure of a block's internal runtime state,
//! separating continuous (derivative-driven) and discrete
//! (event-driven) state variables.

use crate::core::types::{Scalar, SignalValue};

/// A single continuous state variable (driven by derivative).
#[derive(Debug, Clone)]
pub struct ContinuousStateVar {
    /// Unique name within the block.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Initial value at simulation start.
    pub initial_value: Scalar,
    /// Minimum allowable value (optional).
    pub min: Option<Scalar>,
    /// Maximum allowable value (optional).
    pub max: Option<Scalar>,
}

impl ContinuousStateVar {
    pub fn new(name: &str, initial_value: Scalar) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            initial_value,
            min: None,
            max: None,
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_bounds(mut self, min: Scalar, max: Scalar) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }
}

/// A single discrete state variable (updated on events or sample hits).
#[derive(Debug, Clone)]
pub struct DiscreteStateVar {
    /// Unique name within the block.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Initial value at simulation start.
    pub initial_value: SignalValue,
}

impl DiscreteStateVar {
    pub fn new(name: &str, initial_value: SignalValue) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            initial_value,
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
}

/// Complete state declaration for a simulation block.
#[derive(Debug, Clone, Default)]
pub struct StateDeclaration {
    /// Continuous state variables.
    pub continuous: Vec<ContinuousStateVar>,
    /// Discrete state variables.
    pub discrete: Vec<DiscreteStateVar>,
}

impl StateDeclaration {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_continuous(&mut self, var: ContinuousStateVar) {
        self.continuous.push(var);
    }

    pub fn add_discrete(&mut self, var: DiscreteStateVar) {
        self.discrete.push(var);
    }

    /// Number of continuous state variables.
    pub fn continuous_count(&self) -> usize {
        self.continuous.len()
    }

    /// Number of discrete state variables.
    pub fn discrete_count(&self) -> usize {
        self.discrete.len()
    }

    /// Find a continuous variable by name.
    pub fn find_continuous(&self, name: &str) -> Option<&ContinuousStateVar> {
        self.continuous.iter().find(|v| v.name == name)
    }

    /// Find a discrete variable by name.
    pub fn find_discrete(&self, name: &str) -> Option<&DiscreteStateVar> {
        self.discrete.iter().find(|v| v.name == name)
    }

    pub fn is_empty(&self) -> bool {
        self.continuous.is_empty() && self.discrete.is_empty()
    }

    pub fn len(&self) -> usize {
        self.continuous.len() + self.discrete.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_declaration() {
        let mut sd = StateDeclaration::new();
        sd.add_continuous(ContinuousStateVar::new("x1", 0.0)
            .with_description("position")
            .with_bounds(-1e6, 1e6));
        sd.add_discrete(DiscreteStateVar::new("z1", SignalValue::Integer(0)));
        assert_eq!(sd.continuous_count(), 1);
        assert_eq!(sd.discrete_count(), 1);
        assert!(sd.find_continuous("x1").is_some());
        assert!(sd.find_discrete("z1").is_some());
    }
}
