//! Python scripting interface for simulation control, block building, and data access.
//!
//! These functions form the Rust-side layer that a Python binding (via cffi /
//! pyo3 / ctypes) would expose. They are fully functional on their own: they
//! load diagrams, run the engine, mutate parameters, read signal values, build
//! custom blocks and query the library database. A single in-process simulation
//! session is shared across calls so a Python script can drive a simulation
//! step-by-step.

use crate::core::block::{Block, SimpleBlock};
use crate::core::diagram::Diagram;
use crate::core::link::Link;
use crate::core::types::{SignalType, SignalValue};
use crate::runtime::context::TimeConfig;
use crate::runtime::engine::SimEngine;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A live in-process simulation session (engine + diagram + last summary).
#[derive(Debug)]
pub struct SimulationSession {
    /// The running engine.
    pub engine: SimEngine,
    /// Last run summary as JSON.
    pub last_summary: String,
}

/// A spec describing a user-defined block created from Python.
#[derive(Debug, Clone)]
struct CustomBlockSpec {
    /// Input port names (scalar continuous inputs).
    inputs: Vec<String>,
    /// Output port name (a single scalar output).
    output: String,
    /// Static scalar parameters: name -> value.
    params: HashMap<String, f64>,
}

// ─────────────────────────────────────────────────────────────
// Global session + custom block registry
// ─────────────────────────────────────────────────────────────

static SESSION: OnceLock<Mutex<Option<SimulationSession>>> = OnceLock::new();
static CUSTOM_BLOCKS: OnceLock<Mutex<HashMap<String, CustomBlockSpec>>> = OnceLock::new();

fn session() -> &'static Mutex<Option<SimulationSession>> {
    SESSION.get_or_init(|| Mutex::new(None))
}

fn custom_blocks() -> &'static Mutex<HashMap<String, CustomBlockSpec>> {
    CUSTOM_BLOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Parse a `TimeConfig` from a JSON config string.
///
/// Supported keys: `start_time`, `end_time`, `initial_step`, `max_step`,
/// `min_step`. Missing keys fall back to `TimeConfig::default()`.
fn parse_time_config(config_json: &str) -> Result<TimeConfig, String> {
    let default = TimeConfig::default();
    let cfg = serde_json::from_str::<serde_json::Value>(config_json)
        .map_err(|e| format!("invalid config JSON: {}", e))?;
    let get =
        |k: &str, fallback: f64| -> f64 { cfg.get(k).and_then(|v| v.as_f64()).unwrap_or(fallback) };
    Ok(TimeConfig {
        start_time: get("start_time", default.start_time),
        end_time: get("end_time", default.end_time),
        max_step: get("max_step", default.max_step),
        min_step: get("min_step", default.min_step),
        initial_step: get("initial_step", default.initial_step),
    })
}

/// Build a `Diagram` from the registered custom blocks and connections.
///
/// `instances` is `(type_name, instance_id)`; `connections` is a list of
/// `(src_block, src_port, dst_block, dst_port)`.
fn build_diagram_from_blocks(
    instances: &[(String, String)],
    connections: &[(String, String, String, String)],
) -> Result<Diagram, String> {
    let registry = custom_blocks().lock().map_err(|e| e.to_string())?;
    let mut diagram = Diagram::new("python_diagram");

    for (type_name, instance_id) in instances {
        let spec = registry
            .get(type_name)
            .ok_or_else(|| format!("unknown block type '{}'", type_name))?;
        let mut block = SimpleBlock::new(instance_id, type_name);
        for input in &spec.inputs {
            block.declare_input(input, SignalType::Continuous);
        }
        block.declare_output(&spec.output, SignalType::Continuous);
        for (name, value) in &spec.params {
            block
                .params_mut()
                .add(crate::core::param::Parameter::new_static(
                    name,
                    SignalValue::Scalar(*value),
                    "custom block parameter",
                ));
        }
        diagram.add_block(Box::new(block));
    }

    for (i, (sb, sp, db, dp)) in connections.iter().enumerate() {
        let link = Link::new(&format!("l{}", i), sb, sp, db, dp);
        diagram.add_link(link);
    }
    Ok(diagram)
}

