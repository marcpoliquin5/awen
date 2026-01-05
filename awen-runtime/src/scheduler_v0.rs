use anyhow::{anyhow, Result};
/// Scheduler v0.1 - Dynamic Execution Planning for Photonics
///
/// Phase 2, Section 2.2 - Expands from Phase 1.4 StaticScheduler with:
/// - DynamicScheduler for adaptive planning
/// - Coherence deadline propagation
/// - Measurement-conditioned branch scheduling
/// - Resource allocation and validation
/// - Integration with Engine.run_graph()
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

// ============================================================================
// Scheduling Strategy Enumeration
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SchedulingStrategy {
    Static,  // Deterministic, fast, no feedback
    Dynamic, // Adaptive, slower, uses feedback
    Greedy,  // Fast heuristic
    Optimal, // Slow but best (future)
}

// ============================================================================
// Resource Types & Allocation
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Waveguide,
    Coupler,
    Detector,
    Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirement {
    pub resource_type: ResourceType,
    pub count: usize,
    pub exclusive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub allocation_id: String,
    pub waveguides: HashMap<String, Vec<usize>>, // node_id → [waveguide indices]
    pub couplers: HashMap<String, usize>,        // node_id → coupler index
    pub detectors: HashMap<String, usize>,       // measurement_id → detector index
    pub priority: Vec<String>,                   // Execution priority order
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photonica {
    pub device_id: String,
    pub waveguides: usize,
    pub couplers: usize,
    pub detectors: usize,
    pub memory_elements: usize,
}

impl Default for Photonica {
    fn default() -> Self {
        Self {
            device_id: "generic_silicon".to_string(),
            waveguides: 8,
            couplers: 4,
            detectors: 2,
            memory_elements: 3,
        }
    }
}

// ============================================================================
// Scheduling Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingConfig {
    pub strategy: SchedulingStrategy,
    pub optimization_level: u8, // 0-3
    pub min_coherence_margin_ns: u64,
    pub assume_feedback_latency_ns: u64,
    pub available_waveguides: usize,
    pub available_couplers: usize,
    pub available_detectors: usize,
    pub minimize_makespan: bool,
    pub maximize_fidelity: bool,
    pub minimize_resource_usage: bool,
    pub max_phase_duration_ns: u64,
    pub max_total_duration_ns: u64,
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self {
            strategy: SchedulingStrategy::Static,
            optimization_level: 1,
            min_coherence_margin_ns: 100_000, // 100μs
            assume_feedback_latency_ns: 100,  // 100ns
            available_waveguides: 8,
            available_couplers: 4,
            available_detectors: 2,
            minimize_makespan: true,
            maximize_fidelity: true,
            minimize_resource_usage: false,
            max_phase_duration_ns: 1_000_000,  // 1ms
            max_total_duration_ns: 10_000_000, // 10ms
        }
    }
}

// ============================================================================
// Scheduling Feedback (for DynamicScheduler)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingFeedback {
    pub plan_id: String,
    pub actual_execution_time_ns: u64,
    pub fidelity_achieved: f64,
    pub coherence_consumed_ns: u64,
    pub resource_contention: bool,
    pub phase_timings: Vec<u64>, // Actual time per phase
}

// ============================================================================
// Scheduler Implementation
// ============================================================================

pub struct Scheduler {
    config: SchedulingConfig,
    device: Photonica,
    last_plan: Option<ExecutionPlan>,
    feedback_history: Vec<SchedulingFeedback>,
}

impl Scheduler {
    pub fn new(config: SchedulingConfig, device: Photonica) -> Self {
        Scheduler {
            config,
            device,
            last_plan: None,
            feedback_history: Vec::new(),
        }
    }

    /// Main scheduling entry point - generates ExecutionPlan from ComputationGraph
    pub fn schedule(
        &mut self,
        graph: &crate::engine_v2::ComputationGraph,
    ) -> Result<ExecutionPlan> {
        match self.config.strategy {
            SchedulingStrategy::Static => self.schedule_static(graph),
            SchedulingStrategy::Dynamic => self.schedule_dynamic(graph),
            SchedulingStrategy::Greedy => self.schedule_greedy(graph),
            SchedulingStrategy::Optimal => Err(anyhow!("OptimalScheduler not yet implemented")),
        }
    }

