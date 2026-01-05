// AWEN Calibration Integration Tests
// End-to-end validation of calibration, drift detection, and recalibration loops

use awen_runtime::calibration::{
    CalibrationExecutor, CalibrationKernel, CalibrationSchedule, CalibrationState, CostFunction,
    DriftDetector, Measurement, MeasurementAction, MeasurementStep, OptimizerAlgorithm,
    OptimizerConfig, RecalibrationAction, ReferenceCalibrationExecutor, SafetyConstraints,
    ThresholdDriftDetector, Urgency,
};
use std::collections::HashMap;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 1: Basic Calibration Execution
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_calibration_execution() {
    let kernel = CalibrationKernel {
        id: "mzi_calibration".to_string(),
        target_nodes: vec!["mzi_0".to_string()],
        parameters_to_tune: vec!["phase_upper".to_string()],
        cost_function: CostFunction::Minimize {
            expression: "1.0 - extinction_ratio".to_string(),
            target_value: Some(0.01),
        },
        measurement_sequence: vec![MeasurementStep {
            step_id: "measure_extinction".to_string(),
            action: MeasurementAction::ReadSensor {
                sensor_id: "detector_0".to_string(),
                integration_time_ns: 1000,
            },
            expected_duration_ns: 2000,
        }],
        optimizer_config: OptimizerConfig {
            algorithm: OptimizerAlgorithm::NelderMead {
                initial_simplex_size: 0.1,
            },
            max_iterations: 50,
            convergence_threshold: 0.01,
            initial_guess: None,
        },
        safety_constraints: SafetyConstraints::default(),
        schedule: CalibrationSchedule::PreRun,
    };

    let executor = ReferenceCalibrationExecutor::new();
    let calibration_state = executor
        .execute_calibration(&kernel, None)
        .expect("Calibration failed");

    // Verify calibration state
    assert_eq!(calibration_state.version, 1);
    assert_eq!(calibration_state.node_calibrations.len(), 1);
    assert!(calibration_state.node_calibrations.contains_key("mzi_0"));

    let node_calib = &calibration_state.node_calibrations["mzi_0"];
    assert!(node_calib.parameters.contains_key("phase_upper"));
    assert!(node_calib.metadata.convergence_iterations > 0);
    assert!(node_calib.metadata.cost_function_value < 1.0);

    println!(
        "✓ Calibration executed: version {}, cost = {:.4}",
        calibration_state.version, node_calib.metadata.cost_function_value
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 2: Calibration State Versioning
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_calibration_versioning() {
    let kernel = create_test_calibration_kernel();
    let executor = ReferenceCalibrationExecutor::new();

    // Execute initial calibration
    let state_v1 = executor
        .execute_calibration(&kernel, None)
        .expect("Calibration v1 failed");

    assert_eq!(state_v1.version, 1);
    assert!(state_v1.provenance.parent_calibration_id.is_none());

    // Execute recalibration (version should increment)
    let state_v2 = executor
        .execute_calibration(&kernel, Some(&state_v1))
        .expect("Calibration v2 failed");

    assert_eq!(state_v2.version, 2);
    assert_eq!(
        state_v2.provenance.parent_calibration_id,
        Some(state_v1.calibration_id.clone())
    );

    // Execute another recalibration
    let state_v3 = executor
        .execute_calibration(&kernel, Some(&state_v2))
        .expect("Calibration v3 failed");

    assert_eq!(state_v3.version, 3);
    assert_eq!(
        state_v3.provenance.parent_calibration_id,
        Some(state_v2.calibration_id.clone())
    );

    println!("✓ Calibration versioning: v1 → v2 → v3 with parent tracking");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 3: Drift Detection
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_drift_detection_no_drift() {
    let calibration_state = create_test_calibration_state();

    // Measurements match calibrated values (no drift)
    let measurements = vec![Measurement {
        measurement_id: "meas-001".to_string(),
        timestamp_ns: 1000,
        sensor_id: "phase".to_string(),
        value: 1.0, // Matches nominal
        unit: "radians".to_string(),
    }];

    let detector = ThresholdDriftDetector::new(0.1); // 10% threshold
    let report = detector
        .detect_drift(&calibration_state, &measurements)
        .expect("Drift detection failed");

    assert!(!report.drift_detected);
    assert_eq!(report.recommended_action, RecalibrationAction::NoAction);

    println!("✓ Drift detection: no drift detected within 10% threshold");
}

#[test]
fn test_drift_detection_medium_drift() {
    let calibration_state = create_test_calibration_state();

    // Measurement shows 15% drift
    let measurements = vec![Measurement {
        measurement_id: "meas-002".to_string(),
        timestamp_ns: 2000,
        sensor_id: "phase".to_string(),
        value: 1.15, // 15% above nominal 1.0
        unit: "radians".to_string(),
    }];

    let detector = ThresholdDriftDetector::new(0.1); // 10% threshold
    let report = detector
        .detect_drift(&calibration_state, &measurements)
        .expect("Drift detection failed");

    assert!(report.drift_detected);
    assert_eq!(report.drift_metrics.len(), 1);
    assert!(report.drift_metrics[0].threshold_exceeded);
    assert!(report.drift_metrics[0].delta > 0.1);

    match report.recommended_action {
        RecalibrationAction::Recalibrate { urgency, .. } => {
            assert_eq!(urgency, Urgency::Medium);
        }
        _ => panic!("Expected Recalibrate action"),
    }

    println!("✓ Drift detection: medium drift (15%) detected, urgency = Medium");
}

#[test]
fn test_drift_detection_high_drift() {
    let calibration_state = create_test_calibration_state();

    // Measurement shows 25% drift (high urgency)
    let measurements = vec![Measurement {
        measurement_id: "meas-003".to_string(),
        timestamp_ns: 3000,
        sensor_id: "phase".to_string(),
        value: 1.25, // 25% above nominal
        unit: "radians".to_string(),
    }];

    let detector = ThresholdDriftDetector::new(0.1); // 10% threshold
    let report = detector
        .detect_drift(&calibration_state, &measurements)
        .expect("Drift detection failed");

    assert!(report.drift_detected);

    match report.recommended_action {
        RecalibrationAction::Recalibrate { urgency, .. } => {
            assert_eq!(urgency, Urgency::High); // >2x threshold → High urgency
        }
        _ => panic!("Expected Recalibrate action"),
    }

    println!("✓ Drift detection: high drift (25%) detected, urgency = High");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 4: Full Calibration Loop (Run → Drift → Recalibrate)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_full_calibration_loop() {
    let kernel = create_test_calibration_kernel();
    let executor = ReferenceCalibrationExecutor::new();

    // Step 1: Initial calibration
    let state_v1 = executor
        .execute_calibration(&kernel, None)
        .expect("Initial calibration failed");

    assert_eq!(state_v1.version, 1);
    println!("Step 1: Initial calibration (v1) complete");

    // Step 2: Simulate drift
    let measurements = vec![Measurement {
        measurement_id: "drift-meas".to_string(),
        timestamp_ns: 5000,
        sensor_id: "phase".to_string(),
        value: 1.2, // 20% drift
        unit: "radians".to_string(),
    }];

    // Step 3: Detect drift
    let detector = ThresholdDriftDetector::new(0.1);
    let drift_report = detector
        .detect_drift(&state_v1, &measurements)
        .expect("Drift detection failed");

    assert!(drift_report.drift_detected);
    println!("Step 2: Drift detected (20% above nominal)");

    // Step 4: Recalibrate
    let state_v2 = executor
        .execute_calibration(&kernel, Some(&state_v1))
        .expect("Recalibration failed");

    assert_eq!(state_v2.version, 2);
    assert_eq!(
        state_v2.provenance.parent_calibration_id,
        Some(state_v1.calibration_id.clone())
    );
    println!("Step 3: Recalibration (v2) complete, parent = v1");

    // Step 5: Verify provenance lineage
    assert_eq!(state_v1.version, 1);
    assert_eq!(state_v2.version, 2);
    assert!(state_v1.provenance.parent_calibration_id.is_none());
    assert!(state_v2.provenance.parent_calibration_id.is_some());

    println!("✓ Full calibration loop: v1 → drift → v2 with provenance tracked");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 5: Safety Constraint Validation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_safety_constraints_hard_limit_violation() {
    let executor = ReferenceCalibrationExecutor::new();

    // Create calibration state with parameter exceeding hard limit
    let mut node_params = HashMap::new();
    node_params.insert("voltage".to_string(), 15.0); // Exceeds 10V limit

    let calibration_state = CalibrationState {
        calibration_id: "calib-unsafe".to_string(),
        version: 1,
        timestamp: chrono::Utc::now().to_rfc3339(),
        node_calibrations: {
            let mut map = HashMap::new();
            map.insert(
                "mzi_0".to_string(),
                awen_runtime::calibration::NodeCalibration {
                    node_id: "mzi_0".to_string(),
                    parameters: node_params,
                    metadata: awen_runtime::calibration::NodeCalibrationMetadata {
                        cost_function_value: 0.01,
                        convergence_iterations: 10,
                        measurement_snr_db: 20.0,
                        confidence: 0.95,
                        calibration_duration_seconds: 1.0,
                    },
                },
            );
            map
        },
        provenance: awen_runtime::calibration::CalibrationProvenance::default(),
    };

    // Safety constraints with hard limit
    let mut safety = SafetyConstraints::default();
    safety
        .hard_limits
        .insert("voltage".to_string(), (0.0, 10.0));

    let result = executor.apply_calibration(&calibration_state, &safety);

    assert!(result.is_err());
    println!("✓ Safety constraint violation: hard limit (10V) enforced, rejected 15V");
}

#[test]
fn test_safety_constraints_within_limits() {
    let executor = ReferenceCalibrationExecutor::new();

    // Create calibration state with parameter within limits
    let mut node_params = HashMap::new();
    node_params.insert("voltage".to_string(), 5.0); // Within 10V limit

    let calibration_state = CalibrationState {
        calibration_id: "calib-safe".to_string(),
        version: 1,
        timestamp: chrono::Utc::now().to_rfc3339(),
        node_calibrations: {
            let mut map = HashMap::new();
            map.insert(
                "mzi_0".to_string(),
                awen_runtime::calibration::NodeCalibration {
                    node_id: "mzi_0".to_string(),
                    parameters: node_params,
                    metadata: awen_runtime::calibration::NodeCalibrationMetadata {
                        cost_function_value: 0.01,
                        convergence_iterations: 10,
                        measurement_snr_db: 20.0,
                        confidence: 0.95,
                        calibration_duration_seconds: 1.0,
                    },
                },
            );
            map
        },
        provenance: awen_runtime::calibration::CalibrationProvenance::default(),
    };

    let mut safety = SafetyConstraints::default();
    safety
        .hard_limits
        .insert("voltage".to_string(), (0.0, 10.0));

    let result = executor.apply_calibration(&calibration_state, &safety);

    assert!(result.is_ok());
    println!("✓ Safety constraint validation: 5V within 10V limit, accepted");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 6: Optimizer Convergence
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_optimizer_convergence() {
    let kernel = CalibrationKernel {
        id: "optimizer_test".to_string(),
        target_nodes: vec!["test_node".to_string()],
        parameters_to_tune: vec!["param1".to_string(), "param2".to_string()],
        cost_function: CostFunction::Minimize {
            expression: "param_sum".to_string(),
            target_value: Some(0.0),
        },
        measurement_sequence: vec![MeasurementStep {
            step_id: "m0".to_string(),
            action: MeasurementAction::ReadSensor {
                sensor_id: "phase".to_string(),
                integration_time_ns: 1000,
            },
            expected_duration_ns: 1000,
        }],
        optimizer_config: OptimizerConfig {
            algorithm: OptimizerAlgorithm::NelderMead {
                initial_simplex_size: 0.5,
            },
            max_iterations: 100,
            convergence_threshold: 0.01,
            initial_guess: Some({
                let mut guess = HashMap::new();
                guess.insert("param1".to_string(), 1.0);
                guess.insert("param2".to_string(), 1.0);
                guess
            }),
        },
        safety_constraints: SafetyConstraints::default(),
        schedule: CalibrationSchedule::PreRun,
    };

    let executor = ReferenceCalibrationExecutor::new();
    let state = executor
        .execute_calibration(&kernel, None)
        .expect("Calibration failed");

    let node_calib = &state.node_calibrations["test_node"];

    // Verify convergence
    assert!(node_calib.metadata.cost_function_value < 0.1);
    assert!(node_calib.metadata.convergence_iterations <= 100);

    println!(
        "✓ Optimizer convergence: {} iterations, final cost = {:.6}",
        node_calib.metadata.convergence_iterations, node_calib.metadata.cost_function_value
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 7: Calibration State Serialization
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_calibration_state_serialization() {
    let state = create_test_calibration_state();

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&state).expect("Serialization failed");

    // Deserialize back
    let deserialized: CalibrationState =
        serde_json::from_str(&json).expect("Deserialization failed");

    // Verify round-trip
    assert_eq!(state.calibration_id, deserialized.calibration_id);
    assert_eq!(state.version, deserialized.version);
    assert_eq!(
        state.node_calibrations.len(),
        deserialized.node_calibrations.len()
    );

    println!("✓ Calibration state serialization: JSON round-trip successful");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 8: Multiple Node Calibration
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_multiple_node_calibration() {
    let kernel = CalibrationKernel {
        id: "multi_node_calibration".to_string(),
        target_nodes: vec![
            "mzi_0".to_string(),
            "mzi_1".to_string(),
            "mzi_2".to_string(),
        ],
        parameters_to_tune: vec!["phase".to_string()],
        cost_function: CostFunction::Minimize {
            expression: "total_loss".to_string(),
            target_value: Some(0.05),
        },
        measurement_sequence: vec![],
        optimizer_config: OptimizerConfig {
            algorithm: OptimizerAlgorithm::NelderMead {
                initial_simplex_size: 0.1,
            },
            max_iterations: 50,
            convergence_threshold: 0.01,
            initial_guess: None,
        },
        safety_constraints: SafetyConstraints::default(),
        schedule: CalibrationSchedule::PreRun,
    };

    let executor = ReferenceCalibrationExecutor::new();
    let state = executor
        .execute_calibration(&kernel, None)
        .expect("Multi-node calibration failed");

    // Verify all nodes calibrated
    assert_eq!(state.node_calibrations.len(), 3);
    assert!(state.node_calibrations.contains_key("mzi_0"));
    assert!(state.node_calibrations.contains_key("mzi_1"));
    assert!(state.node_calibrations.contains_key("mzi_2"));

    for (node_id, node_calib) in &state.node_calibrations {
        assert!(node_calib.parameters.contains_key("phase"));
        println!(
            "  - Node {}: phase = {:.3}",
            node_id, node_calib.parameters["phase"]
        );
    }

    println!("✓ Multiple node calibration: 3 nodes calibrated successfully");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 9: Calibration Provenance Tracking
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_calibration_provenance_tracking() {
    let kernel = create_test_calibration_kernel();
    let executor = ReferenceCalibrationExecutor::new();

    let state = executor
        .execute_calibration(&kernel, None)
        .expect("Calibration failed");

    // Verify provenance fields
    assert_eq!(state.provenance.calibration_kernel_id, "test_kernel");
    assert!(state.provenance.optimizer_algorithm.contains("NelderMead"));
    assert!(state.provenance.measurement_count > 0);
    assert_eq!(state.provenance.hardware_revision, "v0.2");
    assert!(state.provenance.temperature_c.is_some());
    assert!(state.provenance.seed.is_some());

    println!(
        "✓ Calibration provenance: kernel_id = {}, algorithm = {}, measurements = {}",
        state.provenance.calibration_kernel_id,
        state.provenance.optimizer_algorithm,
        state.provenance.measurement_count
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Test 10: Get Current Calibration
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_get_current_calibration() {
    let kernel = create_test_calibration_kernel();
    let executor = ReferenceCalibrationExecutor::new();

    // Execute calibration
    let state_v1 = executor
        .execute_calibration(&kernel, None)
        .expect("Calibration failed");

    // Retrieve current calibration
    let current = executor
        .get_current_calibration()
        .expect("Failed to get current calibration");

    assert_eq!(current.calibration_id, state_v1.calibration_id);
    assert_eq!(current.version, 1);

    // Execute recalibration
    let state_v2 = executor
        .execute_calibration(&kernel, Some(&state_v1))
        .expect("Recalibration failed");

    // Retrieve updated calibration
    let current = executor
        .get_current_calibration()
        .expect("Failed to get current calibration");

    assert_eq!(current.calibration_id, state_v2.calibration_id);
    assert_eq!(current.version, 2);

    println!("✓ Get current calibration: v1 → v2 state tracked correctly");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Helper Functions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn create_test_calibration_kernel() -> CalibrationKernel {
    CalibrationKernel {
        id: "test_kernel".to_string(),
        target_nodes: vec!["mzi_0".to_string()],
        parameters_to_tune: vec!["phase".to_string()],
        cost_function: CostFunction::Minimize {
            expression: "loss".to_string(),
            target_value: Some(0.01),
        },
        measurement_sequence: vec![MeasurementStep {
            step_id: "measure_phase".to_string(),
            action: MeasurementAction::ReadSensor {
                sensor_id: "phase".to_string(),
                integration_time_ns: 1000,
            },
            expected_duration_ns: 1500,
        }],
        optimizer_config: OptimizerConfig {
            algorithm: OptimizerAlgorithm::NelderMead {
                initial_simplex_size: 0.1,
            },
            max_iterations: 50,
            convergence_threshold: 0.01,
            initial_guess: Some({
                let mut guess = HashMap::new();
                guess.insert("phase".to_string(), 0.5);
                guess
            }),
        },
        safety_constraints: SafetyConstraints::default(),
        schedule: CalibrationSchedule::PreRun,
    }
}

fn create_test_calibration_state() -> CalibrationState {
    CalibrationState {
        calibration_id: "test-calib-001".to_string(),
        version: 1,
        timestamp: chrono::Utc::now().to_rfc3339(),
        node_calibrations: {
            let mut map = HashMap::new();
            map.insert(
                "mzi_0".to_string(),
                awen_runtime::calibration::NodeCalibration {
                    node_id: "mzi_0".to_string(),
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert("phase".to_string(), 1.0);
                        params
                    },
                    metadata: awen_runtime::calibration::NodeCalibrationMetadata {
                        cost_function_value: 0.01,
                        convergence_iterations: 10,
                        measurement_snr_db: 20.0,
                        confidence: 0.95,
                        calibration_duration_seconds: 1.0,
                    },
                },
            );
            map
        },
        provenance: awen_runtime::calibration::CalibrationProvenance::default(),
    }
}
