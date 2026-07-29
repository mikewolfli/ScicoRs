//! Plugin system: manifest, trait, manager, and registries.

use std::collections::HashMap;
use crate::core::block::Block;

/// Plugin manifest metadata.
#[derive(Debug, Clone)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub api_version: String,
    pub entry_point: String,
}

impl PluginManifest {
    pub fn new(name: &str, version: &str, author: &str) -> Self {
        Self { name: name.to_string(), version: version.to_string(), author: author.to_string(),
            description: String::new(), api_version: "1.0".to_string(), entry_point: String::new() }
    }
}

/// Plugin trait for extendable functionality.
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &PluginManifest;
    fn initialize(&mut self) -> Result<(), String> { Ok(()) }
    fn register_blocks(&self, _registry: &mut BlockRegistry) -> Result<(), String> { Ok(()) }
    fn register_solvers(&self, _registry: &mut SolverRegistry) -> Result<(), String> { Ok(()) }
    fn register_postprocessors(&self, _registry: &mut PostProcessorRegistry) -> Result<(), String> { Ok(()) }
    fn shutdown(&mut self) -> Result<(), String> { Ok(()) }
}

/// Registry for Block types.
pub struct BlockRegistry {
    pub block_types: HashMap<String, Box<dyn Fn() -> Box<dyn Block>>>,
}

impl BlockRegistry {
    pub fn new() -> Self { Self { block_types: HashMap::new() } }
    pub fn register(&mut self, type_name: &str, factory: Box<dyn Fn() -> Box<dyn Block>>) {
        self.block_types.insert(type_name.to_string(), factory);
    }
    pub fn create_block(&self, type_name: &str) -> Option<Box<dyn Block>> {
        self.block_types.get(type_name).map(|f| f())
    }
}

/// Registry for ODE solver types.
pub struct SolverRegistry {
    pub solvers: HashMap<String, Box<dyn Fn() -> Box<dyn crate::runtime::solver::OdeSolver>>>,
}

impl SolverRegistry {
    pub fn new() -> Self { Self { solvers: HashMap::new() } }
    pub fn register(&mut self, name: &str, factory: Box<dyn Fn() -> Box<dyn crate::runtime::solver::OdeSolver>>) {
        self.solvers.insert(name.to_string(), factory);
    }
}

/// Post-processor trait.
pub trait PostProcessor: Send + Sync {
    fn name(&self) -> &str;
    fn process(&self, _data: &crate::postproc::recorder::DataRecorder) -> Result<crate::postproc::reporting::SimulationReport, String> {
        Err("Not implemented".to_string())
    }
}

/// Registry for post-processor types.
pub struct PostProcessorRegistry {
    pub processors: HashMap<String, Box<dyn Fn() -> Box<dyn PostProcessor>>>,
}

impl PostProcessorRegistry {
    pub fn new() -> Self { Self { processors: HashMap::new() } }
    pub fn register(&mut self, name: &str, factory: Box<dyn Fn() -> Box<dyn PostProcessor>>) {
        self.processors.insert(name.to_string(), factory);
    }
}

/// Plugin manager for loading and managing plugins.
pub struct PluginManager {
    pub plugins: Vec<Box<dyn Plugin>>,
    pub block_registry: BlockRegistry,
    pub solver_registry: SolverRegistry,
    pub postprocessor_registry: PostProcessorRegistry,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugins: Vec::new(), block_registry: BlockRegistry::new(), solver_registry: SolverRegistry::new(), postprocessor_registry: PostProcessorRegistry::new() }
    }

    pub fn load_from_directory(&mut self, path: &str) -> Result<Vec<String>, String> {
        let entries = std::fs::read_dir(path).map_err(|e| format!("Read dir error: {}", e))?;
        let mut loaded = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| format!("Entry error: {}", e))?;
            let p = entry.path();
            if p.extension().is_some_and(|ext| ext == "json" || ext == "toml") {
                loaded.push(p.to_string_lossy().to_string());
            }
        }
        Ok(loaded)
    }

    pub fn load_plugin(&mut self, manifest_path: &str) -> Result<(), String> {
        let _content = std::fs::read_to_string(manifest_path).map_err(|e| format!("Read error: {}", e))?;
        Ok(())
    }

    pub fn initialize_all(&mut self) -> Result<(), String> {
        for plugin in &mut self.plugins {
            plugin.initialize()?;
            plugin.register_blocks(&mut self.block_registry)?;
            plugin.register_solvers(&mut self.solver_registry)?;
            plugin.register_postprocessors(&mut self.postprocessor_registry)?;
        }
        Ok(())
    }

    pub fn list_plugins(&self) -> Vec<&PluginManifest> {
        self.plugins.iter().map(|p| p.manifest()).collect()
    }
}

impl Default for BlockRegistry { fn default() -> Self { Self::new() } }
impl Default for SolverRegistry { fn default() -> Self { Self::new() } }
impl Default for PostProcessorRegistry { fn default() -> Self { Self::new() } }
impl Default for PluginManager { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    struct TestPlugin { manifest: PluginManifest }
    impl TestPlugin {
        fn new() -> Self { Self { manifest: PluginManifest::new("test", "1.0", "author") } }
    }
    impl Plugin for TestPlugin {
        fn manifest(&self) -> &PluginManifest { &self.manifest }
    }

    #[test]
    fn test_plugin_manifest() {
        let m = PluginManifest::new("p", "1.0", "a");
        assert_eq!(m.name, "p");
    }
    #[test]
    fn test_block_registry() {
        let mut reg = BlockRegistry::new();
        reg.register("const", Box::new(|| Box::new(SimpleBlock::new("c", "Constant"))));
        assert!(reg.create_block("const").is_some());
        assert!(reg.create_block("nonexistent").is_none());
    }
    #[test]
    fn test_plugin_manager_create() {
        let mgr = PluginManager::new();
        assert!(mgr.list_plugins().is_empty());
    }
    #[test]
    fn test_plugin_manager_load_plugin() {
        let path = "/tmp/test_manifest.json";
        std::fs::write(path, "{}").unwrap();
        let mut mgr = PluginManager::new();
        assert!(mgr.load_plugin(path).is_ok());
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn test_plugin_trait() {
        let mut p = TestPlugin::new();
        assert!(p.initialize().is_ok());
        assert_eq!(p.manifest().version, "1.0");
    }
    #[test]
    fn test_solver_registry() {
        let reg = SolverRegistry::new();
        assert!(reg.solvers.is_empty());
    }
    #[test]
    fn test_postprocessor_registry() {
        let reg = PostProcessorRegistry::new();
        assert!(reg.processors.is_empty());
    }
}
