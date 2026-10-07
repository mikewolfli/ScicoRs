// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Machine-readable validation results and a human-readable summary (Phase 40).
//!
//! # Never count an un-executed item as passed
//!
//! The central guarantee of this module is that the four outcome states —
//! passed, failed, skipped and not-applicable — are tracked separately. A
//! skipped or not-applicable item never increments the passed count, so a
//! validation summary cannot be made to look green by omitting work.
//!
//! # Error budget
//!
//! A [`ToleranceBudget`] records how the allowed total error is apportioned
//! across contributing sources (discretisation, iteration, interpolation,
//! floating-point). It exists so that a tolerance can be *derived* rather than
//! guessed, and so that a change to one component's budget is visible in
//! review.

use crate::core::types::Scalar;

use super::benchmark::ValidationError;

/// The outcome of a single validation item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CheckStatus {
    /// The item executed and met its criterion.
    Passed,
    /// The item executed and failed its criterion.
    Failed,
    /// The item was intentionally not executed (e.g. feature disabled, dataset
    /// absent). Counts separately from passed.
    Skipped,
    /// The item does not apply to the current model/configuration. Also counted
    /// separately from passed.
    NotApplicable,
}

impl CheckStatus {
    /// Short machine-readable label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::NotApplicable => "not_applicable",
        }
    }

    /// Whether this status contributes to the passed count.
    pub fn counts_as_passed(self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// One line of a validation report.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CheckRecord {
    /// Identifier of the benchmark/check.
    pub id: String,
    /// Category label (e.g. the benchmark category's `as_str`).
    pub category: String,
    /// Outcome.
    pub status: CheckStatus,
    /// Observed error, when the item produced one.
    pub observed_error: Option<Scalar>,
    /// Applied tolerance, when the item has one.
    pub tolerance: Option<Scalar>,
    /// Why the item was skipped or not applicable, or a pass/fail detail.
    pub detail: String,
}

impl CheckRecord {
    /// Construct a passed record.
    pub fn passed(
        id: impl Into<String>,
        category: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category: category.into(),
            status: CheckStatus::Passed,
            observed_error: None,
            tolerance: None,
            detail: detail.into(),
        }
    }

    /// Construct a failed record.
    pub fn failed(
        id: impl Into<String>,
        category: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category: category.into(),
            status: CheckStatus::Failed,
            observed_error: None,
            tolerance: None,
            detail: detail.into(),
        }
    }

    /// Construct a skipped record with a reason.
    pub fn skipped(
        id: impl Into<String>,
        category: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category: category.into(),
            status: CheckStatus::Skipped,
            observed_error: None,
            tolerance: None,
            detail: reason.into(),
        }
    }

    /// Construct a not-applicable record with a reason.
    pub fn not_applicable(
        id: impl Into<String>,
        category: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            category: category.into(),
            status: CheckStatus::NotApplicable,
            observed_error: None,
            tolerance: None,
            detail: reason.into(),
        }
    }

    /// Attach an observed error and tolerance for reporting.
    pub fn with_errors(mut self, observed: Scalar, tolerance: Scalar) -> Self {
        self.observed_error = Some(observed);
        self.tolerance = Some(tolerance);
        self
    }
}

/// Counts of the four outcome states.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CheckCounts {
    /// Number of passed items.
    pub passed: usize,
    /// Number of failed items.
    pub failed: usize,
    /// Number of skipped items.
    pub skipped: usize,
    /// Number of not-applicable items.
    pub not_applicable: usize,
}

impl CheckCounts {
    /// Total number of records of all statuses.
    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped + self.not_applicable
    }

    /// Whether there were no failures. Skipped/not-applicable items do not by
    /// themselves make a run "not ok", but they are always visible in the
    /// counts so they cannot masquerade as passed.
    pub fn is_ok(&self) -> bool {
        self.failed == 0
    }
}

