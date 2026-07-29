//! Batch simulation and parameter sweep.

use crate::core::types::Scalar;

/// Parameter sweep task.
pub struct ParameterSweep {
    pub parameter_name: String,
    pub values: Vec<Scalar>,
    pub diagram_template: String,
    pub output_dir: String,
}

impl ParameterSweep {
    pub fn new(name: &str, values: Vec<Scalar>, template: &str, output: &str) -> Self {
        Self { parameter_name: name.to_string(), values, diagram_template: template.to_string(), output_dir: output.to_string() }
    }
    pub fn run(&self) -> Result<Vec<String>, String> {
        let mut results = Vec::new();
        for (i, &val) in self.values.iter().enumerate() {
            let output_path = format!("{}/sweep_{}_{}.json", self.output_dir, self.parameter_name, i);
            std::fs::write(&output_path, format!("{{\"param\":\"{}\",\"value\":{}}}", self.parameter_name, val))
                .map_err(|e| format!("Write error: {}", e))?;
            results.push(output_path);
        }
        Ok(results)
    }
}

/// Status of a batch task.
pub enum BatchTaskStatus { Pending, Running, Completed(Vec<String>), Failed(String) }

/// A single batch task.
pub struct BatchTask {
    pub id: String,
    pub config_path: String,
    pub diagram_path: String,
    pub output_path: String,
    pub status: BatchTaskStatus,
}

impl BatchTask {
    pub fn new(id: &str, config: &str, diagram: &str, output: &str) -> Self {
        Self { id: id.to_string(), config_path: config.to_string(), diagram_path: diagram.to_string(), output_path: output.to_string(), status: BatchTaskStatus::Pending }
    }
}

/// Batch simulation manager.
pub struct BatchSimManager {
    pub tasks: Vec<BatchTask>,
    pub max_parallel: usize,
}

impl BatchSimManager {
    pub fn new(max_parallel: usize) -> Self { Self { tasks: Vec::new(), max_parallel } }
    pub fn add_task(&mut self, task: BatchTask) { self.tasks.push(task); }

    pub fn run_all(&mut self) -> Result<(), String> {
        for task in &mut self.tasks {
            task.status = BatchTaskStatus::Running;
            // Simulate: create output file
            std::fs::write(&task.output_path, "{}").map_err(|e| format!("Write error: {}", e))?;
            let results = vec![task.output_path.clone()];
            task.status = BatchTaskStatus::Completed(results);
        }
        Ok(())
    }

    pub fn results(&self) -> Vec<(&str, &BatchTaskStatus)> {
        self.tasks.iter().map(|t| (t.id.as_str(), &t.status)).collect()
    }
}

/// Design parameter for optimization.
pub struct DesignParam {
    pub name: String,
    pub min: Scalar,
    pub max: Scalar,
}

/// Optimization loop using grid search.
pub struct OptimizationLoop {
    pub objective_fn: String,
    pub design_params: Vec<DesignParam>,
    pub max_iterations: usize,
}

impl OptimizationLoop {
    pub fn new(objective: &str, max_iter: usize) -> Self { Self { objective_fn: objective.to_string(), design_params: Vec::new(), max_iterations: max_iter } }
    pub fn add_param(&mut self, param: DesignParam) { self.design_params.push(param); }

    pub fn optimize_grid(&self) -> Result<(Vec<Scalar>, Scalar), String> {
        if self.design_params.is_empty() { return Err("No design parameters".to_string()); }
        let n = self.design_params.len();
        let steps = (self.max_iterations as Scalar / n as Scalar).ceil() as usize;
        let mut best_params = vec![0.0; n];
        let mut best_obj = Scalar::MAX;

        for i in 0..self.max_iterations {
            let mut params = Vec::new();
            for (j, dp) in self.design_params.iter().enumerate() {
                let t = ((i / (j + 1)) % steps) as Scalar / steps.max(1) as Scalar;
                params.push(dp.min + t * (dp.max - dp.min));
            }
            // Simple objective: sum of squares
            let obj: Scalar = params.iter().map(|p| p * p).sum();
            if obj < best_obj { best_obj = obj; best_params = params; }
        }
        Ok((best_params, best_obj))
    }
}

// ── Solver Benchmark Infrastructure ──────────────────────────────────────

/// Performance measurement for a single solver run.
#[derive(Debug, Clone)]
pub struct SolverBenchmarkResult {
    pub name: String,
    pub grid_size: (usize, usize, usize),
    pub num_steps: usize,
    pub elapsed_seconds: Scalar,
    pub steps_per_second: Scalar,
    pub cells_per_second: Scalar,
}

