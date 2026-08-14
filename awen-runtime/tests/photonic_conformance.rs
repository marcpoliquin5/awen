use awen_runtime::chokepoint::{ExecContext, NonBypassableGateway};
use awen_runtime::photonic::{
    migrate_v5_document, ClassicalOperationFamily, ClassicalOperationKind, ClassicalProgram,
    InteropProgram, MigrationSeverity, MigrationStatus, PhotonicProgram, QuantumMeasurementFamily,
    QuantumProgram, QuantumResult,
};
use awen_runtime::ExecutionChokepoint;
use jsonschema::JSONSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .to_path_buf()
}

fn fixture<T: DeserializeOwned>(name: &str) -> T {
    serde_json::from_slice(
        &std::fs::read(root().join("awen-spec/fixtures").join(name)).expect("fixture bytes"),
    )
    .expect("typed fixture")
}

fn schema(name: &str) -> JSONSchema {
    let value: Value = serde_json::from_slice(
        &std::fs::read(root().join("awen-spec/schemas").join(name)).expect("schema bytes"),
    )
    .expect("schema JSON");
    JSONSchema::options()
        .compile(&value)
        .expect("schema compiles")
}

fn context(id: &str, timestamp: u64) -> ExecContext {
    ExecContext {
        run_id: id.to_string(),
        timestamp_ns: timestamp,
    }
}