/// An apportionment of the total allowed error across its sources.
///
/// Each component carries a name and a relative weight; the budget's `total`
/// is the overall allowed error. The individual budgets are `weight * total`,
/// so the parts always sum back to the whole (the weights are validated to sum
/// to 1 within a small tolerance).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToleranceBudget {
    /// Overall allowed error.
    pub total: Scalar,
    /// Named components with their relative weights.
    pub components: Vec<(String, Scalar)>,
}

impl ToleranceBudget {
    /// Build an error budget, requiring the component weights to sum to 1.
    pub fn new(total: Scalar, components: Vec<(String, Scalar)>) -> Result<Self, ValidationError> {
        if !total.is_finite() || total < 0.0 {
            return Err(ValidationError::InvalidTolerance { value: total });
        }
        if components.is_empty() {
            return Err(ValidationError::ConvergenceData {
                detail: "tolerance budget needs at least one component".to_string(),
            });
        }
        let sum: Scalar = components.iter().map(|(_, w)| w).sum();
        if (sum - 1.0).abs() > 1e-9 {
            return Err(ValidationError::ConvergenceData {
                detail: format!("budget component weights must sum to 1, got {sum}"),
            });
        }
        for (_, w) in &components {
            if !w.is_finite() || *w < 0.0 {
                return Err(ValidationError::InvalidTolerance { value: *w });
            }
        }
        Ok(Self { total, components })
    }

    /// The budgeted error for a named component, or `None` if absent.
    pub fn component_budget(&self, name: &str) -> Option<Scalar> {
        self.components
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, w)| w * self.total)
    }

    /// Sum of the individual component budgets (equals `total` for a valid
    /// budget).
    pub fn sum_of_components(&self) -> Scalar {
        self.components.iter().map(|(_, w)| w * self.total).sum()
    }
}

/// A complete, machine-readable validation report.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValidationReport {
    /// Human-readable title of the run.
    pub title: String,
    /// Software version the report was produced for.
    pub version: String,
    /// The records, in execution order.
    pub records: Vec<CheckRecord>,
    /// The optional error budget underpinning the run's tolerances.
    pub budget: Option<ToleranceBudget>,
}

