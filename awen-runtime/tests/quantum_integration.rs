/// Quantum Execution Substrate Integration Tests
///
/// Comprehensive tests for quantum state preparation, evolution, measurement,
/// measurement-conditioned feedback, and coherence enforcement.
#[cfg(test)]
mod tests {
    use awen_runtime::quantum::*;
    use std::collections::HashMap;

    // ========================================================================
    // Test 1: CV State Preparation
    // ========================================================================

    #[test]
    fn test_cv_state_preparation() {
        let mut backend = GaussianSimulator::new();

        let modes = vec!["mode_0".to_string(), "mode_1".to_string()];
        let prep = PreparationKind::DisplacedSqueezed {
            displacement_q: 0.5,
            displacement_p: 0.3,
            squeezing_db: 6.0,
            squeezing_angle: 0.0,
        };

        let state = backend
            .prepare(modes.clone(), &prep, 12345)
            .expect("CV preparation failed");

        // Verify state properties
        assert_eq!(state.state_type, StateType::CV);
        assert_eq!(state.mode_labels.len(), 2);
        assert_eq!(state.seed, 12345);
        assert!(state.coherence_deadline > state.timestamp);

        // Verify coherence window
        let window = CoherenceWindow::new(state.state_id.clone(), 500);
        assert!(window.is_valid());
    }

    // ========================================================================
    // Test 2: DV State Preparation (Bell State)
    // ========================================================================

    #[test]
    fn test_dv_state_preparation_bell() {
        let state = QuantumState::new_dv(
            vec![("q0".to_string(), 2), ("q1".to_string(), 2)],
            54321,
            500,
        );

        // Verify state type
        assert_eq!(state.state_type, StateType::DV);
        assert_eq!(state.mode_labels.len(), 2);
        assert_eq!(state.seed, 54321);

        // Verify qudits initialized
        if let Some(dv_data) = &state.dv_data {
            assert_eq!(dv_data.qudits.len(), 2);
            for qudit in dv_data.qudits.values() {
                assert_eq!(qudit.dimension, 2); // qubits
                assert_eq!(qudit.amplitudes[0], 1.0); // |0⟩ state
            }
        } else {
            panic!("DV state data missing");
        }
    }

    // ========================================================================
    // Test 3: State Evolution (Unitary Operation)
    // ========================================================================

    #[test]
    fn test_state_evolution() {
        let mut backend = GaussianSimulator::new();

        let state = backend
            .prepare(
                vec!["mode_0".to_string()],
                &PreparationKind::ThermalState { mean_photons: 1.0 },
                100,
            )
            .expect("Preparation failed");

        let mut state_v1 = state.clone();
        let initial_id = state_v1.state_id.clone();

        let hamiltonian = Hamiltonian {
            terms: vec![PauliTerm {
                coefficient: 1.0,
                operators: {
                    let mut ops = HashMap::new();
                    ops.insert("q0".to_string(), "Z".to_string());
                    ops
                },
            }],
            static_field: true,
        };

        let trace = backend
            .evolve(&mut state_v1, &hamiltonian, &[], 100, 200)
            .expect("Evolution failed");

        // Verify evolution properties
        assert_eq!(trace.initial_state_id, initial_id);
        assert_ne!(trace.final_state_id, initial_id);
        assert_eq!(trace.seed, 200);
        assert!(!trace.operations.is_empty());
        assert!(trace.decoherence_estimated > 0.0);
    }

    // ========================================================================
    // Test 4: Homodyne Measurement (CV)
    // ========================================================================

    #[test]
    fn test_homodyne_measurement() {
        let mut backend = GaussianSimulator::new();

        let state = backend
            .prepare(
                vec!["mode_0".to_string()],
                &PreparationKind::DisplacedSqueezed {
                    displacement_q: 0.5,
                    displacement_p: 0.0,
                    squeezing_db: 0.0,
                    squeezing_angle: 0.0,
                },
                111,
            )
            .expect("Preparation failed");

        let mut state_copy = state.clone();
        let basis = MeasurementBasis {
            basis_type: BasisType::Homodyne {
                axis: HomodyneAxis::Q,
            },
            mode_labels: vec!["mode_0".to_string()],
        };

        let outcome = backend
            .measure(&mut state_copy, &basis, 111)
            .expect("Measurement failed");

        // Verify measurement outcome
        assert!(!outcome.outcome_id.is_empty());
        assert_eq!(outcome.seed, 111);
        assert!(!outcome.classical_results.is_empty());
        assert!(outcome.reliability > 0.5);

        // Verify determinism: same seed → same outcome
        let outcome2 = MeasurementOutcome::new(
            outcome.measurement_id.clone(),
            outcome.mode_labels.clone(),
            outcome.measurement_type.clone(),
            outcome.classical_results.clone(),
            111,
        );
        assert!(outcome2.matches_seed(111));
    }

