//! Block type factory registry.
//!
//! Deserialization ([`crate::core::diagram_ser::json_to_diagram`]) preserves a
//! block's *type name* but cannot reconstruct its runtime behaviour on its own,
//! because the concrete `Block` implementations live in code, not in the file.
//! This module supplies the missing half: a registry mapping a block type name
//! to a constructor, so a saved diagram can be rebuilt into a simulatable one.
//!
//! [`register_builtin_blocks`] covers every block type defined in
//! [`crate::blocks`]. Constructor arguments that are not recoverable from the
//! file use the documented defaults below; every parameter that *is* stored in
//! the diagram is re-applied afterwards by [`BlockFactory::reconstruct`].
//!
//! ```
//! use scico_rs::blocks::{BlockFactory, register_builtin_blocks};
//! use scico_rs::core::diagram_ser::{diagram_to_json, json_to_diagram};
//!
//! // Register the built-in block library once.
//! let mut factory = BlockFactory::new();
//! register_builtin_blocks(&mut factory);
//!
//! // Something that is not registered reports the missing type by name.
//! let mut other = BlockFactory::new();
//! assert!(other.create("Gain", "g1").is_err());
//! assert_eq!(factory.create("Gain", "g1").unwrap().block_type(), "Gain");
//! # let _ = (diagram_to_json, json_to_diagram);
//! ```

use crate::core::block::{Block, SimpleBlock};
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::types::{Scalar, SignalValue};
use std::collections::HashMap;

/// A constructor for a concrete block type, keyed later by an instance id.
pub type BlockConstructor = fn(&str) -> Box<dyn Block>;

/// Default capacity used when reconstructing a `Scope` whose capacity was not
/// serialized.
const DEFAULT_SCOPE_CAPACITY: usize = 1024;
/// Default record limit for a reconstructed `DataRecorder`.
const DEFAULT_RECORDER_CAPACITY: usize = 10_000;
/// Default display prefix for a reconstructed `NumericDisplay`.
const DEFAULT_DISPLAY_PREFIX: &str = "";
/// Default point budget for a reconstructed `ChartBuffer`.
const DEFAULT_CHART_POINTS: usize = 10_000;
/// Default sample period (seconds) for reconstructed discrete blocks. The
/// engine overwrites this each step through `set_step`.
const DEFAULT_SAMPLE_PERIOD: Scalar = 0.01;
/// Default state-space matrix dimension for a reconstructed `StateSpaceSystem`.
const DEFAULT_STATE_DIM: usize = 1;

/// Registry of block type name to constructor.
#[derive(Clone, Default)]
pub struct BlockFactory {
    constructors: HashMap<String, BlockConstructor>,
}

impl std::fmt::Debug for BlockFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockFactory")
            .field("registered_types", &self.registered_types())
            .finish()
    }
}

