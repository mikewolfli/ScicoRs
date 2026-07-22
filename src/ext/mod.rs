//! Extension and Plugin System
//!
//! Provides a plugin/extension framework for loading external
//! modules, custom blocks, and third-party extensions at runtime.

use crate::core::block::Block;
use crate::core::error::SimError;
use std::collections::HashMap;

/// Metadata about a registered extension.
#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

/// A factory function that creates a block instance by type name.
pub type BlockFactory = fn(&str, &HashMap<String, f64>) -> Result<Box<dyn Block>, SimError>;

/// The extension registry — manages registered block types and extensions.
#[derive(Debug, Default)]
pub struct ExtensionRegistry {
    extensions: Vec<ExtensionInfo>,
    block_factories: HashMap<String, BlockFactory>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an extension.
    pub fn register_extension(&mut self, info: ExtensionInfo) {
        self.extensions.push(info);
    }

    /// Register a block factory.
    pub fn register_block_type(&mut self, type_name: &str, factory: BlockFactory) {
        self.block_factories.insert(type_name.to_string(), factory);
    }

    /// Create a block of the given type using a registered factory.
    pub fn create_block(
        &self,
        type_name: &str,
        id: &str,
        params: &HashMap<String, f64>,
    ) -> Result<Box<dyn Block>, SimError> {
        match self.block_factories.get(type_name) {
            Some(factory) => factory(id, params),
            None => Err(SimError::runtime(format!(
                "unknown block type: {type_name}"
            ))),
        }
    }

    /// Check if a block type is registered.
    pub fn has_block_type(&self, type_name: &str) -> bool {
        self.block_factories.contains_key(type_name)
    }

    /// List all registered block types.
    pub fn block_types(&self) -> Vec<&str> {
        self.block_factories.keys().map(|s| s.as_str()).collect()
    }

    /// List all registered extensions.
    pub fn extensions(&self) -> &[ExtensionInfo] {
        &self.extensions
    }
}
