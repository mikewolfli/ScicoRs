//! Diagram validation logic.
//!
//! Provides comprehensive validation rules for checking diagram
//! consistency: duplicate IDs, missing blocks/ports, direction
//! mismatches, cycles, unconnected ports, and parameter errors.

use crate::core::diagram::Diagram;
use crate::core::error::{ErrorCode, SimError};
use std::collections::HashSet;

/// Result of a diagram validation pass.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the diagram is valid.
    pub is_valid: bool,
    /// List of validation errors (unified `SimError` format).
    pub errors: Vec<SimError>,
    /// List of non-fatal warnings.
    pub warnings: Vec<String>,
}

/// Run all validation rules on a diagram.
pub fn validate_diagram(diagram: &Diagram) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // 1. Check for duplicate block IDs.
    let mut seen_blocks = HashSet::new();
    for (id, _) in diagram.blocks() {
        if !seen_blocks.insert(id.clone()) {
            errors.push(SimError::new(ErrorCode::DuplicateBlockId, format!("duplicate block ID: {id}")));
        }
    }

    // 2. Check for duplicate link IDs.
    let mut seen_links = HashSet::new();
    for link in diagram.links().iter() {
        if !seen_links.insert(link.id.clone()) {
            errors.push(SimError::new(ErrorCode::DuplicateLinkId, format!("duplicate link ID: {}", link.id)));
        }
    }

    // 3. Check that all link source/destination blocks exist.
    for link in diagram.links().iter() {
        if diagram.get_block(&link.source.0).is_none() {
            errors.push(SimError::new(ErrorCode::MissingBlock, format!("missing block: {}", link.source.0)));
        }
        if diagram.get_block(&link.destination.0).is_none() {
            errors.push(SimError::new(ErrorCode::MissingBlock, format!("missing block: {}", link.destination.0)));
        }
    }

    // 4. Check that all link ports exist on their blocks.
    for link in diagram.links().iter() {
        if let Some(block) = diagram.get_block(&link.source.0)
            && block.ports().get(&link.source.1).is_none()
        {
            errors.push(SimError::new(ErrorCode::InvalidPortRef, format!("missing port {}.{}", link.source.0, link.source.1)));
        }
        if let Some(block) = diagram.get_block(&link.destination.0)
            && block.ports().get(&link.destination.1).is_none()
        {
            errors.push(SimError::new(ErrorCode::InvalidPortRef, format!("missing port {}.{}", link.destination.0, link.destination.1)));
        }
    }

    // 5. Check port direction correctness.
    for link in diagram.links().iter() {
        if let Some(block) = diagram.get_block(&link.source.0)
            && let Some(port) = block.ports().get(&link.source.1)
            && port.is_input()
        {
            errors.push(SimError::new(
                ErrorCode::PortDirectionMismatch,
                format!("source port '{}.{}' is an input port, must be output", link.source.0, link.source.1),
            ).with_context(format!("link={}", link.id)));
        }
        if let Some(block) = diagram.get_block(&link.destination.0)
            && let Some(port) = block.ports().get(&link.destination.1)
            && port.is_output()
        {
            errors.push(SimError::new(
                ErrorCode::PortDirectionMismatch,
                format!("destination port '{}.{}' is an output port, must be input", link.destination.0, link.destination.1),
            ).with_context(format!("link={}", link.id)));
        }
    }

    // 5b. Check signal type compatibility between linked ports.
    for link in diagram.links().iter() {
        let src_type = diagram.get_block(&link.source.0)
            .and_then(|b| b.ports().get(&link.source.1))
            .map(|p| p.signal_type);
        let dst_type = diagram.get_block(&link.destination.0)
            .and_then(|b| b.ports().get(&link.destination.1))
            .map(|p| p.signal_type);
        if let (Some(src), Some(dst)) = (src_type, dst_type)
            && src != dst
        {
            errors.push(SimError::new(
                ErrorCode::SignalTypeMismatchLink,
                format!("signal type mismatch on link '{}': source '{}' is {:?}, destination '{}' is {:?}",
                    link.id, link.source.1, src, link.destination.1, dst),
            ));
        }
    }

    // 6. Check for cycles.
    let topo = diagram.links().topological_sort();
    if topo.is_none() {
        errors.push(SimError::new(ErrorCode::CycleDetected, "cycle detected in diagram topology"));
    }

    // 7. Check for unconnected input ports.
    for (block_id, block) in diagram.blocks() {
        for port in block.ports().inputs() {
            let connected = diagram.links().iter().any(|l| {
                l.destination.0 == *block_id && l.destination.1 == port.id
            });
            if !connected {
                let io = block.io_declaration();
                let is_required = io.find_input(&port.id).map(|d| d.required).unwrap_or(true);
                if is_required {
                    errors.push(SimError::new(
                        ErrorCode::UnconnectedInput,
                        format!("unconnected input port '{}.{}'", block_id, port.id),
                    ));
                }
            }
        }
    }

    // 8. Check for dangling (unconnected) output ports.
    for (block_id, block) in diagram.blocks() {
        for port in block.ports().outputs() {
            let connected = diagram.links().iter().any(|l| {
                l.source.0 == *block_id && l.source.1 == port.id
            });
            if !connected {
                warnings.push(format!("dangling output '{}.{}'", block_id, port.id));
            }
        }
    }

    // 9. Check block configuration (parameters, I/O declarations).
    for (block_id, block) in diagram.blocks() {
        if let Err(issues) = block.validate_configuration() {
            for issue in &issues {
                errors.push(SimError::new(
                    ErrorCode::ValidationError,
                    format!("block '{}' configuration error: {}", block_id, issue),
                ));
            }
        }
    }

    ValidationResult {
        is_valid: errors.is_empty(),
        errors,
        warnings,
    }
}

