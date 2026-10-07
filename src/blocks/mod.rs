// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Standard Block Library (Phase 9).
//!
//! Provides built-in simulation blocks for signal generation, math
//! operations, logic, continuous/discrete control, and observation.
//!
//! # Submodules
//!
//! - **sources**: signal generators (const, sine, square, step, pulse, noise)
//! - **math**: arithmetic, trigonometric, matrix operations
//! - **logic**: boolean gates, comparators, multiplexers
//! - **continuous**: integrator, PID, transfer function, state-space
//! - **discrete_ctrl**: unit delay, discrete filter, discrete PID
//! - **sinks**: scope, data recorder, display
//! - **factory**: type-name registry for reconstructing deserialized diagrams

pub mod continuous;
pub mod discrete_ctrl;
pub mod factory;
pub mod logic;
pub mod math;
pub mod sinks;
pub mod sources;

pub use factory::{BlockFactory, register_builtin_blocks};

pub use continuous::{Integrator, PIDController, StateSpaceSystem, TransferFunction};
pub use discrete_ctrl::{DiscreteFilter, DiscreteIntegratorBlock, DiscretePID, UnitDelay};
pub use logic::{
    Comparator, LogicAnd, LogicNot, LogicOr, LogicXor, Multiplexer, Saturation, Switch,
};
pub use math::{Adder, Divider, Gain, MatrixMultiply, Multiplier, Subtractor, TrigFunction};
pub use sinks::{ChartBuffer, DataRecorder, NumericDisplay, Scope};
pub use sources::{
    ConstantSource, NoiseSource, NoiseType, PulseSource, SineSource, SquareSource, StepSource,
};
