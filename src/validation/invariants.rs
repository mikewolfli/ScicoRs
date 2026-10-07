// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Registrable physical invariants (Phase 40).
//!
//! # Explicit, scoped registration
//!
//! An invariant is a conserved (or monotonically changing) quantity: total
//! mass, energy, charge, or probability. Crucially, not every invariant applies
//! to every model — a charge-conservation check is meaningless for a
//! gravity-only problem, and forcing it on globally would produce spurious
//! failures. This module therefore requires each invariant to be *registered*
//! against the models it applies to, declares the monitored quantity, its
//! characteristic scale, a tolerance and the allowed source/sink terms, and
//! reports an explicit [`InvariantOutcome::NotApplicable`] when asked to check a
//! model the invariant was not registered for.

use crate::core::types::Scalar;

use super::benchmark::ValidationError;

/// The physical bookkeeping an invariant performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InvariantKind {
    /// Conserved absolutely (drift must be ~0).
    Mass,
    /// Conserved except through declared boundaries / sources.
    Energy,
    /// Conserved in closed systems (net charge).
    Charge,
    /// Normalised probability, e.g. `sum |psi|^2 = 1`.
    Probability,
}

impl InvariantKind {
    /// Short machine-readable label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mass => "mass",
            Self::Energy => "energy",
            Self::Charge => "charge",
            Self::Probability => "probability",
        }
    }
}

/// An allowed non-zero change in the monitored quantity.
///
/// A conserving scheme has no sources or sinks; a scheme with an open boundary
/// declares the boundary flux here so that it is not mistaken for a
/// conservation failure.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceSink {
    /// Name of the term, e.g. `"inflow"`, `"radiative_loss"`.
    pub name: String,
    /// Per-report-interval contribution to the monitored quantity, in the same
    /// units as the invariant. May be negative (a sink).
    pub amount: Scalar,
}

impl SourceSink {
    /// Construct a named source/sink term.
    pub fn new(name: impl Into<String>, amount: Scalar) -> Self {
        Self {
            name: name.into(),
            amount,
        }
    }
}

/// A registered physical invariant.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Invariant {
    /// Stable name, e.g. `"total_mass"`.
    pub name: String,
    /// The kind of bookkeeping performed.
    pub kind: InvariantKind,
    /// Units of the monitored quantity, e.g. `"kg"`, `"J"`, `"C"`, `"1"`.
    pub unit: String,
    /// Characteristic magnitude used to convert an absolute tolerance into a
    /// relative one.
    pub scale: Scalar,
    /// Allowed relative drift, `|drift| / scale <= tolerance`.
    pub tolerance: Scalar,
    /// Allowed source/sink terms; empty means a strictly conserving check.
    pub sources: Vec<SourceSink>,
    /// The model identifiers this invariant applies to. Checking a model not in
    /// this list yields [`InvariantOutcome::NotApplicable`].
    pub applies_to: Vec<String>,
}

/// The verdict of an invariant check.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InvariantOutcome {
    /// The invariant held (drift within tolerance after accounting for
    /// declared sources).
    Held {
        /// Relative drift, net of declared sources.
        relative_drift: Scalar,
    },
    /// The invariant was violated.
    Violated {
        /// Relative drift, net of declared sources.
        relative_drift: Scalar,
        /// The tolerance that was exceeded.
        tolerance: Scalar,
    },
    /// The invariant is not registered for the queried model.
    NotApplicable {
        /// The model that was queried.
        model: String,
    },
    /// The check could not be performed (bad inputs).
    Invalid {
        /// Explanation.
        detail: String,
    },
}

impl InvariantOutcome {
    /// Whether the invariant held.
    pub fn passed(&self) -> bool {
        matches!(self, Self::Held { .. })
    }
}

