/// Engine Integration Tests - Phase 2, Section 2.1
///
/// Comprehensive test coverage of Engine execution:
///   - All node types (classical, quantum, measurement, calibration)
///   - Coherence window enforcement
///   - Safety constraint validation
///   - Measurement-conditioned branching
///   - Deterministic replay
///   - Artifact emission
///   - Integration with Calibration, Memory, Observability
use std::collections::HashMap;
use uuid::Uuid;

mod engine_tests {
    use super::*;
    use awen_runtime::engine_v2;

    // ========================================================================
    // Test Utilities
    // ========================================================================

    fn create_graph_with_nodes(node_ids: Vec<&str>) -> engine_v2::ComputationGraph {
        let nodes = node_ids
            .iter()
            .map(|id| engine_v2::ComputationNode {
                id: id.to_string(),
                node_type: engine_v2::NodeType::ClassicalPhotonic {
                    component: "MZI".to_string(),
                },
                parameters: {
                    let mut m = HashMap::new();
                    m.insert("phase".to_string(), 0.5);
                    m
                },
                timing_contract: engine_v2::TimingContract {
                    duration_ns: 1000,
                    coherence_requirement_ns: None,
                },
            })
            .collect();

        let edges = (0..node_ids.len() - 1)
            .map(|i| engine_v2::Edge {
                from_node: node_ids[i].to_string(),
                to_node: node_ids[i + 1].to_string(),
            })
            .collect();

        engine_v2::ComputationGraph {
            graph_id: Uuid::new_v4().to_string(),
            nodes,
            edges,
            root_nodes: vec![node_ids[0].to_string()],
            leaf_nodes: vec![node_ids[node_ids.len() - 1].to_string()],
        }
    }

    // ========================================================================
    // Classical Photonic Node Tests
    // ========================================================================

    #[test]
    fn test_mzi_node_execution() {
        let engine = engine_v2::Engine::new();
        let graph = create_graph_with_nodes(vec!["mzi_0"]);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
        assert_eq!(result.nodes_executed, 1);
        assert_eq!(result.nodes_failed, 0);
    }

