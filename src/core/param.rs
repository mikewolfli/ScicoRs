//! Parameter system for simulation blocks.
//!
//! Supports static configuration, runtime-variable parameters,
//! and expression-bound parameters evaluated on the fly.

use crate::core::types::{Scalar, SignalValue};
use std::collections::HashMap;
use std::sync::Arc;

/// Describes the mutability of a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMutability {
    /// Fixed at compile time; never changes.
    Static,
    /// Can be changed at runtime before simulation starts.
    Config,
    /// Can be changed during simulation.
    Tunable,
}

/// A single parameter value with metadata.
#[derive(Debug, Clone)]
pub struct Parameter {
    /// The parameter's unique name within its scope.
    pub name: String,
    /// The current value.
    pub value: SignalValue,
    /// Human-readable description.
    pub description: String,
    /// Mutability level.
    pub mutability: ParamMutability,
}

impl Parameter {
    /// Create a new static parameter.
    pub fn new_static(name: &str, value: SignalValue, description: &str) -> Self {
        Self {
            name: name.to_string(),
            value,
            description: description.to_string(),
            mutability: ParamMutability::Static,
        }
    }

    /// Create a new configurable parameter.
    pub fn new_config(name: &str, value: SignalValue, description: &str) -> Self {
        Self {
            name: name.to_string(),
            value,
            description: description.to_string(),
            mutability: ParamMutability::Config,
        }
    }

    /// Create a new tunable parameter.
    pub fn new_tunable(name: &str, value: SignalValue, description: &str) -> Self {
        Self {
            name: name.to_string(),
            value,
            description: description.to_string(),
            mutability: ParamMutability::Tunable,
        }
    }
}

/// A function type for evaluating expression-bound parameters.
pub type ParamExprFn = Arc<dyn Fn(&HashMap<String, Scalar>) -> Scalar + Send + Sync>;

/// An expression-bound parameter that evaluates a function on access.
#[derive(Clone)]
pub struct ExpressionParameter {
    pub name: String,
    pub description: String,
    pub expression: ParamExprFn,
    pub dependencies: Vec<String>,
}

impl std::fmt::Debug for ExpressionParameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExpressionParameter")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("dependencies", &self.dependencies)
            .field("expression", &"<closure>")
            .finish()
    }
}

impl ExpressionParameter {
    pub fn evaluate(&self, context: &HashMap<String, Scalar>) -> Scalar {
        (self.expression)(context)
    }
}

/// A collection of parameters keyed by name.
#[derive(Clone, Default)]
pub struct ParameterSet {
    params: HashMap<String, Parameter>,
    exprs: HashMap<String, ExpressionParameter>,
}

impl std::fmt::Debug for ParameterSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParameterSet")
            .field("param_count", &self.params.len())
            .field("expr_count", &self.exprs.len())
            .finish()
    }
}

impl ParameterSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, param: Parameter) {
        self.params.insert(param.name.clone(), param);
    }

    pub fn add_expr(&mut self, expr: ExpressionParameter) {
        self.exprs.insert(expr.name.clone(), expr);
    }

    pub fn get(&self, name: &str) -> Option<&Parameter> {
        self.params.get(name)
    }

    pub fn get_scalar(&self, name: &str) -> Option<Scalar> {
        self.params.get(name).and_then(|p| match &p.value {
            SignalValue::Scalar(v) => Some(*v),
            _ => None,
        })
    }

    pub fn set(&mut self, name: &str, value: SignalValue) -> Option<()> {
        let param = self.params.get_mut(name)?;
        if param.mutability != ParamMutability::Static {
            param.value = value;
            Some(())
        } else {
            None
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.params.contains_key(name) || self.exprs.contains_key(name)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.params.keys().chain(self.exprs.keys())
    }

    pub fn len(&self) -> usize {
        self.params.len() + self.exprs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.params.is_empty() && self.exprs.is_empty()
    }
}