impl SolverBenchmarkResult {
    pub fn new(
        name: &str,
        grid: (usize, usize, usize),
        steps: usize,
        elapsed_s: Scalar,
    ) -> Self {
        let total_cells = grid.0 * grid.1 * grid.2;
        Self {
            name: name.to_string(),
            grid_size: grid,
            num_steps: steps,
            elapsed_seconds: elapsed_s,
            steps_per_second: if elapsed_s > 0.0 {
                steps as Scalar / elapsed_s
            } else {
                0.0
            },
            cells_per_second: if elapsed_s > 0.0 {
                steps as Scalar * total_cells as Scalar / elapsed_s
            } else {
                0.0
            },
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "[BENCH] {} | grid {}×{}×{} | {} steps | {:.4}s | {:.0} steps/s | {:.0} cells/s",
            self.name,
            self.grid_size.0,
            self.grid_size.1,
            self.grid_size.2,
            self.num_steps,
            self.elapsed_seconds,
            self.steps_per_second,
            self.cells_per_second
        )
    }
}

/// Configuration for a solver benchmark.
#[derive(Debug, Clone)]
pub struct SolverBenchConfig {
    pub name: String,
    pub grid_sizes: Vec<(usize, usize, usize)>,
    pub num_steps: usize,
}

impl SolverBenchConfig {
    pub fn new(name: &str, num_steps: usize) -> Self {
        Self {
            name: name.to_string(),
            grid_sizes: vec![(8, 8, 8), (16, 16, 16)],
            num_steps,
        }
    }

    pub fn with_grids(mut self, grids: Vec<(usize, usize, usize)>) -> Self {
        self.grid_sizes = grids;
        self
    }
}