impl BlockFactory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a constructor under `type_name`, replacing any prior entry.
    pub fn register(&mut self, type_name: &str, ctor: BlockConstructor) {
        self.constructors.insert(type_name.to_string(), ctor);
    }

    /// Whether a constructor is registered for `type_name`.
    pub fn contains(&self, type_name: &str) -> bool {
        self.constructors.contains_key(type_name)
    }

    /// All registered type names, sorted for deterministic output.
    pub fn registered_types(&self) -> Vec<&String> {
        let mut names: Vec<&String> = self.constructors.keys().collect();
        names.sort();
        names
    }

    /// Construct a block of `type_name` with the given instance id.
    pub fn create(&self, type_name: &str, id: &str) -> Result<Box<dyn Block>, SimError> {
        let ctor = self.constructors.get(type_name).ok_or_else(|| {
            SimError::parse_error(format!(
                "unknown block type '{}'; register it with BlockFactory::register",
                type_name
            ))
        })?;
        Ok(ctor(id))
    }

    /// Replace every block in `diagram` with a freshly constructed instance of
    /// its declared type, re-applying the parameters that the concrete block
    /// also declares.
    ///
    /// Unknown block types are reported as a single error listing them, so a
    /// half-reconstructed diagram is never silently returned. If any
    /// construction fails the diagram is left untouched. Returns the number of
    /// blocks reconstructed.
    pub fn reconstruct(&self, diagram: &mut Diagram) -> Result<usize, SimError> {
        let ids: Vec<String> = diagram.blocks().map(|(id, _)| id.clone()).collect();

        let mut unknown: Vec<String> = Vec::new();
        for id in &ids {
            if let Some(block) = diagram.get_block(id)
                && !self.contains(block.block_type())
            {
                unknown.push(format!("{} ({})", block.block_type(), id));
            }
        }
        if !unknown.is_empty() {
            unknown.sort();
            unknown.dedup();
            return Err(SimError::parse_error(format!(
                "no constructor registered for block type(s): {}",
                unknown.join(", ")
            )));
        }

        // Build every replacement first so a failure cannot half-migrate the
        // diagram.
        let mut replacements: Vec<(String, Box<dyn Block>)> = Vec::with_capacity(ids.len());
        for id in &ids {
            let Some(old) = diagram.get_block(id) else {
                continue;
            };
            let type_name = old.block_type().to_string();
            // Snapshot the serialized parameters before constructing.
            let params: Vec<crate::core::param::Parameter> = old
                .params()
                .param_keys()
                .filter_map(|k| old.params().get(k).cloned())
                .collect();

            let mut fresh = self.create(&type_name, id)?;
            for p in params {
                // Only overwrite a parameter the concrete block actually
                // declares, so a stale file cannot inject arbitrary parameters
                // into a runtime block.
                if fresh.params().get(&p.name).is_some() {
                    fresh.params_mut().add(p);
                }
            }
            replacements.push((id.clone(), fresh));
        }

        for (id, block) in replacements {
            diagram.replace_block(&id, block)?;
        }
        Ok(ids.len())
    }
}