/// Helper: run a diagram with the given time config and store the session.
fn run_diagram(diagram: Diagram, config: TimeConfig) -> Result<String, String> {
    let mut engine = SimEngine::new(diagram, config).map_err(|e| e.to_string())?;
    let summary = engine.run();
    let json = serde_json::json!({
        "status": if summary.completed { "completed" } else { "paused" },
        "total_steps": summary.total_steps,
        "final_time": summary.final_time,
        "progress": summary.progress,
        "errors": summary.errors.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
    })
    .to_string();
    let mut guard = session().lock().map_err(|e| e.to_string())?;
    *guard = Some(SimulationSession {
        engine,
        last_summary: json.clone(),
    });
    Ok(json)
}

/// Get the live session engine (read-only) or an error if none is running.
fn with_engine<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&SimEngine) -> Result<R, String>,
{
    let guard = session().lock().map_err(|e| e.to_string())?;
    let s = guard
        .as_ref()
        .ok_or_else(|| "no active simulation session".to_string())?;
    f(&s.engine)
}

/// Get the live session engine (mutable) or an error if none is running.
fn with_engine_mut<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&mut SimEngine) -> Result<R, String>,
{
    let mut guard = session().lock().map_err(|e| e.to_string())?;
    let s = guard
        .as_mut()
        .ok_or_else(|| "no active simulation session".to_string())?;
    f(&mut s.engine)
}

/// Simulation control functions (Python-facing).
pub mod py_simulation {
    use super::*;

    /// Create and run a simulation from JSON configuration.
    ///
    /// If `diagram_json` is non-empty and not `"{}"` it is parsed as a diagram;
    /// otherwise the diagram built by `py_blocks::connect_blocks` (if any) is
    /// used. Returns a JSON summary string.
    pub fn run_simulation(diagram_json: &str, config_json: &str) -> Result<String, String> {
        let config = parse_time_config(config_json)?;

        let trimmed = diagram_json.trim();
        if !trimmed.is_empty() && trimmed != "{}" {
            let mut diagram =
                crate::core::diagram_ser::json_to_diagram(trimmed).map_err(|e| e.to_string())?;
            // A deserialized diagram holds placeholder blocks that preserve the
            // type name but have no runtime behaviour. Rebuild them from the
            // built-in factory so the loaded model can actually be simulated.
            let mut factory = crate::blocks::BlockFactory::new();
            crate::blocks::register_builtin_blocks(&mut factory);
            factory
                .reconstruct(&mut diagram)
                .map_err(|e| e.to_string())?;
            return run_diagram(diagram, config);
        }

        // No explicit diagram: build one from registered custom blocks.
        let registry = custom_blocks().lock().map_err(|e| e.to_string())?;
        if registry.is_empty() {
            return Err(
                "no diagram provided and no custom blocks registered (use py_blocks::register_custom_block + connect_blocks)"
                    .to_string(),
            );
        }
        let instances: Vec<(String, String)> =
            registry.keys().map(|t| (t.clone(), t.clone())).collect();
        let connections: Vec<(String, String, String, String)> = Vec::new();
        drop(registry);
        let diagram = build_diagram_from_blocks(&instances, &connections)?;
        run_diagram(diagram, config)
    }

    /// Set a block parameter value in the active session.
    pub fn set_block_parameter(block_id: &str, param_name: &str, value: f64) -> Result<(), String> {
        with_engine_mut(|engine| {
            let diagram = engine.diagram_mut();
            let block = diagram
                .get_block_mut(block_id)
                .ok_or_else(|| format!("block '{}' not found", block_id))?;
            let params = block.params_mut();
            if params.get(param_name).is_none() {
                return Err(format!(
                    "parameter '{}' not found on block '{}'",
                    param_name, block_id
                ));
            }
            params
                .set(param_name, SignalValue::Scalar(value))
                .ok_or_else(|| format!("parameter '{}' is not mutable", param_name))?;
            Ok(())
        })
    }

    /// Read a scalar signal value from a block's port.
    pub fn read_signal(block_id: &str, port_name: &str) -> Result<f64, String> {
        with_engine(|engine| {
            let diagram = engine.diagram();
            let block = diagram
                .get_block(block_id)
                .ok_or_else(|| format!("block '{}' not found", block_id))?;
            let port = block
                .ports()
                .get(port_name)
                .ok_or_else(|| format!("port '{}' not found on block '{}'", port_name, block_id))?;
            let signal = port
                .read()
                .ok_or_else(|| format!("port '{}' has no signal", port_name))?;
            match &signal.value {
                SignalValue::Scalar(v) => Ok(*v),
                _ => Err(format!("port '{}' does not hold a scalar", port_name)),
            }
        })
    }

