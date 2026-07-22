//! Dependency declaration for block composition.
//!
//! Declares inter-block dependencies used for automatic wiring
//! and component assembly. Each dependency connects a consumer
//! port to a provider block's output port.

/// A single dependency declaration: consumer port -> provider port.
#[derive(Debug, Clone)]
pub struct DependencyDecl {
    /// The ID of the provider block.
    pub provider_block: String,
    /// The output port name on the provider.
    pub provider_port: String,
    /// The input port name on the consumer.
    pub consumer_port: String,
    /// Human-readable description.
    pub description: String,
}

impl DependencyDecl {
    pub fn new(provider_block: &str, provider_port: &str, consumer_port: &str) -> Self {
        Self {
            provider_block: provider_block.to_string(),
            provider_port: provider_port.to_string(),
            consumer_port: consumer_port.to_string(),
            description: String::new(),
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
}

/// A set of dependency declarations for a block.
#[derive(Debug, Clone, Default)]
pub struct DependencySet {
    /// Ordered list of dependency declarations.
    pub dependencies: Vec<DependencyDecl>,
}

impl DependencySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, dep: DependencyDecl) {
        self.dependencies.push(dep);
    }

    pub fn iter(&self) -> impl Iterator<Item = &DependencyDecl> {
        self.dependencies.iter()
    }

    pub fn len(&self) -> usize {
        self.dependencies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dependencies.is_empty()
    }

    /// Find dependencies targeting a specific consumer port.
    pub fn for_consumer_port(&self, port: &str) -> Vec<&DependencyDecl> {
        self.dependencies.iter().filter(|d| d.consumer_port == port).collect()
    }

    /// Find dependencies from a specific provider.
    pub fn from_provider(&self, block: &str) -> Vec<&DependencyDecl> {
        self.dependencies.iter().filter(|d| d.provider_block == block).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_decl() {
        let mut ds = DependencySet::new();
        ds.add(DependencyDecl::new("sensor", "out", "in")
            .with_description("sensor reading"));
        ds.add(DependencyDecl::new("controller", "cmd", "setpoint"));
        assert_eq!(ds.len(), 2);
        assert_eq!(ds.for_consumer_port("in").len(), 1);
        assert_eq!(ds.from_provider("sensor").len(), 1);
    }
}