/// Build a registry pre-loaded with every block type defined in this crate.
pub fn register_builtin_blocks(factory: &mut BlockFactory) {
    use crate::blocks::{continuous, discrete_ctrl, logic, math, sinks, sources};

    // ── Sources ──────────────────────────────────────────────────────────
    // ── Sources ───────────────────────────────────────
    factory.register("ConstantSource", |id| {
        Box::new(sources::ConstantSource::new(id, SignalValue::Scalar(0.0)))
    });
    factory.register("SineSource", |id| {
        Box::new(sources::SineSource::new(id, 1.0, 1.0, 0.0, 0.0))
    });
    factory.register("SquareSource", |id| {
        Box::new(sources::SquareSource::new(id, 1.0, 1.0, 0.5, 0.0))
    });
    factory.register("StepSource", |id| {
        Box::new(sources::StepSource::new(id, 0.0, 1.0, 0.0))
    });
    factory.register("PulseSource", |id| {
        Box::new(sources::PulseSource::new(id, 1.0, 0.1, Some(0.5), 0.0))
    });
    factory.register("NoiseSource", |id| {
        Box::new(sources::NoiseSource::new(
            id,
            0.0,
            1.0,
            sources::NoiseType::Gaussian,
            None,
        ))
    });

    // ── Sinks ────────────────────────────────────────────────────────────
    factory.register("Scope", |id| {
        Box::new(sinks::Scope::new(id, DEFAULT_SCOPE_CAPACITY))
    });
    factory.register("DataRecorder", |id| {
        Box::new(sinks::DataRecorder::new(
            id,
            Some(DEFAULT_RECORDER_CAPACITY),
        ))
    });
    factory.register("NumericDisplay", |id| {
        Box::new(sinks::NumericDisplay::new(id, DEFAULT_DISPLAY_PREFIX))
    });
    factory.register("ChartBuffer", |id| {
        Box::new(sinks::ChartBuffer::new(id, DEFAULT_CHART_POINTS))
    });

    // ── Math ─────────────────────────────────────────────────────────────
    factory.register("Adder", |id| Box::new(math::Adder::new(id, 1.0, 1.0, 0.0)));
    factory.register("Subtractor", |id| Box::new(math::Subtractor::new(id)));
    factory.register("Multiplier", |id| Box::new(math::Multiplier::new(id)));
    factory.register("Divider", |id| Box::new(math::Divider::new(id)));
    factory.register("Gain", |id| Box::new(math::Gain::new(id, 1.0)));
    // `TrigFunction` derives its type name from the operation
    // (`Trig_Sin`, `Trig_Cos`, ...), so each op is registered under the name
    // the block actually reports.
    factory.register("Trig_Sin", |id| {
        Box::new(math::TrigFunction::new(id, math::TrigOp::Sin))
    });
    factory.register("Trig_Cos", |id| {
        Box::new(math::TrigFunction::new(id, math::TrigOp::Cos))
    });
    factory.register("Trig_Tan", |id| {
        Box::new(math::TrigFunction::new(id, math::TrigOp::Tan))
    });
    factory.register("Trig_Asin", |id| {
        Box::new(math::TrigFunction::new(id, math::TrigOp::Asin))
    });
    factory.register("Trig_Acos", |id| {
        Box::new(math::TrigFunction::new(id, math::TrigOp::Acos))
    });
    factory.register("Trig_Atan", |id| {
        Box::new(math::TrigFunction::new(id, math::TrigOp::Atan))
    });
    factory.register("Trig_Exp", |id| {
        Box::new(math::TrigFunction::new(id, math::TrigOp::Exp))
    });
    factory.register("Trig_Log", |id| {
        Box::new(math::TrigFunction::new(id, math::TrigOp::Log))
    });
    factory.register("MatrixMultiply", |id| {
        Box::new(math::MatrixMultiply::new(id, [[1.0, 0.0], [0.0, 1.0]]))
    });

    // ── Logic ────────────────────────────────────────────────────────────
    factory.register("LogicAnd", |id| Box::new(logic::LogicAnd::new(id)));
    factory.register("LogicOr", |id| Box::new(logic::LogicOr::new(id)));
    factory.register("LogicNot", |id| Box::new(logic::LogicNot::new(id)));
    factory.register("LogicXor", |id| Box::new(logic::LogicXor::new(id)));
    factory.register("Comparator", |id| Box::new(logic::Comparator::new(id)));
    factory.register("Multiplexer", |id| Box::new(logic::Multiplexer::new(id)));
    factory.register("Saturation", |id| {
        Box::new(logic::Saturation::new(id, -1.0, 1.0))
    });
    factory.register("Switch", |id| Box::new(logic::Switch::new(id, 0.5)));

    // ── Continuous ───────────────────────────────────────────────────────
    factory.register("Integrator", |id| {
        Box::new(continuous::Integrator::new(id, 0.0))
    });
    factory.register("PIDController", |id| {
        Box::new(continuous::PIDController::new(id, 1.0, 0.0, 0.0))
    });
    factory.register("TransferFunction", |id| {
        Box::new(continuous::TransferFunction::new(
            id,
            vec![1.0],
            vec![1.0, 1.0],
        ))
    });
    factory.register("StateSpaceSystem", |id| {
        Box::new(continuous::StateSpaceSystem::new(
            id,
            vec![vec![0.0; DEFAULT_STATE_DIM]; DEFAULT_STATE_DIM],
            vec![0.0; DEFAULT_STATE_DIM],
            vec![0.0; DEFAULT_STATE_DIM],
            0.0,
        ))
    });

    // ── Discrete / control ───────────────────────────────────────────────
    factory.register("UnitDelay", |id| {
        Box::new(discrete_ctrl::UnitDelay::new(id))
    });
    factory.register("DiscreteIntegrator", |id| {
        Box::new(discrete_ctrl::DiscreteIntegratorBlock::new(
            id,
            DEFAULT_SAMPLE_PERIOD,
            0.0,
        ))
    });
    factory.register("FIRFilter", |id| {
        Box::new(discrete_ctrl::DiscreteFilter::new_fir(id, &[1.0]))
    });
    factory.register("IIRFilter", |id| {
        Box::new(discrete_ctrl::DiscreteFilter::new_iir(id, &[1.0], &[1.0]))
    });
    factory.register("DiscretePID", |id| {
        Box::new(discrete_ctrl::DiscretePID::new(
            id,
            1.0,
            0.0,
            0.0,
            DEFAULT_SAMPLE_PERIOD,
        ))
    });
}