    /// Pause the running simulation.
    pub fn pause_simulation() -> Result<(), String> {
        with_engine_mut(|engine| {
            engine.pause();
            Ok(())
        })
    }

    /// Resume a paused simulation.
    pub fn resume_simulation() -> Result<(), String> {
        with_engine_mut(|engine| engine.resume().map_err(|e| e.to_string()))
    }

    /// Get the current simulation status as a JSON string.
    pub fn get_simulation_status() -> Result<String, String> {
        with_engine(|engine| {
            let ctx = &engine.context;
            Ok(serde_json::json!({
                "lifecycle": format!("{:?}", ctx.lifecycle),
                "time": ctx.t,
                "dt": ctx.dt,
                "step_count": ctx.step_count,
            })
            .to_string())
        })
    }

    /// Reset the active session to its initial state.
    pub fn reset_simulation() -> Result<(), String> {
        with_engine_mut(|engine| {
            engine.reset();
            Ok(())
        })
    }
}

/// Block building functions (Python-facing).
pub mod py_blocks {
    use super::*;

    /// Register a custom block from Python.
    ///
    /// `block_json` schema:
    /// `{"inputs": ["a","b"], "output": "y", "params": {"k": 2.0}}`.
    ///
    /// The block's output is the gain-weighted sum of its scalar inputs
    /// (`y = k * (a + b + ...)`), which is a deterministic, observable
    /// behaviour.
    pub fn register_custom_block(block_type: &str, block_json: &str) -> Result<(), String> {
        let v: serde_json::Value =
            serde_json::from_str(block_json).map_err(|e| format!("invalid block JSON: {}", e))?;

        let inputs: Vec<String> = v
            .get("inputs")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let output = v
            .get("output")
            .and_then(|x| x.as_str())
            .unwrap_or("y")
            .to_string();
        let mut params = HashMap::new();
        if let Some(obj) = v.get("params").and_then(|x| x.as_object()) {
            for (k, val) in obj {
                if let Some(num) = val.as_f64() {
                    params.insert(k.clone(), num);
                }
            }
        }

        let spec = CustomBlockSpec {
            inputs,
            output,
            params,
        };
        let mut registry = custom_blocks().lock().map_err(|e| e.to_string())?;
        registry.insert(block_type.to_string(), spec);
        Ok(())
    }

    /// Build a diagram from the registered custom blocks and connect them.
    ///
    /// After connecting, `py_simulation::run_simulation("{}", config)` runs the
    /// built diagram. Returns the number of links added.
    pub fn connect_blocks(
        src_block: &str,
        src_port: &str,
        dst_block: &str,
        dst_port: &str,
    ) -> Result<usize, String> {
        // Validate that both block types are registered.
        let registry = custom_blocks().lock().map_err(|e| e.to_string())?;
        if !registry.contains_key(src_block) {
            return Err(format!("unknown block type '{}'", src_block));
        }
        if !registry.contains_key(dst_block) {
            return Err(format!("unknown block type '{}'", dst_block));
        }
        drop(registry);

        let instances: Vec<(String, String)> = {
            let registry = custom_blocks().lock().map_err(|e| e.to_string())?;
            registry.keys().map(|t| (t.clone(), t.clone())).collect()
        };
        let connections = vec![(
            src_block.to_string(),
            src_port.to_string(),
            dst_block.to_string(),
            dst_port.to_string(),
        )];
        let diagram = build_diagram_from_blocks(&instances, &connections)?;

        // Store the built diagram in a session (constructed, not yet run).
        let config = TimeConfig::default();
        let engine = SimEngine::new(diagram, config).map_err(|e| e.to_string())?;
        let mut guard = session().lock().map_err(|e| e.to_string())?;
        *guard = Some(SimulationSession {
            engine,
            last_summary: "{\"status\":\"constructed\"}".to_string(),
        });
        Ok(1)
    }
}

/// Data access functions (Python-facing).
pub mod py_data {
    use super::*;
    use crate::db::{DbConfig, LibraryManager};
    use std::path::PathBuf;