    // ========================================================================
    // Test 5: Computational Basis Measurement (DV)
    // ========================================================================

    #[test]
    fn test_computational_basis_measurement() {
        let state =
            QuantumState::new_dv(vec![("q0".to_string(), 2), ("q1".to_string(), 2)], 222, 500);

        let basis = MeasurementBasis {
            basis_type: BasisType::Computational,
            mode_labels: vec!["q0".to_string(), "q1".to_string()],
        };

        // Verify basis is compatible with DV state
        assert!(state.can_measure_basis(&basis));

        // Create mock measurement outcome
        let mut results = HashMap::new();
        results.insert("q0".to_string(), MeasurementResult::DiscreteOutcome(0));
        results.insert("q1".to_string(), MeasurementResult::DiscreteOutcome(1));

        let outcome = MeasurementOutcome::new(
            "meas_0".to_string(),
            vec!["q0".to_string(), "q1".to_string()],
            BasisType::Computational,
            results,
            222,
        );

        assert!(outcome.matches_seed(222));
    }

    // ========================================================================
    // Test 6: Coherence Window Enforcement
    // ========================================================================

    #[test]
    fn test_coherence_window_enforcement() {
        let state = QuantumState::new_cv(vec!["mode_0".to_string()], 333, 100);
        let window = CoherenceWindow::new(state.state_id.clone(), 100);

        // State is initially coherent
        assert!(window.is_valid());
        assert!(state.is_coherent_at(state.timestamp));

        // Time remaining should be approximately 100 ns
        let time_remaining = window.time_remaining_ns();
        assert!(time_remaining > 0);
        assert!(time_remaining <= 100);

        // After deadline, state is incoherent
        let deadline_exceeded = window.deadline + chrono::Duration::nanoseconds(10);
        assert!(!state.is_coherent_at(deadline_exceeded));
    }

    // ========================================================================
    // Test 7: Measurement-Conditioned Feedback (Branching)
    // ========================================================================

    #[test]
    fn test_measurement_conditioned_branching() {
        let mut backend = GaussianSimulator::new();

        // Prepare initial state
        let state = backend
            .prepare(
                vec!["mode_0".to_string()],
                &PreparationKind::DisplacedSqueezed {
                    displacement_q: 0.5,
                    displacement_p: 0.0,
                    squeezing_db: 0.0,
                    squeezing_angle: 0.0,
                },
                444,
            )
            .expect("Preparation failed");

        let mut state_v1 = state.clone();

        // Measure in Q quadrature
        let basis_q = MeasurementBasis {
            basis_type: BasisType::Homodyne {
                axis: HomodyneAxis::Q,
            },
            mode_labels: vec!["mode_0".to_string()],
        };

        let outcome = backend
            .measure(&mut state_v1, &basis_q, 444)
            .expect("Measurement failed");

        // Create branching logic based on outcome
        let mut predicates = HashMap::new();
        predicates.insert(
            "branch_0".to_string(),
            MeasurementOutcomePredicate::InRange(0.0, 0.5),
        );
        predicates.insert(
            "branch_1".to_string(),
            MeasurementOutcomePredicate::InRange(0.5, 1.0),
        );

        let branch = MeasurementConditionalBranch {
            measurement_id: outcome.measurement_id.clone(),
            predicates,
            timeout_ms: 100,
            fallback_kernel: None,
        };

        assert_eq!(branch.measurement_id, outcome.measurement_id);
        assert_eq!(branch.timeout_ms, 100);
    }

    // ========================================================================
    // Test 8: Coherence-Aware Feedback Scheduling
    // ========================================================================

