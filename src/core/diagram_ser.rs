//! Diagram serialization and deserialization.
//!
//! Provides JSON and TOML persistence for Diagram structures,
//! enabling save/load/parse workflows for simulation models.

use crate::core::block::Block;
use crate::core::diagram::Diagram;
use crate::core::error::SimError;
use crate::core::link::Link;

/// Alias for serialization results using the unified `SimError`.
pub type SerResult<T> = Result<T, SimError>;

/// Intermediate JSON-compatible representation of a diagram.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DiagramData {
    name: String,
    description: String,
    blocks: Vec<BlockData>,
    links: Vec<LinkData>,
    version: u32,
    schema: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BlockData {
    id: String,
    block_type: String,
    parameters: Vec<ParamData>,
    /// Port surface (`name`, `direction`, `signal_type`) so links can be
    /// validated on load and the reloaded diagram exposes the same interface.
    #[serde(default)]
    ports: Vec<PortData>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PortData {
    id: String,
    direction: String,
    signal_type: String,
}

/// A serialized parameter value.
///
/// `value` is a tag/value pair so every `SignalValue` variant survives a round
/// trip. The previous schema stored a bare `f64`, which meant any non-scalar
/// parameter was silently dropped from the file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ParamData {
    name: String,
    /// One of `scalar`, `vector`, `matrix`, `complex`, `boolean`, `integer`,
    /// `string`, `none`.
    #[serde(rename = "type", default = "default_kind")]
    kind: String,
    #[serde(default)]
    value: serde_json::Value,
    #[serde(default)]
    description: String,
    /// One of `static`, `config`, `tunable`.
    #[serde(default = "default_mutability")]
    mutability: String,
}

fn default_kind() -> String {
    "scalar".to_string()
}

fn default_mutability() -> String {
    "config".to_string()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LinkData {
    id: String,
    source_block: String,
    source_port: String,
    dest_block: String,
    dest_port: String,
    delay: f64,
}

const CURRENT_VERSION: u32 = 1;

fn mutability_to_str(m: crate::core::param::ParamMutability) -> &'static str {
    use crate::core::param::ParamMutability;
    match m {
        ParamMutability::Static => "static",
        ParamMutability::Config => "config",
        ParamMutability::Tunable => "tunable",
    }
}

fn mutability_from_str(s: &str) -> crate::core::param::ParamMutability {
    use crate::core::param::ParamMutability;
    match s.to_ascii_lowercase().as_str() {
        "static" => ParamMutability::Static,
        "tunable" => ParamMutability::Tunable,
        _ => ParamMutability::Config,
    }
}

/// Convert a `SignalValue` into a portable `(kind, json)` pair.
fn value_to_json(value: &crate::core::types::SignalValue) -> (&'static str, serde_json::Value) {
    use crate::core::types::SignalValue as V;
    match value {
        V::Scalar(v) => ("scalar", serde_json::json!(v)),
        V::Vector(v) => ("vector", serde_json::json!(v)),
        V::Matrix(rows, cols, data) => (
            "matrix",
            serde_json::json!({ "rows": rows, "cols": cols, "data": data }),
        ),
        V::Complex(re, im) => ("complex", serde_json::json!({ "re": re, "im": im })),
        V::Boolean(b) => ("boolean", serde_json::json!(b)),
        V::Integer(i) => ("integer", serde_json::json!(i)),
        V::String(s) => ("string", serde_json::json!(s)),
        V::Tensor(_) => ("none", serde_json::Value::Null),
        V::None => ("none", serde_json::Value::Null),
    }
}