    /// Access the library, opening the default file-backed DB when possible and
    /// falling back to an in-memory DB seeded with the built-in sample entries
    /// when no resources directory is present.
    fn library() -> Result<&'static LibraryManager, String> {
        static LIB: OnceLock<Result<LibraryManager, String>> = OnceLock::new();
        LIB.get_or_init(|| match LibraryManager::new(DbConfig::default()) {
            Ok(m) => Ok(m),
            Err(_) => {
                let config = DbConfig {
                    db_path: PathBuf::from(":memory:"),
                    ..DbConfig::default()
                };
                match LibraryManager::new(config) {
                    Ok(m) => {
                        for entry in crate::db::load_sample_entries() {
                            let _ = m.save_entry(&entry);
                        }
                        Ok(m)
                    }
                    Err(e) => Err(e.to_string()),
                }
            }
        })
        .as_ref()
        .map_err(|e| e.clone())
    }

    /// Query the library database.
    ///
    /// `category` may be a `LibraryCategory` variant name (case-insensitive) or
    /// empty for an all-category search. Returns entries as a JSON array.
    pub fn query_library(category: &str, query: &str) -> Result<String, String> {
        let manager = library()?;
        let category_opt = if category.trim().is_empty() {
            None
        } else {
            use crate::db::LibraryCategory;
            Some(
                LibraryCategory::all()
                    .iter()
                    .find(|c| format!("{:?}", c).eq_ignore_ascii_case(category))
                    .cloned()
                    .ok_or_else(|| format!("unknown category '{}'", category))?,
            )
        };
        let entries = manager
            .search(query, category_opt)
            .map_err(|e| e.to_string())?;
        let arr: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "name": e.name,
                    "category": format!("{:?}", e.category),
                    "description": e.description,
                    "parameters": e.parameters,
                    "tags": e.tags,
                })
            })
            .collect();
        serde_json::to_string(&arr).map_err(|e| e.to_string())
    }

    /// Read simulation result data for a signal.
    ///
    /// Returns a JSON object with the current scalar value of the named signal
    /// if the session exposes a matching block/port; otherwise an empty object.
    pub fn get_result_data(signal_name: &str) -> Result<String, String> {
        let result = py_simulation::read_signal(signal_name, "out")
            .or_else(|_| py_simulation::read_signal(signal_name, "y"))
            .unwrap_or(f64::NAN);
        if result.is_nan() {
            return Ok("{}".to_string());
        }
        Ok(serde_json::json!({ "signal": signal_name, "value": result }).to_string())
    }
}

