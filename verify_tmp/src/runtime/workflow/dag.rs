//! Workflow DAG — directed acyclic graph of simulation tasks.
//!
//! Defines the core data structures for representing a computation
//! workflow as a DAG of tasks connected by typed edges. Supports
//! topological ordering, critical path analysis, parallel stage
//! decomposition, and validation.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::core::error::SimError;

/// Classifies the type of data flowing along an edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeDataType {
    /// Continuous or discrete numerical data.
    Data,
    /// Signal propagation between blocks.
    Signal,
    /// Event-driven trigger or notification.
    Event,
    /// Control flow dependency (e.g. sequencing).
    Control,
}

/// A single unit of work within a workflow DAG.
#[derive(Debug, Clone)]
pub struct WorkflowTask {
    /// Unique identifier within the DAG.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Optional reference to a simulation block.
    pub block_id: Option<String>,
    /// Scheduling priority (lower = higher priority).
    pub priority: u32,
    /// Estimated computational cost for critical-path analysis.
    pub estimated_cost: f64,
}

/// A directed dependency edge between two workflow tasks.
#[derive(Debug, Clone)]
pub struct WorkflowEdge {
    /// Source task identifier.
    pub source: String,
    /// Destination task identifier.
    pub destination: String,
    /// Classification of the data flowing along this edge.
    pub data_type: EdgeDataType,
    /// Optional propagation delay.
    pub delay: Option<f64>,
}

/// A directed acyclic graph of workflow tasks.
#[derive(Debug, Clone)]
pub struct WorkflowDAG {
    /// Human-readable name for this workflow.
    pub name: String,
    /// Tasks keyed by their unique identifier.
    pub tasks: HashMap<String, WorkflowTask>,
    /// Ordered list of dependency edges.
    pub edges: Vec<WorkflowEdge>,
}