    /// Static scheduling - deterministic, topological sort based
    fn schedule_static(
        &mut self,
        graph: &crate::engine_v2::ComputationGraph,
    ) -> Result<ExecutionPlan> {
        let plan_id = Uuid::new_v4().to_string();
        let mut phases = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = graph.root_nodes.clone();
        let mut current_phase = 0u64;
        let mut total_duration_ns = 0u64;

        // Topological sort with phase assignment
        while !queue.is_empty() {
            let mut phase_nodes = Vec::new();
            let mut next_queue = Vec::new();

            for node_id in queue {
                if visited.insert(node_id.clone()) {
                    phase_nodes.push(node_id.clone());

                    // Find successors
                    for edge in &graph.edges {
                        if edge.from_node == node_id && !visited.contains(&edge.to_node) {
                            next_queue.push(edge.to_node.clone());
                        }
                    }
                }
            }

            if !phase_nodes.is_empty() {
                let phase_duration = 1000u64; // 1μs per phase
                total_duration_ns += phase_duration;

                phases.push(ExecutionPhase {
                    phase_id: current_phase as usize,
                    nodes_to_execute: phase_nodes,
                    is_parallel: true,
                    duration_ns: phase_duration,
                    resource_requirements: None,
                    coherence_deadline_ns: Some(self.config.max_total_duration_ns),
                });

                current_phase += 1;
            }

            queue = next_queue;
        }

        let plan = ExecutionPlan {
            plan_id,
            graph_id: graph.graph_id.clone(),
            phases,
            total_duration_ns,
            resource_allocation: None,
        };

        self.last_plan = Some(plan.clone());
        Ok(plan)
    }

    /// Dynamic scheduling - adaptive with feedback integration
    fn schedule_dynamic(
        &mut self,
        graph: &crate::engine_v2::ComputationGraph,
    ) -> Result<ExecutionPlan> {
        // Start with static schedule
        let mut plan = self.schedule_static(graph)?;

        // If we have feedback, adjust schedule
        if let Some(feedback) = self.feedback_history.last() {
            // Adjust based on previous execution:
            // - If coherence was tight, move nodes earlier
            // - If resources were contended, space out nodes more
            // - If fidelity was low, reduce parallelism

            if feedback.coherence_consumed_ns > self.config.max_total_duration_ns / 2 {
                // Coherence pressure detected - be more conservative
                self.config.optimization_level = self.config.optimization_level.saturating_sub(1);
            }

            if feedback.resource_contention {
                // Resource contention detected - serialize more phases
                plan = self.serialize_phases(plan)?;
            }
        }

        self.last_plan = Some(plan.clone());
        Ok(plan)
    }

    /// Greedy scheduling - fast heuristic (future)
    fn schedule_greedy(
        &mut self,
        graph: &crate::engine_v2::ComputationGraph,
    ) -> Result<ExecutionPlan> {
        // For now, fall back to static
        self.schedule_static(graph)
    }

    /// Serialize overlapping phases to reduce resource contention
    fn serialize_phases(&self, mut plan: ExecutionPlan) -> Result<ExecutionPlan> {
        // Move each phase to its own time slot if needed
        let mut serialized_phases = Vec::new();
        let mut time_offset = 0u64;

        for phase in plan.phases {
            let new_phase = ExecutionPhase {
                phase_id: serialized_phases.len(),
                nodes_to_execute: phase.nodes_to_execute,
                is_parallel: false, // Serialize: no parallelism
                duration_ns: phase.duration_ns,
                resource_requirements: phase.resource_requirements,
                coherence_deadline_ns: Some(time_offset + phase.duration_ns),
            };

            time_offset += phase.duration_ns;
            serialized_phases.push(new_phase);
        }

        plan.phases = serialized_phases;
        plan.total_duration_ns = time_offset;
        Ok(plan)
    }

    /// Add feedback from execution
    pub fn add_feedback(&mut self, feedback: SchedulingFeedback) {
        self.feedback_history.push(feedback);
    }

    /// Validate a schedule against device constraints
    pub fn validate_schedule(
        &self,
        plan: &ExecutionPlan,
        graph: &crate::engine_v2::ComputationGraph,
    ) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check 1: All nodes scheduled
        let scheduled_nodes: std::collections::HashSet<_> = plan
            .phases
            .iter()
            .flat_map(|p| p.nodes_to_execute.iter().cloned())
            .collect();

        for node in &graph.nodes {
            if !scheduled_nodes.contains(&node.id) {
                warnings.push(format!("Node {} not scheduled", node.id));
            }
        }

        // Check 2: Total duration within max
        if plan.total_duration_ns > self.config.max_total_duration_ns {
            warnings.push(format!(
                "Schedule duration {} exceeds max {}",
                plan.total_duration_ns, self.config.max_total_duration_ns
            ));
        }

