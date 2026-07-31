//! Plugin system: manifest, trait, manager, and registries.

use crate::core::block::Block;
use std::collections::HashMap;

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
        Self {
            name: name.to_string(),
            version: version.to_string(),
            author: author.to_string(),
            description: String::new(),
            api_version: "1.0".to_string(),
            entry_point: String::new(),
        }
    }

    /// Parse a plugin manifest from a JSON document.
    ///
    /// Expected fields: `name`, `version`, `author`, `description`,
    /// `api_version`, `entry_point` (all optional except `name`/`version`).
    pub fn from_json(json: &str) -> Result<Self, String> {
        let v: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("invalid manifest JSON: {}", e))?;
        let obj = v
            .as_object()
            .ok_or_else(|| "manifest JSON must be an object".to_string())?;
        let get = |k: &str| -> String {
            obj.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string()
        };
        let name = get("name");
        if name.is_empty() {
            return Err("manifest is missing required 'name' field".to_string());
        }
        let version = get("version");
        if version.is_empty() {
            return Err("manifest is missing required 'version' field".to_string());
        }
        Ok(Self {
            name,
            version,
            author: get("author"),
            description: get("description"),
            api_version: {
                let v = get("api_version");
                if v.is_empty() { "1.0".to_string() } else { v }
            },
            entry_point: get("entry_point"),
        })
    }

    /// Parse a plugin manifest from a TOML document.
    pub fn from_toml(toml: &str) -> Result<Self, String> {
        let v: toml::Value =
            toml::from_str(toml).map_err(|e| format!("invalid manifest TOML: {}", e))?;
        let get =
            |k: &str| -> String { v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string() };
        let name = get("name");
        if name.is_empty() {
            return Err("manifest is missing required 'name' field".to_string());
        }
        let version = get("version");
        if version.is_empty() {
            return Err("manifest is missing required 'version' field".to_string());
        }
        Ok(Self {
            name,
            version,
            author: get("author"),
            description: get("description"),
            api_version: {
                let v = get("api_version");
                if v.is_empty() { "1.0".to_string() } else { v }
            },
            entry_point: get("entry_point"),
        })
    }
}