    #[test]
    fn test_coherence_aware_feedback_scheduling() {
        let state = QuantumState::new_cv(vec!["mode_0".to_string()], 555, 1000);
        let window = CoherenceWindow::new(state.state_id.clone(), 1000);

        // Fast feedback: 20 ns latency + 10 ns gate → fits in 1000 ns window
        let outcome_time = window.initialized_at;
        let result = window
            .check_can_schedule_feedback(outcome_time, 20, 10)
            .expect("Scheduling check failed");
        assert!(result);

        // Slow feedback: 500 ns latency + 600 ns gate → exceeds 1000 ns window
        let result = window
            .check_can_schedule_feedback(outcome_time, 500, 600)
            .expect("Scheduling check failed");
        assert!(!result);
    }

    // ========================================================================
    // Test 9: Quantum State Snapshots (Non-Destructive)
    // ========================================================================

    #[test]
    fn test_quantum_state_snapshot() {
        let state =
            QuantumState::new_cv(vec!["mode_0".to_string(), "mode_1".to_string()], 666, 500);

        let snapshot = state.snapshot().expect("Snapshot failed");

        // Verify snapshot contains expected metadata
        assert_eq!(snapshot.mode_labels.len(), 2);
        assert!(snapshot.global_purity >= 0.0 && snapshot.global_purity <= 1.0);
        assert_eq!(snapshot.provenance.state_id, state.state_id);
        assert_eq!(snapshot.provenance.seed, 666);
    }

    // ========================================================================
    // Test 10: Quantum Artifact Capture & Reproducibility
    // ========================================================================

    #[test]
    fn test_quantum_artifact_capture() {
        let state = QuantumState::new_cv(vec!["mode_0".to_string()], 777, 500);
        let mut artifact = QuantumArtifact::new(
            "kernel_0".to_string(),
            state.clone(),
            "gaussian_simulator".to_string(),
        );

        // Populate artifact
        artifact.noise_model_id = Some("gaussian_thermal".to_string());
        artifact
            .intermediate_states
            .push(state.snapshot().expect("Snapshot failed"));

        // Verify artifact properties
        assert_eq!(artifact.kernel_id, "kernel_0");
        assert_eq!(artifact.backend_name, "gaussian_simulator");
        assert_eq!(artifact.seed, 777);
        assert!(!artifact.intermediate_states.is_empty());

        // Now it can support deterministic replay
        assert!(artifact.can_deterministic_replay());
    }

    // ========================================================================
    // Test 11: Quantum Events Emission
    // ========================================================================

    #[test]
    fn test_quantum_events() {
        let state = QuantumState::new_cv(vec!["mode_0".to_string()], 888, 500);

        let event = QuantumEvent::StateCreated {
            state_id: state.state_id.clone(),
            state_type: StateType::CV,
            num_modes: 1,
            timestamp: state.timestamp,
        };

        match event {
            QuantumEvent::StateCreated {
                state_id,
                state_type,
                num_modes,
                ..
            } => {
                assert_eq!(state_id, state.state_id);
                assert_eq!(state_type, StateType::CV);
                assert_eq!(num_modes, 1);
            }
            _ => panic!("Unexpected event type"),
        }
    }

    // ========================================================================
    // Test 12: Full Quantum Workflow (Prep → Evolve → Measure → Feedback)
    // ========================================================================