impl Invariant {
    /// Register a new invariant, validating its parameters.
    ///
    /// Fails for an empty name, a non-positive scale, a negative tolerance, or
    /// an empty applicability list (an invariant that applies to nothing cannot
    /// legitimately be registered).
    pub fn register(
        name: impl Into<String>,
        kind: InvariantKind,
        unit: impl Into<String>,
        scale: Scalar,
        tolerance: Scalar,
        sources: Vec<SourceSink>,
        applies_to: Vec<String>,
    ) -> Result<Self, ValidationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(ValidationError::ConvergenceData {
                detail: "invariant name must not be empty".to_string(),
            });
        }
        if !scale.is_finite() || scale <= 0.0 {
            return Err(ValidationError::InvalidTolerance { value: scale });
        }
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(ValidationError::InvalidTolerance { value: tolerance });
        }
        if applies_to.is_empty() {
            return Err(ValidationError::InvariantNotApplicable {
                name,
                model: "<none>".to_string(),
            });
        }
        Ok(Self {
            name,
            kind,
            unit: unit.into(),
            scale,
            tolerance,
            sources,
            applies_to,
        })
    }

    /// Whether this invariant is registered for `model`.
    pub fn applies_to_model(&self, model: &str) -> bool {
        self.applies_to.iter().any(|m| m == model)
    }

    /// Sum of declared source/sink contributions per report interval.
    pub fn net_source(&self) -> Scalar {
        self.sources.iter().map(|s| s.amount).sum()
    }

    /// Check the invariant for `model` given the monitored quantity's initial
    /// and final values.
    ///
    /// The observed change is reduced by the declared net source/sink before
    /// the tolerance is applied, so a legitimate open-boundary flux does not
    /// register as a violation.
    pub fn check(&self, model: &str, initial: Scalar, final_value: Scalar) -> InvariantOutcome {
        if !self.applies_to_model(model) {
            return InvariantOutcome::NotApplicable {
                model: model.to_string(),
            };
        }
        if !initial.is_finite() || !final_value.is_finite() {
            return InvariantOutcome::Invalid {
                detail: "initial or final value is not finite".to_string(),
            };
        }
        let observed_change = final_value - initial;
        let unexplained = observed_change - self.net_source();
        let relative_drift = unexplained / self.scale;
        if relative_drift.abs() <= self.tolerance {
            InvariantOutcome::Held { relative_drift }
        } else {
            InvariantOutcome::Violated {
                relative_drift,
                tolerance: self.tolerance,
            }
        }
    }
}

/// A registry of invariants and the models they may be checked against.
///
/// The registry is deliberately not a global singleton: each validation run
/// owns its registry so that a domain can enable exactly the invariants it
/// knows apply, rather than inheriting a global set.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InvariantRegistry {
    invariants: Vec<Invariant>,
}