impl ValidationReport {
    /// Start an empty report.
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            version: version.into(),
            records: Vec::new(),
            budget: None,
        }
    }

    /// Attach an error budget.
    pub fn with_budget(mut self, budget: ToleranceBudget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Append a record.
    pub fn push(&mut self, record: CheckRecord) {
        self.records.push(record);
    }

    /// Tally the records by status.
    pub fn counts(&self) -> CheckCounts {
        let mut counts = CheckCounts::default();
        for record in &self.records {
            match record.status {
                CheckStatus::Passed => counts.passed += 1,
                CheckStatus::Failed => counts.failed += 1,
                CheckStatus::Skipped => counts.skipped += 1,
                CheckStatus::NotApplicable => counts.not_applicable += 1,
            }
        }
        counts
    }

    /// Serialize to compact JSON.
    pub fn to_json(&self) -> Result<String, ValidationError> {
        serde_json::to_string(self).map_err(|e| ValidationError::ConvergenceData {
            detail: format!("failed to serialize report: {e}"),
        })
    }

    /// A human-readable multi-line summary.
    ///
    /// The summary always prints all four counts, so a reader cannot mistake a
    /// run with skipped items for a fully-passing one.
    pub fn summary(&self) -> String {
        let c = self.counts();
        let mut out = String::new();
        out.push_str(&format!("Validation report: {}\n", self.title));
        out.push_str(&format!("  version: {}\n", self.version));
        out.push_str(&format!(
            "  passed: {} / failed: {} / skipped: {} / not_applicable: {} (total {})\n",
            c.passed,
            c.failed,
            c.skipped,
            c.not_applicable,
            c.total()
        ));
        if let Some(budget) = &self.budget {
            out.push_str(&format!(
                "  error budget: total {:.3e} over {} components\n",
                budget.total,
                budget.components.len()
            ));
            for (name, weight) in &budget.components {
                out.push_str(&format!(
                    "    - {name}: {:.3e} (weight {:.2})\n",
                    weight * budget.total,
                    weight
                ));
            }
        }
        out.push_str(&format!(
            "  overall: {}\n",
            if c.is_ok() { "OK" } else { "FAILED" }
        ));
        for record in &self.records {
            if record.status == CheckStatus::Failed {
                out.push_str(&format!(
                    "    FAIL {} [{}]: {}\n",
                    record.id, record.category, record.detail
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_track_each_status_separately() {
        let mut report = ValidationReport::new("run", "1.0.0");
        report.push(CheckRecord::passed("a", "unit", "ok"));
        report.push(CheckRecord::failed("b", "physics", "off"));
        report.push(CheckRecord::skipped("c", "physics", "no dataset"));
        report.push(CheckRecord::not_applicable("d", "physics", "wrong model"));

        let c = report.counts();
        assert_eq!(c.passed, 1);
        assert_eq!(c.failed, 1);
        assert_eq!(c.skipped, 1);
        assert_eq!(c.not_applicable, 1);
        assert_eq!(c.total(), 4);
        assert!(!c.is_ok());
    }

    #[test]
    fn skipped_is_not_counted_as_passed() {
        let mut report = ValidationReport::new("only-skipped", "1.0.0");
        report.push(CheckRecord::skipped("x", "unit", "skipped"));
        report.push(CheckRecord::skipped("y", "unit", "skipped"));
        let c = report.counts();
        assert_eq!(c.passed, 0);
        assert_eq!(c.skipped, 2);
        assert!(c.is_ok(), "skips alone do not fail a run");
    }

    #[test]
    fn not_applicable_is_not_counted_as_passed() {
        let mut report = ValidationReport::new("na", "1.0.0");
        report.push(CheckRecord::not_applicable("z", "physics", "n/a"));
        let c = report.counts();
        assert_eq!(c.passed, 0);
        assert_eq!(c.not_applicable, 1);
    }

    #[test]
    fn summary_prints_all_four_counts() {
        let mut report = ValidationReport::new("t", "9.9.9");
        report.push(CheckRecord::passed("a", "unit", "ok"));
        report.push(CheckRecord::skipped("b", "unit", "skip"));
        let text = report.summary();
        assert!(text.contains("passed: 1"));
        assert!(text.contains("skipped: 1"));
        assert!(text.contains("not_applicable: 0"));
        assert!(text.contains("failed: 0"));
    }

    #[test]
    fn json_round_trips() {
        let mut report = ValidationReport::new("t", "1.0.0");
        report.push(CheckRecord::passed("a", "unit", "ok").with_errors(1e-3, 1e-2));
        let json = report.to_json().unwrap();
        let back: ValidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, back);
    }

    #[test]
    fn budget_components_must_sum_to_one() {
        let bad = ToleranceBudget::new(
            1e-3,
            vec![
                ("discretisation".to_string(), 0.5),
                ("iteration".to_string(), 0.2),
            ],
        );
        assert!(bad.is_err());

        let good = ToleranceBudget::new(
            1e-3,
            vec![
                ("discretisation".to_string(), 0.6),
                ("iteration".to_string(), 0.3),
                ("floating_point".to_string(), 0.1),
            ],
        )
        .unwrap();
        assert!((good.component_budget("discretisation").unwrap() - 0.6e-3).abs() < 1e-15);
        assert!((good.sum_of_components() - 1e-3).abs() < 1e-15);
        assert!(good.component_budget("missing").is_none());
    }

    #[test]
    fn budget_rejects_negative_total() {
        let err = ToleranceBudget::new(-1.0, vec![("x".to_string(), 1.0)]);
        assert!(err.is_err());
    }

    #[test]
    fn failed_record_reported_in_summary() {
        let mut report = ValidationReport::new("t", "1.0.0");
        report.push(CheckRecord::failed("bad", "physics", "off by a lot"));
        let text = report.summary();
        assert!(text.contains("FAIL bad"));
        assert!(text.contains("overall: FAILED"));
    }
}