impl WorkflowDAG {
    /// Create a new empty workflow DAG with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            tasks: HashMap::new(),
            edges: Vec::new(),
        }
    }

    /// Add a task to the DAG.
    ///
    /// Returns an error if a task with the same `id` already exists.
    pub fn add_task(&mut self, task: WorkflowTask) -> Result<(), SimError> {
        if self.tasks.contains_key(&task.id) {
            return Err(SimError::new(
                crate::core::error::ErrorCode::DuplicateBlockId,
                format!("task '{}' already exists in DAG '{}'", task.id, self.name),
            ));
        }
        self.tasks.insert(task.id.clone(), task);
        Ok(())
    }

    /// Remove a task and all edges referencing it.
    pub fn remove_task(&mut self, id: &str) {
        self.tasks.remove(id);
        self.edges.retain(|e| e.source != id && e.destination != id);
    }

    /// Add a dependency edge between two tasks.
    ///
    /// Returns an error if either endpoint does not exist in the DAG.
    pub fn add_edge(&mut self, edge: WorkflowEdge) -> Result<(), SimError> {
        if !self.tasks.contains_key(&edge.source) {
            return Err(SimError::runtime(format!(
                "edge source '{}' is not a task in DAG '{}'",
                edge.source, self.name
            )));
        }
        if !self.tasks.contains_key(&edge.destination) {
            return Err(SimError::runtime(format!(
                "edge destination '{}' is not a task in DAG '{}'",
                edge.destination, self.name
            )));
        }
        self.edges.push(edge);
        Ok(())
    }

    /// Remove edges matching the given source and destination.
    pub fn remove_edge(&mut self, source: &str, dest: &str) {
        self.edges
            .retain(|e| e.source != source || e.destination != dest);
    }

    /// Get an immutable reference to a task by ID.
    pub fn get_task(&self, id: &str) -> Option<&WorkflowTask> {
        self.tasks.get(id)
    }

    /// Get a mutable reference to a task by ID.
    pub fn get_task_mut(&mut self, id: &str) -> Option<&mut WorkflowTask> {
        self.tasks.get_mut(id)
    }

    /// Return the number of tasks in the DAG.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Return the number of edges in the DAG.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Returns `true` if the DAG contains no tasks.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Perform a topological sort using Kahn's algorithm.
    ///
    /// Returns `Ok(sorted_ids)` on success, or `Err(cycle_nodes)` listing
    /// the task IDs involved in one or more cycles.
    pub fn topological_sort(&self) -> Result<Vec<String>, Vec<String>> {
        // Build in-degree map and adjacency list
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

        for task_id in self.tasks.keys() {
            in_degree.entry(task_id.as_str()).or_insert(0);
            adjacency.entry(task_id.as_str()).or_default();
        }

        for edge in &self.edges {
            if self.tasks.contains_key(&edge.source) && self.tasks.contains_key(&edge.destination) {
                adjacency
                    .get_mut(edge.source.as_str())
                    .expect("source task must exist")
                    .push(edge.destination.as_str());
                *in_degree
                    .get_mut(edge.destination.as_str())
                    .expect("destination task must exist") += 1;
            }
        }

        // Collect nodes with in-degree 0
        let mut queue: VecDeque<&str> = VecDeque::new();
        for (id, degree) in &in_degree {
            if *degree == 0 {
                queue.push_back(id);
            }
        }

        let mut sorted: Vec<String> = Vec::with_capacity(self.tasks.len());
        while let Some(node) = queue.pop_front() {
            sorted.push(node.to_string());
            if let Some(neighbors) = adjacency.get(node) {
                for neighbor in neighbors {
                    let degree = in_degree.get_mut(neighbor).expect("neighbor must exist");
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        if sorted.len() == self.tasks.len() {
            Ok(sorted)
        } else {
            // Remaining nodes (still with in-degree > 0) are in cycles
            let mut cycle_nodes: Vec<String> = in_degree
                .iter()
                .filter(|(_, d)| **d > 0)
                .map(|(id, _)| id.to_string())
                .collect();
            cycle_nodes.sort();
            Err(cycle_nodes)
        }
    }

    /// Group tasks into parallelisable stages by dependency depth.
    ///
    /// Tasks at the same depth have no inter-dependencies and can
    /// execute in parallel. Returns a vector of stages, where each
    /// stage is a group of task IDs at the same topological level.
    ///
    /// Returns an empty vector if the DAG contains a cycle.
    pub fn parallel_stages(&self) -> Vec<Vec<String>> {
        let order = match self.topological_sort() {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };

        // Assign each node a depth = max(predecessor depths) + 1
        let mut depths: HashMap<&str, usize> = HashMap::new();
        for task_id in &order {
            let pred_depth = self
                .predecessors(task_id)
                .iter()
                .map(|p| depths.get(p.id.as_str()).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            depths.insert(task_id.as_str(), pred_depth + 1);
        }

        let max_depth = depths.values().copied().max().unwrap_or(0);
        if max_depth == 0 {
            return Vec::new();
        }

        let mut stages: Vec<Vec<String>> = vec![Vec::new(); max_depth];
        for (task_id, depth) in &depths {
            stages[depth - 1].push((*task_id).to_string());
        }

        stages
    }

    /// Compute the critical path — the sequence of tasks with the
    /// highest total estimated cost.
    ///
    /// Returns an empty vector if the DAG contains a cycle.
    pub fn critical_path(&self) -> Vec<String> {
        let order = match self.topological_sort() {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };

        // DP: for each node, track the maximum accumulated cost and its predecessor
        let mut cost_to: HashMap<&str, f64> = HashMap::new();
        let mut predecessor: HashMap<&str, Option<String>> = HashMap::new();

        for task_id in &order {
            if let Some(task) = self.tasks.get(task_id.as_str()) {
                cost_to.insert(task_id.as_str(), task.estimated_cost);
                predecessor.insert(task_id.as_str(), None);

                // Find the predecessor that maximises cost to this node
                let preds = self.predecessors(task_id);
                for pred in &preds {
                    let pred_cost = cost_to.get(pred.id.as_str()).copied().unwrap_or(0.0);
                    let candidate = pred_cost + task.estimated_cost;
                    let current = cost_to[task_id.as_str()];
                    if candidate > current {
                        cost_to.insert(task_id.as_str(), candidate);
                        predecessor.insert(task_id.as_str(), Some(pred.id.clone()));
                    }
                }
            }
        }

        // Find the node with the highest accumulated cost
        let best_node = order
            .iter()
            .max_by(|a, b| {
                cost_to
                    .get(a.as_str())
                    .unwrap_or(&0.0)
                    .partial_cmp(cost_to.get(b.as_str()).unwrap_or(&0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| s.as_str())
            .unwrap_or("");

        if best_node.is_empty() {
            return Vec::new();
        }

        // Backtrack to reconstruct the critical path
        let mut path: Vec<String> = Vec::new();
        let mut current = Some(best_node.to_string());
        while let Some(node) = current {
            path.push(node.clone());
            current = predecessor
                .get(node.as_str())
                .and_then(|p| p.as_ref().cloned());
        }
        path.reverse();
        path
    }

    /// Get all direct predecessors (incoming edges) of a task.
    pub fn predecessors(&self, task_id: &str) -> Vec<&WorkflowTask> {
        self.edges
            .iter()
            .filter(|e| e.destination == task_id && self.tasks.contains_key(&e.source))
            .filter_map(|e| self.tasks.get(&e.source))
            .collect()
    }

    /// Get all direct successors (outgoing edges) of a task.
    pub fn successors(&self, task_id: &str) -> Vec<&WorkflowTask> {
        self.edges
            .iter()
            .filter(|e| e.source == task_id && self.tasks.contains_key(&e.destination))
            .filter_map(|e| self.tasks.get(&e.destination))
            .collect()
    }

    /// Validate the DAG structure.
    ///
    /// Checks for:
    /// - Cycles (topological sort failure)
    /// - Unreachable tasks (no path from any source)
    /// - Dangling edges (referencing non-existent tasks)
    ///
    /// Returns `Ok(())` on success or `Err(errors)` with a list of
    /// human-readable error messages.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors: Vec<String> = Vec::new();

        // Check for cycles
        if let Err(cycle_ids) = self.topological_sort() {
            errors.push(format!(
                "cycle detected involving tasks: {}",
                cycle_ids.join(", ")
            ));
        }

        // Identify source nodes (no incoming edges)
        let has_incoming: HashSet<&str> =
            self.edges.iter().map(|e| e.destination.as_str()).collect();
        let sources: Vec<&str> = self
            .tasks
            .keys()
            .filter(|id| !has_incoming.contains(id.as_str()))
            .map(|id| id.as_str())
            .collect();

        if sources.is_empty() && !self.tasks.is_empty() {
            errors.push("no source tasks found (every task has an incoming edge)".to_string());
        }

        // BFS from all sources to find unreachable tasks
        let mut reachable: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<&str> = sources.iter().copied().collect();
        while let Some(node) = queue.pop_front() {
            if !reachable.insert(node) {
                continue;
            }
            for edge in &self.edges {
                if edge.source == node {
                    queue.push_back(edge.destination.as_str());
                }
            }
        }

        for task_id in self.tasks.keys() {
            if !reachable.contains(task_id.as_str()) {
                errors.push(format!(
                    "task '{}' is not reachable from any source task",
                    task_id
                ));
            }
        }

        // Check for dangling edges
        for edge in &self.edges {
            if !self.tasks.contains_key(&edge.source) {
                errors.push(format!(
                    "edge references non-existent source task '{}'",
                    edge.source
                ));
            }
            if !self.tasks.contains_key(&edge.destination) {
                errors.push(format!(
                    "edge references non-existent destination task '{}'",
                    edge.destination
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for WorkflowDAG {
    fn default() -> Self {
        Self::new("default")
    }
}

/// Helper to construct a simple linear DAG for testing.
#[cfg(test)]
fn make_linear_dag() -> WorkflowDAG {
    let mut dag = WorkflowDAG::new("linear");
    dag.add_task(WorkflowTask {
        id: "t1".into(),
        name: "Task 1".into(),
        block_id: None,
        priority: 1,
        estimated_cost: 1.0,
    })
    .unwrap();
    dag.add_task(WorkflowTask {
        id: "t2".into(),
        name: "Task 2".into(),
        block_id: None,
        priority: 2,
        estimated_cost: 2.0,
    })
    .unwrap();
    dag.add_task(WorkflowTask {
        id: "t3".into(),
        name: "Task 3".into(),
        block_id: None,
        priority: 3,
        estimated_cost: 3.0,
    })
    .unwrap();
    dag.add_edge(WorkflowEdge {
        source: "t1".into(),
        destination: "t2".into(),
        data_type: EdgeDataType::Data,
        delay: None,
    })
    .unwrap();
    dag.add_edge(WorkflowEdge {
        source: "t2".into(),
        destination: "t3".into(),
        data_type: EdgeDataType::Data,
        delay: None,
    })
    .unwrap();
    dag
}

/// Helper to construct a branching DAG for testing.
#[cfg(test)]
fn make_branching_dag() -> WorkflowDAG {
    let mut dag = WorkflowDAG::new("branching");
    for i in 1..=6 {
        dag.add_task(WorkflowTask {
            id: format!("t{i}"),
            name: format!("Task {i}"),
            block_id: None,
            priority: i,
            estimated_cost: i as f64,
        })
        .unwrap();
    }
    // t1 -> t2, t1 -> t3
    dag.add_edge(WorkflowEdge {
        source: "t1".into(),
        destination: "t2".into(),
        data_type: EdgeDataType::Data,
        delay: None,
    })
    .unwrap();
    dag.add_edge(WorkflowEdge {
        source: "t1".into(),
        destination: "t3".into(),
        data_type: EdgeDataType::Data,
        delay: None,
    })
    .unwrap();
    // t2 -> t4, t3 -> t4
    dag.add_edge(WorkflowEdge {
        source: "t2".into(),
        destination: "t4".into(),
        data_type: EdgeDataType::Data,
        delay: None,
    })
    .unwrap();
    dag.add_edge(WorkflowEdge {
        source: "t3".into(),
        destination: "t4".into(),
        data_type: EdgeDataType::Data,
        delay: None,
    })
    .unwrap();
    // t4 -> t5, t4 -> t6
    dag.add_edge(WorkflowEdge {
        source: "t4".into(),
        destination: "t5".into(),
        data_type: EdgeDataType::Data,
        delay: None,
    })
    .unwrap();
    dag.add_edge(WorkflowEdge {
        source: "t4".into(),
        destination: "t6".into(),
        data_type: EdgeDataType::Data,
        delay: None,
    })
    .unwrap();
    dag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_creation() {
        let dag = WorkflowDAG::new("test_dag");
        assert_eq!(dag.name, "test_dag");
        assert!(dag.is_empty());
        assert_eq!(dag.task_count(), 0);
        assert_eq!(dag.edge_count(), 0);
    }

    #[test]
    fn test_dag_default() {
        let dag = WorkflowDAG::default();
        assert_eq!(dag.name, "default");
        assert!(dag.is_empty());
    }

    #[test]
    fn test_add_remove_tasks() {
        let mut dag = WorkflowDAG::new("test");
        let task = WorkflowTask {
            id: "a".into(),
            name: "Alpha".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        };
        assert!(dag.add_task(task).is_ok());
        assert_eq!(dag.task_count(), 1);

        // Duplicate ID should fail
        let dup = WorkflowTask {
            id: "a".into(),
            name: "Duplicate".into(),
            block_id: None,
            priority: 2,
            estimated_cost: 2.0,
        };
        let result = dag.add_task(dup);
        assert!(result.is_err());

        // Remove and verify
        dag.remove_task("a");
        assert_eq!(dag.task_count(), 0);
    }

    #[test]
    fn test_add_remove_edges() {
        let mut dag = WorkflowDAG::new("test");
        dag.add_task(WorkflowTask {
            id: "a".into(),
            name: "A".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "b".into(),
            name: "B".into(),
            block_id: None,
            priority: 2,
            estimated_cost: 2.0,
        })
        .unwrap();

        let edge = WorkflowEdge {
            source: "a".into(),
            destination: "b".into(),
            data_type: EdgeDataType::Data,
            delay: None,
        };
        assert!(dag.add_edge(edge).is_ok());
        assert_eq!(dag.edge_count(), 1);

        dag.remove_edge("a", "b");
        assert_eq!(dag.edge_count(), 0);
    }

    #[test]
    fn test_add_edge_missing_task() {
        let mut dag = WorkflowDAG::new("test");
        let edge = WorkflowEdge {
            source: "missing".into(),
            destination: "also_missing".into(),
            data_type: EdgeDataType::Control,
            delay: None,
        };
        assert!(dag.add_edge(edge).is_err());
    }

    #[test]
    fn test_topological_sort_linear() {
        let dag = make_linear_dag();
        let sorted = dag.topological_sort().expect("linear DAG must sort");
        assert_eq!(sorted, vec!["t1", "t2", "t3"]);
    }

    #[test]
    fn test_topological_sort_branching() {
        let dag = make_branching_dag();
        let sorted = dag.topological_sort().expect("branching DAG must sort");
        // t1 must come first, t5 and t6 must come after t4
        assert_eq!(sorted[0], "t1");
        assert!(
            sorted.iter().position(|id| id == "t2").unwrap()
                < sorted.iter().position(|id| id == "t4").unwrap()
        );
        assert!(
            sorted.iter().position(|id| id == "t3").unwrap()
                < sorted.iter().position(|id| id == "t4").unwrap()
        );
        assert!(
            sorted.iter().position(|id| id == "t4").unwrap()
                < sorted.iter().position(|id| id == "t5").unwrap()
        );
        assert!(
            sorted.iter().position(|id| id == "t4").unwrap()
                < sorted.iter().position(|id| id == "t6").unwrap()
        );
    }

    #[test]
    fn test_cycle_detection() {
        let mut dag = WorkflowDAG::new("cycle");
        dag.add_task(WorkflowTask {
            id: "a".into(),
            name: "A".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "b".into(),
            name: "B".into(),
            block_id: None,
            priority: 2,
            estimated_cost: 2.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "c".into(),
            name: "C".into(),
            block_id: None,
            priority: 3,
            estimated_cost: 3.0,
        })
        .unwrap();

        // a -> b -> c -> a (cycle)
        dag.add_edge(WorkflowEdge {
            source: "a".into(),
            destination: "b".into(),
            data_type: EdgeDataType::Control,
            delay: None,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "b".into(),
            destination: "c".into(),
            data_type: EdgeDataType::Control,
            delay: None,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "c".into(),
            destination: "a".into(),
            data_type: EdgeDataType::Control,
            delay: None,
        })
        .unwrap();

        let result = dag.topological_sort();
        assert!(result.is_err());
        let cycle_nodes = result.unwrap_err();
        assert_eq!(cycle_nodes.len(), 3);
        assert!(cycle_nodes.contains(&"a".to_string()));
        assert!(cycle_nodes.contains(&"b".to_string()));
        assert!(cycle_nodes.contains(&"c".to_string()));
    }

    #[test]
    fn test_parallel_stages() {
        let dag = make_branching_dag();
        let stages = dag.parallel_stages();
        // Expected: stage 0 = [t1], stage 1 = [t2, t3], stage 2 = [t4], stage 3 = [t5, t6]
        assert_eq!(stages.len(), 4);
        assert_eq!(stages[0], vec!["t1"]);
        assert_eq!(stages[1].len(), 2);
        assert!(stages[1].contains(&"t2".to_string()));
        assert!(stages[1].contains(&"t3".to_string()));
        assert_eq!(stages[2], vec!["t4"]);
        assert_eq!(stages[3].len(), 2);
        assert!(stages[3].contains(&"t5".to_string()));
        assert!(stages[3].contains(&"t6".to_string()));
    }

    #[test]
    fn test_parallel_stages_cycle() {
        let mut dag = WorkflowDAG::new("cycle");
        dag.add_task(WorkflowTask {
            id: "a".into(),
            name: "A".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "b".into(),
            name: "B".into(),
            block_id: None,
            priority: 2,
            estimated_cost: 2.0,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "a".into(),
            destination: "b".into(),
            data_type: EdgeDataType::Data,
            delay: None,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "b".into(),
            destination: "a".into(),
            data_type: EdgeDataType::Data,
            delay: None,
        })
        .unwrap();

        let stages = dag.parallel_stages();
        assert!(stages.is_empty());
    }

    #[test]
    fn test_critical_path() {
        let dag = make_branching_dag();
        let path = dag.critical_path();
        // t1 (1) -> t3 (3) -> t4 (4) -> t6 (6) = 14 (or t5=5)
        // The longest path has total cost: 1+3+4+6=14
        // t1 -> t2 -> t4 -> t6 = 1+2+4+6=13
        // t1 -> t3 -> t4 -> t5 = 1+3+4+5=13
        // So critical path should be t1 -> t3 -> t4 -> t6
        assert!(!path.is_empty());
        assert_eq!(path[0], "t1");
        assert!(path.contains(&"t4".to_string()));
    }

    #[test]
    fn test_critical_path_cycle() {
        let mut dag = WorkflowDAG::new("cycle");
        dag.add_task(WorkflowTask {
            id: "a".into(),
            name: "A".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "b".into(),
            name: "B".into(),
            block_id: None,
            priority: 2,
            estimated_cost: 2.0,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "a".into(),
            destination: "b".into(),
            data_type: EdgeDataType::Data,
            delay: None,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "b".into(),
            destination: "a".into(),
            data_type: EdgeDataType::Data,
            delay: None,
        })
        .unwrap();

        let path = dag.critical_path();
        assert!(path.is_empty());
    }

    #[test]
    fn test_predecessors_successors() {
        let dag = make_branching_dag();
        let preds = dag.predecessors("t4");
        assert_eq!(preds.len(), 2);
        let pred_ids: Vec<&str> = preds.iter().map(|t| t.id.as_str()).collect();
        assert!(pred_ids.contains(&"t2"));
        assert!(pred_ids.contains(&"t3"));

        let succs = dag.successors("t1");
        assert_eq!(succs.len(), 2);
        let succ_ids: Vec<&str> = succs.iter().map(|t| t.id.as_str()).collect();
        assert!(succ_ids.contains(&"t2"));
        assert!(succ_ids.contains(&"t3"));
    }

    #[test]
    fn test_validate_valid() {
        let dag = make_linear_dag();
        assert!(dag.validate().is_ok());

        let dag2 = make_branching_dag();
        assert!(dag2.validate().is_ok());
    }

    #[test]
    fn test_validate_empty() {
        let dag = WorkflowDAG::new("empty");
        assert!(dag.validate().is_ok());
    }

    #[test]
    fn test_validate_cycle() {
        let mut dag = WorkflowDAG::new("cycle");
        dag.add_task(WorkflowTask {
            id: "a".into(),
            name: "A".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "b".into(),
            name: "B".into(),
            block_id: None,
            priority: 2,
            estimated_cost: 2.0,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "a".into(),
            destination: "b".into(),
            data_type: EdgeDataType::Control,
            delay: None,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "b".into(),
            destination: "a".into(),
            data_type: EdgeDataType::Control,
            delay: None,
        })
        .unwrap();

        let result = dag.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("cycle")));
    }

    #[test]
    fn test_validate_dangling_edge() {
        let mut dag = WorkflowDAG::new("dangling");
        dag.add_task(WorkflowTask {
            id: "a".into(),
            name: "A".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();
        // Manually push an edge to a non-existent task (bypass add_edge validation)
        dag.edges.push(WorkflowEdge {
            source: "a".into(),
            destination: "ghost".into(),
            data_type: EdgeDataType::Data,
            delay: None,
        });

        let result = dag.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("ghost")));
    }

    #[test]
    fn test_remove_task_also_removes_edges() {
        let mut dag = WorkflowDAG::new("cleanup");
        dag.add_task(WorkflowTask {
            id: "a".into(),
            name: "A".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();
        dag.add_task(WorkflowTask {
            id: "b".into(),
            name: "B".into(),
            block_id: None,
            priority: 2,
            estimated_cost: 2.0,
        })
        .unwrap();
        dag.add_edge(WorkflowEdge {
            source: "a".into(),
            destination: "b".into(),
            data_type: EdgeDataType::Signal,
            delay: None,
        })
        .unwrap();

        assert_eq!(dag.edge_count(), 1);
        dag.remove_task("a");
        assert_eq!(dag.edge_count(), 0);
        assert_eq!(dag.task_count(), 1);

        // Also remove the remaining task
        dag.remove_task("b");
        assert_eq!(dag.task_count(), 0);
    }

    #[test]
    fn test_get_task() {
        let mut dag = WorkflowDAG::new("getter");
        dag.add_task(WorkflowTask {
            id: "x".into(),
            name: "X".into(),
            block_id: Some("block_x".into()),
            priority: 5,
            estimated_cost: 10.0,
        })
        .unwrap();

        let task = dag.get_task("x").expect("task should exist");
        assert_eq!(task.name, "X");
        assert_eq!(task.block_id.as_deref(), Some("block_x"));

        let task_mut = dag.get_task_mut("x").expect("mutable task");
        task_mut.priority = 10;
        assert_eq!(dag.get_task("x").unwrap().priority, 10);
    }

    #[test]
    fn test_tasks_without_block_id() {
        let mut dag = WorkflowDAG::new("abstract");
        dag.add_task(WorkflowTask {
            id: "custom".into(),
            name: "Custom Logic".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 5.0,
        })
        .unwrap();

        let task = dag.get_task("custom").unwrap();
        assert!(task.block_id.is_none());
    }

    #[test]
    fn test_single_node_topological_sort() {
        let mut dag = WorkflowDAG::new("single");
        dag.add_task(WorkflowTask {
            id: "only".into(),
            name: "Only Task".into(),
            block_id: None,
            priority: 1,
            estimated_cost: 1.0,
        })
        .unwrap();

        let sorted = dag.topological_sort().expect("single node must sort");
        assert_eq!(sorted, vec!["only"]);
    }
}
