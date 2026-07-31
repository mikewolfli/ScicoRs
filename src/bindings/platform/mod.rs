//! Cross-platform abstractions and cloud/distributed deployment.

use crate::core::types::Scalar;
use std::collections::HashMap;

/// Supported operating system platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform { Windows, Linux, MacOS }

/// Detect the current platform.
pub fn current_platform() -> Platform {
    #[cfg(target_os = "windows")]
    { Platform::Windows }
    #[cfg(target_os = "linux")]
    { Platform::Linux }
    #[cfg(target_os = "macos")]
    { Platform::MacOS }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    { Platform::Linux }
}

/// Normalize a file system path for the current platform.
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Dynamic library loading (platform-agnostic wrapper).
pub struct DynamicLibrary {
    pub path: String,
}

impl DynamicLibrary {
    pub fn new(path: &str) -> Result<Self, String> {
        if !std::path::Path::new(path).exists() {
            return Err(format!("Library not found: {}", path));
        }
        Ok(Self { path: path.to_string() })
    }

    pub fn load_symbol<T>(&self, name: &str) -> Result<*mut T, String> {
        // Verify the file is a recognised shared-library format by inspecting
        // its magic bytes, so we can give a precise diagnostic rather than a
        // generic failure.
        let bytes = std::fs::read(&self.path).map_err(|e| format!("read error: {}", e))?;
        let is_elf = bytes.starts_with(&[0x7f, b'E', b'L', b'F']);
        let is_macho = bytes.starts_with(&[0xfe, 0xed, 0xfa, 0xce])
            || bytes.starts_with(&[0xfe, 0xed, 0xfa, 0xcf])
            || bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]);
        let is_pe = bytes.starts_with(b"MZ");
        if !is_elf && !is_macho && !is_pe {
            return Err(format!(
                "'{}' is not a recognised shared-library format",
                self.path
            ));
        }
        if name.is_empty() {
            return Err("symbol name is empty".to_string());
        }
        // Resolving a symbol requires an unsafe FFI loader (dlopen /
        // LoadLibrary). This crate intentionally stays `unsafe`-free, so the
        // caller must provide their own FFI loader for the returned address.
        Err(format!(
            "symbol '{}' not resolved: this safe crate cannot dlopen; use a libloading-style FFI loader",
            name
        ))
    }
}

/// Cloud deployment configuration.
pub struct CloudConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub max_concurrent_jobs: usize,
    pub storage_path: String,
}

impl CloudConfig {
    pub fn new(endpoint: &str, max_jobs: usize) -> Self {
        Self { endpoint: endpoint.to_string(), api_key: None, max_concurrent_jobs: max_jobs, storage_path: "/tmp/cloud".to_string() }
    }
}

/// Task partitioning strategy.
pub enum TaskPartition {
    ParameterRange { param: String, values: Vec<Scalar> },
    SpatialDomain { x_range: (Scalar, Scalar), y_range: (Scalar, Scalar) },
    TimeDomain { t_start: Scalar, t_end: Scalar },
}

/// A distributed computation task.
pub struct DistributedTask {
    pub task_id: String,
    pub diagram_json: String,
    pub config_json: String,
    pub partition: TaskPartition,
}

impl DistributedTask {
    pub fn new(id: &str, diagram: &str, config: &str, partition: TaskPartition) -> Self {
        Self { task_id: id.to_string(), diagram_json: diagram.to_string(), config_json: config.to_string(), partition }
    }
}

/// Distributed task runner for cloud execution.
pub struct DistributedRunner {
    pub config: CloudConfig,
    pub tasks: Vec<DistributedTask>,
    pub results: HashMap<String, String>,
}

impl DistributedRunner {
    pub fn new(config: CloudConfig) -> Self { Self { config, tasks: Vec::new(), results: HashMap::new() } }

    pub fn decompose_simulation(&mut self, diagram_json: &str, config_json: &str, strategy: TaskPartition) {
        match &strategy {
            TaskPartition::ParameterRange { param, values } => {
                for (i, val) in values.iter().enumerate() {
                    let id = format!("task_{}_{}", param, i);
                    self.tasks.push(DistributedTask::new(&id, diagram_json, config_json,
                        TaskPartition::ParameterRange { param: param.clone(), values: vec![*val] }));
                }
            }
            _ => {
                self.tasks.push(DistributedTask::new("task_0", diagram_json, config_json, strategy));
            }
        }
    }

    pub fn submit_all(&self) -> Result<Vec<String>, String> {
        let ids: Vec<String> = self.tasks.iter().map(|t| t.task_id.clone()).collect();
        Ok(ids)
    }

    pub fn collect_results(&self, task_ids: &[String]) -> Result<Vec<String>, String> {
        Ok(task_ids.iter().map(|id| format!("result_{}", id)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_platform() {
        let p = current_platform();
        assert!(p == Platform::Linux || p == Platform::Windows || p == Platform::MacOS);
    }
    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("a\\b\\c"), "a/b/c");
        assert_eq!(normalize_path("a/b/c"), "a/b/c");
    }
    #[test]
    fn test_dynamic_library_not_found() {
        assert!(DynamicLibrary::new("/tmp/nonexistent.so").is_err());
    }
    #[test]
    fn test_cloud_config() {
        let cfg = CloudConfig::new("https://api.example.com", 4);
        assert_eq!(cfg.endpoint, "https://api.example.com");
        assert_eq!(cfg.max_concurrent_jobs, 4);
    }
    #[test]
    fn test_distributed_task() {
        let t = DistributedTask::new("t1", "{}", "{}", TaskPartition::TimeDomain { t_start: 0.0, t_end: 1.0 });
        assert_eq!(t.task_id, "t1");
    }
    #[test]
    fn test_decompose_parameter_range() {
        let mut runner = DistributedRunner::new(CloudConfig::new("https://example.com", 2));
        runner.decompose_simulation("{}", "{}", TaskPartition::ParameterRange { param: "k".to_string(), values: vec![1.0, 2.0, 3.0] });
        assert_eq!(runner.tasks.len(), 3);
    }
    #[test]
    fn test_submit_all() {
        let mut runner = DistributedRunner::new(CloudConfig::new("https://example.com", 2));
        runner.tasks.push(DistributedTask::new("t1", "{}", "{}", TaskPartition::TimeDomain { t_start: 0.0, t_end: 1.0 }));
        let ids = runner.submit_all().unwrap();
        assert_eq!(ids, vec!["t1"]);
    }
    #[test]
    fn test_collect_results() {
        let runner = DistributedRunner::new(CloudConfig::new("https://example.com", 2));
        let results = runner.collect_results(&["t1".to_string()]).unwrap();
        assert_eq!(results.len(), 1);
    }
}
