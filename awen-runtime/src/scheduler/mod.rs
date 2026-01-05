// AWEN Scheduler Module
// Timing, resource allocation, and coherence-aware execution planning

use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use crate::ir::Graph;
use crate::state::CoherenceWindow;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Core Scheduler Trait
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Scheduler trait for generating execution plans from IR graphs
pub trait Scheduler: Send + Sync {
    /// Generate execution plan from IR and constraints
    fn schedule(
        &self,
        graph: &Graph,
        constraints: &SchedulingConstraints,
        seed: u64,
    ) -> Result<ExecutionPlan>;

    /// Validate existing plan against current resource state
    fn validate_plan(
        &self,
        plan: &ExecutionPlan,
        current_state: &ResourceState,
    ) -> Result<()>;
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Scheduling Constraints
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchedulingConstraints {
    pub coherence_windows: Vec<CoherenceWindow>,
    pub feedback_loops: Vec<FeedbackLoop>,
    pub timing_constraints: Vec<TimingConstraint>,
    pub resource_limits: ResourceLimits,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeedbackLoop {
    pub id: String,
    pub measurement_node: String,
    pub control_node: String,
    pub deadline_ns: u64,
    pub priority: Priority,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Background = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimingConstraint {
    pub id: String,
    pub constraint_type: ConstraintType,
    pub bound_ns: u64,
    pub violation_action: ViolationAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConstraintType {
    HardDeadline { node_id: String },
    MinimumSeparation { node_a: String, node_b: String },
    MaximumLatency { src_node: String, dst_node: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ViolationAction {
    Abort,
    Alert,
    Degrade,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_wavelengths: usize,
    pub max_memory_slots: usize,
    pub max_concurrent_operations: usize,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Execution Plan
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub id: String,
    pub seed: u64,
    pub algorithm: String,
    pub makespan_ns: u64,
    pub critical_path: Vec<String>,
    pub schedule: HashMap<String, ScheduledNode>,
    pub resource_usage: ResourceUsageReport,
    pub provenance: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduledNode {
    pub node_id: String,
    pub start_time_ns: u64,
    pub end_time_ns: u64,
    pub allocated_resources: Vec<ResourceAllocation>,
    pub coherence_window_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub resource_type: String,
    pub resource_id: String,
    pub start_ns: u64,
    pub end_ns: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceUsageReport {
    pub wavelengths_used: usize,
    pub memory_slots_used: usize,
    pub peak_concurrent_operations: usize,
    pub average_parallelism: f64,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Resource State Tracking
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceState {
    pub available_wavelengths: Vec<WavelengthChannel>,
    pub available_memory_slots: Vec<String>,
    pub device_availability: HashMap<String, bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WavelengthChannel {
    pub lambda_nm: f64,
    pub bandwidth_ghz: f64,
    pub dispersion_ps_per_nm: f64,
    pub skew_compensation_ns: f64,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Static Scheduler Implementation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct StaticScheduler;

impl StaticScheduler {
    pub fn new() -> Self {
        StaticScheduler
    }

    /// Compute critical path (longest dependency chain)
    fn compute_critical_path(&self, graph: &Graph) -> Result<Vec<String>> {
        // Build dependency graph
        let mut node_depths: HashMap<String, u64> = HashMap::new();
        let mut node_latencies: HashMap<String, u64> = HashMap::new();

        // Initialize all nodes with depth 0
        for node in &graph.nodes {
            node_depths.insert(node.id.clone(), 0);
            // Use default latency if not specified
            node_latencies.insert(node.id.clone(), 100); // 100ns default
        }

        // Compute depths via topological traversal
        let mut changed = true;
        while changed {
            changed = false;
            for edge in &graph.edges {
                let src_depth = node_depths.get(&edge.src).copied().unwrap_or(0);
                let src_latency = node_latencies.get(&edge.src).copied().unwrap_or(100);
                let edge_delay = edge.delay_ns.unwrap_or(0.0) as u64;
                let new_depth = src_depth + src_latency + edge_delay;

                let dst_depth = node_depths.get(&edge.dst).copied().unwrap_or(0);
                if new_depth > dst_depth {
                    node_depths.insert(edge.dst.clone(), new_depth);
                    changed = true;
                }
            }
        }

        // Find nodes on critical path (max depth)
        let max_depth = node_depths.values().max().copied().unwrap_or(0);
        let critical_nodes: Vec<String> = node_depths
            .iter()
            .filter(|(_, &depth)| depth == max_depth)
            .map(|(id, _)| id.clone())
            .collect();

        Ok(critical_nodes)
    }

    /// Allocate resources for a node
    fn allocate_resources(
        &self,
        node_id: &str,
        start_time: u64,
        end_time: u64,
        resource_state: &mut ResourceState,
    ) -> Result<Vec<ResourceAllocation>> {
        let mut allocations = Vec::new();

        // Allocate wavelength (simple: take first available)
        if !resource_state.available_wavelengths.is_empty() {
            let wavelength = resource_state.available_wavelengths[0].lambda_nm;
            allocations.push(ResourceAllocation {
                resource_type: "wavelength".to_string(),
                resource_id: format!("{}nm", wavelength),
                start_ns: start_time,
                end_ns: end_time,
            });
        }

        // Allocate memory slot if needed
        if !resource_state.available_memory_slots.is_empty() {
            let slot = resource_state.available_memory_slots[0].clone();
            allocations.push(ResourceAllocation {
                resource_type: "memory".to_string(),
                resource_id: slot,
                start_ns: start_time,
                end_ns: end_time,
            });
        }

        Ok(allocations)
    }

    /// Validate coherence constraints
    fn validate_coherence(
        &self,
        scheduled_node: &ScheduledNode,
        constraints: &SchedulingConstraints,
    ) -> Result<()> {
        if let Some(window_id) = &scheduled_node.coherence_window_id {
            let window = constraints
                .coherence_windows
                .iter()
                .find(|w| &w.id == window_id)
                .ok_or_else(|| anyhow!("Coherence window {} not found", window_id))?;

            // Check temporal containment
            if scheduled_node.start_time_ns < window.start_time_ns {
                return Err(anyhow!(
                    "Node {} starts before coherence window",
                    scheduled_node.node_id
                ));
            }

            let window_end = window.start_time_ns + window.duration_ns;
            if scheduled_node.end_time_ns > window_end {
                return Err(anyhow!(
                    "Node {} ends after coherence window",
                    scheduled_node.node_id
                ));
            }

            // Check fidelity threshold (exponential decay model)
            let duration = scheduled_node.end_time_ns - scheduled_node.start_time_ns;
            let fidelity = (-1.0 * (duration as f64) / (window.duration_ns as f64)).exp();
            if fidelity < window.fidelity_threshold {
                return Err(anyhow!(
                    "Node {} fidelity {} below threshold {}",
                    scheduled_node.node_id,
                    fidelity,
                    window.fidelity_threshold
                ));
            }
        }

        Ok(())
    }

    /// Validate feedback loop deadlines
    fn validate_feedback_loops(
        &self,
        schedule: &HashMap<String, ScheduledNode>,
        constraints: &SchedulingConstraints,
    ) -> Result<()> {
        for feedback_loop in &constraints.feedback_loops {
            let measurement_node = schedule
                .get(&feedback_loop.measurement_node)
                .ok_or_else(|| anyhow!("Measurement node {} not in schedule", feedback_loop.measurement_node))?;

            let control_node = schedule
                .get(&feedback_loop.control_node)
                .ok_or_else(|| anyhow!("Control node {} not in schedule", feedback_loop.control_node))?;

            let latency = control_node.start_time_ns - measurement_node.end_time_ns;
            if latency > feedback_loop.deadline_ns {
                return Err(anyhow!(
                    "Feedback loop {} latency {}ns exceeds deadline {}ns",
                    feedback_loop.id,
                    latency,
                    feedback_loop.deadline_ns
                ));
            }
        }

        Ok(())
    }
}

impl Scheduler for StaticScheduler {
    fn schedule(
        &self,
        graph: &Graph,
        constraints: &SchedulingConstraints,
        seed: u64,
    ) -> Result<ExecutionPlan> {
        // Phase 1: Dependency analysis
        let critical_path = self.compute_critical_path(graph)?;

        // Phase 2: Topological sort
        let mut schedule: HashMap<String, ScheduledNode> = HashMap::new();
        let mut node_end_times: HashMap<String, u64> = HashMap::new();
        let mut resource_state = ResourceState {
            available_wavelengths: vec![
                WavelengthChannel {
                    lambda_nm: 1550.0,
                    bandwidth_ghz: 100.0,
                    dispersion_ps_per_nm: 17.0,
                    skew_compensation_ns: 0.0,
                },
                WavelengthChannel {
                    lambda_nm: 1551.0,
                    bandwidth_ghz: 100.0,
                    dispersion_ps_per_nm: 17.0,
                    skew_compensation_ns: 0.17, // Compensate 170ps skew for 10km
                },
            ],
            available_memory_slots: vec!["mem_0".to_string(), "mem_1".to_string()],
            device_availability: HashMap::new(),
        };

        // Phase 3: Schedule nodes in topological order
        for node in &graph.nodes {
            // Compute earliest start time based on dependencies
            let mut earliest_start = 0u64;
            for edge in &graph.edges {
                if edge.dst == node.id {
                    let src_end = node_end_times.get(&edge.src).copied().unwrap_or(0);
                    let edge_delay = edge.delay_ns.unwrap_or(0.0) as u64;
                    earliest_start = earliest_start.max(src_end + edge_delay);
                }
            }

            // Default node latency
            let node_latency = 100u64; // 100ns

            // Allocate resources
            let allocations = self.allocate_resources(
                &node.id,
                earliest_start,
                earliest_start + node_latency,
                &mut resource_state,
            )?;

            // Find coherence window if needed (heuristic: use first available)
            let coherence_window_id = if !constraints.coherence_windows.is_empty() {
                Some(constraints.coherence_windows[0].id.clone())
            } else {
                None
            };

            let scheduled_node = ScheduledNode {
                node_id: node.id.clone(),
                start_time_ns: earliest_start,
                end_time_ns: earliest_start + node_latency,
                allocated_resources: allocations,
                coherence_window_id,
            };

            // Validate coherence constraints
            self.validate_coherence(&scheduled_node, constraints)?;

            node_end_times.insert(node.id.clone(), scheduled_node.end_time_ns);
            schedule.insert(node.id.clone(), scheduled_node);
        }

        // Phase 4: Validate feedback loops
        self.validate_feedback_loops(&schedule, constraints)?;

        // Compute makespan
        let makespan = node_end_times.values().max().copied().unwrap_or(0);

        // Build resource usage report
        let resource_usage = ResourceUsageReport {
            wavelengths_used: 2,
            memory_slots_used: 2,
            peak_concurrent_operations: graph.nodes.len(),
            average_parallelism: (graph.nodes.len() as f64) / (makespan as f64 / 100.0),
        };

        // Build provenance
        let mut provenance = HashMap::new();
        provenance.insert("algorithm".to_string(), "static_v0.1".to_string());
        provenance.insert("seed".to_string(), seed.to_string());
        provenance.insert("graph_nodes".to_string(), graph.nodes.len().to_string());
        provenance.insert("makespan_ns".to_string(), makespan.to_string());

        Ok(ExecutionPlan {
            id: format!("exec-plan-{}", seed),
            seed,
            algorithm: "static_v0.1".to_string(),
            makespan_ns: makespan,
            critical_path,
            schedule,
            resource_usage,
            provenance,
        })
    }

    fn validate_plan(
        &self,
        plan: &ExecutionPlan,
        _current_state: &ResourceState,
    ) -> Result<()> {
        // Validate that schedule is consistent
        for (node_id, scheduled_node) in &plan.schedule {
            if scheduled_node.node_id != *node_id {
                return Err(anyhow!("Schedule inconsistency for node {}", node_id));
            }
            if scheduled_node.end_time_ns <= scheduled_node.start_time_ns {
                return Err(anyhow!("Invalid time range for node {}", node_id));
            }
        }

        Ok(())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Unit Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Node, Edge};

    #[test]
    fn test_static_scheduler_determinism() {
        let graph = Graph {
            version: "0.2".to_string(),
            metadata: HashMap::new(),
            nodes: vec![
                Node {
                    id: "node_0".to_string(),
                    node_type: "Source".to_string(),
                    params: None,
                },
                Node {
                    id: "node_1".to_string(),
                    node_type: "MZI".to_string(),
                    params: None,
                },
            ],
            edges: vec![Edge {
                src: "node_0".to_string(),
                dst: "node_1".to_string(),
                delay_ns: Some(10.0),
            }],
        };

        let constraints = SchedulingConstraints {
            coherence_windows: vec![],
            feedback_loops: vec![],
            timing_constraints: vec![],
            resource_limits: ResourceLimits {
                max_wavelengths: 4,
                max_memory_slots: 4,
                max_concurrent_operations: 10,
            },
        };

        let scheduler = StaticScheduler::new();
        let seed = 42;

        // Run scheduler twice with same seed
        let plan1 = scheduler.schedule(&graph, &constraints, seed).unwrap();
        let plan2 = scheduler.schedule(&graph, &constraints, seed).unwrap();

        // Plans should be identical
        assert_eq!(plan1.seed, plan2.seed);
        assert_eq!(plan1.makespan_ns, plan2.makespan_ns);
        assert_eq!(plan1.schedule.len(), plan2.schedule.len());
    }

    #[test]
    fn test_critical_path_computation() {
        let graph = Graph {
            version: "0.2".to_string(),
            metadata: HashMap::new(),
            nodes: vec![
                Node {
                    id: "a".to_string(),
                    node_type: "Source".to_string(),
                    params: None,
                },
                Node {
                    id: "b".to_string(),
                    node_type: "MZI".to_string(),
                    params: None,
                },
                Node {
                    id: "c".to_string(),
                    node_type: "Detector".to_string(),
                    params: None,
                },
            ],
            edges: vec![
                Edge {
                    src: "a".to_string(),
                    dst: "b".to_string(),
                    delay_ns: Some(10.0),
                },
                Edge {
                    src: "b".to_string(),
                    dst: "c".to_string(),
                    delay_ns: Some(20.0),
                },
            ],
        };

        let scheduler = StaticScheduler::new();
        let critical_path = scheduler.compute_critical_path(&graph).unwrap();

        // Should identify longest chain
        assert!(!critical_path.is_empty());
    }

    #[test]
    fn test_resource_allocation() {
        let mut resource_state = ResourceState {
            available_wavelengths: vec![WavelengthChannel {
                lambda_nm: 1550.0,
                bandwidth_ghz: 100.0,
                dispersion_ps_per_nm: 17.0,
                skew_compensation_ns: 0.0,
            }],
            available_memory_slots: vec!["mem_0".to_string()],
            device_availability: HashMap::new(),
        };

        let scheduler = StaticScheduler::new();
        let allocations = scheduler
            .allocate_resources("test_node", 0, 100, &mut resource_state)
            .unwrap();

        // Should allocate wavelength and memory
        assert_eq!(allocations.len(), 2);
        assert_eq!(allocations[0].resource_type, "wavelength");
        assert_eq!(allocations[1].resource_type, "memory");
    }

    #[test]
    fn test_execution_plan_validation() {
        let mut schedule = HashMap::new();
        schedule.insert(
            "node_0".to_string(),
            ScheduledNode {
                node_id: "node_0".to_string(),
                start_time_ns: 0,
                end_time_ns: 100,
                allocated_resources: vec![],
                coherence_window_id: None,
            },
        );

        let plan = ExecutionPlan {
            id: "test-plan".to_string(),
            seed: 42,
            algorithm: "static_v0.1".to_string(),
            makespan_ns: 100,
            critical_path: vec!["node_0".to_string()],
            schedule,
            resource_usage: ResourceUsageReport {
                wavelengths_used: 1,
                memory_slots_used: 1,
                peak_concurrent_operations: 1,
                average_parallelism: 1.0,
            },
            provenance: HashMap::new(),
        };

        let resource_state = ResourceState {
            available_wavelengths: vec![],
            available_memory_slots: vec![],
            device_availability: HashMap::new(),
        };

        let scheduler = StaticScheduler::new();
        assert!(scheduler.validate_plan(&plan, &resource_state).is_ok());
    }
}