#[test]
fn independent_classical_quantum_result_and_interop_schemas_match_rust_contracts() {
    let classical: ClassicalProgram = fixture("classical_photonic_program.json");
    let quantum: QuantumProgram = fixture("quantum_photonic_program.json");
    let result: QuantumResult = fixture("quantum_photonic_result.json");
    let interop: InteropProgram = fixture("photonic_interop_program.json");

    classical.validate().expect("classical verifier");
    quantum.validate().expect("quantum verifier");
    let mut sealed_result = result.clone();
    sealed_result
        .seal_replay(&quantum)
        .expect("seal reference replay evidence");
    assert_eq!(
        result.program_fingerprint,
        sealed_result.program_fingerprint
    );
    assert_eq!(result.replay_fingerprint, sealed_result.replay_fingerprint);
    result.validate_against(&quantum).expect("result verifier");
    interop.validate().expect("interop verifier");

    for (schema_name, instance) in [
        (
            "awen_photonic_program.v1.json",
            serde_json::to_value(&classical).expect("classical value"),
        ),
        (
            "awen_qphotonic_program.v1.json",
            serde_json::to_value(&quantum).expect("quantum value"),
        ),
        (
            "awen_qphotonic_result.v1.json",
            serde_json::to_value(&result).expect("result value"),
        ),
        (
            "awen_photonic_interop.v1.json",
            serde_json::to_value(&interop).expect("interop value"),
        ),
    ] {
        let validator = schema(schema_name);
        if let Err(errors) = validator.validate(&instance) {
            panic!(
                "{schema_name} rejected its Rust value: {}",
                errors
                    .map(|error| error.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        };
    }
}

#[test]
fn typed_gateway_preserves_dialect_contracts_instead_of_dispatching_strings() {
    let gateway = NonBypassableGateway::new();
    let classical =
        PhotonicProgram::Classical(Box::new(fixture("classical_photonic_program.json")));
    let quantum = PhotonicProgram::Quantum(Box::new(fixture("quantum_photonic_program.json")));
    let interop = PhotonicProgram::Interop(Box::new(fixture("photonic_interop_program.json")));

    for (index, program) in [classical, quantum, interop].iter().enumerate() {
        let result = gateway.execute(program, &context(&format!("typed-{index}"), index as u64));
        assert!(
            result.ok,
            "typed gateway rejected program: {:?}",
            result.details
        );
    }

    let traversal = gateway.execute(
        &PhotonicProgram::Classical(Box::new(fixture("classical_photonic_program.json"))),
        &context("../escape", 99),
    );
    assert!(!traversal.ok, "artifact path traversal run id was accepted");
}

#[test]
fn classical_gemm_and_quantum_operations_cannot_cross_validate() {
    let quantum: QuantumProgram = fixture("quantum_photonic_program.json");
    let quantum_value = serde_json::to_value(&quantum).expect("quantum value");
    assert!(serde_json::from_value::<ClassicalProgram>(quantum_value.clone()).is_err());
    assert!(!schema("awen_photonic_program.v1.json").is_valid(&quantum_value));

    let mut classical_gemm: ClassicalProgram = fixture("classical_photonic_program.json");
    classical_gemm.signals[0].shape = vec![4, 4];
    classical_gemm.signals[1].shape = vec![4, 4];
    classical_gemm.operations[0].inputs = vec!["input".to_string(), "input".to_string()];
    classical_gemm.operations[0].outputs = vec!["output-a".to_string()];
    classical_gemm.operations[0].kind = ClassicalOperationKind::Gemm {
        m: 4,
        n: 4,
        k: 4,
        transpose_lhs: false,
        transpose_rhs: false,
    };
    classical_gemm.capabilities.operations =
        std::collections::BTreeSet::from([ClassicalOperationFamily::Gemm]);
    classical_gemm.outputs = vec!["output-a".to_string()];
    classical_gemm.validate().expect("classical GEMM verifier");
    let mut invalid_shape = classical_gemm.clone();
    invalid_shape.signals[1].shape = vec![4, 3];
    assert!(invalid_shape.validate().is_err());
    let classical_value = serde_json::to_value(classical_gemm).expect("classical GEMM value");
    assert!(!schema("awen_qphotonic_program.v1.json").is_valid(&classical_value));
    assert!(serde_json::from_value::<QuantumProgram>(classical_value).is_err());
}

#[test]
fn dialect_verifiers_keep_precision_calibration_measurement_and_replay_semantics() {
    let gateway = NonBypassableGateway::new();
    let mut classical: ClassicalProgram = fixture("classical_photonic_program.json");
    classical.operations[0].precision.optical_effective_bits = 0;
    let rejected = gateway.execute(
        &PhotonicProgram::Classical(Box::new(classical)),
        &context("bad-precision", 1),
    );
    assert!(!rejected.ok);

    let mut classical: ClassicalProgram = fixture("classical_photonic_program.json");
    classical.operations[0].transfer.calibration_fingerprint = "mutable".to_string();
    assert!(
        !gateway
            .execute(
                &PhotonicProgram::Classical(Box::new(classical)),
                &context("bad-calibration", 2),
            )
            .ok
    );

    let mut mistimed: ClassicalProgram = fixture("classical_photonic_program.json");
    let mut output_c = mistimed.signals[1].clone();
    output_c.id = "output-c".to_string();
    mistimed.signals.push(output_c);
    let mut phase = mistimed.operations[0].clone();
    phase.op_id = "phase".to_string();
    phase.inputs = vec!["output-a".to_string()];
    phase.outputs = vec!["output-c".to_string()];
    phase.kind = ClassicalOperationKind::AnalogTransform {
        transform: awen_runtime::photonic::AnalogTransformKind::PhaseShift,
        phase_radians: Some(0.25),
        power_ratio: None,
    };
    phase.timing.start_ns = 10;
    phase.timing.dependencies = vec!["split".to_string()];
    mistimed.operations.push(phase);
    assert!(mistimed.validate().is_err());

    let mut quantum: QuantumProgram = fixture("quantum_photonic_program.json");
    let measurement = quantum
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation.kind,
                awen_runtime::photonic::QuantumOperationKind::Measure { .. }
            )
        })
        .expect("measurement");
    if let awen_runtime::photonic::QuantumOperationKind::Measure { basis, .. } =
        &mut measurement.kind
    {
        *basis = QuantumMeasurementFamily::HomodyneQ;
    }
    assert!(quantum.validate().is_err());

    let quantum: QuantumProgram = fixture("quantum_photonic_program.json");
    let mut result: QuantumResult = fixture("quantum_photonic_result.json");
    result.outcome_counts.insert("00".to_string(), 40);
    result.outcome_counts.insert("11".to_string(), 60);
    assert!(result.validate_against(&quantum).is_err());
}

