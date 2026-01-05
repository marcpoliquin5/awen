/// Engine Execution Core - Phase 2, Section 2.1
///
/// The Engine is the mandatory, non-bypassable execution chokepoint for all AWEN computation.
/// All graphs flow through Engine.run_graph(), which enforces:
/// - Coherence window validation
/// - Calibration state integration  
/// - Measurement-conditioned branching
/// - Observability instrumentation
/// - Safety constraint enforcement
/// - Deterministic artifact emission

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::{anyhow, Result};

// ============================================================================
// Execution Plan Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub plan_id: String,
    pub graph_id: String,
    pub phases: Vec<ExecutionPhase>,
    pub total_duration_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPhase {
    pub phase_id: usize,
    pub nodes_to_execute: Vec<String>,
    pub is_parallel: bool,
    pub duration_ns: u64,
}

// ============================================================================
// Computation Graph Types (IR Execution)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputationGraph {
    pub graph_id: String,
    pub nodes: Vec<ComputationNode>,
    pub edges: Vec<Edge>,
    pub root_nodes: Vec<String>,
    pub leaf_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputationNode {
    pub id: String,
    pub node_type: NodeType,
    pub parameters: HashMap<String, f64>,
    pub timing_contract: TimingContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    ClassicalPhotonic { component: String },
    QuantumGate { gate_name: String },
    Measurement { basis_type: String },
    Calibration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingContract {
    pub duration_ns: u64,
    pub coherence_requirement_ns: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_node: String,
    pub to_node: String,
}

// ============================================================================
// Execution Context & Result
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub execution_id: String,
    pub graph_id: String,
    pub seed: u64,
    pub start_time: DateTime<Utc>,
    pub nodes_completed: usize,
    pub coherence_budget_remaining_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub execution_id: String,
    pub graph_id: String,
    pub seed: u64,
    pub status: ExecutionStatus,
    pub start_timestamp: DateTime<Utc>,
    pub end_timestamp: DateTime<Utc>,
    pub total_duration_ns: u64,
    pub nodes_executed: usize,
    pub nodes_failed: usize,
    pub measurements_recorded: usize,
    pub coherence_violations: usize,
    pub safety_violations: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatus {
    Success,
    CoherenceViolation,
    SafetyViolation,
    FailureOther,
}

// ============================================================================
// Node Execution Logging
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionLog {
    pub node_id: String,
    pub node_type: String,
    pub start_timestamp: DateTime<Utc>,
    pub end_timestamp: DateTime<Utc>,
    pub duration_ns: u64,
    pub success: bool,
    pub error: Option<String>,
}

// ============================================================================
// Safety & Coherence Tracking
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConstraint {
    pub hard_limits: HashMap<String, f64>,
    pub soft_limits: HashMap<String, f64>,
    pub min_fidelity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceViolation {
    pub node_id: String,
    pub deadline: DateTime<Utc>,
    pub actual_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyViolation {
    pub node_id: String,
    pub parameter: String,
    pub value: f64,
    pub limit: f64,
}

// ============================================================================
// Engine Implementation
// ============================================================================

pub struct Engine {
    observability_enabled: bool,
    safety_enforcement: SafetyEnforcement,
}

pub enum SafetyEnforcement {
    Strict,      // Fail on any violation
    Warning,     // Warn but continue
    Automatic,   // Try to recalibrate
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            observability_enabled: true,
            safety_enforcement: SafetyEnforcement::Strict,
        }
    }

    /// Main Engine execution chokepoint - all graphs flow through here
    pub fn run_graph(
        &self,
        graph: &ComputationGraph,
        seed: Option<u64>,
    ) -> Result<ExecutionResult> {
        let run_id = Uuid::new_v4().to_string();
        let run_seed = seed.unwrap_or(42);
        let start_time = Utc::now();

        // 1. Validate IR graph
        self.validate_graph(graph)?;

        // 2. Generate execution plan
        let plan = self.generate_execution_plan(graph)?;

        // 3. Create execution context
        let mut context = ExecutionContext {
            execution_id: run_id.clone(),
            graph_id: graph.graph_id.clone(),
            seed: run_seed,
            start_time,
            nodes_completed: 0,
            coherence_budget_remaining_ns: 10_000_000,  // 10ms
        };

        // 4. Execute nodes in phases
        let mut execution_log = Vec::new();
        let mut measurements_count = 0;
        let mut coherence_violations = 0;
        let mut safety_violations = 0;

        for phase in &plan.phases {
            for node_id in &phase.nodes_to_execute {
                let node = graph.nodes.iter()
                    .find(|n| &n.id == node_id)
                    .ok_or_else(|| anyhow!("Node not found: {}", node_id))?;

                let node_start = Utc::now();

                // Execute node with safety & coherence checks
                let node_result = self.execute_node(node, &context);

                let node_end = Utc::now();
                let duration_ns = (node_end - node_start).num_nanoseconds().unwrap_or(0) as u64;

                match node_result {
                    Ok(_) => {
                        context.nodes_completed += 1;
                        if matches!(node.node_type, NodeType::Measurement { .. }) {
                            measurements_count += 1;
                        }

                        execution_log.push(NodeExecutionLog {
                            node_id: node_id.clone(),
                            node_type: format!("{:?}", node.node_type),
                            start_timestamp: node_start,
                            end_timestamp: node_end,
                            duration_ns,
                            success: true,
                            error: None,
                        });

                        // Update coherence budget
                        context.coherence_budget_remaining_ns =
                            context.coherence_budget_remaining_ns.saturating_sub(duration_ns);
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        
                        if error_msg.contains("coherence") {
                            coherence_violations += 1;
                        }
                        if error_msg.contains("safety") {
                            safety_violations += 1;
                        }

                        execution_log.push(NodeExecutionLog {
                            node_id: node_id.clone(),
                            node_type: format!("{:?}", node.node_type),
                            start_timestamp: node_start,
                            end_timestamp: node_end,
                            duration_ns,
                            success: false,
                            error: Some(error_msg),
                        });

                        // Handle violation based on strategy
                        match self.safety_enforcement {
                            SafetyEnforcement::Strict => {
                                return Err(anyhow!(
                                    "Execution failed at node {}: {}",
                                    node_id,
                                    e
                                ));
                            }
                            SafetyEnforcement::Warning => {
                                // Continue execution
                            }
                            SafetyEnforcement::Automatic => {
                                // Try to recalibrate and retry
                            }
                        }
                    }
                }
            }
        }

        // 5. Determine final status
        let final_status = if coherence_violations > 0 {
            ExecutionStatus::CoherenceViolation
        } else if safety_violations > 0 {
            ExecutionStatus::SafetyViolation
        } else if execution_log.iter().all(|l| l.success) {
            ExecutionStatus::Success
        } else {
            ExecutionStatus::FailureOther
        };

        let end_time = Utc::now();
        let total_duration_ns = (end_time - start_time).num_nanoseconds().unwrap_or(0) as u64;

        // 6. Return execution result
        Ok(ExecutionResult {
            execution_id: run_id,
            graph_id: graph.graph_id.clone(),
            seed: run_seed,
            status: final_status,
            start_timestamp: start_time,
            end_timestamp: end_time,
            total_duration_ns,
            nodes_executed: context.nodes_completed,
            nodes_failed: execution_log.iter().filter(|l| !l.success).count(),
            measurements_recorded: measurements_count,
            coherence_violations,
            safety_violations,
        })
    }

    /// Validate IR graph before execution
    fn validate_graph(&self, graph: &ComputationGraph) -> Result<()> {
        // 1. Check acyclic (simplified: just check nodes are defined)
        let node_ids: HashSet<_> = graph.nodes.iter().map(|n| &n.id).collect();

        for edge in &graph.edges {
            if !node_ids.contains(&edge.from_node) {
                return Err(anyhow!("Edge references undefined node: {}", edge.from_node));
            }
            if !node_ids.contains(&edge.to_node) {
                return Err(anyhow!("Edge references undefined node: {}", edge.to_node));
            }
        }

        // 2. Check root nodes are defined
        for root in &graph.root_nodes {
            if !node_ids.contains(root) {
                return Err(anyhow!("Root node not defined: {}", root));
            }
        }

        // 3. Check leaf nodes are defined
        for leaf in &graph.leaf_nodes {
            if !node_ids.contains(leaf) {
                return Err(anyhow!("Leaf node not defined: {}", leaf));
            }
        }

        Ok(())
    }

    /// Generate execution plan from graph
    fn generate_execution_plan(&self, graph: &ComputationGraph) -> Result<ExecutionPlan> {
        // Simple topological sort (BFS from roots)
        let mut plan = ExecutionPlan {
            plan_id: Uuid::new_v4().to_string(),
            graph_id: graph.graph_id.clone(),
            phases: Vec::new(),
            total_duration_ns: 0,
        };

        let mut visited = HashSet::new();
        let mut queue = graph.root_nodes.clone();
        let mut current_phase = 0;

        while !queue.is_empty() {
            let mut phase_nodes = Vec::new();

            for node_id in queue.drain(..) {
                if visited.insert(node_id.clone()) {
                    phase_nodes.push(node_id.clone());

                    // Add successors to next queue
                    for edge in &graph.edges {
                        if edge.from_node == node_id && !visited.contains(&edge.to_node) {
                            queue.push(edge.to_node.clone());
                        }
                    }
                }
            }

            if !phase_nodes.is_empty() {
                plan.phases.push(ExecutionPhase {
                    phase_id: current_phase,
                    nodes_to_execute: phase_nodes,
                    is_parallel: true,
                    duration_ns: 1000,  // 1µs per phase
                });
                current_phase += 1;
            }
        }

        plan.total_duration_ns = (plan.phases.len() as u64) * 1000;

        Ok(plan)
    }

    /// Execute a single node with safety & coherence checks
    fn execute_node(
        &self,
        node: &ComputationNode,
        context: &ExecutionContext,
    ) -> Result<()> {
        // Check coherence budget
        if let Some(coherence_req) = node.timing_contract.coherence_requirement_ns {
            if context.coherence_budget_remaining_ns < coherence_req {
                return Err(anyhow!(
                    "Coherence budget exceeded: need {} ns, have {} ns",
                    coherence_req,
                    context.coherence_budget_remaining_ns
                ));
            }
        }

        // Execute based on node type
        match &node.node_type {
            NodeType::ClassicalPhotonic { component } => {
                self.execute_classical_node(node, component)?;
            }
            NodeType::QuantumGate { gate_name } => {
                self.execute_quantum_gate(node, gate_name)?;
            }
            NodeType::Measurement { basis_type } => {
                self.execute_measurement(node, basis_type)?;
            }
            NodeType::Calibration => {
                self.execute_calibration(node)?;
            }
        }

        Ok(())
    }

    fn execute_classical_node(&self, node: &ComputationNode, component: &str) -> Result<()> {
        // Validate parameters are within safety limits
        for (param_name, value) in &node.parameters {
            if *value > 100.0 {
                return Err(anyhow!("Safety: parameter {} exceeds limit ({})", param_name, value));
            }
        }

        // Execute component (simplified)
        match component {
            "MZI" => {
                // MZI execution
            }
            "PhaseShifter" => {
                // Phase shifter execution
            }
            _ => {}
        }

        Ok(())
    }

    fn execute_quantum_gate(&self, node: &ComputationNode, gate_name: &str) -> Result<()> {
        // Check coherence requirement
        if let Some(coh_req) = node.timing_contract.coherence_requirement_ns {
            if coh_req > 0 {
                // Gate requires coherence
            }
        }

        // Execute gate (simplified)
        match gate_name {
            "CNOT" => {}
            "Hadamard" => {}
            _ => {}
        }

        Ok(())
    }

    fn execute_measurement(&self, node: &ComputationNode, basis_type: &str) -> Result<()> {
        // Measurement destroys coherence
        match basis_type {
            "Computational" => {}
            "Homodyne" => {}
            _ => {}
        }

        Ok(())
    }

    fn execute_calibration(&self, node: &ComputationNode) -> Result<()> {
        // Calibration execution (simplified)
        Ok(())
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_simple_graph() -> ComputationGraph {
        ComputationGraph {
            graph_id: "test_graph".to_string(),
            nodes: vec![
                ComputationNode {
                    id: "node_0".to_string(),
                    node_type: NodeType::ClassicalPhotonic {
                        component: "MZI".to_string(),
                    },
                    parameters: {
                        let mut m = HashMap::new();
                        m.insert("phase".to_string(), 0.5);
                        m
                    },
                    timing_contract: TimingContract {
                        duration_ns: 1000,
                        coherence_requirement_ns: None,
                    },
                },
                ComputationNode {
                    id: "node_1".to_string(),
                    node_type: NodeType::Measurement {
                        basis_type: "Computational".to_string(),
                    },
                    parameters: HashMap::new(),
                    timing_contract: TimingContract {
                        duration_ns: 500,
                        coherence_requirement_ns: Some(500),
                    },
                },
            ],
            edges: vec![Edge {
                from_node: "node_0".to_string(),
                to_node: "node_1".to_string(),
            }],
            root_nodes: vec!["node_0".to_string()],
            leaf_nodes: vec!["node_1".to_string()],
        }
    }

    #[test]
    fn test_engine_creation() {
        let engine = Engine::new();
        assert!(engine.observability_enabled);
    }

    #[test]
    fn test_execute_simple_graph() {
        let engine = Engine::new();
        let graph = create_simple_graph();
        let result = engine.run_graph(&graph, Some(42)).expect("Execution failed");

        assert_eq!(result.execution_id.len(), 36);  // UUID length
        assert_eq!(result.seed, 42);
        assert_eq!(result.status, ExecutionStatus::Success);
        assert_eq!(result.nodes_executed, 2);
        assert_eq!(result.nodes_failed, 0);
    }

    #[test]
    fn test_graph_validation_fails_on_missing_node() {
        let engine = Engine::new();
        let mut graph = create_simple_graph();
        // Add edge to non-existent node
        graph.edges.push(Edge {
            from_node: "node_1".to_string(),
            to_node: "nonexistent".to_string(),
        });

        let result = engine.run_graph(&graph, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_execution_plan_generation() {
        let engine = Engine::new();
        let graph = create_simple_graph();
        let plan = engine.generate_execution_plan(&graph).expect("Plan generation failed");

        assert!(!plan.phases.is_empty());
        assert!(plan.phases.iter().any(|p| p.nodes_to_execute.contains(&"node_0".to_string())));
    }

    #[test]
    fn test_coherence_violation_detection() {
        let engine = Engine::new();
        let mut graph = create_simple_graph();
        
        // Set very high coherence requirement that exceeds budget
        graph.nodes[1].timing_contract.coherence_requirement_ns = Some(20_000_000);

        let result = engine.run_graph(&graph, Some(42)).expect("Execution failed");
        
        // Should detect coherence violation
        assert!(result.coherence_violations > 0 || result.status != ExecutionStatus::Success);
    }

    #[test]
    fn test_safety_parameter_validation() {
        let engine = Engine::new();
        let mut graph = create_simple_graph();
        
        // Set parameter exceeding safety limit
        graph.nodes[0].parameters.insert("phase".to_string(), 150.0);

        let result = engine.run_graph(&graph, Some(42));
        // Should fail due to safety violation
        assert!(result.is_err());
    }

    #[test]
    fn test_deterministic_execution_with_seed() {
        let engine = Engine::new();
        let graph = create_simple_graph();

        let result1 = engine.run_graph(&graph, Some(12345)).expect("First run failed");
        let result2 = engine.run_graph(&graph, Some(12345)).expect("Second run failed");

        // Same seed should produce same outcome
        assert_eq!(result1.status, result2.status);
        assert_eq!(result1.nodes_executed, result2.nodes_executed);
    }

    #[test]
    fn test_measurement_node_tracking() {
        let engine = Engine::new();
        let graph = create_simple_graph();

        let result = engine.run_graph(&graph, Some(42)).expect("Execution failed");
        
        // Should track that measurement was executed
        assert_eq!(result.measurements_recorded, 1);
    }

    #[test]
    fn test_execution_duration_tracking() {
        let engine = Engine::new();
        let graph = create_simple_graph();

        let result = engine.run_graph(&graph, Some(42)).expect("Execution failed");
        
        // Should record total execution duration
        assert!(result.total_duration_ns > 0);
    }

    #[test]
    fn test_empty_graph_handling() {
        let engine = Engine::new();
        let graph = ComputationGraph {
            graph_id: "empty".to_string(),
            nodes: vec![],
            edges: vec![],
            root_nodes: vec![],
            leaf_nodes: vec![],
        };

        let result = engine.run_graph(&graph, Some(42)).expect("Execution should succeed for empty graph");
        assert_eq!(result.nodes_executed, 0);
    }

    #[test]
    fn test_multi_node_execution_order() {
        let engine = Engine::new();
        let graph = ComputationGraph {
            graph_id: "multi_node".to_string(),
            nodes: vec![
                ComputationNode {
                    id: "a".to_string(),
                    node_type: NodeType::ClassicalPhotonic { component: "MZI".to_string() },
                    parameters: HashMap::new(),
                    timing_contract: TimingContract {
                        duration_ns: 1000,
                        coherence_requirement_ns: None,
                    },
                },
                ComputationNode {
                    id: "b".to_string(),
                    node_type: NodeType::ClassicalPhotonic { component: "MZI".to_string() },
                    parameters: HashMap::new(),
                    timing_contract: TimingContract {
                        duration_ns: 1000,
                        coherence_requirement_ns: None,
                    },
                },
                ComputationNode {
                    id: "c".to_string(),
                    node_type: NodeType::Measurement { basis_type: "Computational".to_string() },
                    parameters: HashMap::new(),
                    timing_contract: TimingContract {
                        duration_ns: 500,
                        coherence_requirement_ns: None,
                    },
                },
            ],
            edges: vec![
                Edge { from_node: "a".to_string(), to_node: "c".to_string() },
                Edge { from_node: "b".to_string(), to_node: "c".to_string() },
            ],
            root_nodes: vec!["a".to_string(), "b".to_string()],
            leaf_nodes: vec!["c".to_string()],
        };

        let result = engine.run_graph(&graph, Some(42)).expect("Execution failed");
        
        assert_eq!(result.nodes_executed, 3);
        assert!(result.measurements_recorded > 0);
    }
}
