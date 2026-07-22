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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ParamData {
    name: String,
    value: f64,
    mutable: bool,
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

/// Serialize a Diagram to a JSON string.
pub fn diagram_to_json(diagram: &Diagram) -> SerResult<String> {
    let block_data: Vec<BlockData> = diagram
        .blocks()
        .map(|(id, block)| {
            let params: Vec<ParamData> = block
                .params()
                .keys()
                .filter_map(|name| {
                    block.params().get_scalar(name).map(|v| ParamData {
                        name: name.clone(),
                        value: v,
                        mutable: true,
                    })
                })
                .collect();
            BlockData {
                id: id.clone(),
                block_type: block.block_type().to_string(),
                parameters: params,
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
/// Note: This creates untyped blocks; callers should register block factories
/// for full reconstruction.
pub fn json_to_diagram(json: &str) -> Result<Diagram, SimError> {
    let data: DiagramData =
        serde_json::from_str(json).map_err(|e| SimError::parse_error(e.to_string()))?;

    let mut diagram = Diagram::new(&data.name);
    diagram.description = data.description;

    // Create placeholder blocks (type info preserved but no runtime block logic).
    for bd in &data.blocks {
        let mut block = crate::core::block::SimpleBlock::new(&bd.id, &bd.block_type);
        for pd in &bd.parameters {
            block
                .params_mut()
                .add(crate::core::param::Parameter::new_config(
                    &pd.name,
                    crate::core::types::SignalValue::Scalar(pd.value),
                    "deserialized parameter",
                ));
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
        if ld.delay != 0.0 {
            link.delay = ld.delay;
        }
        diagram.add_link(link);
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

    #[test]
    fn test_json_roundtrip() {
        let mut diagram = Diagram::new("test_rt");
        let b1 = SimpleBlock::new("b1", "Source");
        let b2 = SimpleBlock::new("b2", "Sink");
        diagram.add_block(Box::new(b1));
        diagram.add_block(Box::new(b2));
        diagram.add_link(Link::new("l1", "b1", "out", "b2", "in"));

        let json = diagram_to_json(&diagram).unwrap();
        let restored = json_to_diagram(&json).unwrap();

        assert_eq!(restored.name, "test_rt");
        assert_eq!(restored.block_count(), 2);
        assert_eq!(restored.link_count(), 1);
    }

    #[test]
    fn test_toml_roundtrip() {
        let mut diagram = Diagram::new("toml_test");
        let b = SimpleBlock::new("src", "Const");
        diagram.add_block(Box::new(b));

        let toml = diagram_to_toml(&diagram).unwrap();
        let restored = toml_to_diagram(&toml).unwrap();
        assert_eq!(restored.name, "toml_test");
        assert_eq!(restored.block_count(), 1);
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
