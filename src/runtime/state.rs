//! Unified simulation state management.
//!
//! Provides `ContinuousState` (derivative-driven vector), `DiscreteState`
//! (event-updated variables), and `SimStateManager` which ties both together.
//! State snapshots enable save/restore for checkpointing and rollback.

use crate::core::state::StateDeclaration;
use crate::core::types::{Scalar, SignalValue};
use std::collections::HashMap;

/// A snapshot of the full simulation state at a point in time.
#[derive(Debug, Clone, PartialEq)]
pub struct StateSnapshot {
    /// Continuous state values.
    pub x: Vec<Scalar>,
    /// Continuous state derivatives.
    pub dx: Vec<Scalar>,
    /// Discrete state values.
    pub z: Vec<SignalValue>,
}

/// Continuous state vector: named variables with derivative tracking.
#[derive(Debug, Clone)]
pub struct ContinuousState {
    /// Variable names (index ↔ name mapping).
    names: Vec<String>,
    /// Name → index lookup.
    name_to_idx: HashMap<String, usize>,
    /// Current state values x.
    x: Vec<Scalar>,
    /// State derivatives dx/dt.
    dx: Vec<Scalar>,
    /// Initial values (for reset).
    initials: Vec<Scalar>,
}

impl ContinuousState {
    /// Create a new continuous state from name and initial value pairs.
    pub fn new(names: &[&str], initials: &[Scalar]) -> Self {
        assert_eq!(
            names.len(),
            initials.len(),
            "names and initials must have the same length"
        );
        let name_to_idx: HashMap<String, usize> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.to_string(), i))
            .collect();
        Self {
            names: names.iter().map(|n| n.to_string()).collect(),
            name_to_idx,
            x: initials.to_vec(),
            dx: vec![0.0; names.len()],
            initials: initials.to_vec(),
        }
    }

    /// Build from a `StateDeclaration` (Phase 1 type).
    pub fn from_declaration(decl: &StateDeclaration) -> Self {
        let names: Vec<&str> = decl.continuous.iter().map(|v| v.name.as_str()).collect();
        let initials: Vec<Scalar> = decl.continuous.iter().map(|v| v.initial_value).collect();
        Self::new(&names, &initials)
    }

    /// Number of continuous state variables.
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// Returns `true` if there are no state variables.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// Get a state value by name.
    pub fn get(&self, name: &str) -> Option<Scalar> {
        self.name_to_idx.get(name).copied().map(|i| self.x[i])
    }

    /// Get a state value by index.
    pub fn get_index(&self, index: usize) -> Scalar {
        self.x[index]
    }

    /// Get all state values as a slice.
    pub fn values(&self) -> &[Scalar] {
        &self.x
    }

    /// Get a mutable reference to all state values.
    pub fn values_mut(&mut self) -> &mut [Scalar] {
        &mut self.x
    }

    /// Set a state value by name.
    pub fn set(&mut self, name: &str, value: Scalar) -> Option<()> {
        let i = self.name_to_idx.get(name).copied()?;
        self.x[i] = value;
        Some(())
    }

    /// Set a state value by index.
    pub fn set_index(&mut self, index: usize, value: Scalar) {
        self.x[index] = value;
    }

    /// Get the derivative for a variable by name.
    pub fn derivative(&self, name: &str) -> Option<Scalar> {
        self.name_to_idx.get(name).copied().map(|i| self.dx[i])
    }

    /// Get the derivative at an index.
    pub fn derivative_index(&self, index: usize) -> Scalar {
        self.dx[index]
    }

    /// Get all derivatives as a slice.
    pub fn derivatives(&self) -> &[Scalar] {
        &self.dx
    }

    /// Set all derivatives at once.
    pub fn set_derivatives(&mut self, derivatives: &[Scalar]) {
        assert_eq!(
            derivatives.len(),
            self.dx.len(),
            "derivatives slice length must match state size"
        );
        self.dx.copy_from_slice(derivatives);
    }

    /// Set an individual derivative by index.
    pub fn set_derivative(&mut self, index: usize, value: Scalar) {
        self.dx[index] = value;
    }

    /// Integrate state using forward Euler: x += dt * dx.
    pub fn integrate_euler(&mut self, dt: Scalar) {
        for i in 0..self.x.len() {
            self.x[i] += dt * self.dx[i];
        }
    }

    /// Reset all state to initial values and zero derivatives.
    pub fn reset(&mut self) {
        self.x.copy_from_slice(&self.initials);
        self.dx.fill(0.0);
    }

    /// Capture a snapshot of the current state.
    pub fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            x: self.x.clone(),
            dx: self.dx.clone(),
            z: Vec::new(),
        }
    }

    /// Restore state from a snapshot (continuous portion only).
    pub fn restore(&mut self, snapshot: &StateSnapshot) {
        assert_eq!(
            snapshot.x.len(),
            self.x.len(),
            "snapshot x length mismatch"
        );
        assert_eq!(
            snapshot.dx.len(),
            self.dx.len(),
            "snapshot dx length mismatch"
        );
        self.x.copy_from_slice(&snapshot.x);
        self.dx.copy_from_slice(&snapshot.dx);
    }

    /// Get the list of variable names.
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

