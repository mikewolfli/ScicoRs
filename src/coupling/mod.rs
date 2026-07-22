//! Unified Multi-Physics Coupling Bus
//!
//! Provides the infrastructure for coupling multiple physical domains
//! together in a co-simulation. Handles data exchange between domains,
//! coupling scheme configuration, and convergence management.

use crate::core::error::SimError;
use crate::core::types::{Scalar, Time};
use std::collections::HashMap;

/// Direction of coupling data exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CouplingDirection {
    /// Forward coupling: domain A → domain B.
    Forward,
    /// Backward coupling: domain B → domain A.
    Backward,
    /// Bidirectional coupling.
    Bidirectional,
}

/// The method used to exchange data between coupled domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CouplingMethod {
    /// Weak (explicit) coupling — each domain solves independently, exchanges once per step.
    Weak,
    /// Strong (implicit) coupling — iterates between domains until convergence.
    Strong,
    /// Gauss-Seidel style — domains solved sequentially, latest values used immediately.
    GaussSeidel,
    /// Jacobi style — all domains solved in parallel, exchange after all complete.
    Jacobi,
}

/// A variable exchanged between coupled domains.
#[derive(Debug, Clone)]
pub struct CouplingVariable {
    pub name: String,
    pub value: Scalar,
    pub unit: String,
    pub direction: CouplingDirection,
}

/// A coupling interface connecting two physical domains.
#[derive(Debug)]
pub struct CouplingInterface {
    pub name: String,
    pub domain_a: String,
    pub domain_b: String,
    pub method: CouplingMethod,
    pub variables: Vec<CouplingVariable>,
    /// Relaxation factor for stability (0.0 - 1.0).
    pub relaxation: Scalar,
    /// Maximum coupling iterations (strong coupling).
    pub max_iterations: usize,
    /// Convergence tolerance for coupling iterations.
    pub tolerance: Scalar,
}

impl CouplingInterface {
    pub fn new(name: &str, domain_a: &str, domain_b: &str, method: CouplingMethod) -> Self {
        Self {
            name: name.to_string(),
            domain_a: domain_a.to_string(),
            domain_b: domain_b.to_string(),
            method,
            variables: Vec::new(),
            relaxation: 0.5,
            max_iterations: 20,
            tolerance: 1e-6,
        }
    }

    pub fn add_variable(&mut self, var: CouplingVariable) {
        self.variables.push(var);
    }
}

/// A bus that manages multiple coupled domains.
#[derive(Debug, Default)]
pub struct CouplingBus {
    interfaces: Vec<CouplingInterface>,
    /// Cache of exchanged values: (interface_name, var_name) -> value
    exchange_cache: HashMap<(String, String), Scalar>,
}

impl CouplingBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_interface(&mut self, interface: CouplingInterface) {
        self.interfaces.push(interface);
    }

    pub fn interfaces(&self) -> &[CouplingInterface] {
        &self.interfaces
    }

    /// Perform a single coupling step across all interfaces.
    pub fn step(&mut self, _time: Time) -> Result<(), SimError> {
        for interface in &self.interfaces {
            match interface.method {
                CouplingMethod::Weak => {
                    // Simple one-shot exchange
                    for var in &interface.variables {
                        self.exchange_cache
                            .insert((interface.name.clone(), var.name.clone()), var.value);
                    }
                }
                CouplingMethod::Strong => {
                    // Iterative coupling
                    for _iter in 0..interface.max_iterations {
                        let mut max_change = 0.0;
                        for var in &interface.variables {
                            let prev = self
                                .exchange_cache
                                .get(&(interface.name.clone(), var.name.clone()))
                                .copied()
                                .unwrap_or(var.value);
                            let new_val = (1.0 - interface.relaxation) * prev
                                + interface.relaxation * var.value;
                            let change = (new_val - prev).abs();
                            if change > max_change {
                                max_change = change;
                            }
                            self.exchange_cache
                                .insert((interface.name.clone(), var.name.clone()), new_val);
                        }
                        if max_change < interface.tolerance {
                            break;
                        }
                    }
                }
                _ => {
                    // Gauss-Seidel / Jacobi
                    for var in &interface.variables {
                        self.exchange_cache
                            .insert((interface.name.clone(), var.name.clone()), var.value);
                    }
                }
            }
        }
        Ok(())
    }

    /// Get the exchanged value of a coupled variable.
    pub fn get_exchanged(&self, interface_name: &str, var_name: &str) -> Option<Scalar> {
        self.exchange_cache
            .get(&(interface_name.to_string(), var_name.to_string()))
            .copied()
    }
}