/// Construct a plain [`SimpleBlock`].
///
/// Useful as a registration target for types that only need to exist
/// structurally (for example in a test fixture).
pub fn simple_block(id: &str, block_type: &str) -> Box<dyn Block> {
    Box::new(SimpleBlock::new(id, block_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diagram::Diagram;

    fn builtin() -> BlockFactory {
        let mut f = BlockFactory::new();
        register_builtin_blocks(&mut f);
        f
    }

    #[test]
    fn test_all_builtin_types_are_constructible() {
        let factory = builtin();
        let types = factory.registered_types();
        assert!(
            types.len() >= 25,
            "expected the full block library to be registered, got {} types",
            types.len()
        );
        for name in types {
            let block = factory
                .create(name, "instance")
                .unwrap_or_else(|e| panic!("constructor for '{name}' must work: {e}"));
            // A constructed block must report the type it was registered under,
            // otherwise the registry and the implementation disagree.
            assert_eq!(
                block.block_type(),
                name.as_str(),
                "registered as '{name}' but constructed block reports '{}'",
                block.block_type()
            );
            assert_eq!(block.id(), "instance");
        }
    }

    #[test]
    fn test_create_rejects_unknown_type() {
        let factory = builtin();
        let err = match factory.create("Nope", "x") {
            Ok(_) => panic!("creating an unregistered type must fail"),
            Err(e) => format!("{e}"),
        };
        assert!(err.contains("unknown block type 'Nope'"), "got: {err}");
    }

    #[test]
    fn test_registered_types_are_sorted_and_deduplicated() {
        let factory = builtin();
        let types = factory.registered_types();
        let mut sorted = types.clone();
        sorted.sort();
        assert_eq!(types, sorted, "types must be returned sorted");
        let unique: std::collections::HashSet<_> = types.iter().collect();
        assert_eq!(unique.len(), types.len());
    }

    #[test]
    fn test_reconstruct_replaces_placeholder_blocks() {
        let mut d = Diagram::new("d");
        d.add_block(simple_block("g1", "Gain"));
        d.add_block(simple_block("i1", "Integrator"));
        let factory = builtin();
        let n = factory.reconstruct(&mut d).unwrap();
        assert_eq!(n, 2);
        assert_eq!(d.get_block("g1").unwrap().block_type(), "Gain");
        assert_eq!(d.get_block("i1").unwrap().block_type(), "Integrator");
        // The reconstructed Integrator must expose its real port surface, which
        // the placeholder did not have.
        assert!(
            d.get_block("i1").unwrap().ports().get("u").is_some(),
            "the real block's ports must be present after reconstruct"
        );
    }

    /// A failed reconstruction must not leave the diagram half-migrated.
    #[test]
    fn test_reconstruct_is_all_or_nothing() {
        let mut d = Diagram::new("d");
        d.add_block(simple_block("g1", "Gain"));
        d.add_block(simple_block("bad", "NoSuchType"));
        let factory = builtin();
        assert!(factory.reconstruct(&mut d).is_err());
        // The first block must still be the placeholder: nothing was swapped.
        assert_eq!(d.get_block("g1").unwrap().block_type(), "Gain");
        assert!(
            d.get_block("g1").unwrap().ports().is_empty(),
            "the diagram must be untouched after a failed reconstruct"
        );
    }

    #[test]
    fn test_reconstruct_reports_every_unknown_type() {
        let mut d = Diagram::new("d");
        d.add_block(simple_block("a", "Alpha"));
        d.add_block(simple_block("b", "Beta"));
        let factory = builtin();
        let err = format!("{}", factory.reconstruct(&mut d).unwrap_err());
        assert!(err.contains("Alpha"), "got: {err}");
        assert!(err.contains("Beta"), "got: {err}");
    }

    #[test]
    fn test_replace_block_rejects_id_mismatch() {
        let mut d = Diagram::new("d");
        d.add_block(simple_block("a", "A"));
        let err = d.replace_block("a", simple_block("b", "B")).unwrap_err();
        assert!(format!("{err}").contains("does not match target id"));
    }

    #[test]
    fn test_replace_block_rejects_missing_target() {
        let mut d = Diagram::new("d");
        let err = d
            .replace_block("ghost", simple_block("ghost", "G"))
            .unwrap_err();
        assert!(format!("{err}").contains("no block with id"));
    }
}