/// Discrete state vector: named variables updated on events or sample hits.
#[derive(Debug, Clone)]
pub struct DiscreteState {
    /// Variable names.
    names: Vec<String>,
    /// Name → index lookup.
    name_to_idx: HashMap<String, usize>,
    /// Current discrete state values z.
    z: Vec<SignalValue>,
    /// Initial values (for reset).
    initials: Vec<SignalValue>,
}

impl DiscreteState {
    /// Create a new discrete state from name and initial value pairs.
    pub fn new(names: &[&str], initials: &[SignalValue]) -> Self {
        assert_eq!(
            names.len(),
            initials.len(),
            "names and initials must have the same length"
        );
        let name_to_idx: HashMap<String, usize> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.to_string(), i))
            .collect();
        Self {
            names: names.iter().map(|n| n.to_string()).collect(),
            name_to_idx,
            z: initials.to_vec(),
            initials: initials.to_vec(),
        }
    }

    /// Create a new discrete state from owned name and value vectors.
    pub fn from_vec(names: Vec<String>, initials: Vec<SignalValue>) -> Self {
        let name_to_idx: HashMap<String, usize> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), i))
            .collect();
        let z = initials.clone();
        Self {
            names,
            name_to_idx,
            z,
            initials,
        }
    }

    /// Build from a `StateDeclaration` (Phase 1 type).
    pub fn from_declaration(decl: &StateDeclaration) -> Self {
        let names: Vec<&str> = decl.discrete.iter().map(|v| v.name.as_str()).collect();
        let initials: Vec<SignalValue> =
            decl.discrete.iter().map(|v| v.initial_value.clone()).collect();
        Self::new(&names, &initials)
    }

    /// Number of discrete state variables.
    pub fn len(&self) -> usize {
        self.z.len()
    }

    /// Returns `true` if there are no state variables.
    pub fn is_empty(&self) -> bool {
        self.z.is_empty()
    }

    /// Get a state value by name.
    pub fn get(&self, name: &str) -> Option<&SignalValue> {
        self.name_to_idx.get(name).copied().map(|i| &self.z[i])
    }

    /// Get a state value by index.
    pub fn get_index(&self, index: usize) -> &SignalValue {
        &self.z[index]
    }

    /// Get all state values.
    pub fn values(&self) -> &[SignalValue] {
        &self.z
    }

    /// Set a state value by name.
    pub fn set(&mut self, name: &str, value: SignalValue) -> Option<()> {
        let i = self.name_to_idx.get(name).copied()?;
        self.z[i] = value;
        Some(())
    }

    /// Set a state value by index.
    pub fn set_index(&mut self, index: usize, value: SignalValue) {
        self.z[index] = value;
    }

    /// Reset all state to initial values.
    pub fn reset(&mut self) {
        for (i, v) in self.initials.iter().enumerate() {
            self.z[i] = v.clone();
        }
    }

    /// Capture a snapshot of discrete state.
    pub fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            x: Vec::new(),
            dx: Vec::new(),
            z: self.z.clone(),
        }
    }

    /// Restore discrete state from a snapshot.
    pub fn restore(&mut self, snapshot: &StateSnapshot) {
        assert_eq!(
            snapshot.z.len(),
            self.z.len(),
            "snapshot z length mismatch"
        );
        for (i, v) in snapshot.z.iter().enumerate() {
            self.z[i] = v.clone();
        }
    }

    /// Get the list of variable names.
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