#[test]
fn gaussian_cv_measurement_feed_forward_and_coherence_are_verified_independently() {
    let gateway = NonBypassableGateway::new();
    let gaussian: QuantumProgram = fixture("quantum_gaussian_feed_forward.json");
    gaussian
        .validate()
        .expect("Gaussian CV feed-forward verifier");
    assert!(schema("awen_qphotonic_program.v1.json")
        .is_valid(&serde_json::to_value(&gaussian).expect("Gaussian value")));
    assert!(
        gateway
            .execute(
                &PhotonicProgram::Quantum(Box::new(gaussian.clone())),
                &context("gaussian-feed-forward", 3),
            )
            .ok
    );

    let mut invalid_target = gaussian.clone();
    invalid_target.operations.swap(1, 2);
    assert!(invalid_target.validate().is_err());

    let mut over_budget = gaussian;
    over_budget.execution.coherence_budget_ns = 20;
    assert!(over_budget.validate().is_err());

    let mut invalid_covariance: QuantumProgram = fixture("quantum_gaussian_feed_forward.json");
    if let awen_runtime::photonic::QuantumInitialState::Gaussian { covariance, .. } =
        &mut invalid_covariance.initial_state
    {
        *covariance = vec![vec![1.0, 2.0], vec![2.0, 1.0]];
    }
    assert!(invalid_covariance.validate().is_err());

    let mut reused_destroyed_mode: QuantumProgram = fixture("quantum_photonic_program.json");
    reused_destroyed_mode
        .operations
        .push(awen_runtime::photonic::QuantumOperation {
            op_id: "after-measurement".to_string(),
            modes: vec!["q0".to_string()],
            coherence_cost_ns: 1,
            kind: awen_runtime::photonic::QuantumOperationKind::Gate {
                gate_spec: awen_runtime::photonic::QuantumGate::Fourier,
            },
        });
    assert!(reused_destroyed_mode.validate().is_err());
}

#[test]
fn v5_migration_classifies_prefixed_ops_and_diagnoses_ambiguous_semantics() {
    let legacy: Value = fixture("photonic_v5_ambiguous.json");
    let report = migrate_v5_document(&legacy).expect("migration report");
    assert_eq!(report.status, MigrationStatus::Rejected);
    assert_eq!(report.operations.len(), 2);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == MigrationSeverity::Error
            && diagnostic.code == "ambiguous_or_unsupported_operation"
            && diagnostic.op_id.as_deref() == Some("unknown-measurement")
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == MigrationSeverity::Warning
            && diagnostic.code == "explicit_interop_required"
    }));
    let report_value = serde_json::to_value(report).expect("report value");
    assert!(schema("awen_photonic_v5_migration.v1.json").is_valid(&report_value));
    let mut contradictory = report_value.clone();
    contradictory["operations"][0]["gate"] = serde_json::json!("fourier");
    assert!(!schema("awen_photonic_v5_migration.v1.json").is_valid(&contradictory));
    let mut inconsistent_status = report_value;
    inconsistent_status["status"] = serde_json::json!("migrated");
    assert!(!schema("awen_photonic_v5_migration.v1.json").is_valid(&inconsistent_status));
    assert!(migrate_v5_document(&serde_json::json!({"ir_version":"v5","ops":[]})).is_err());
}

#[test]
fn migration_cli_preserves_a_rejected_report_and_succeeds_after_ambiguity_is_removed() {
    let temporary = tempfile::tempdir().expect("temporary migration directory");
    let rejected_output = temporary.path().join("rejected.json");
    let rejected = Command::new(env!("CARGO_BIN_EXE_awenctl"))
        .args([
            "migrate-photonic-v5",
            root()
                .join("awen-spec/fixtures/photonic_v5_ambiguous.json")
                .to_str()
                .expect("legacy path"),
            "--output",
            rejected_output.to_str().expect("rejected report path"),
        ])
        .output()
        .expect("rejected migration CLI");
    assert!(!rejected.status.success());
    let report: Value = serde_json::from_slice(
        &std::fs::read(&rejected_output).expect("preserved rejected report"),
    )
    .expect("rejected report JSON");
    assert_eq!(report["status"], "rejected");

    let mut clear: Value = fixture("photonic_v5_ambiguous.json");
    clear["ops"].as_array_mut().expect("ops array").pop();
    let clear_input = temporary.path().join("clear-v5.json");
    std::fs::write(
        &clear_input,
        serde_json::to_vec_pretty(&clear).expect("clear legacy JSON"),
    )
    .expect("clear legacy fixture");
    let migrated_output = temporary.path().join("migrated.json");
    let migrated = Command::new(env!("CARGO_BIN_EXE_awenctl"))
        .args([
            "migrate-photonic-v5",
            clear_input.to_str().expect("clear input path"),
            "--output",
            migrated_output.to_str().expect("migrated report path"),
        ])
        .output()
        .expect("successful migration CLI");
    assert!(
        migrated.status.success(),
        "{}",
        String::from_utf8_lossy(&migrated.stderr)
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(&migrated_output).expect("migrated report bytes"))
            .expect("migrated report JSON");
    assert_eq!(report["status"], "migrated");
    assert!(schema("awen_photonic_v5_migration.v1.json").is_valid(&report));
}