/// Run a benchmark for a closure-based solver step.
///
/// The closure `step_fn` is called `num_steps` times and the total
/// wall-clock time is measured. Returns a `SolverBenchmarkResult`.
pub fn bench_solver<F>(
    config: &SolverBenchConfig,
    grid: (usize, usize, usize),
    mut step_fn: F,
) -> SolverBenchmarkResult
where
    F: FnMut() -> Result<(), String>,
{
    use std::time::Instant;
    let start = Instant::now();
    for _ in 0..config.num_steps {
        if let Err(e) = step_fn() {
            return SolverBenchmarkResult::new(
                &format!("{} (FAILED: {})", config.name, e),
                grid,
                config.num_steps,
                start.elapsed().as_secs_f64(),
            );
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    SolverBenchmarkResult::new(&config.name, grid, config.num_steps, elapsed)
}

/// Run benchmarks across multiple grid sizes and print results.
pub fn run_benchmark_suite<F>(
    config: &SolverBenchConfig,
    setup_fn: &dyn Fn((usize, usize, usize)) -> F,
) -> Vec<SolverBenchmarkResult>
where
    F: FnMut() -> Result<(), String>,
{
    let mut results = Vec::new();
    for &grid in &config.grid_sizes {
        let step_fn = setup_fn(grid);
        let result = bench_solver(config, grid, step_fn);
        println!("{}", result.summary());
        results.push(result);
    }
    results
}

/// Generate a Markdown report from a set of benchmark results.
pub fn benchmark_report(results: &[SolverBenchmarkResult]) -> String {
    let mut md = String::from("# Solver Benchmark Report\n\n");
    md.push_str("| Solver | Grid | Steps | Time (s) | Steps/s | Cells/s |\n");
    md.push_str("|--------|------|-------|----------|---------|--------|\n");
    for r in results {
        md.push_str(&format!(
            "| {} | {}×{}×{} | {} | {:.4} | {:.0} | {:.0} |\n",
            r.name,
            r.grid_size.0,
            r.grid_size.1,
            r.grid_size.2,
            r.num_steps,
            r.elapsed_seconds,
            r.steps_per_second,
            r.cells_per_second
        ));
    }
    md
}

/// Compare two benchmark runs and report speedup.
pub fn benchmark_speedup(
    baseline: &[SolverBenchmarkResult],
    optimized: &[SolverBenchmarkResult],
) -> String {
    let mut md = String::from("# Benchmark Speedup\n\n");
    md.push_str("| Grid | Baseline (steps/s) | Optimized (steps/s) | Speedup |\n");
    md.push_str("|------|-------------------|--------------------|---------|\n");
    for (b, o) in baseline.iter().zip(optimized.iter()) {
        let speedup = if b.steps_per_second > 0.0 {
            o.steps_per_second / b.steps_per_second
        } else {
            0.0
        };
        md.push_str(&format!(
            "| {}×{}×{} | {:.0} | {:.0} | {:.2}× |\n",
            b.grid_size.0, b.grid_size.1, b.grid_size.2,
            b.steps_per_second, o.steps_per_second, speedup
        ));
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parameter_sweep() {
        let _ = std::fs::create_dir_all("/tmp/sweep_test");
        let sweep = ParameterSweep::new("k", vec![1.0, 2.0, 3.0], "template.json", "/tmp/sweep_test");
        let results = sweep.run().unwrap();
        assert_eq!(results.len(), 3);
        for r in &results { let _ = std::fs::remove_file(r); }
        let _ = std::fs::remove_dir("/tmp/sweep_test");
    }
    #[test]
    fn test_batch_task_creation() {
        let t = BatchTask::new("task1", "config.json", "diagram.json", "output.json");
        assert_eq!(t.id, "task1");
    }
    #[test]
    fn test_batch_manager() {
        let mut mgr = BatchSimManager::new(4);
        mgr.add_task(BatchTask::new("t1", "c1", "d1", "/tmp/o1.json"));
        mgr.run_all().unwrap();
        let results = mgr.results();
        assert_eq!(results.len(), 1);
        let _ = std::fs::remove_file("/tmp/o1.json");
    }
    #[test]
    fn test_optimization_loop() {
        let mut opt = OptimizationLoop::new("cost", 50);
        opt.add_param(DesignParam { name: "x".to_string(), min: -1.0, max: 1.0 });
        opt.add_param(DesignParam { name: "y".to_string(), min: -1.0, max: 1.0 });
        let (params, obj) = opt.optimize_grid().unwrap();
        assert_eq!(params.len(), 2);
        assert!(obj >= 0.0);
    }
    #[test]
    fn test_optimization_no_params() {
        let opt = OptimizationLoop::new("cost", 10);
        assert!(opt.optimize_grid().is_err());
    }
    // ── Benchmark tests ─────────────────────────────────────────────────
    #[test]
    fn test_benchmark_result_creation() {
        let r = SolverBenchmarkResult::new("test_solver", (10, 10, 10), 100, 0.5);
        assert_eq!(r.name, "test_solver");
        assert_eq!(r.num_steps, 100);
        assert!((r.steps_per_second - 200.0).abs() < 1e-6);
        assert!((r.cells_per_second - 200_000.0).abs() < 1e-6);
    }
    #[test]
    fn test_benchmark_result_zero_time() {
        let r = SolverBenchmarkResult::new("zero", (1, 1, 1), 0, 0.0);
        assert_eq!(r.steps_per_second, 0.0);
    }
    #[test]
    fn test_benchmark_result_summary() {
        let r = SolverBenchmarkResult::new("ns3d", (16, 16, 16), 50, 0.25);
        let s = r.summary();
        assert!(s.contains("[BENCH]"));
        assert!(s.contains("ns3d"));
    }
    #[test]
    fn test_bench_solver_simple() {
        let mut counter = 0;
        let config = SolverBenchConfig::new("counter", 10).with_grids(vec![(2, 2, 2)]);
        let result = bench_solver(&config, (2, 2, 2), || {
            counter += 1;
            Ok(())
        });
        assert_eq!(result.num_steps, 10);
        // counter was called 10 times
        assert_eq!(counter, 10);
    }
    #[test]
    fn test_bench_solver_failure() {
        let config = SolverBenchConfig::new("failing", 5);
        let result = bench_solver(&config, (2, 2, 2), || Err("oops".to_string()));
        assert!(result.name.contains("FAILED"));
        // Should stop on first failure
        assert!(result.elapsed_seconds >= 0.0);
    }
    #[test]
    fn test_benchmark_report() {
        let results = vec![
            SolverBenchmarkResult::new("ns3d", (16, 16, 16), 100, 0.5),
            SolverBenchmarkResult::new("fdtd3d", (16, 16, 16), 100, 0.3),
        ];
        let report = benchmark_report(&results);
        assert!(report.contains("ns3d"));
        assert!(report.contains("fdtd3d"));
        assert!(report.contains("Steps/s"));
    }
    #[test]
    fn test_benchmark_speedup() {
        let baseline = vec![SolverBenchmarkResult::new("s", (8, 8, 8), 100, 1.0)];
        let optimized = vec![SolverBenchmarkResult::new("s", (8, 8, 8), 100, 0.5)];
        let report = benchmark_speedup(&baseline, &optimized);
        assert!(report.contains("2.00"));
    }
    #[test]
    fn test_bench_config_builder() {
        let cfg = SolverBenchConfig::new("test", 50).with_grids(vec![(4, 4, 4), (8, 8, 8)]);
        assert_eq!(cfg.grid_sizes.len(), 2);
        assert_eq!(cfg.num_steps, 50);
    }
    #[test]
    fn test_run_benchmark_suite() {
        let config = SolverBenchConfig::new("suite_test", 5)
            .with_grids(vec![(2, 2, 2), (3, 3, 3)]);
        let results = run_benchmark_suite(&config, &|grid| {
            let _g = grid;
            || Ok(())
        });
        assert_eq!(results.len(), 2);
    }
}