/// Unified access to both continuous and discrete simulation state.
#[derive(Debug, Clone)]
pub struct SimStateManager {
    /// Continuous state (derivative-driven).
    pub continuous: ContinuousState,
    /// Discrete state (event-updated).
    pub discrete: DiscreteState,
}

impl SimStateManager {
    /// Create a new state manager from individual state objects.
    pub fn new(continuous: ContinuousState, discrete: DiscreteState) -> Self {
        Self { continuous, discrete }
    }

    /// Build from a `StateDeclaration`, extracting continuous and discrete parts.
    pub fn from_declaration(decl: &StateDeclaration) -> Self {
        Self {
            continuous: ContinuousState::from_declaration(decl),
            discrete: DiscreteState::from_declaration(decl),
        }
    }

    /// Create an empty (zero-variable) state manager.
    pub fn empty() -> Self {
        Self {
            continuous: ContinuousState::new(&[], &[]),
            discrete: DiscreteState::new(&[], &[]),
        }
    }

    /// Total number of state variables (continuous + discrete).
    pub fn total_len(&self) -> usize {
        self.continuous.len() + self.discrete.len()
    }

    /// Capture a snapshot of the entire state.
    pub fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            x: self.continuous.values().to_vec(),
            dx: self.continuous.derivatives().to_vec(),
            z: self.discrete.values().to_vec(),
        }
    }

    /// Restore the entire state from a snapshot.
    pub fn restore(&mut self, snapshot: &StateSnapshot) {
        self.continuous.restore(snapshot);
        self.discrete.restore(snapshot);
    }

    /// Reset both continuous and discrete state to initial values.
    pub fn reset(&mut self) {
        self.continuous.reset();
        self.discrete.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_continuous_state_create_and_access() {
        let cs = ContinuousState::new(&["x1", "x2"], &[0.0, 1.0]);
        assert_eq!(cs.len(), 2);
        assert!(!cs.is_empty());
        assert_eq!(cs.get("x1"), Some(0.0));
        assert_eq!(cs.get("x2"), Some(1.0));
        assert_eq!(cs.get_index(0), 0.0);
        assert_eq!(cs.get_index(1), 1.0);
    }

    #[test]
    fn test_continuous_state_set() {
        let mut cs = ContinuousState::new(&["x"], &[0.0]);
        cs.set("x", 42.0).unwrap();
        assert_eq!(cs.get("x"), Some(42.0));
    }

    #[test]
    fn test_continuous_state_derivatives() {
        let mut cs = ContinuousState::new(&["x"], &[0.0]);
        cs.set_derivatives(&[5.0]);
        assert_eq!(cs.derivative("x"), Some(5.0));
        assert_eq!(cs.derivative_index(0), 5.0);
    }

    #[test]
    fn test_continuous_state_integrate_euler() {
        let mut cs = ContinuousState::new(&["x"], &[0.0]);
        cs.set_derivatives(&[2.0]);
        cs.integrate_euler(0.5);
        assert!((cs.get("x").unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_continuous_state_reset() {
        let mut cs = ContinuousState::new(&["x"], &[0.0]);
        cs.set("x", 99.0).unwrap();
        cs.set_derivatives(&[3.0]);
        cs.reset();
        assert_eq!(cs.get("x"), Some(0.0));
        assert_eq!(cs.derivative("x"), Some(0.0));
    }

    #[test]
    fn test_continuous_state_snapshot_restore() {
        let mut cs = ContinuousState::new(&["x", "y"], &[0.0, 0.0]);
        cs.set("x", 3.0).unwrap();
        cs.set("y", 4.0).unwrap();
        cs.set_derivatives(&[1.0, 2.0]);
        let snap = cs.snapshot();
        cs.reset();
        cs.restore(&snap);
        assert_eq!(cs.get("x"), Some(3.0));
        assert_eq!(cs.get("y"), Some(4.0));
        assert_eq!(cs.derivative("x"), Some(1.0));
        assert_eq!(cs.derivative("y"), Some(2.0));
    }

    #[test]
    fn test_discrete_state_create_and_access() {
        let mut ds = DiscreteState::new(
            &["z1", "z2"],
            &[SignalValue::Integer(0), SignalValue::Boolean(false)],
        );
        assert_eq!(ds.len(), 2);
        assert_eq!(ds.get("z1"), Some(&SignalValue::Integer(0)));
        ds.set("z2", SignalValue::Boolean(true)).unwrap();
        assert_eq!(ds.get("z2"), Some(&SignalValue::Boolean(true)));
    }

    #[test]
    fn test_discrete_state_reset() {
        let mut ds = DiscreteState::new(&["z"], &[SignalValue::Integer(0)]);
        ds.set("z", SignalValue::Integer(100)).unwrap();
        ds.reset();
        assert_eq!(ds.get("z"), Some(&SignalValue::Integer(0)));
    }

    #[test]
    fn test_discrete_state_snapshot_restore() {
        let mut ds = DiscreteState::new(&["z"], &[SignalValue::Integer(0)]);
        ds.set("z", SignalValue::Integer(42)).unwrap();
        let snap = ds.snapshot();
        ds.reset();
        ds.restore(&snap);
        assert_eq!(ds.get("z"), Some(&SignalValue::Integer(42)));
    }

    #[test]
    fn test_state_manager_from_declaration() {
        let mut decl = StateDeclaration::new();
        decl.add_continuous(crate::core::state::ContinuousStateVar::new("pos", 0.0));
        decl.add_discrete(crate::core::state::DiscreteStateVar::new("mode", SignalValue::Integer(1)));
        let mgr = SimStateManager::from_declaration(&decl);
        assert_eq!(mgr.continuous.len(), 1);
        assert_eq!(mgr.discrete.len(), 1);
        assert_eq!(mgr.total_len(), 2);
    }

    #[test]
    fn test_state_manager_snapshot_restore() {
        let mut mgr = SimStateManager::new(
            ContinuousState::new(&["x"], &[0.0]),
            DiscreteState::new(&["z"], &[SignalValue::Integer(0)]),
        );
        mgr.continuous.set("x", 10.0).unwrap();
        mgr.discrete.set("z", SignalValue::Integer(5)).unwrap();
        let snap = mgr.snapshot();
        mgr.reset();
        mgr.restore(&snap);
        assert_eq!(mgr.continuous.get("x"), Some(10.0));
        assert_eq!(mgr.discrete.get("z"), Some(&SignalValue::Integer(5)));
    }

    #[test]
    fn test_state_manager_empty() {
        let mgr = SimStateManager::empty();
        assert_eq!(mgr.total_len(), 0);
    }

    #[test]
    fn test_continuous_state_names() {
        let cs = ContinuousState::new(&["a", "b", "c"], &[1.0, 2.0, 3.0]);
        assert_eq!(cs.names(), &["a", "b", "c"]);
    }

    #[test]
    fn test_discrete_state_names() {
        let ds = DiscreteState::new(&["a", "b"], &[SignalValue::Scalar(0.0), SignalValue::Scalar(1.0)]);
        assert_eq!(ds.names(), &["a", "b"]);
    }

    #[test]
    fn test_continuous_state_values_slice() {
        let cs = ContinuousState::new(&["x"], &[std::f64::consts::PI]);
        assert!((cs.values()[0] - std::f64::consts::PI).abs() < 1e-12);
    }
}
