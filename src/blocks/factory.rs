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
            // Snapshot the serialized configuration before constructing. Use
            // `configuration()` because a concrete block keeps its settings in
            // typed fields, not necessarily in the parameter bag.
            let saved: Vec<crate::core::param::Parameter> = old.configuration();

            let mut fresh = self.create(&type_name, id)?;
            // Hand the restored parameters to the concrete block through the
            // documented `configuration`/`apply_configuration` channel, so typed
            // fields (not just the parameter bag) are populated.
            let applied = fresh.apply_configuration(&saved);
            if applied != saved.len() {
                let ignored: Vec<&str> = saved
                    .iter()
                    .filter(|p| fresh.params().get(&p.name).is_none())
                    .map(|p| p.name.as_str())
                    .collect();
                log_ignored_parameters(&type_name, id, &ignored);
            }
            replacements.push((id.clone(), fresh));
        }

        for (id, block) in replacements {
            diagram.replace_block(&id, block)?;
        }
        Ok(ids.len())
    }
}

/// Note, on stderr, that a saved parameter was not accepted by the concrete
/// block. Parameters the block does not declare are dropped (a stale file must
/// not inject arbitrary settings), but the drop must be observable rather than
/// silent.
fn log_ignored_parameters(type_name: &str, id: &str, ignored: &[&str]) {
    if !ignored.is_empty() {
        eprintln!(
            "[WARN] BlockFactory: block '{}' ({}) does not accept saved parameter(s): {}",
            id,
            type_name,
            ignored.join(", ")
        );
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

    /// The decisive test: a saved block's *configuration* must survive into the
    /// reconstructed block. Without it, a reloaded diagram silently simulates
    /// with constructor defaults (a saved `k = 5.0` becoming `k = 1.0`).
    ///
    /// This asserts the *observable* behaviour — the gain's actual output — so
    /// it cannot pass on a block that merely carries the right number in a
    /// parameter bag while its computation ignores it.
    #[test]
    fn test_reconstruct_preserves_block_configuration() {
        use crate::core::types::{SignalType, SignalValue};

        let mut d = Diagram::new("cfg");
        // A placeholder carrying the saved configuration, as `json_to_diagram`
        // would build it.
        let mut placeholder = simple_block("g1", "Gain");
        // `simple_block` returns a `Box<dyn Block>`, so declare the ports through
        // the port API rather than the concrete `SimpleBlock` helpers.
        placeholder.ports_mut().add(crate::core::port::Port::new(
            "u",
            crate::core::types::PortDirection::Input,
            SignalType::Continuous,
        ));
        placeholder.ports_mut().add(crate::core::port::Port::new(
            "y",
            crate::core::types::PortDirection::Output,
            SignalType::Continuous,
        ));
        placeholder
            .params_mut()
            .add(crate::core::param::Parameter::new_tunable(
                "k",
                SignalValue::Scalar(5.0),
                "Gain",
            ));
        d.add_block(placeholder);

        let factory = builtin();
        factory.reconstruct(&mut d).unwrap();

        let block = d.get_block("g1").unwrap();
        assert_eq!(block.block_type(), "Gain");

        // Drive the block: u = 3.0, so a gain of 5.0 must produce 15.0. The
        // constructor default (k = 1.0) would produce 3.0.
        let mut d = d;
        let block = d.get_block_mut("g1").unwrap();
        block
            .ports_mut()
            .get_mut("u")
            .unwrap()
            .write(crate::core::signal::Signal::new(
                SignalType::Continuous,
                SignalValue::Scalar(3.0),
                0.0,
            ));
        block.output().unwrap();
        let y = block
            .ports()
            .get("y")
            .and_then(|p| p.read())
            .and_then(|s| s.as_scalar());
        assert_eq!(
            y,
            Some(15.0),
            "the restored gain must be applied in the computation; 3.0 means the \
             saved configuration was silently discarded"
        );
    }

    /// Every registered block type must declare its configuration, otherwise a
    /// saved diagram silently reloads with constructor defaults. This walks the
    /// whole registry and fails on any type that reports nothing to save.
    ///
    /// The only exemptions are blocks whose constructor defaults *are* their
    /// whole configuration (pure pass-through arithmetic such as `Multiplier`,
    /// and sinks that carry no tuning at all); those are listed explicitly so
    /// adding a new configurable block without wiring it up is a test failure.
    #[test]
    fn test_every_registered_block_declares_its_configuration() {
        const NO_SCALAR_CONFIG: [&str; 17] = [
            // Stateless arithmetic with no tunable parameters.
            "Subtractor",
            "Multiplier",
            "LogicAnd",
            "LogicOr",
            "LogicNot",
            "LogicXor",
            "Multiplexer",
            "UnitDelay",
            "Trig_Sin",
            "Trig_Cos",
            "Trig_Tan",
            "Trig_Asin",
            "Trig_Acos",
            "Trig_Atan",
            "Trig_Exp",
            "Trig_Log",
            // Sinks record data rather than being configured.
            "Scope",
        ];

        let factory = builtin();
        let mut missing: Vec<&str> = Vec::new();
        for name in factory.registered_types() {
            let block = factory.create(name, "probe").unwrap();
            if block.configuration().is_empty() {
                // A block with no configuration is fine only if it is a known
                // stateless/sink type, or if it genuinely has nothing beyond
                // constructor defaults.
                let has_fields = !block.params().is_empty();
                if has_fields || !NO_SCALAR_CONFIG.contains(&name.as_str()) {
                    missing.push(name.as_str());
                }
            }
        }
        assert!(
            missing.is_empty(),
            "these block types cannot round-trip their configuration and would \
             silently reload with defaults: {missing:?}"
        );
    }

    /// A scalar-configured block must survive the full save/reconstruct cycle
    /// with its value intact, not just its type name.
    #[test]
    fn test_scalar_configuration_survives_reconstruction_for_several_types() {
        use crate::core::types::SignalValue;

        // (block type, field name, saved value, expected value after reload)
        let cases = [
            ("SineSource", "amplitude", 7.5, 7.5),
            ("Integrator", "initial", 3.25, 3.25),
            ("Saturation", "min", -2.5, -2.5),
            ("Saturation", "max", 4.25, 4.25),
            ("Switch", "threshold", 0.125, 0.125),
            ("Comparator", "hysteresis", 0.75, 0.75),
            ("DiscretePID", "ki", 9.5, 9.5),
            ("Divider", "epsilon", 1e-9, 1e-9),
        ];

        for (block_type, field, saved, expected) in cases {
            let mut d = Diagram::new("cfg");
            let mut placeholder = simple_block("b1", block_type);
            placeholder
                .params_mut()
                .add(crate::core::param::Parameter::new_tunable(
                    field,
                    SignalValue::Scalar(saved),
                    "probe",
                ));
            d.add_block(placeholder);

            let factory = builtin();
            factory
                .reconstruct(&mut d)
                .unwrap_or_else(|e| panic!("{block_type}: reconstruct failed: {e}"));

            let block = d.get_block("b1").unwrap();
            let actual = block
                .configuration()
                .into_iter()
                .find(|p| p.name == field)
                .map(|p| p.value);
            assert_eq!(
                actual,
                Some(SignalValue::Scalar(expected)),
                "{block_type}.{field} must survive reconstruction"
            );
        }
    }

    /// Every registered block's configuration must survive a real JSON save and
    /// reload. The earlier registry test only asserted that `configuration()`
    /// was non-empty, which let a block whose values could not be *encoded*
    /// through the rules (an in-band `NaN` separator became `null`, which the
    /// reader rejects) pass while producing an unloadable file.
    #[test]
    fn test_every_registered_block_survives_a_json_roundtrip() {
        use crate::core::diagram_ser::{diagram_to_json, json_to_diagram};

        let factory = builtin();
        let mut failures: Vec<String> = Vec::new();

        for name in factory.registered_types() {
            let mut d = Diagram::new("probe");
            let block = factory.create(name, "b1").unwrap();
            d.add_block(block);
            // Declare the ports so the loader's link validation has something
            // to check; there are no links, so this is just the surface.
            let json = match diagram_to_json(&d) {
                Ok(j) => j,
                Err(e) => {
                    failures.push(format!("{name}: serialize failed: {e}"));
                    continue;
                }
            };
            match json_to_diagram(&json) {
                Ok(back) => {
                    // And the reconstructed block must still be constructible.
                    let mut back = back;
                    if let Err(e) = factory.reconstruct(&mut back) {
                        failures.push(format!("{name}: reconstruct failed: {e}"));
                    }
                }
                Err(e) => failures.push(format!("{name}: reload failed: {e}")),
            }
        }

        assert!(
            failures.is_empty(),
            "these block types cannot survive a save/reload cycle:\n  {}",
            failures.join("\n  ")
        );
    }

    /// A saved configuration must survive as a *value*, not just as a key.
    #[test]
    fn test_discrete_filter_coefficients_survive_json_roundtrip() {
        use crate::blocks::discrete_ctrl::DiscreteFilter;
        use crate::core::diagram_ser::{diagram_to_json, json_to_diagram};

        // An IIR filter is the case that used to produce an unloadable file.
        let mut d = Diagram::new("iir");
        d.add_block(Box::new(DiscreteFilter::new_iir(
            "f1",
            &[1.0, 0.5],
            &[1.0, -0.25],
        )));

        let json = diagram_to_json(&d).expect("an IIR filter must be serializable");
        assert!(
            !json.contains("null"),
            "no configuration value may encode as null; got:\n{json}"
        );
        let mut back = json_to_diagram(&json).expect("the saved filter must be loadable");
        let mut factory = builtin();
        register_builtin_blocks(&mut factory);
        factory.reconstruct(&mut back).unwrap();

        // Compare impulse responses: same coefficients => same filter behaviour.
        let original = DiscreteFilter::new_iir("f1", &[1.0, 0.5], &[1.0, -0.25]);
        assert_eq!(
            back.get_block("f1").unwrap().configuration().len(),
            original.configuration().len(),
            "the reloaded filter must report the same configuration surface"
        );
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
