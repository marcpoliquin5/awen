//! Artifact storage integration tests (Phase 2.6.1)
//!
//! Tests core artifact lifecycle: bundle creation, export, deterministic ID generation.
//! Full round-trip testing deferred to Phase 2.6.1 (import functionality).

use awen_runtime::ir::{Graph, Node};
use awen_runtime::storage::{
    capture_environment, compute_deterministic_id, export_bundle, short_id, ArtifactType,
    BundleBuilder, ExportFormat,
};
use std::collections::HashMap;
use tempfile::TempDir;

// Helper: Create a simple test IR graph
fn create_test_graph() -> Graph {
    Graph {
        nodes: vec![Node {
            id: "x0".to_string(),
            node_type: "RX".to_string(),
            params: [(String::from("theta"), std::f64::consts::FRAC_PI_2)]
                .iter()
                .cloned()
                .collect(),
            measure_mode: None,
            conditional_branches: None,
        }],
        edges: vec![],
        metadata: HashMap::new(),
    }
}

// Helper: Create test parameters
fn create_test_params() -> HashMap<String, f64> {
    let mut params = HashMap::new();
    params.insert("theta".to_string(), std::f64::consts::FRAC_PI_2);
    params
}

#[test]
fn test_01_artifact_bundle_creation() {
    let ir = create_test_graph();
    let params = create_test_params();

    let bundle = BundleBuilder::new(ir, ArtifactType::Run)
        .with_initial_parameters(params)
        .with_results(serde_json::json!({"measurements": [0, 1, 0]}))
        .build()
        .expect("Should create bundle");

    assert_eq!(bundle.artifact_type, ArtifactType::Run);
    assert!(!bundle.artifact_id.is_empty());
    assert!(!bundle.manifest.created_at.is_empty());
}

#[test]
fn test_02_artifact_deterministic_id() {
    let ir = create_test_graph();
    let params = create_test_params();

    let id1 =
        compute_deterministic_id(&ir, &params, None, Some(42), "0.6.0").expect("Should compute ID");
    let id2 =
        compute_deterministic_id(&ir, &params, None, Some(42), "0.6.0").expect("Should compute ID");

    // Same inputs → same ID
    assert_eq!(id1, id2);
    assert!(id1.starts_with("awen_"));
    assert_eq!(id1.len(), 69); // "awen_" (5) + 64 hex chars
}

#[test]
fn test_03_artifact_short_id() {
    let ir = create_test_graph();
    let params = create_test_params();

    let full_id =
        compute_deterministic_id(&ir, &params, None, Some(42), "0.6.0").expect("Should compute ID");
    let short = short_id(&full_id);

    assert!(short.starts_with("awen_"));
    assert_eq!(short.len(), 21); // "awen_" (5) + 16 hex chars
    assert!(full_id.starts_with(short));
}

#[test]
fn test_04_artifact_export_directory() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let artifacts_dir = temp_dir.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("Should create artifacts dir");

    let ir = create_test_graph();
    let params = create_test_params();

    let bundle = BundleBuilder::new(ir, ArtifactType::Run)
        .with_initial_parameters(params)
        .with_results(serde_json::json!({"measurements": [0, 1, 0]}))
        .build()
        .expect("Should create bundle");

    let export_path = export_bundle(&bundle, &artifacts_dir, ExportFormat::Directory)
        .expect("Should export bundle");

    assert!(export_path.exists());
    assert!(export_path.join("manifest.json").exists());
    assert!(export_path.join("ir").exists());
    assert!(export_path.join("parameters").exists());
}

#[test]
fn test_05_artifact_manifest_json() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let artifacts_dir = temp_dir.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("Should create artifacts dir");

    let ir = create_test_graph();
    let params = create_test_params();

    let bundle = BundleBuilder::new(ir, ArtifactType::Run)
        .with_initial_parameters(params)
        .with_results(serde_json::json!({"measurements": [0, 1, 0]}))
        .build()
        .expect("Should create bundle");

    let export_path = export_bundle(&bundle, &artifacts_dir, ExportFormat::Directory)
        .expect("Should export bundle");

    let manifest_content =
        std::fs::read_to_string(export_path.join("manifest.json")).expect("Should read manifest");

    assert!(manifest_content.contains("\"schema_version\""));
    assert!(manifest_content.contains("\"artifact_id\""));
    assert!(manifest_content.contains("\"artifact_type\""));
    assert!(manifest_content.contains("\"created_at\""));
    assert!(manifest_content.contains("\"awen_runtime_version\""));
}