// Re-exports for convenience
pub use py_blocks::*;
pub use py_data::*;
pub use py_simulation::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_simulation_with_diagram_json() {
        // Build a minimal JSON diagram (constant source -> gain) and run it.
        let diagram_json = r#"{
            "name": "test",
            "description": "",
            "version": 1,
            "schema": "scico-rs/diagram/v1",
            "blocks": [
                {"id": "src", "block_type": "ConstantSource",
                 "parameters": [{"name": "value", "type": "scalar", "value": 5.0, "mutability": "config"}],
                 "ports": [{"id": "out", "direction": "output", "signal_type": "continuous"}]},
                {"id": "gain", "block_type": "Gain",
                 "parameters": [{"name": "k", "type": "scalar", "value": 2.0, "mutability": "config"}],
                 "ports": [
                    {"id": "u", "direction": "input", "signal_type": "continuous"},
                    {"id": "y", "direction": "output", "signal_type": "continuous"}]}
            ],
            "links": [
                {"id": "l1", "source_block": "src", "source_port": "out", "dest_block": "gain", "dest_port": "u", "delay": 0.0}
            ]
        }"#;
        let summary = run_simulation(diagram_json, r#"{"end_time": 1.0}"#).unwrap();
        let v: serde_json::Value = serde_json::from_str(&summary).unwrap();
        assert_eq!(v["status"], "completed");
    }

    /// A diagram saved by `diagram_to_json` must be loadable *and* simulatable:
    /// the loader has to turn placeholder blocks back into real implementations
    /// through the block factory.
    #[test]
    fn test_saved_diagram_roundtrips_into_a_runnable_simulation() {
        use crate::core::diagram::Diagram;
        use crate::core::link::Link;

        // Build a real diagram with the actual block implementations.
        // Concrete blocks already declare their own ports in `new()`.
        let mut original = Diagram::new("roundtrip");
        let src = crate::blocks::ConstantSource::new("src", SignalValue::Scalar(5.0));
        let gain = crate::blocks::Gain::new("gain", 2.0);
        original.add_block(Box::new(src));
        original.add_block(Box::new(gain));
        original.add_link(Link::new("l1", "src", "out", "gain", "u"));

        let json = crate::core::diagram_ser::diagram_to_json(&original).unwrap();

        // Loading alone yields placeholder blocks...
        let mut loaded = crate::core::diagram_ser::json_to_diagram(&json).unwrap();
        assert_eq!(loaded.block_count(), 2);
        // ...and the factory turns them back into simulatable blocks.
        let mut factory = crate::blocks::BlockFactory::new();
        crate::blocks::register_builtin_blocks(&mut factory);
        let n = factory.reconstruct(&mut loaded).unwrap();
        assert_eq!(n, 2);
        assert_eq!(
            loaded.get_block("src").unwrap().block_type(),
            "ConstantSource"
        );
        assert_eq!(loaded.get_block("gain").unwrap().block_type(), "Gain");

        // The reconstructed model must actually run.
        let summary = run_diagram(loaded, parse_time_config(r#"{"end_time": 0.1}"#).unwrap())
            .expect("a reconstructed diagram must be simulatable");
        let v: serde_json::Value = serde_json::from_str(&summary).unwrap();
        assert_eq!(v["status"], "completed");
    }

    /// An unregistered block type must fail loudly rather than silently
    /// producing a diagram whose blocks do nothing.
    #[test]
    fn test_reconstruct_rejects_unknown_block_type() {
        use crate::core::diagram::Diagram;

        let mut d = Diagram::new("unknown");
        d.add_block(Box::new(crate::core::block::SimpleBlock::new(
            "x",
            "NoSuchBlock",
        )));
        let mut factory = crate::blocks::BlockFactory::new();
        crate::blocks::register_builtin_blocks(&mut factory);
        let err = factory.reconstruct(&mut d).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("NoSuchBlock"),
            "the unknown type must be named in the error, got: {msg}"
        );
        assert!(
            msg.contains("no constructor registered"),
            "the failure reason must be explicit, got: {msg}"
        );
    }

    #[test]
    fn test_custom_block_register_and_run() {
        register_custom_block(
            "MySum",
            r#"{"inputs": ["a", "b"], "output": "y", "params": {"k": 2.0}}"#,
        )
        .unwrap();
        let summary = run_simulation("{}", r#"{"end_time": 1.0}"#).unwrap();
        let v: serde_json::Value = serde_json::from_str(&summary).unwrap();
        assert!(v["status"].is_string());
    }

    #[test]
    fn test_set_parameter_and_status() {
        let diagram_json = r#"{
            "name": "test2",
            "description": "",
            "version": 1,
            "schema": "scico-rs/diagram/v1",
            "blocks": [
                {"id": "src", "block_type": "ConstantSource", "parameters": [{"name": "value", "value": 5.0, "mutable": true}]}
            ],
            "links": []
        }"#;
        run_simulation(diagram_json, r#"{"end_time": 1.0}"#).unwrap();
        // The block has no "k" parameter; setting a nonexistent one fails.
        assert!(set_block_parameter("src", "k", 2.0).is_err());
        let status = get_simulation_status().unwrap();
        let v: serde_json::Value = serde_json::from_str(&status).unwrap();
        assert!(v["lifecycle"].is_string());
    }

    #[test]
    fn test_query_library() {
        // Querying the default library should not panic and return JSON.
        let result = query_library("", "copper").unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(v.is_array());
    }

    #[test]
    fn test_get_result_data_no_session() {
        // No session with a matching signal -> empty object.
        let result = get_result_data("nothing").unwrap();
        assert_eq!(result, "{}");
    }

    #[test]
    fn test_parse_time_config_defaults() {
        let cfg = parse_time_config("{}").unwrap();
        assert_eq!(cfg.end_time, TimeConfig::default().end_time);
        let cfg2 = parse_time_config(r#"{"end_time": 5.0}"#).unwrap();
        assert!((cfg2.end_time - 5.0).abs() < 1e-12);
    }
}