impl InvariantRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            invariants: Vec::new(),
        }
    }

    /// Register an invariant. Returns `Err` on a duplicate name.
    pub fn add(&mut self, invariant: Invariant) -> Result<(), ValidationError> {
        if self.invariants.iter().any(|i| i.name == invariant.name) {
            return Err(ValidationError::ConvergenceData {
                detail: format!("invariant '{}' already registered", invariant.name),
            });
        }
        self.invariants.push(invariant);
        Ok(())
    }

    /// Number of registered invariants.
    pub fn len(&self) -> usize {
        self.invariants.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.invariants.is_empty()
    }

    /// The invariants registered for a given model.
    pub fn for_model(&self, model: &str) -> Vec<&Invariant> {
        self.invariants
            .iter()
            .filter(|i| i.applies_to_model(model))
            .collect()
    }

    /// Check one registered invariant by name for a model.
    ///
    /// Returns [`InvariantOutcome::NotApplicable`] when the named invariant is
    /// unknown or not registered for the model, so a caller can never silently
    /// treat "we did not check" as "it held".
    pub fn check(
        &self,
        name: &str,
        model: &str,
        initial: Scalar,
        final_value: Scalar,
    ) -> InvariantOutcome {
        match self.invariants.iter().find(|i| i.name == name) {
            Some(inv) => inv.check(model, initial, final_value),
            None => InvariantOutcome::NotApplicable {
                model: model.to_string(),
            },
        }
    }

    /// Check every invariant registered for `model`.
    ///
    /// `initial` and `final_value` are looked up by invariant name; a missing
    /// entry yields [`InvariantOutcome::Invalid`] rather than a pass.
    pub fn check_model(
        &self,
        model: &str,
        values: &[(String, Scalar, Scalar)],
    ) -> Vec<(String, InvariantOutcome)> {
        self.for_model(model)
            .into_iter()
            .map(|inv| {
                let outcome = match values.iter().find(|(n, _, _)| n == &inv.name) {
                    Some((_, initial, final_value)) => inv.check(model, *initial, *final_value),
                    None => InvariantOutcome::Invalid {
                        detail: format!("no values supplied for invariant '{}'", inv.name),
                    },
                };
                (inv.name.clone(), outcome)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mass_invariant() -> Invariant {
        Invariant::register(
            "total_mass",
            InvariantKind::Mass,
            "kg",
            100.0,
            1e-9,
            vec![],
            vec!["compressible_flow".to_string()],
        )
        .unwrap()
    }

    #[test]
    fn held_when_conserved_within_tolerance() {
        let inv = mass_invariant();
        // tiny drift of 1e-8 kg on scale 100 -> 1e-10 relative.
        let out = inv.check("compressible_flow", 100.0, 100.0 + 1e-8);
        assert!(out.passed(), "outcome {out:?}");
    }

    #[test]
    fn violated_when_drift_exceeds_tolerance() {
        let inv = mass_invariant();
        // drift of 1e-3 on scale 100 -> 1e-5 relative, above 1e-9.
        let out = inv.check("compressible_flow", 100.0, 100.001);
        assert!(matches!(out, InvariantOutcome::Violated { .. }));
    }

    #[test]
    fn not_applicable_for_unregistered_model() {
        let inv = mass_invariant();
        let out = inv.check("gravity_only", 100.0, 100.0);
        match out {
            InvariantOutcome::NotApplicable { model } => assert_eq!(model, "gravity_only"),
            other => panic!("expected NotApplicable, got {other:?}"),
        }
    }

    #[test]
    fn declared_source_offsets_a_legitimate_flux() {
        let inv = Invariant::register(
            "total_energy",
            InvariantKind::Energy,
            "J",
            1000.0,
            1e-9,
            vec![SourceSink::new("inflow", 0.5)],
            vec!["open_channel".to_string()],
        )
        .unwrap();
        // Energy rises by exactly the declared inflow: not a violation.
        let out = inv.check("open_channel", 1000.0, 1000.5);
        assert!(out.passed(), "outcome {out:?}");
        // A rise larger than the declared inflow is a violation.
        let bad = inv.check("open_channel", 1000.0, 1001.0);
        assert!(matches!(bad, InvariantOutcome::Violated { .. }));
    }

    #[test]
    fn registration_rejects_empty_applicability() {
        let err = Invariant::register(
            "charge",
            InvariantKind::Charge,
            "C",
            1.0,
            1e-9,
            vec![],
            vec![],
        );
        assert!(err.is_err());
    }

    #[test]
    fn registry_rejects_duplicate_names() {
        let mut reg = InvariantRegistry::new();
        reg.add(mass_invariant()).unwrap();
        assert!(reg.add(mass_invariant()).is_err());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn registry_check_unknown_name_is_not_applicable() {
        let reg = InvariantRegistry::new();
        let out = reg.check("missing", "any_model", 1.0, 1.0);
        assert!(matches!(out, InvariantOutcome::NotApplicable { .. }));
        assert!(!out.passed());
    }

    #[test]
    fn registry_check_model_reports_per_invariant() {
        let mut reg = InvariantRegistry::new();
        reg.add(mass_invariant()).unwrap();
        reg.add(
            Invariant::register(
                "total_energy",
                InvariantKind::Energy,
                "J",
                50.0,
                1e-6,
                vec![],
                vec!["compressible_flow".to_string()],
            )
            .unwrap(),
        )
        .unwrap();

        let values = vec![
            ("total_mass".to_string(), 100.0, 100.0),
            ("total_energy".to_string(), 50.0, 60.0), // violates
        ];
        let results = reg.check_model("compressible_flow", &values);
        assert_eq!(results.len(), 2);
        assert!(
            results
                .iter()
                .find(|(n, _)| n == "total_mass")
                .unwrap()
                .1
                .passed()
        );
        assert!(matches!(
            results.iter().find(|(n, _)| n == "total_energy").unwrap().1,
            InvariantOutcome::Violated { .. }
        ));
    }

    #[test]
    fn missing_values_yield_invalid_not_pass() {
        let mut reg = InvariantRegistry::new();
        reg.add(mass_invariant()).unwrap();
        let results = reg.check_model("compressible_flow", &[]);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].1, InvariantOutcome::Invalid { .. }));
        assert!(!results[0].1.passed());
    }
}