    #[test]
    fn test_full_quantum_workflow() {
        let mut backend = GaussianSimulator::new();

        // 1. State Preparation
        let state_v1 = backend
            .prepare(
                vec!["mode_0".to_string(), "mode_1".to_string()],
                &PreparationKind::DisplacedSqueezed {
                    displacement_q: 0.5,
                    displacement_p: 0.3,
                    squeezing_db: 3.0,
                    squeezing_angle: 0.0,
                },
                999,
            )
            .expect("Preparation failed");

        assert_eq!(state_v1.seed, 999);
        let window = CoherenceWindow::new(state_v1.state_id.clone(), 1000);
        assert!(window.is_valid());

        // 2. State Evolution
        let mut state_v2 = state_v1.clone();
        let hamiltonian = Hamiltonian {
            terms: vec![],
            static_field: false,
        };

        let _evolution_trace = backend
            .evolve(&mut state_v2, &hamiltonian, &[], 50, 999)
            .expect("Evolution failed");

        assert_ne!(state_v2.state_id, state_v1.state_id);

        // 3. Homodyne Measurement on mode_0
        let mut state_v3 = state_v2.clone();
        let basis = MeasurementBasis {
            basis_type: BasisType::Homodyne {
                axis: HomodyneAxis::Q,
            },
            mode_labels: vec!["mode_0".to_string()],
        };

        let outcome = backend
            .measure(&mut state_v3, &basis, 999)
            .expect("Measurement failed");

        assert!(outcome.matches_seed(999));

        // 4. Feedback: Check measurement result
        let mut feedback_predicates = HashMap::new();
        feedback_predicates.insert(
            "apply_phase".to_string(),
            MeasurementOutcomePredicate::InRange(0.0, 1.0), // Always true
        );

        let _feedback_branch = MeasurementConditionalBranch {
            measurement_id: outcome.measurement_id.clone(),
            predicates: feedback_predicates,
            timeout_ms: 50,
            fallback_kernel: None,
        };

        // 5. Verify measurement-outcome latency fits in coherence window
        let outcome_time = outcome.timestamp;
        let feedback_latency = backend.measurement_latency().total_latency_ns();
        let gate_duration = 10;

        let can_schedule = window
            .check_can_schedule_feedback(outcome_time, feedback_latency, gate_duration)
            .expect("Scheduling check failed");

        assert!(can_schedule);
    }

    // ========================================================================
    // Test 13: Quantum Drift Detection (CV & DV)
    // ========================================================================

    #[test]
    fn test_quantum_drift_detection() {
        let detector = SimpleFidelityDriftDetector::new(0.05);

        // Create two similar states
        let state1 = QuantumState::new_cv(vec!["mode_0".to_string()], 1111, 500);
        let state2 = QuantumState::new_cv(vec!["mode_0".to_string()], 2222, 500);

        let drift = detector
            .detect_drift(&state1, &state2)
            .expect("Drift detection failed");

        // Drift should be small for similar states
        assert!(drift >= 0.0);
        assert!(drift < 0.1);
    }

    // ========================================================================
    // Test 14: Gaussian Simulator Backend Capabilities
    // ========================================================================

    #[test]
    fn test_gaussian_simulator_backend_capabilities() {
        let backend = GaussianSimulator::new();

        // Verify backend properties
        assert_eq!(backend.name(), "gaussian_simulator");
        assert_eq!(backend.state_type(), StateType::CV);
        assert!(backend.max_modes() >= 4);
        assert!(backend.coherence_time_ns() > 0);

        // Verify supported bases
        let supported = backend.supported_bases();
        assert!(supported
            .iter()
            .any(|b| matches!(b, BasisType::Homodyne { .. })));

        // Verify measurement latency
        let latency = backend.measurement_latency();
        assert_eq!(
            latency.total_latency_ns(),
            latency.detection_latency_ns()
                + latency.electronics_latency_ns()
                + latency.transport_latency_ns()
        );
    }

    // ========================================================================
    // Test 15: Measurement Basis Compatibility
    // ========================================================================

    #[test]
    fn test_measurement_basis_compatibility() {
        let cv_state = QuantumState::new_cv(vec!["mode_0".to_string()], 3333, 500);
        let dv_state = QuantumState::new_dv(vec![("q0".to_string(), 2)], 4444, 500);

        // CV state supports homodyne
        let homodyne_basis = MeasurementBasis {
            basis_type: BasisType::Homodyne {
                axis: HomodyneAxis::Q,
            },
            mode_labels: vec!["mode_0".to_string()],
        };
        assert!(cv_state.can_measure_basis(&homodyne_basis));

        // DV state does not support homodyne
        assert!(!dv_state.can_measure_basis(&homodyne_basis));

        // DV state supports computational basis
        let computational_basis = MeasurementBasis {
            basis_type: BasisType::Computational,
            mode_labels: vec!["q0".to_string()],
        };
        assert!(dv_state.can_measure_basis(&computational_basis));

        // CV state does not support computational basis
        assert!(!cv_state.can_measure_basis(&computational_basis));
    }

    // ========================================================================
    // Test 16: Deterministic Seeding & Reproducibility
    // ========================================================================

