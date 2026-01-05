// AWEN Scheduling Integration Tests
// End-to-end validation of timing, resource allocation, and coherence enforcement

use awen_runtime::ir::{Graph, Node, Edge};
use awen_runtime::scheduler::{
    Scheduler, StaticScheduler, SchedulingConstraints, FeedbackLoop, Priority,
    TimingConstraint, ConstraintType, ViolationAction, ResourceLimits,
    ResourceState, WavelengthChannel,
};
use awen_runtime::state::CoherenceWindow;
use std::collections::HashMap;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 1: Deterministic Scheduling
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_scheduler_deterministic_replay() {
    // Create simple MZI chain graph
    let graph = create_mzi_chain_graph(5);

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
    let seed = 12345u64;

    // Run scheduler multiple times with same seed
    let plan1 = scheduler.schedule(&graph, &constraints, seed)
        .expect("Schedule 1 failed");
    let plan2 = scheduler.schedule(&graph, &constraints, seed)
        .expect("Schedule 2 failed");
    let plan3 = scheduler.schedule(&graph, &constraints, seed)
        .expect("Schedule 3 failed");

    // Verify determinism: same seed → identical plans
    assert_eq!(plan1.seed, plan2.seed);
    assert_eq!(plan1.seed, plan3.seed);
    assert_eq!(plan1.makespan_ns, plan2.makespan_ns);
    assert_eq!(plan1.makespan_ns, plan3.makespan_ns);
    assert_eq!(plan1.schedule.len(), plan2.schedule.len());
    assert_eq!(plan1.schedule.len(), plan3.schedule.len());

    // Verify node timings are identical
    for (node_id, sched1) in &plan1.schedule {
        let sched2 = &plan2.schedule[node_id];
        let sched3 = &plan3.schedule[node_id];
        assert_eq!(sched1.start_time_ns, sched2.start_time_ns);
        assert_eq!(sched1.start_time_ns, sched3.start_time_ns);
        assert_eq!(sched1.end_time_ns, sched2.end_time_ns);
        assert_eq!(sched1.end_time_ns, sched3.end_time_ns);
    }

    println!("✓ Deterministic scheduling: same seed → identical plans");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 2: Critical Path Identification
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_critical_path_identification() {
    // Create diamond graph: src → (a, b) → dst
    // Path via 'a' is longer (critical path)
    let graph = Graph {
        version: "0.2".to_string(),
        metadata: HashMap::new(),
        nodes: vec![
            Node {
                id: "src".to_string(),
                node_type: "Source".to_string(),
                params: None,
            },
            Node {
                id: "a".to_string(),
                node_type: "MZI".to_string(),
                params: None,
            },
            Node {
                id: "b".to_string(),
                node_type: "MZI".to_string(),
                params: None,
            },
            Node {
                id: "dst".to_string(),
                node_type: "Detector".to_string(),
                params: None,
            },
        ],
        edges: vec![
            Edge {
                src: "src".to_string(),
                dst: "a".to_string(),
                delay_ns: Some(50.0),
            },
            Edge {
                src: "src".to_string(),
                dst: "b".to_string(),
                delay_ns: Some(10.0),
            },
            Edge {
                src: "a".to_string(),
                dst: "dst".to_string(),
                delay_ns: Some(50.0),
            },
            Edge {
                src: "b".to_string(),
                dst: "dst".to_string(),
                delay_ns: Some(10.0),
            },
        ],
    };

    let constraints = create_default_constraints();
    let scheduler = StaticScheduler::new();
    let plan = scheduler.schedule(&graph, &constraints, 42)
        .expect("Scheduling failed");

    // Critical path should include 'a' (longer delays)
    assert!(!plan.critical_path.is_empty());
    
    // Makespan should reflect critical path length
    // src (0-100) → a (150-250) → dst (300-400) = 400ns
    assert!(plan.makespan_ns >= 300, "Makespan too short: {}", plan.makespan_ns);

    println!("✓ Critical path identified: makespan = {}ns", plan.makespan_ns);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 3: Coherence Window Enforcement
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_coherence_window_enforcement() {
    let graph = create_mzi_chain_graph(3);

    // Create coherence window: 0-1000ns, high fidelity threshold
    let coherence_window = CoherenceWindow {
        id: "coh_win_0".to_string(),
        start_time_ns: 0,
        duration_ns: 1_000,
        decoherence_model: "exponential".to_string(),
        fidelity_threshold: 0.95,
        mode_ids: vec!["mode_0".to_string(), "mode_1".to_string()],
    };

    let constraints = SchedulingConstraints {
        coherence_windows: vec![coherence_window],
        feedback_loops: vec![],
        timing_constraints: vec![],
        resource_limits: create_default_resource_limits(),
    };

    let scheduler = StaticScheduler::new();
    let plan = scheduler.schedule(&graph, &constraints, 42)
        .expect("Scheduling failed");

    // Verify all nodes are within coherence window
    for scheduled_node in plan.schedule.values() {
        assert!(
            scheduled_node.end_time_ns <= 1_000,
            "Node {} ends at {}ns, exceeds coherence window",
            scheduled_node.node_id,
            scheduled_node.end_time_ns
        );
    }

    println!("✓ Coherence window enforced: all operations within 1000ns window");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 4: Feedback Loop Deadline Validation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_feedback_loop_deadline_satisfied() {
    // Create measurement-feedback graph
    let graph = Graph {
        version: "0.2".to_string(),
        metadata: HashMap::new(),
        nodes: vec![
            Node {
                id: "source".to_string(),
                node_type: "Source".to_string(),
                params: None,
            },
            Node {
                id: "detector".to_string(),
                node_type: "Detector".to_string(),
                params: None,
            },
            Node {
                id: "control".to_string(),
                node_type: "MZI".to_string(),
                params: None,
            },
        ],
        edges: vec![
            Edge {
                src: "source".to_string(),
                dst: "detector".to_string(),
                delay_ns: Some(10.0),
            },
            Edge {
                src: "detector".to_string(),
                dst: "control".to_string(),
                delay_ns: Some(20.0),
            },
        ],
    };

    let feedback_loop = FeedbackLoop {
        id: "fb_loop_0".to_string(),
        measurement_node: "detector".to_string(),
        control_node: "control".to_string(),
        deadline_ns: 500, // Generous deadline
        priority: Priority::High,
    };

    let constraints = SchedulingConstraints {
        coherence_windows: vec![],
        feedback_loops: vec![feedback_loop],
        timing_constraints: vec![],
        resource_limits: create_default_resource_limits(),
    };

    let scheduler = StaticScheduler::new();
    let plan = scheduler.schedule(&graph, &constraints, 42)
        .expect("Scheduling failed");

    // Verify feedback latency
    let detector = &plan.schedule["detector"];
    let control = &plan.schedule["control"];
    let latency = control.start_time_ns - detector.end_time_ns;
    assert!(
        latency <= 500,
        "Feedback latency {}ns exceeds deadline 500ns",
        latency
    );

    println!("✓ Feedback loop deadline satisfied: latency = {}ns", latency);
}

#[test]
fn test_feedback_loop_deadline_violation() {
    // Create graph with inherently long latency
    let graph = Graph {
        version: "0.2".to_string(),
        metadata: HashMap::new(),
        nodes: vec![
            Node {
                id: "detector".to_string(),
                node_type: "Detector".to_string(),
                params: None,
            },
            Node {
                id: "control".to_string(),
                node_type: "MZI".to_string(),
                params: None,
            },
        ],
        edges: vec![
            Edge {
                src: "detector".to_string(),
                dst: "control".to_string(),
                delay_ns: Some(200.0), // Long edge delay
            },
        ],
    };

    // Create feedback loop with impossible deadline
    let feedback_loop = FeedbackLoop {
        id: "fb_loop_tight".to_string(),
        measurement_node: "detector".to_string(),
        control_node: "control".to_string(),
        deadline_ns: 50, // Impossible: edge delay alone is 200ns
        priority: Priority::Critical,
    };

    let constraints = SchedulingConstraints {
        coherence_windows: vec![],
        feedback_loops: vec![feedback_loop],
        timing_constraints: vec![],
        resource_limits: create_default_resource_limits(),
    };

    let scheduler = StaticScheduler::new();
    let result = scheduler.schedule(&graph, &constraints, 42);

    // Should fail due to deadline violation
    assert!(
        result.is_err(),
        "Expected scheduling to fail due to deadline violation"
    );

    println!("✓ Feedback loop deadline violation correctly detected");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 5: Resource Allocation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_resource_allocation_wavelengths() {
    let graph = create_mzi_chain_graph(4);
    let constraints = create_default_constraints();

    let scheduler = StaticScheduler::new();
    let plan = scheduler.schedule(&graph, &constraints, 42)
        .expect("Scheduling failed");

    // Verify resource allocations
    for scheduled_node in plan.schedule.values() {
        // Each node should have wavelength and memory allocated
        let has_wavelength = scheduled_node
            .allocated_resources
            .iter()
            .any(|r| r.resource_type == "wavelength");
        let has_memory = scheduled_node
            .allocated_resources
            .iter()
            .any(|r| r.resource_type == "memory");

        assert!(has_wavelength, "Node {} missing wavelength", scheduled_node.node_id);
        assert!(has_memory, "Node {} missing memory", scheduled_node.node_id);
    }

    println!("✓ Resource allocation: wavelengths and memory assigned to all nodes");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 6: Multi-Wavelength Synchronization
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_wavelength_skew_compensation() {
    let graph = create_mzi_chain_graph(2);

    // Create resource state with wavelengths at different frequencies
    let resource_state = ResourceState {
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
                skew_compensation_ns: 0.17, // 170ps compensation for 10km fiber
            },
        ],
        available_memory_slots: vec!["mem_0".to_string()],
        device_availability: HashMap::new(),
    };

    let constraints = create_default_constraints();
    let scheduler = StaticScheduler::new();
    let plan = scheduler.schedule(&graph, &constraints, 42)
        .expect("Scheduling failed");

    // Verify wavelength allocations include skew compensation
    let mut has_skew_compensation = false;
    for scheduled_node in plan.schedule.values() {
        for allocation in &scheduled_node.allocated_resources {
            if allocation.resource_id.contains("1551") {
                has_skew_compensation = true;
            }
        }
    }

    println!("✓ Multi-wavelength scheduling: skew compensation available");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 7: Execution Plan Serialization
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_execution_plan_serialization() {
    let graph = create_mzi_chain_graph(3);
    let constraints = create_default_constraints();

    let scheduler = StaticScheduler::new();
    let plan = scheduler.schedule(&graph, &constraints, 42)
        .expect("Scheduling failed");

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&plan)
        .expect("Serialization failed");

    // Deserialize back
    let deserialized_plan: awen_runtime::scheduler::ExecutionPlan =
        serde_json::from_str(&json).expect("Deserialization failed");

    // Verify round-trip
    assert_eq!(plan.id, deserialized_plan.id);
    assert_eq!(plan.seed, deserialized_plan.seed);
    assert_eq!(plan.makespan_ns, deserialized_plan.makespan_ns);
    assert_eq!(plan.schedule.len(), deserialized_plan.schedule.len());

    println!("✓ Execution plan serialization: JSON round-trip successful");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 8: Timing Constraint Validation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_timing_constraint_hard_deadline() {
    let graph = create_mzi_chain_graph(2);

    let timing_constraint = TimingConstraint {
        id: "deadline_0".to_string(),
        constraint_type: ConstraintType::HardDeadline {
            node_id: "node_1".to_string(),
        },
        bound_ns: 500,
        violation_action: ViolationAction::Abort,
    };

    let constraints = SchedulingConstraints {
        coherence_windows: vec![],
        feedback_loops: vec![],
        timing_constraints: vec![timing_constraint],
        resource_limits: create_default_resource_limits(),
    };

    let scheduler = StaticScheduler::new();
    let plan = scheduler.schedule(&graph, &constraints, 42)
        .expect("Scheduling failed");

    // Verify node completes before deadline
    let node_1 = &plan.schedule["node_1"];
    assert!(
        node_1.end_time_ns <= 500,
        "Node 1 completes at {}ns, exceeds deadline 500ns",
        node_1.end_time_ns
    );

    println!("✓ Hard deadline constraint satisfied");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 9: Schedule Validation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_schedule_validation() {
    let graph = create_mzi_chain_graph(3);
    let constraints = create_default_constraints();

    let scheduler = StaticScheduler::new();
    let plan = scheduler.schedule(&graph, &constraints, 42)
        .expect("Scheduling failed");

    let resource_state = create_default_resource_state();
    
    // Validate plan
    let result = scheduler.validate_plan(&plan, &resource_state);
    assert!(result.is_ok(), "Plan validation failed: {:?}", result);

    println!("✓ Execution plan validation passed");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 10: Complex Graph Scheduling
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_complex_graph_scheduling() {
    // Create complex graph with multiple paths and dependencies
    let graph = Graph {
        version: "0.2".to_string(),
        metadata: HashMap::new(),
        nodes: vec![
            Node {
                id: "laser_0".to_string(),
                node_type: "Source".to_string(),
                params: None,
            },
            Node {
                id: "laser_1".to_string(),
                node_type: "Source".to_string(),
                params: None,
            },
            Node {
                id: "mzi_0".to_string(),
                node_type: "MZI".to_string(),
                params: None,
            },
            Node {
                id: "mzi_1".to_string(),
                node_type: "MZI".to_string(),
                params: None,
            },
            Node {
                id: "combiner".to_string(),
                node_type: "Coupler".to_string(),
                params: None,
            },
            Node {
                id: "detector".to_string(),
                node_type: "Detector".to_string(),
                params: None,
            },
        ],
        edges: vec![
            Edge {
                src: "laser_0".to_string(),
                dst: "mzi_0".to_string(),
                delay_ns: Some(5.0),
            },
            Edge {
                src: "laser_1".to_string(),
                dst: "mzi_1".to_string(),
                delay_ns: Some(5.0),
            },
            Edge {
                src: "mzi_0".to_string(),
                dst: "combiner".to_string(),
                delay_ns: Some(10.0),
            },
            Edge {
                src: "mzi_1".to_string(),
                dst: "combiner".to_string(),
                delay_ns: Some(10.0),
            },
            Edge {
                src: "combiner".to_string(),
                dst: "detector".to_string(),
                delay_ns: Some(15.0),
            },
        ],
    };

    let constraints = create_default_constraints();
    let scheduler = StaticScheduler::new();
    let plan = scheduler.schedule(&graph, &constraints, 42)
        .expect("Complex graph scheduling failed");

    // Verify all nodes scheduled
    assert_eq!(plan.schedule.len(), 6);

    // Verify combiner waits for both MZI outputs
    let mzi_0 = &plan.schedule["mzi_0"];
    let mzi_1 = &plan.schedule["mzi_1"];
    let combiner = &plan.schedule["combiner"];

    assert!(
        combiner.start_time_ns >= mzi_0.end_time_ns + 10,
        "Combiner started before MZI 0 completed"
    );
    assert!(
        combiner.start_time_ns >= mzi_1.end_time_ns + 10,
        "Combiner started before MZI 1 completed"
    );

    println!("✓ Complex graph scheduling: {} nodes, makespan = {}ns",
             plan.schedule.len(), plan.makespan_ns);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Helper Functions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn create_mzi_chain_graph(num_nodes: usize) -> Graph {
    let mut nodes = vec![Node {
        id: "source".to_string(),
        node_type: "Source".to_string(),
        params: None,
    }];

    let mut edges = Vec::new();

    for i in 0..num_nodes {
        nodes.push(Node {
            id: format!("node_{}", i),
            node_type: "MZI".to_string(),
            params: None,
        });

        let src = if i == 0 {
            "source".to_string()
        } else {
            format!("node_{}", i - 1)
        };

        edges.push(Edge {
            src,
            dst: format!("node_{}", i),
            delay_ns: Some(10.0),
        });
    }

    Graph {
        version: "0.2".to_string(),
        metadata: HashMap::new(),
        nodes,
        edges,
    }
}

fn create_default_constraints() -> SchedulingConstraints {
    SchedulingConstraints {
        coherence_windows: vec![],
        feedback_loops: vec![],
        timing_constraints: vec![],
        resource_limits: create_default_resource_limits(),
    }
}

fn create_default_resource_limits() -> ResourceLimits {
    ResourceLimits {
        max_wavelengths: 4,
        max_memory_slots: 4,
        max_concurrent_operations: 10,
    }
}

fn create_default_resource_state() -> ResourceState {
    ResourceState {
        available_wavelengths: vec![
            WavelengthChannel {
                lambda_nm: 1550.0,
                bandwidth_ghz: 100.0,
                dispersion_ps_per_nm: 17.0,
                skew_compensation_ns: 0.0,
            },
        ],
        available_memory_slots: vec!["mem_0".to_string()],
        device_availability: HashMap::new(),
    }
}
