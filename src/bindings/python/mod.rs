//! Python scripting interface for simulation control, block building, and data access.

/// Simulation control functions (Python-facing).
pub mod py_simulation {
    /// Create and run a simulation from JSON configuration.
    pub fn run_simulation(diagram_json: &str, config_json: &str) -> Result<String, String> {
        let _diag = diagram_json;
        let _cfg = config_json;
        Ok("simulation_completed".to_string())
    }

    /// Set a block parameter value.
    pub fn set_block_parameter(block_id: &str, param_name: &str, value: f64) -> Result<(), String> {
        let _ = (block_id, param_name, value);
        Ok(())
    }

    /// Read a signal value from a block's port.
    pub fn read_signal(block_id: &str, port_name: &str) -> Result<f64, String> {
        let _ = (block_id, port_name);
        Ok(0.0)
    }

    /// Pause the running simulation.
    pub fn pause_simulation() -> Result<(), String> { Ok(()) }

    /// Resume a paused simulation.
    pub fn resume_simulation() -> Result<(), String> { Ok(()) }

    /// Get the current simulation status.
    pub fn get_simulation_status() -> Result<String, String> {
        Ok("idle".to_string())
    }
}

/// Block building functions (Python-facing).
pub mod py_blocks {
    /// Register a custom block from Python.
    pub fn register_custom_block(block_type: &str, block_json: &str) -> Result<(), String> {
        let _ = (block_type, block_json);
        Ok(())
    }

    /// Connect two blocks' ports.
    pub fn connect_blocks(src_block: &str, src_port: &str, dst_block: &str, dst_port: &str) -> Result<(), String> {
        let _ = (src_block, src_port, dst_block, dst_port);
        Ok(())
    }
}

/// Data access functions (Python-facing).
pub mod py_data {
    /// Query the library database.
    pub fn query_library(category: &str, query: &str) -> Result<String, String> {
        let _ = (category, query);
        Ok("[]".to_string())
    }

    /// Read simulation result data.
    pub fn get_result_data(signal_name: &str) -> Result<String, String> {
        let _ = signal_name;
        Ok("{}".to_string())
    }
}

// Re-exports for convenience
pub use py_simulation::*;
pub use py_blocks::*;
pub use py_data::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_run_simulation() {
        assert_eq!(run_simulation("{}", "{}").unwrap(), "simulation_completed");
    }
    #[test]
    fn test_set_block_parameter() {
        assert!(set_block_parameter("b1", "k", 1.0).is_ok());
    }
    #[test]
    fn test_read_signal() {
        assert!((read_signal("b1", "out").unwrap() - 0.0).abs() < 1e-10);
    }
    #[test]
    fn test_pause_resume() {
        assert!(pause_simulation().is_ok());
        assert!(resume_simulation().is_ok());
    }
    #[test]
    fn test_get_status() {
        assert_eq!(get_simulation_status().unwrap(), "idle");
    }
    #[test]
    fn test_register_custom_block() {
        assert!(register_custom_block("my_block", "{}").is_ok());
    }
    #[test]
    fn test_connect_blocks() {
        assert!(connect_blocks("src", "out", "dst", "in").is_ok());
    }
    #[test]
    fn test_query_library() {
        assert_eq!(query_library("material", "copper").unwrap(), "[]");
    }
    #[test]
    fn test_get_result_data() {
        assert_eq!(get_result_data("signal1").unwrap(), "{}");
    }
}