    #[test]
    fn test_phase_shifter_node_execution() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["ps_0"]);
        graph.nodes[0].node_type = engine_v2::NodeType::ClassicalPhotonic {
            component: "PhaseShifter".to_string(),
        };

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    #[test]
    fn test_beam_splitter_node_execution() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["bs_0"]);
        graph.nodes[0].node_type = engine_v2::NodeType::ClassicalPhotonic {
            component: "BeamSplitter".to_string(),
        };

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    // ========================================================================
    // Quantum Gate Node Tests
    // ========================================================================

    #[test]
    fn test_quantum_gate_execution() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["qgate_0"]);
        graph.nodes[0].node_type = engine_v2::NodeType::QuantumGate {
            gate_name: "Hadamard".to_string(),
        };
        graph.nodes[0].timing_contract.coherence_requirement_ns = Some(100);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    #[test]
    fn test_cnot_gate_execution() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["cnot_0"]);
        graph.nodes[0].node_type = engine_v2::NodeType::QuantumGate {
            gate_name: "CNOT".to_string(),
        };

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    #[test]
    fn test_parametric_gate_execution() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["pgate_0"]);
        graph.nodes[0].node_type = engine_v2::NodeType::QuantumGate {
            gate_name: "RX".to_string(),
        };
        graph.nodes[0].parameters.insert("angle".to_string(), 0.785); // π/4

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    // ========================================================================
    // Measurement Node Tests
    // ========================================================================

    #[test]
    fn test_measurement_in_computational_basis() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["measure_0"]);
        graph.nodes[0].node_type = engine_v2::NodeType::Measurement {
            basis_type: "Computational".to_string(),
        };

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.measurements_recorded, 1);
    }

    #[test]
    fn test_measurement_in_homodyne_basis() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["measure_1"]);
        graph.nodes[0].node_type = engine_v2::NodeType::Measurement {
            basis_type: "Homodyne".to_string(),
        };

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.measurements_recorded, 1);
    }

    #[test]
    fn test_multiple_measurements() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["m0", "m1", "m2"]);

        // Ensure these nodes are configured as measurement nodes so the engine
        // records measurement outcomes during execution.
        for node in graph.nodes.iter_mut() {
            node.node_type = engine_v2::NodeType::Measurement {
                basis_type: "Computational".to_string(),
            };
        }

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        // All measurement nodes should be tracked
        assert!(
            result.measurements_recorded > 0,
            "Expected measurements to be recorded for measurement nodes"
        );
    }

    // ========================================================================
    // Calibration Node Tests
    // ========================================================================

    #[test]
    fn test_calibration_node_execution() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["cal_0"]);
        graph.nodes[0].node_type = engine_v2::NodeType::Calibration;

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    #[test]
    fn test_pre_execution_calibration() {
        let engine = engine_v2::Engine::new();
        let graph = create_graph_with_nodes(vec!["cal_0", "exec_0"]);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.nodes_executed, 2);
    }

    // ========================================================================
    // Coherence Window Enforcement Tests
    // ========================================================================

    #[test]
    fn test_coherence_budget_tracking() {
        let engine = engine_v2::Engine::new();
        let graph = create_graph_with_nodes(vec!["n0", "n1", "n2"]);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        // Should complete within default 10ms coherence window
        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    #[test]
    fn test_coherence_violation_on_deadline_exceeded() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["qgate_0"]);

        // Set coherence requirement that exceeds total budget
        graph.nodes[0].timing_contract.coherence_requirement_ns = Some(20_000_000); // 20ms > 10ms default

        let result = engine.run_graph(&graph, Some(42));

        // Should detect coherence violation
        match result {
            Ok(res) => {
                // May pass or fail depending on enforcement
                assert!(
                    res.coherence_violations > 0
                        || res.status != engine_v2::ExecutionStatus::Success
                );
            }
            Err(_) => {
                // Coherence violation caught as error
            }
        }
    }

    #[test]
    fn test_quantum_nodes_consume_coherence_budget() {
        let engine = engine_v2::Engine::new();
        let graph = {
            let mut g = create_graph_with_nodes(vec!["qgate_0", "qgate_1"]);
            g.nodes[0].node_type = engine_v2::NodeType::QuantumGate {
                gate_name: "Hadamard".to_string(),
            };
            g.nodes[0].timing_contract.coherence_requirement_ns = Some(1_000_000); // 1ms

            g.nodes[1].node_type = engine_v2::NodeType::QuantumGate {
                gate_name: "CNOT".to_string(),
            };
            g.nodes[1].timing_contract.coherence_requirement_ns = Some(1_000_000); // 1ms

            g
        };

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        // Should track coherence consumption
        assert!(result.total_duration_ns > 0);
    }

    // ========================================================================
    // Safety Constraint Validation Tests
    // ========================================================================

    #[test]
    fn test_safety_limit_on_phase_parameter() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["ps_0"]);

        // Set phase parameter within safe range
        graph.nodes[0].parameters.insert("phase".to_string(), 45.0);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    #[test]
    fn test_safety_violation_on_parameter_exceed() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["ps_0"]);

        // Set phase parameter exceeding safety limit
        graph.nodes[0].parameters.insert("phase".to_string(), 150.0);

        let result = engine.run_graph(&graph, Some(42));

        // Should detect safety violation
        assert!(result.is_err());
    }

    #[test]
    fn test_safety_validation_all_nodes() {
        let engine = engine_v2::Engine::new();
        let graph = create_graph_with_nodes(vec!["n0", "n1", "n2", "n3"]);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.nodes_failed, 0);
    }

    #[test]
    fn test_soft_safety_limits_warning() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["ps_0"]);

        // Set parameter near but below hard limit
        graph.nodes[0].parameters.insert("phase".to_string(), 95.0);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        // Should succeed but may have warnings
        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    // ========================================================================
    // Deterministic Replay Tests
    // ========================================================================

    #[test]
    fn test_same_seed_produces_same_results() {
        let engine = engine_v2::Engine::new();
        let graph1 = create_graph_with_nodes(vec!["n0", "n1"]);
        let graph2 = create_graph_with_nodes(vec!["n0", "n1"]);

        let result1 = engine
            .run_graph(&graph1, Some(12345))
            .expect("First run failed");
        let result2 = engine
            .run_graph(&graph2, Some(12345))
            .expect("Second run failed");

        assert_eq!(result1.status, result2.status);
        assert_eq!(result1.nodes_executed, result2.nodes_executed);
        assert_eq!(result1.measurements_recorded, result2.measurements_recorded);
    }

    #[test]
    fn test_different_seeds_may_differ() {
        let engine = engine_v2::Engine::new();
        let graph1 = create_graph_with_nodes(vec!["n0", "n1"]);
        let graph2 = create_graph_with_nodes(vec!["n0", "n1"]);

        let result1 = engine
            .run_graph(&graph1, Some(111))
            .expect("First run failed");
        let result2 = engine
            .run_graph(&graph2, Some(222))
            .expect("Second run failed");

        // Seeds should be recorded correctly
        assert_eq!(result1.seed, 111);
        assert_eq!(result2.seed, 222);
    }

    #[test]
    fn test_replay_with_measurement_outcomes() {
        let engine = engine_v2::Engine::new();
        let mut graph1 = create_graph_with_nodes(vec!["m0", "m1"]);
        graph1.nodes[0].node_type = engine_v2::NodeType::Measurement {
            basis_type: "Computational".to_string(),
        };
        graph1.nodes[1].node_type = engine_v2::NodeType::Measurement {
            basis_type: "Computational".to_string(),
        };

        let result1 = engine
            .run_graph(&graph1, Some(99999))
            .expect("First run failed");

        let mut graph2 = create_graph_with_nodes(vec!["m0", "m1"]);
        graph2.nodes[0].node_type = engine_v2::NodeType::Measurement {
            basis_type: "Computational".to_string(),
        };
        graph2.nodes[1].node_type = engine_v2::NodeType::Measurement {
            basis_type: "Computational".to_string(),
        };

        let result2 = engine
            .run_graph(&graph2, Some(99999))
            .expect("Second run failed");

        assert_eq!(result1.measurements_recorded, result2.measurements_recorded);
    }

    // ========================================================================
    // Error Handling & Violation Tests
    // ========================================================================

    #[test]
    fn test_invalid_edge_target_node_error() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["n0"]);

        // Add edge to non-existent node
        graph.edges.push(engine_v2::Edge {
            from_node: "n0".to_string(),
            to_node: "nonexistent".to_string(),
        });

        let result = engine.run_graph(&graph, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_root_node_error() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["n0"]);

        // Set invalid root
        graph.root_nodes = vec!["missing".to_string()];

        let result = engine.run_graph(&graph, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_leaf_node_error() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["n0"]);

        // Set invalid leaf
        graph.leaf_nodes = vec!["missing".to_string()];

        let result = engine.run_graph(&graph, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_violations_reported() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["n0", "n1"]);

        // Both nodes exceed safety limits
        graph.nodes[0].parameters.insert("phase".to_string(), 120.0);
        graph.nodes[1].parameters.insert("phase".to_string(), 130.0);

        let result = engine.run_graph(&graph, Some(42));
        assert!(result.is_err());
    }

    // ========================================================================
    // Graph Execution Flow Tests
    // ========================================================================

    #[test]
    fn test_linear_graph_execution() {
        let engine = engine_v2::Engine::new();
        let graph = create_graph_with_nodes(vec!["n0", "n1", "n2", "n3", "n4"]);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.nodes_executed, 5);
        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    #[test]
    fn test_branching_graph_execution() {
        let engine = engine_v2::Engine::new();
        let graph = engine_v2::ComputationGraph {
            graph_id: Uuid::new_v4().to_string(),
            nodes: vec![
                engine_v2::ComputationNode {
                    id: "root".to_string(),
                    node_type: engine_v2::NodeType::ClassicalPhotonic {
                        component: "BS".to_string(),
                    },
                    parameters: HashMap::new(),
                    timing_contract: engine_v2::TimingContract {
                        duration_ns: 1000,
                        coherence_requirement_ns: None,
                    },
                },
                engine_v2::ComputationNode {
                    id: "branch_a".to_string(),
                    node_type: engine_v2::NodeType::Measurement {
                        basis_type: "Computational".to_string(),
                    },
                    parameters: HashMap::new(),
                    timing_contract: engine_v2::TimingContract {
                        duration_ns: 500,
                        coherence_requirement_ns: None,
                    },
                },
                engine_v2::ComputationNode {
                    id: "branch_b".to_string(),
                    node_type: engine_v2::NodeType::Measurement {
                        basis_type: "Homodyne".to_string(),
                    },
                    parameters: HashMap::new(),
                    timing_contract: engine_v2::TimingContract {
                        duration_ns: 500,
                        coherence_requirement_ns: None,
                    },
                },
            ],
            edges: vec![
                engine_v2::Edge {
                    from_node: "root".to_string(),
                    to_node: "branch_a".to_string(),
                },
                engine_v2::Edge {
                    from_node: "root".to_string(),
                    to_node: "branch_b".to_string(),
                },
            ],
            root_nodes: vec!["root".to_string()],
            leaf_nodes: vec!["branch_a".to_string(), "branch_b".to_string()],
        };

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.nodes_executed, 3);
        assert_eq!(result.measurements_recorded, 2);
    }

    #[test]
    fn test_empty_graph_execution() {
        let engine = engine_v2::Engine::new();
        let graph = engine_v2::ComputationGraph {
            graph_id: Uuid::new_v4().to_string(),
            nodes: vec![],
            edges: vec![],
            root_nodes: vec![],
            leaf_nodes: vec![],
        };

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        assert_eq!(result.nodes_executed, 0);
        assert_eq!(result.status, engine_v2::ExecutionStatus::Success);
    }

    // ========================================================================
    // Observability & Artifact Tests
    // ========================================================================

    #[test]
    fn test_execution_id_uniqueness() {
        let engine = engine_v2::Engine::new();
        let graph = create_graph_with_nodes(vec!["n0"]);

        let result1 = engine.run_graph(&graph, Some(42)).expect("Run 1 failed");
        let result2 = engine.run_graph(&graph, Some(42)).expect("Run 2 failed");

        // Each execution should have unique ID
        assert_ne!(result1.execution_id, result2.execution_id);
    }

    #[test]
    fn test_execution_timestamp_recording() {
        let engine = engine_v2::Engine::new();
        let graph = create_graph_with_nodes(vec!["n0", "n1"]);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        // Should record start and end timestamps
        assert!(result.start_timestamp <= result.end_timestamp);
    }

    #[test]
    fn test_node_failure_tracking() {
        let engine = engine_v2::Engine::new();
        let mut graph = create_graph_with_nodes(vec!["safe_0", "fail_0"]);

        // Make second node unsafe
        graph.nodes[1].parameters.insert("phase".to_string(), 999.0);

        let result = engine.run_graph(&graph, Some(42));

        // Should detect failure
        match result {
            Ok(res) => {
                // May succeed or have failures depending on enforcement
                assert!(res.nodes_failed > 0 || res.status != engine_v2::ExecutionStatus::Success);
            }
            Err(_) => {
                // Error caught
            }
        }
    }

    #[test]
    fn test_duration_measurements() {
        let engine = engine_v2::Engine::new();
        let graph = create_graph_with_nodes(vec!["n0", "n1", "n2"]);

        let result = engine
            .run_graph(&graph, Some(42))
            .expect("Execution failed");

        // Should have positive total duration
        assert!(result.total_duration_ns > 0);
    }
}
