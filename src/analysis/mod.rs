// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Analysis layer: objectives, sensitivity, calibration, uncertainty and
//! optimization (Phase 36).
//!
//! This module turns the existing parameter-sweep / batch execution into an
//! explainable experiment-design and model-calibration workflow. It is
//! deliberately decoupled from the simulation engine through the
//! [`SimulationFunction`] trait, so the same analysis code operates on synthetic
//! analytic problems (for verification) and on real bounded simulation runs
//! (wrapped by [`simulation::DiagramFunction`]).
//!
//! # Data contracts
//!
//! * [`objective::ParameterSpec`] — a stable parameter path, value, bounds,
//!   whether it may be estimated, and its transform.
//! * [`objective::ObservationSet`] — sampled observations with weights and a
//!   missing-value policy.
//! * [`objective::ObjectiveSpec`] — output mapping, loss, weights, regularizer
//!   and infeasible-result policy.
//! * [`calibration::AnalysisResult`] — best parameters, objective, constraint
//!   status, evaluation counts, convergence state and per-run traceable IDs.

pub mod calibration;
pub mod experiment;
pub mod objective;
pub mod optimization;
pub mod sensitivity;
pub mod simulation;
pub mod uncertainty;

pub use calibration::{
    AnalysisResult, CalibrationError, CalibrationProblem, ConstraintStatus, FitDiagnostics,
    ParameterEstimate, calibrate, levenberg_marquardt,
};
pub use experiment::{
    AggregatedResult, FactorDesign, FactorLevel, SamplePlan, aggregate_batches, fraction_successful,
};
pub use objective::{
    InfeasiblePolicy, LossFunction, ObjectiveSpec, ObservationError, ObservationSet, ParameterSpec,
    ParameterTransform,
};
pub use optimization::{
    BoundedOptimizer, NelderMeadOptimizer, OptimizerConfig, OptimizerResult, OptimizerStopReason,
    multi_start, nelder_mead,
};
pub use sensitivity::{
    DifferenceScheme, ParameterSensitivity, SensitivityError, SensitivityResult,
    SensitivitySummary, finite_difference_gradient, local_sensitivity, normalized_sensitivity,
};
pub use simulation::{DiagramFunction, FnSimulationFunction, SimulationFunction, SimulationRecord};
pub use uncertainty::{
    MonteCarloSummary, SamplingScheme, SeededRng, UncertaintyConfig, UncertaintyResult,
    latin_hypercube, monte_carlo,
};
