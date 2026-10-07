// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Numerical validation, convergence studies and a credibility baseline
//! (Phase 40).
//!
//! This top-level module establishes a uniform workflow for answering the
//! question "is this numerical result trustworthy?" — as opposed to "does the
//! code run without erroring". It is deliberately free of solver logic; it
//! consumes the outputs of other modules and produces evidence.
//!
//! # Sub-modules
//!
//! * [`benchmark`] — versioned benchmark problems with sourced reference values
//!   and mandatory, justified tolerances.
//! * [`convergence`] — time-step / mesh refinement studies and observed-order
//!   estimation that refuses to fabricate an order from inadequate data.
//! * [`invariants`] — explicitly registered physical invariants (mass, energy,
//!   charge, probability) with declared source/sink terms and applicability.
//! * [`comparison`] — result-field alignment, error norms and statistical
//!   comparison, reporting alignment error separately from solver error.
//! * [`report`] — machine-readable, serde-serializable validation results with
//!   separate passed/failed/skipped/not-applicable accounting and an error
//!   budget.
//!
//! # Design commitments
//!
//! 1. **No fabricated confidence.** A convergence order, a pass, or a "held"
//!    invariant is only ever reported when the underlying data support it.
//! 2. **Provenance is data.** Reference values carry their source, unit and
//!    precision; tolerances carry their reason.
//! 3. **Nothing un-executed counts as passed.** The report tracks skipped and
//!    not-applicable items separately.

pub mod benchmark;
pub mod comparison;
pub mod convergence;
pub mod invariants;
pub mod report;

pub use benchmark::{
    Benchmark, BenchmarkCategory, BenchmarkOutcome, ErrorNorm, ReferenceSource, ReferenceValue,
    Tolerance, ValidationError,
};
pub use comparison::{
    ComparisonReport, Series, StatisticalComparison, alignment_error, compare_populations,
    compare_series,
};
pub use convergence::{
    ConvergenceConfig, ConvergenceOutcome, ConvergenceStudy, RefinementKind, RefinementLevel,
    analytic_levels, estimate_order, geometric_sizes,
};
pub use invariants::{Invariant, InvariantKind, InvariantOutcome, InvariantRegistry, SourceSink};
pub use report::{CheckCounts, CheckRecord, CheckStatus, ToleranceBudget, ValidationReport};