        // Check 3: Coherence deadlines feasible
        for phase in &plan.phases {
            if let Some(deadline) = phase.coherence_deadline_ns {
                if deadline < phase.duration_ns {
                    warnings.push(format!(
                        "Phase {} coherence deadline {} < duration {}",
                        phase.phase_id, deadline, phase.duration_ns
                    ));
                }
            }
        }

        Ok(warnings)
    }
}

// ============================================================================
// ExecutionPlan (shared with Engine)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub plan_id: String,
    pub graph_id: String,
    pub phases: Vec<ExecutionPhase>,
    pub total_duration_ns: u64,
    pub resource_allocation: Option<ResourceAllocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPhase {
    pub phase_id: usize,
    pub nodes_to_execute: Vec<String>,
    pub is_parallel: bool,
    pub duration_ns: u64,
    pub resource_requirements: Option<Vec<ResourceRequirement>>,
    pub coherence_deadline_ns: Option<u64>,
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_simple_graph() -> crate::engine_v2::ComputationGraph {
        use std::collections::HashMap;

        crate::engine_v2::ComputationGraph {
            graph_id: "test_graph".to_string(),
            nodes: vec![
                crate::engine_v2::ComputationNode {
                    id: "node_0".to_string(),
                    node_type: crate::engine_v2::NodeType::ClassicalPhotonic {
                        component: "MZI".to_string(),
                    },
                    parameters: HashMap::new(),
                    timing_contract: crate::engine_v2::TimingContract {
                        duration_ns: 1000,
                        coherence_requirement_ns: None,
                    },
                },
                crate::engine_v2::ComputationNode {
                    id: "node_1".to_string(),
                    node_type: crate::engine_v2::NodeType::Measurement {
                        basis_type: "Computational".to_string(),
                    },
                    parameters: HashMap::new(),
                    timing_contract: crate::engine_v2::TimingContract {
                        duration_ns: 500,
                        coherence_requirement_ns: None,
                    },
                },
            ],
            edges: vec![crate::engine_v2::Edge {
                from_node: "node_0".to_string(),
                to_node: "node_1".to_string(),
            }],
            root_nodes: vec!["node_0".to_string()],
            leaf_nodes: vec!["node_1".to_string()],
        }
    }

    #[test]
    fn test_scheduler_creation() {
        let config = SchedulingConfig::default();
        let device = Photonica::default();
        let _scheduler = Scheduler::new(config, device);
    }

    #[test]
    fn test_static_scheduling() {
        let config = SchedulingConfig {
            strategy: SchedulingStrategy::Static,
            ..Default::default()
        };
        let device = Photonica::default();
        let mut scheduler = Scheduler::new(config, device);
        let graph = create_simple_graph();

        let plan = scheduler.schedule(&graph).expect("Scheduling failed");

        assert_eq!(plan.graph_id, "test_graph");
        assert!(!plan.phases.is_empty());
        assert!(plan.total_duration_ns > 0);
    }

    #[test]
    fn test_schedule_determinism() {
        let config = SchedulingConfig {
            strategy: SchedulingStrategy::Static,
            ..Default::default()
        };
        let device = Photonica::default();
        let mut scheduler1 = Scheduler::new(config.clone(), device.clone());
        let mut scheduler2 = Scheduler::new(config, device);

        let graph = create_simple_graph();
        let plan1 = scheduler1.schedule(&graph).expect("Scheduling 1 failed");
        let plan2 = scheduler2.schedule(&graph).expect("Scheduling 2 failed");

        // Plans should have same structure (even if different IDs)
        assert_eq!(plan1.phases.len(), plan2.phases.len());
        assert_eq!(plan1.total_duration_ns, plan2.total_duration_ns);
    }

    #[test]
    fn test_phase_assignment() {
        let config = SchedulingConfig {
            strategy: SchedulingStrategy::Static,
            ..Default::default()
        };
        let device = Photonica::default();
        let mut scheduler = Scheduler::new(config, device);
        let graph = create_simple_graph();

        let plan = scheduler.schedule(&graph).expect("Scheduling failed");

        // Should have at least 2 phases (node_0 then node_1)
        assert!(plan.phases.len() >= 2);
    }

    #[test]
    fn test_dynamic_scheduling_with_feedback() {
        let config = SchedulingConfig {
            strategy: SchedulingStrategy::Dynamic,
            ..Default::default()
        };
        let device = Photonica::default();
        let mut scheduler = Scheduler::new(config, device);
        let graph = create_simple_graph();

        // First schedule (no feedback)
        let plan1 = scheduler.schedule(&graph).expect("First schedule failed");

        // Add feedback indicating resource contention
        let feedback = SchedulingFeedback {
            plan_id: plan1.plan_id.clone(),
            actual_execution_time_ns: 5000,
            fidelity_achieved: 0.95,
            coherence_consumed_ns: 8_000_000, // High coherence usage
            resource_contention: true,
            phase_timings: vec![1000, 1000],
        };
        scheduler.add_feedback(feedback);

        // Second schedule (with feedback)
        let plan2 = scheduler.schedule(&graph).expect("Second schedule failed");

        // Both should be valid
        assert!(!plan1.phases.is_empty());
        assert!(!plan2.phases.is_empty());
    }

    #[test]
    fn test_schedule_validation_passes() {
        let config = SchedulingConfig::default();
        let device = Photonica::default();
        let mut scheduler = Scheduler::new(config, device);
        let graph = create_simple_graph();

        let plan = scheduler.schedule(&graph).expect("Scheduling failed");
        let warnings = scheduler
            .validate_schedule(&plan, &graph)
            .expect("Validation failed");

        // Should have few or no warnings for simple graph
        assert!(warnings.len() <= 2);
    }

    #[test]
    fn test_resource_availability() {
        let config = SchedulingConfig::default();
        let mut device = Photonica::default();
        device.couplers = 2; // Limit couplers

        let scheduler = Scheduler::new(config, device);
        assert_eq!(scheduler.device.couplers, 2);
    }

    #[test]
    fn test_empty_graph_scheduling() {
        let config = SchedulingConfig::default();
        let device = Photonica::default();
        let mut scheduler = Scheduler::new(config, device);

        let empty_graph = crate::engine_v2::ComputationGraph {
            graph_id: "empty".to_string(),
            nodes: vec![],
            edges: vec![],
            root_nodes: vec![],
            leaf_nodes: vec![],
        };

        let plan = scheduler.schedule(&empty_graph).expect("Scheduling failed");
        assert!(plan.phases.is_empty() || plan.total_duration_ns == 0);
    }

    #[test]
    fn test_large_graph_scheduling() {
        let config = SchedulingConfig::default();
        let device = Photonica::default();
        let mut scheduler = Scheduler::new(config, device);

        // Create 50-node linear graph
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for i in 0..50 {
            use std::collections::HashMap;
            nodes.push(crate::engine_v2::ComputationNode {
                id: format!("node_{}", i),
                node_type: crate::engine_v2::NodeType::ClassicalPhotonic {
                    component: "MZI".to_string(),
                },
                parameters: HashMap::new(),
                timing_contract: crate::engine_v2::TimingContract {
                    duration_ns: 1000,
                    coherence_requirement_ns: None,
                },
            });

            if i > 0 {
                edges.push(crate::engine_v2::Edge {
                    from_node: format!("node_{}", i - 1),
                    to_node: format!("node_{}", i),
                });
            }
        }

        let large_graph = crate::engine_v2::ComputationGraph {
            graph_id: "large".to_string(),
            nodes,
            edges,
            root_nodes: vec!["node_0".to_string()],
            leaf_nodes: vec!["node_49".to_string()],
        };

        let plan = scheduler.schedule(&large_graph).expect("Scheduling failed");
        assert!(plan.phases.len() >= 10); // Should have multiple phases
        assert!(plan.total_duration_ns > 0);
    }

    #[test]
    fn test_serialization_of_contended_schedule() {
        let config = SchedulingConfig {
            strategy: SchedulingStrategy::Dynamic,
            ..Default::default()
        };
        let device = Photonica::default();
        let scheduler = Scheduler::new(config, device);

        let graph = create_simple_graph();
        let plan = crate::engine_v2::ComputationGraph {
            graph_id: "test".to_string(),
            nodes: graph.nodes,
            edges: graph.edges,
            root_nodes: graph.root_nodes,
            leaf_nodes: graph.leaf_nodes,
        };

        let mut contended_plan = ExecutionPlan {
            plan_id: Uuid::new_v4().to_string(),
            graph_id: "test".to_string(),
            phases: vec![ExecutionPhase {
                phase_id: 0,
                nodes_to_execute: vec!["node_0".to_string()],
                is_parallel: true,
                duration_ns: 1000,
                resource_requirements: None,
                coherence_deadline_ns: None,
            }],
            total_duration_ns: 1000,
            resource_allocation: None,
        };

        let serialized = scheduler
            .serialize_phases(contended_plan)
            .expect("Serialization failed");
        assert!(serialized.phases.iter().all(|p| !p.is_parallel));
    }
}