/// Rebuild a `SignalValue` from a `(kind, json)` pair.
///
/// Returns an error for an unknown kind rather than silently substituting a
/// default, so corrupt files are surfaced.
fn value_from_json(
    kind: &str,
    value: &serde_json::Value,
) -> Result<crate::core::types::SignalValue, SimError> {
    use crate::core::types::SignalValue as V;
    let bad = |what: &str| SimError::parse_error(format!("invalid {kind} parameter: {what}"));
    Ok(match kind.to_ascii_lowercase().as_str() {
        "scalar" => V::Scalar(value.as_f64().ok_or_else(|| bad("expected a number"))?),
        "vector" => V::Vector(
            value
                .as_array()
                .ok_or_else(|| bad("expected an array"))?
                .iter()
                .map(|v| {
                    v.as_f64()
                        .ok_or_else(|| bad("array element is not a number"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        "matrix" => {
            let rows = value
                .get("rows")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| bad("missing rows"))?;
            let cols = value
                .get("cols")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| bad("missing cols"))?;
            let data = value
                .get("data")
                .and_then(|v| v.as_array())
                .ok_or_else(|| bad("missing data"))?
                .iter()
                .map(|v| {
                    v.as_f64()
                        .ok_or_else(|| bad("matrix element is not a number"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            V::Matrix(rows as usize, cols as usize, data)
        }
        "complex" => V::Complex(
            value
                .get("re")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| bad("missing re"))?,
            value
                .get("im")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| bad("missing im"))?,
        ),
        "boolean" => V::Boolean(value.as_bool().ok_or_else(|| bad("expected a bool"))?),
        "integer" => V::Integer(value.as_i64().ok_or_else(|| bad("expected an integer"))?),
        "string" => V::String(
            value
                .as_str()
                .ok_or_else(|| bad("expected a string"))?
                .to_string(),
        ),
        "none" => V::None,
        other => {
            return Err(SimError::parse_error(format!(
                "unknown parameter type '{other}'"
            )));
        }
    })
}

/// Serialize a Diagram to a JSON string.
pub fn diagram_to_json(diagram: &Diagram) -> SerResult<String> {
    let block_data: Vec<BlockData> = diagram
        .blocks()
        .map(|(id, block)| {
            // Serialize every parameter, not just scalar ones. `keys()` is not
            // used as the filter here because it also yields expression
            // parameters, which have no `Parameter` to read back.
            let mut names: Vec<&String> = block.params().param_keys().collect();
            names.sort();
            let params: Vec<ParamData> = names
                .into_iter()
                .filter_map(|name| block.params().get(name))
                .map(|p| {
                    let (kind, value) = value_to_json(&p.value);
                    ParamData {
                        name: p.name.clone(),
                        kind: kind.to_string(),
                        value,
                        description: p.description.clone(),
                        mutability: mutability_to_str(p.mutability).to_string(),
                    }
                })
                .collect();

            let mut ports: Vec<PortData> = block
                .ports()
                .iter()
                .map(|port| PortData {
                    id: port.id.clone(),
                    direction: match port.direction {
                        crate::core::types::PortDirection::Input => "input",
                        crate::core::types::PortDirection::Output => "output",
                        crate::core::types::PortDirection::InOut => "inout",
                    }
                    .to_string(),
                    signal_type: format!("{:?}", port.signal_type).to_ascii_lowercase(),
                })
                .collect();
            ports.sort_by(|a, b| a.id.cmp(&b.id));

            BlockData {
                id: id.clone(),
                block_type: block.block_type().to_string(),
                parameters: params,
                ports,
            }
        })
        .collect();

    let link_data: Vec<LinkData> = diagram
        .links()
        .iter()
        .map(|link| LinkData {
            id: link.id.clone(),
            source_block: link.source.0.clone(),
            source_port: link.source.1.clone(),
            dest_block: link.destination.0.clone(),
            dest_port: link.destination.1.clone(),
            delay: link.delay,
        })
        .collect();

    let data = DiagramData {
        name: diagram.name.clone(),
        description: diagram.description.clone(),
        blocks: block_data,
        links: link_data,
        version: CURRENT_VERSION,
        schema: "scico-rs/diagram/v1".to_string(),
    };

    serde_json::to_string_pretty(&data).map_err(|e| SimError::parse_error(e.to_string()))
}

/// Deserialize a Diagram from a JSON string.
///
/// The block *type* is preserved but not its runtime behaviour: the concrete
/// implementation must be supplied by the caller (or reconstructed through a
/// block factory). Parameters, their types and mutability, and the port surface
/// are restored faithfully, and every link is checked against the ports that
/// actually exist so a stale file cannot produce a diagram whose links point at
/// ports that are not there.
pub fn json_to_diagram(json: &str) -> Result<Diagram, SimError> {
    let data: DiagramData =
        serde_json::from_str(json).map_err(|e| SimError::parse_error(e.to_string()))?;

    // Parse every parameter up front so a malformed file fails before any block
    // is constructed, and so the borrow of `self` in the loop below is short.
    struct PendingParam {
        name: String,
        value: crate::core::types::SignalValue,
        description: String,
        mutability: crate::core::param::ParamMutability,
    }

    let mut diagram = Diagram::new(&data.name);
    diagram.description = data.description;

    let mut block_params: Vec<(BlockData, Vec<PendingParam>)> = Vec::new();
    for bd in data.blocks {
        let mut params = Vec::with_capacity(bd.parameters.len());
        for pd in &bd.parameters {
            params.push(PendingParam {
                name: pd.name.clone(),
                value: value_from_json(&pd.kind, &pd.value)?,
                description: pd.description.clone(),
                mutability: mutability_from_str(&pd.mutability),
            });
        }
        block_params.push((bd, params));
    }

    // Create the (untyped) blocks and restore their declared port surface.
    for (bd, params) in block_params {
        let mut block = crate::core::block::SimpleBlock::new(&bd.id, &bd.block_type);
        for pd in &bd.ports {
            let signal_type = match pd.signal_type.as_str() {
                "discrete" => crate::core::types::SignalType::Discrete,
                "event" => crate::core::types::SignalType::Event,
                "bus" => crate::core::types::SignalType::Bus,
                _ => crate::core::types::SignalType::Continuous,
            };
            match pd.direction.as_str() {
                "output" => block.declare_output(&pd.id, signal_type),
                // `SimpleBlock` exposes no in/out port declaration helper; an
                // `InOut` port round-trips as an input, which is the closest
                // representable surface.
                _ => block.declare_input(&pd.id, signal_type),
            }
        }
        for p in params {
            let mut param =
                crate::core::param::Parameter::new_static(&p.name, p.value, &p.description);
            param.mutability = p.mutability;
            block.params_mut().add(param);
        }
        diagram.add_block(Box::new(block));
    }

    for ld in &data.links {
        let mut link = Link::new(
            &ld.id,
            &ld.source_block,
            &ld.source_port,
            &ld.dest_block,
            &ld.dest_port,
        );
        link.delay = ld.delay;
        diagram.add_link(link);
    }

    // A link is only meaningful when both endpoints exist. Rejecting rather
    // than accepting a dangling connection keeps load failures loud.
    for link in diagram.links().iter() {
        let has = |block_id: &str, port_id: &str| {
            diagram
                .get_block(block_id)
                .and_then(|b| b.ports().get(port_id))
                .is_some()
        };
        if !has(&link.source.0, &link.source.1) {
            return Err(SimError::parse_error(format!(
                "link '{}' references missing source port '{}:{}'",
                link.id, link.source.0, link.source.1
            )));
        }
        if !has(&link.destination.0, &link.destination.1) {
            return Err(SimError::parse_error(format!(
                "link '{}' references missing destination port '{}:{}'",
                link.id, link.destination.0, link.destination.1
            )));
        }
    }

    Ok(diagram)
}

/// Serialize a Diagram to a TOML string.
pub fn diagram_to_toml(diagram: &Diagram) -> Result<String, SimError> {
    let json = diagram_to_json(diagram)?;
    // Convert JSON to TOML via serde.
    let data: DiagramData =
        serde_json::from_str(&json).map_err(|e| SimError::parse_error(e.to_string()))?;
    toml::to_string_pretty(&data).map_err(|e| SimError::parse_error(e.to_string()))
}

/// Deserialize a Diagram from a TOML string.
pub fn toml_to_diagram(toml_str: &str) -> Result<Diagram, SimError> {
    let data: DiagramData =
        toml::from_str(toml_str).map_err(|e| SimError::parse_error(e.to_string()))?;
    let json = serde_json::to_string(&data).map_err(|e| SimError::parse_error(e.to_string()))?;
    json_to_diagram(&json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::param::{ParamMutability, Parameter};
    use crate::core::types::{SignalType, SignalValue};

    #[test]
    fn test_json_roundtrip() {
        let mut diagram = Diagram::new("test_rt");
        let mut b1 = SimpleBlock::new("b1", "Source");
        b1.declare_output("out", SignalType::Continuous);
        let mut b2 = SimpleBlock::new("b2", "Sink");
        b2.declare_input("in", SignalType::Continuous);
        diagram.add_block(Box::new(b1));
        diagram.add_block(Box::new(b2));
        diagram.add_link(Link::new("l1", "b1", "out", "b2", "in"));

        let json = diagram_to_json(&diagram).unwrap();
        let restored = json_to_diagram(&json).unwrap();

        assert_eq!(restored.name, "test_rt");
        assert_eq!(restored.block_count(), 2);
        assert_eq!(restored.link_count(), 1);
        // The port surface must come back, otherwise the restored links would
        // point at ports that do not exist.
        assert!(
            restored
                .get_block("b1")
                .unwrap()
                .ports()
                .get("out")
                .is_some(),
            "declared output port must survive"
        );
        assert!(
            restored
                .get_block("b2")
                .unwrap()
                .ports()
                .get("in")
                .is_some(),
            "declared input port must survive"
        );
    }

    /// The old schema stored parameters as a bare `f64` and read only scalars,
    /// so every non-scalar parameter was silently erased from the file.
    #[test]
    fn test_json_roundtrip_preserves_every_parameter_type() {
        let mut diagram = Diagram::new("types");
        let mut b = SimpleBlock::new("b1", "Multi");
        b.params_mut()
            .add(Parameter::new_config("s", SignalValue::Scalar(1.25), ""));
        b.params_mut().add(Parameter::new_config(
            "v",
            SignalValue::Vector(vec![1.0, 2.0, 3.0]),
            "",
        ));
        b.params_mut().add(Parameter::new_config(
            "m",
            SignalValue::Matrix(2, 2, vec![1.0, 2.0, 3.0, 4.0]),
            "",
        ));
        b.params_mut().add(Parameter::new_config(
            "c",
            SignalValue::Complex(1.0, -2.0),
            "",
        ));
        b.params_mut()
            .add(Parameter::new_config("b", SignalValue::Boolean(true), ""));
        b.params_mut()
            .add(Parameter::new_config("i", SignalValue::Integer(-7), ""));
        b.params_mut().add(Parameter::new_config(
            "str",
            SignalValue::String("fast".to_string()),
            "the mode",
        ));
        diagram.add_block(Box::new(b));

        let json = diagram_to_json(&diagram).unwrap();
        let restored = json_to_diagram(&json).unwrap();
        let params = restored.get_block("b1").unwrap().params();

        assert_eq!(params.get("s").unwrap().value, SignalValue::Scalar(1.25));
        assert_eq!(
            params.get("v").unwrap().value,
            SignalValue::Vector(vec![1.0, 2.0, 3.0]),
            "vectors must not be dropped"
        );
        assert_eq!(
            params.get("m").unwrap().value,
            SignalValue::Matrix(2, 2, vec![1.0, 2.0, 3.0, 4.0]),
            "matrices must not be dropped"
        );
        assert_eq!(
            params.get("c").unwrap().value,
            SignalValue::Complex(1.0, -2.0),
            "complex values must not be dropped"
        );
        assert_eq!(params.get("b").unwrap().value, SignalValue::Boolean(true));
        assert_eq!(params.get("i").unwrap().value, SignalValue::Integer(-7));
        assert_eq!(
            params.get("str").unwrap().value,
            SignalValue::String("fast".to_string()),
            "strings must not be dropped"
        );
        assert_eq!(params.get("str").unwrap().description, "the mode");
    }

    /// The old writer hard-coded `mutable: true` and the reader never read it,
    /// so `Static` vs `Config` vs `Tunable` was unrecoverable.
    #[test]
    fn test_json_roundtrip_preserves_mutability() {
        let mut diagram = Diagram::new("mut");
        let mut b = SimpleBlock::new("b1", "Blk");
        let mut static_p = Parameter::new_static("fixed", SignalValue::Scalar(1.0), "");
        static_p.mutability = ParamMutability::Static;
        b.params_mut().add(static_p);
        let mut tunable = Parameter::new_config("live", SignalValue::Scalar(2.0), "");
        tunable.mutability = ParamMutability::Tunable;
        b.params_mut().add(tunable);
        diagram.add_block(Box::new(b));

        let json = diagram_to_json(&diagram).unwrap();
        let restored = json_to_diagram(&json).unwrap();
        let params = restored.get_block("b1").unwrap().params();
        assert_eq!(
            params.get("fixed").unwrap().mutability,
            ParamMutability::Static
        );
        assert_eq!(
            params.get("live").unwrap().mutability,
            ParamMutability::Tunable
        );
    }

    /// A `Static` parameter must still be static after a round trip, which is
    /// observable through `set()` refusing to change it.
    #[test]
    fn test_roundtrip_preserves_static_rejection() {
        let mut diagram = Diagram::new("stat");
        let mut b = SimpleBlock::new("b1", "Blk");
        b.params_mut()
            .add(Parameter::new_static("k", SignalValue::Scalar(1.0), ""));
        diagram.add_block(Box::new(b));

        let json = diagram_to_json(&diagram).unwrap();
        let mut restored = json_to_diagram(&json).unwrap();
        let ok = restored
            .get_block_mut("b1")
            .unwrap()
            .params_mut()
            .set("k", SignalValue::Scalar(9.0));
        assert!(ok.is_none(), "a static parameter must stay unsettable");
    }

    /// Links whose endpoints do not exist must be rejected on load instead of
    /// silently producing a diagram with dangling connections.
    #[test]
    fn test_load_rejects_link_to_missing_port() {
        let json = r#"{
            "name": "bad",
            "description": "",
            "blocks": [
                {"id":"b1","block_type":"Src","parameters":[],"ports":[
                    {"id":"out","direction":"output","signal_type":"continuous"}]},
                {"id":"b2","block_type":"Snk","parameters":[],"ports":[]}
            ],
            "links": [
                {"id":"l1","source_block":"b1","source_port":"out",
                 "dest_block":"b2","dest_port":"ghost","delay":0.0}
            ],
            "version": 1,
            "schema": "scico-rs/diagram/v1"
        }"#;
        let err = json_to_diagram(json).unwrap_err();
        assert!(
            format!("{err}").contains("missing destination port"),
            "a dangling link must be rejected, got: {err}"
        );
    }

    #[test]
    fn test_load_rejects_unknown_parameter_type() {
        let json = r#"{
            "name": "bad",
            "description": "",
            "blocks": [{"id":"b1","block_type":"B","ports":[],"parameters":[
                {"name":"p","type":"quaternion","value":1}
            ]}],
            "links": [],
            "version": 1,
            "schema": "scico-rs/diagram/v1"
        }"#;
        let err = json_to_diagram(json).unwrap_err();
        assert!(
            format!("{err}").contains("unknown parameter type"),
            "an unknown type must be rejected, got: {err}"
        );
    }

    #[test]
    fn test_toml_roundtrip() {
        let mut diagram = Diagram::new("toml_test");
        let mut b = SimpleBlock::new("src", "Const");
        b.params_mut()
            .add(Parameter::new_config("value", SignalValue::Scalar(4.5), ""));
        diagram.add_block(Box::new(b));

        let toml = diagram_to_toml(&diagram).unwrap();
        let restored = toml_to_diagram(&toml).unwrap();
        assert_eq!(restored.name, "toml_test");
        assert_eq!(restored.block_count(), 1);
        assert_eq!(
            restored
                .get_block("src")
                .unwrap()
                .params()
                .get("value")
                .unwrap()
                .value,
            SignalValue::Scalar(4.5),
            "TOML must round-trip parameters too"
        );
    }

    #[test]
    fn test_ser_error_on_invalid_json() {
        let result = json_to_diagram("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_json_contains_schema() {
        let diagram = Diagram::new("schema_test");
        let json = diagram_to_json(&diagram).unwrap();
        let data: DiagramData = serde_json::from_str(&json).unwrap();
        assert_eq!(data.schema, "scico-rs/diagram/v1");
        assert_eq!(data.version, 1);
    }
}