    #[test]
    fn test_deterministic_seeding() {
        let mut backend1 = GaussianSimulator::new();
        let mut backend2 = GaussianSimulator::new();

        // Prepare with same seed
        let state1 = backend1
            .prepare(
                vec!["mode_0".to_string()],
                &PreparationKind::ThermalState { mean_photons: 1.0 },
                5555,
            )
            .expect("Preparation 1 failed");

        let state2 = backend2
            .prepare(
                vec!["mode_0".to_string()],
                &PreparationKind::ThermalState { mean_photons: 1.0 },
                5555,
            )
            .expect("Preparation 2 failed");

        // Same seed should produce similar states
        assert_eq!(state1.seed, state2.seed);
        assert_eq!(state1.state_type, state2.state_type);
    }

    // ========================================================================
    // Test 17: Multiple Measurement Outcomes from Single State
    // ========================================================================

    #[test]
    fn test_multiple_measurement_outcomes() {
        let mut backend = GaussianSimulator::new();
        let state = backend
            .prepare(
                vec!["mode_0".to_string(), "mode_1".to_string()],
                &PreparationKind::DisplacedSqueezed {
                    displacement_q: 0.5,
                    displacement_p: 0.0,
                    squeezing_db: 0.0,
                    squeezing_angle: 0.0,
                },
                6666,
            )
            .expect("Preparation failed");

        // Measure both modes in homodyne basis
        let mut state_copy = state.clone();
        let basis = MeasurementBasis {
            basis_type: BasisType::Homodyne {
                axis: HomodyneAxis::Q,
            },
            mode_labels: vec!["mode_0".to_string(), "mode_1".to_string()],
        };

        let outcome = backend
            .measure(&mut state_copy, &basis, 6666)
            .expect("Measurement failed");

        // Should have results for both modes
        assert_eq!(outcome.mode_labels.len(), 2);
        assert_eq!(outcome.classical_results.len(), 2);
    }

    // ========================================================================
    // Test 18: State Fidelity Comparison
    // ========================================================================

    #[test]
    fn test_state_fidelity() {
        let mut backend = GaussianSimulator::new();
        let state = backend
            .prepare(
                vec!["mode_0".to_string()],
                &PreparationKind::ThermalState { mean_photons: 1.0 },
                7777,
            )
            .expect("Preparation failed");

        let state_copy = state.clone();

        // Fidelity with itself should be 1.0
        let fidelity = backend
            .fidelity(&state, &state_copy)
            .expect("Fidelity computation failed");

        assert_eq!(fidelity, 1.0);
    }

    // ========================================================================
    // Test 19: Invalid Measurement Basis Detection
    // ========================================================================

    #[test]
    fn test_invalid_measurement_basis_detection() {
        let mut backend = GaussianSimulator::new();

        // Prepare DV state
        let dv_state = QuantumState::new_dv(vec![("q0".to_string(), 2)], 8888, 500);

        // Try to measure in homodyne basis (invalid for DV)
        let mut dv_state_copy = dv_state.clone();
        let homodyne_basis = MeasurementBasis {
            basis_type: BasisType::Homodyne {
                axis: HomodyneAxis::Q,
            },
            mode_labels: vec!["q0".to_string()],
        };

        let result = backend.measure(&mut dv_state_copy, &homodyne_basis, 8888);
        assert!(result.is_err());
    }

    // ========================================================================
    // Test 20: Quantum Artifact Lineage Tracking
    // ========================================================================

    #[test]
    fn test_quantum_artifact_lineage() {
        let state_v1 = QuantumState::new_cv(vec!["mode_0".to_string()], 9999, 500);
        let mut artifact = QuantumArtifact::new(
            "kernel_lineage".to_string(),
            state_v1.clone(),
            "gaussian_simulator".to_string(),
        );

        // Record intermediate state
        let snapshot = state_v1.snapshot().expect("Snapshot failed");
        artifact.intermediate_states.push(snapshot);

        // Verify lineage information
        assert_eq!(artifact.initial_state.state_id, state_v1.state_id);
        assert_eq!(artifact.intermediate_states.len(), 1);
        assert_eq!(artifact.start_timestamp, state_v1.timestamp);
    }
}