#[test]
fn test_06_artifact_ir_export() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let artifacts_dir = temp_dir.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("Should create artifacts dir");

    let ir = create_test_graph();
    let params = create_test_params();

    let bundle = BundleBuilder::new(ir, ArtifactType::Run)
        .with_initial_parameters(params)
        .with_results(serde_json::json!({"measurements": [0, 1, 0]}))
        .build()
        .expect("Should create bundle");

    let export_path = export_bundle(&bundle, &artifacts_dir, ExportFormat::Directory)
        .expect("Should export bundle");

    let ir_file = export_path.join("ir/original.json");
    assert!(ir_file.exists());

    let ir_content = std::fs::read_to_string(&ir_file).expect("Should read IR");
    assert!(ir_content.contains("\"nodes\""));
}

#[test]
fn test_07_artifact_parameters_export() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let artifacts_dir = temp_dir.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("Should create artifacts dir");

    let ir = create_test_graph();
    let params = create_test_params();

    let bundle = BundleBuilder::new(ir, ArtifactType::Run)
        .with_initial_parameters(params)
        .with_results(serde_json::json!({"measurements": [0, 1, 0]}))
        .build()
        .expect("Should create bundle");

    let export_path = export_bundle(&bundle, &artifacts_dir, ExportFormat::Directory)
        .expect("Should export bundle");

    let params_file = export_path.join("parameters/initial.json");
    assert!(params_file.exists());

    let params_content = std::fs::read_to_string(&params_file).expect("Should read parameters");
    assert!(params_content.contains("\"theta\""));
    assert!(params_content.contains("1.5707963"));
}

#[test]
fn test_08_artifact_environment_capture() {
    let env = capture_environment();

    assert!(!env.runtime.runtime_name.is_empty());
    assert!(!env.runtime.version.is_empty());
    assert!(!env.system.os.is_empty());
    assert!(!env.system.arch.is_empty());
    assert!(!env.device.device_id.is_empty());
}

#[test]
fn test_09_artifact_multiple_types() {
    let ir = create_test_graph();
    let params = create_test_params();

    for artifact_type in [
        ArtifactType::Run,
        ArtifactType::Gradient,
        ArtifactType::Calibration,
        ArtifactType::Replay,
        ArtifactType::Validation,
    ]
    .iter()
    {
        let bundle = BundleBuilder::new(ir.clone(), artifact_type.clone())
            .with_initial_parameters(params.clone())
            .with_results(serde_json::json!({"measurements": [0, 1, 0]}))
            .build()
            .expect("Should create bundle");

        assert_eq!(bundle.artifact_type, artifact_type.clone());
    }
}

#[test]
fn test_10_artifact_deterministic_with_different_seed() {
    let ir = create_test_graph();
    let params = create_test_params();

    let id1 =
        compute_deterministic_id(&ir, &params, None, Some(42), "0.6.0").expect("Should compute ID");
    let id2 =
        compute_deterministic_id(&ir, &params, None, Some(99), "0.6.0").expect("Should compute ID");

    // Different seed → different ID
    assert_ne!(id1, id2);
}

#[test]
fn test_11_artifact_directory_structure() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let artifacts_dir = temp_dir.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("Should create artifacts dir");

    let ir = create_test_graph();
    let params = create_test_params();

    let bundle = BundleBuilder::new(ir, ArtifactType::Run)
        .with_initial_parameters(params)
        .with_results(serde_json::json!({"measurements": [0, 1, 0]}))
        .build()
        .expect("Should create bundle");

    let export_path = export_bundle(&bundle, &artifacts_dir, ExportFormat::Directory)
        .expect("Should export bundle");

    // Verify expected directories exist
    for dir in &["ir", "parameters", "results", "provenance"] {
        assert!(export_path.join(dir).exists(), "{} directory missing", dir);
    }
}