/// Validate a specific block's configuration against its declarations.
pub fn validate_block_config(block: &dyn crate::core::block::Block) -> Result<(), Vec<String>> {
    block.validate_configuration()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    use crate::core::link::Link;

    fn make_diagram() -> Diagram {
        let mut d = Diagram::new("test");
        let mut src = SimpleBlock::new("src", "Source");
        src.declare_output("out", crate::core::types::SignalType::Continuous);
        let mut sink = SimpleBlock::new("sink", "Sink");
        sink.declare_input("in", crate::core::types::SignalType::Continuous);
        d.add_block(Box::new(src));
        d.add_block(Box::new(sink));
        d.add_link(Link::new("l1", "src", "out", "sink", "in"));
        d
    }

    #[test]
    fn test_valid_diagram() {
        let d = make_diagram();
        let result = validate_diagram(&d);
        assert!(result.is_valid, "expected no validation errors, got {:?}", result.errors);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_missing_block_in_link() {
        let mut d = Diagram::new("test");
        let mut sink = SimpleBlock::new("sink", "Sink");
        sink.declare_input("in", crate::core::types::SignalType::Continuous);
        d.add_block(Box::new(sink));
        d.add_link(Link::new("bad", "nonexistent", "out", "sink", "in"));
        let result = validate_diagram(&d);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.code == ErrorCode::MissingBlock));
    }

    #[test]
    fn test_port_direction_mismatch() {
        let mut d = Diagram::new("test");
        let mut src = SimpleBlock::new("src", "Source");
        src.declare_input("in", crate::core::types::SignalType::Continuous);
        let mut sink = SimpleBlock::new("sink", "Sink");
        sink.declare_input("in2", crate::core::types::SignalType::Continuous);
        d.add_block(Box::new(src));
        d.add_block(Box::new(sink));
        d.add_link(Link::new("l1", "src", "in", "sink", "in2"));
        let result = validate_diagram(&d);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.code == ErrorCode::PortDirectionMismatch));
    }

    #[test]
    fn test_cycle_detection() {
        let mut d = Diagram::new("test_cycle");
        let mut a = SimpleBlock::new("a", "A");
        let mut b = SimpleBlock::new("b", "B");
        a.declare_output("out", crate::core::types::SignalType::Continuous);
        a.declare_input("in", crate::core::types::SignalType::Continuous);
        b.declare_input("in", crate::core::types::SignalType::Continuous);
        b.declare_output("out", crate::core::types::SignalType::Continuous);
        d.add_block(Box::new(a));
        d.add_block(Box::new(b));
        d.add_link(Link::new("l1", "a", "out", "b", "in"));
        d.add_link(Link::new("l2", "b", "out", "a", "in"));
        let result = validate_diagram(&d);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.code == ErrorCode::CycleDetected));
    }

    #[test]
    fn test_unconnected_input() {
        let mut d = Diagram::new("test");
        let mut sink = SimpleBlock::new("sink", "Sink");
        sink.declare_input("in", crate::core::types::SignalType::Continuous);
        d.add_block(Box::new(sink));
        let result = validate_diagram(&d);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.code == ErrorCode::UnconnectedInput));
    }

    #[test]
    fn test_empty_diagram() {
        let d = Diagram::new("empty");
        let result = validate_diagram(&d);
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_single_block_no_links() {
        let mut d = Diagram::new("single");
        let mut b = SimpleBlock::new("b1", "Const");
        b.declare_output("out", crate::core::types::SignalType::Continuous);
        d.add_block(Box::new(b));
        let result = validate_diagram(&d);
        // Single block with only outputs — no errors, only a dangling warning.
        assert!(result.is_valid);
        assert!(result.warnings.iter().any(|w| w.contains("dangling")));
    }

    #[test]
    fn test_signal_type_mismatch() {
        let mut d = Diagram::new("test_type");
        let mut src = SimpleBlock::new("src", "Source");
        src.declare_output("out", crate::core::types::SignalType::Continuous);
        let mut sink = SimpleBlock::new("sink", "Sink");
        sink.declare_input("in", crate::core::types::SignalType::Discrete);
        d.add_block(Box::new(src));
        d.add_block(Box::new(sink));
        d.add_link(Link::new("l1", "src", "out", "sink", "in"));
        let result = validate_diagram(&d);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.code == ErrorCode::SignalTypeMismatchLink));
    }

    #[test]
    fn test_duplicate_link_id() {
        let mut d = Diagram::new("dup_link");
        let mut src = SimpleBlock::new("src", "Source");
        src.declare_output("out", crate::core::types::SignalType::Continuous);
        let mut sink = SimpleBlock::new("sink", "Sink");
        sink.declare_input("in", crate::core::types::SignalType::Continuous);
        d.add_block(Box::new(src));
        d.add_block(Box::new(sink));
        d.add_link(Link::new("l1", "src", "out", "sink", "in"));
        d.add_link(Link::new("l1", "src", "out", "sink", "in"));
        let result = validate_diagram(&d);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.code == ErrorCode::DuplicateLinkId));
    }
}