/// Plugin trait for extendable functionality.
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &PluginManifest;
    fn initialize(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn register_blocks(&self, _registry: &mut BlockRegistry) -> Result<(), String> {
        Ok(())
    }
    fn register_solvers(&self, _registry: &mut SolverRegistry) -> Result<(), String> {
        Ok(())
    }
    fn register_postprocessors(&self, _registry: &mut PostProcessorRegistry) -> Result<(), String> {
        Ok(())
    }
    fn shutdown(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Registry for Block types.
pub struct BlockRegistry {
    pub block_types: HashMap<String, Box<dyn Fn() -> Box<dyn Block>>>,
}

impl BlockRegistry {
    pub fn new() -> Self {
        Self {
            block_types: HashMap::new(),
        }
    }
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
    pub fn new() -> Self {
        Self {
            solvers: HashMap::new(),
        }
    }
    pub fn register(
        &mut self,
        name: &str,
        factory: Box<dyn Fn() -> Box<dyn crate::runtime::solver::OdeSolver>>,
    ) {
        self.solvers.insert(name.to_string(), factory);
    }
}

/// Post-processor trait.
pub trait PostProcessor: Send + Sync {
    fn name(&self) -> &str;
    /// Produce a report from a recorder's captured data.
    ///
    /// The default implementation builds a `SimulationReport` with a
    /// per-signal summary section (min / max / mean / RMS), which is real
    /// observable behaviour for any recorder with recorded signals.
    fn process(
        &self,
        data: &crate::postproc::recorder::DataRecorder,
    ) -> Result<crate::postproc::reporting::SimulationReport, String> {
        use crate::postproc::reporting::{ReportSection, SimulationReport};

        let mut report = SimulationReport::new(
            &format!("{} post-processing report", self.name()),
            "Generated from recorded simulation signals.",
        );
        let mut content = String::new();
        for name in data.signal_names() {
            if let Some(series) = data.get_timeseries(name) {
                if series.is_empty() {
                    continue;
                }
                let mut min = f64::INFINITY;
                let mut max = f64::NEG_INFINITY;
                let mut sum = 0.0;
                let mut sum_sq = 0.0;
                for &v in series {
                    if v.is_finite() {
                        min = min.min(v);
                        max = max.max(v);
                        sum += v;
                        sum_sq += v * v;
                    }
                }
                let n = series.len() as f64;
                content.push_str(&format!(
                    "- {}: n={}, min={:.6}, max={:.6}, mean={:.6}, rms={:.6}\n",
                    name,
                    series.len(),
                    min,
                    max,
                    sum / n,
                    (sum_sq / n).sqrt()
                ));
            }
        }
        if content.is_empty() {
            content.push_str("No signal data recorded.\n");
        }
        report.add_section(ReportSection::new("Signal summary", content.trim_end()));
        Ok(report)
    }
}

/// Registry for post-processor types.
pub struct PostProcessorRegistry {
    pub processors: HashMap<String, Box<dyn Fn() -> Box<dyn PostProcessor>>>,
}

impl PostProcessorRegistry {
    pub fn new() -> Self {
        Self {
            processors: HashMap::new(),
        }
    }
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
        Self {
            plugins: Vec::new(),
            block_registry: BlockRegistry::new(),
            solver_registry: SolverRegistry::new(),
            postprocessor_registry: PostProcessorRegistry::new(),
        }
    }

    pub fn load_from_directory(&mut self, path: &str) -> Result<Vec<String>, String> {
        let entries = std::fs::read_dir(path).map_err(|e| format!("Read dir error: {}", e))?;
        let mut loaded = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| format!("Entry error: {}", e))?;
            let p = entry.path();
            if p.extension()
                .is_some_and(|ext| ext == "json" || ext == "toml")
            {
                loaded.push(p.to_string_lossy().to_string());
            }
        }
        Ok(loaded)
    }

    pub fn load_plugin(&mut self, manifest_path: &str) -> Result<(), String> {
        let content =
            std::fs::read_to_string(manifest_path).map_err(|e| format!("Read error: {}", e))?;
        let manifest = if manifest_path.ends_with(".toml") {
            PluginManifest::from_toml(&content)?
        } else {
            PluginManifest::from_json(&content)?
        };
        self.plugins.push(Box::new(ManifestPlugin { manifest }));
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

impl Default for BlockRegistry {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for SolverRegistry {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for PostProcessorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A plugin loaded from a manifest file. Its extension hooks are the trait
/// defaults; custom behaviour is provided by overriding them in real plugins.
struct ManifestPlugin {
    manifest: PluginManifest,
}

impl Plugin for ManifestPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::SimpleBlock;
    struct TestPlugin {
        manifest: PluginManifest,
    }
    impl TestPlugin {
        fn new() -> Self {
            Self {
                manifest: PluginManifest::new("test", "1.0", "author"),
            }
        }
    }
    impl Plugin for TestPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }
    }

    #[test]
    fn test_plugin_manifest() {
        let m = PluginManifest::new("p", "1.0", "a");
        assert_eq!(m.name, "p");
    }
    #[test]
    fn test_block_registry() {
        let mut reg = BlockRegistry::new();
        reg.register(
            "const",
            Box::new(|| Box::new(SimpleBlock::new("c", "Constant"))),
        );
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
        std::fs::write(
            path,
            r#"{"name":"demo","version":"2.0","author":"me","description":"demo plugin"}"#,
        )
        .unwrap();
        let mut mgr = PluginManager::new();
        assert!(mgr.load_plugin(path).is_ok());
        assert_eq!(mgr.list_plugins().len(), 1);
        assert_eq!(mgr.list_plugins()[0].name, "demo");
        assert_eq!(mgr.list_plugins()[0].version, "2.0");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_plugin_missing_name_rejected() {
        let path = "/tmp/test_manifest_bad.json";
        std::fs::write(path, "{}").unwrap();
        let mut mgr = PluginManager::new();
        assert!(mgr.load_plugin(path).is_err());
        assert!(mgr.list_plugins().is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_manifest_parse_toml() {
        let m = PluginManifest::from_toml(
            "name = \"p1\"\nversion = \"1.0\"\nentry_point = \"libp1.so\"\n",
        )
        .unwrap();
        assert_eq!(m.name, "p1");
        assert_eq!(m.entry_point, "libp1.so");
        assert!(PluginManifest::from_toml("version = \"1.0\"").is_err());
    }

    #[test]
    fn test_default_postprocessor_builds_report() {
        use crate::postproc::recorder::{DataRecorder, RecorderConfig};
        struct Demo;
        impl PostProcessor for Demo {
            fn name(&self) -> &str {
                "demo"
            }
        }
        let mut recorder = DataRecorder::new(RecorderConfig::default());
        let mut signals = std::collections::HashMap::new();
        signals.insert("x".to_string(), 1.0);
        recorder.record(0.0, &signals);
        signals.insert("x".to_string(), 3.0);
        recorder.record(1.0, &signals);
        let report = Demo.process(&recorder).unwrap();
        assert!(report.to_markdown().contains("Signal summary"));
        assert!(report.to_markdown().contains("x"));
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
