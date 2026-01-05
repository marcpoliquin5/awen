//! Integration tests for reproducibility and artifact system

use awen_runtime::ir::Graph;
use awen_runtime::storage::{
    compute_deterministic_id, export_bundle, import_bundle, short_id, validate_bundle,
    ArtifactType, BundleBuilder, ExportFormat,
};
use std::collections::HashMap;
use tempfile::tempdir;

#[test]
fn test_deterministic_id_same_inputs() {
    let ir = Graph {
        nodes: vec![],
        edges: vec![],
        metadata: HashMap::new(),
    };
    let mut params = HashMap::new();
    params.insert("phase".to_string(), 1.57);
    params.insert("amplitude".to_string(), 0.5);

    let id1 = compute_deterministic_id(&ir, &params, None, Some(42), "0.5.0").unwrap();
    let id2 = compute_deterministic_id(&ir, &params, None, Some(42), "0.5.0").unwrap();

    assert_eq!(id1, id2, "Same inputs must produce same ID");
    assert!(id1.starts_with("awen_"));
    assert_eq!(id1.len(), 5 + 64); // "awen_" + SHA256 hex
}

#[test]
fn test_deterministic_id_different_seed() {
    let ir = Graph {
        nodes: vec![],
        edges: vec![],
        metadata: HashMap::new(),
    };
    let params = HashMap::new();

    let id1 = compute_deterministic_id(&ir, &params, None, Some(42), "0.5.0").unwrap();
    let id2 = compute_deterministic_id(&ir, &params, None, Some(43), "0.5.0").unwrap();

    assert_ne!(id1, id2, "Different seeds must produce different IDs");
}

#[test]
fn test_bundle_builder_and_export() {
    let temp_dir = tempdir().unwrap();

    let ir = Graph {
        nodes: vec![],
        edges: vec![],
        metadata: HashMap::new(),
    };
    let mut params = HashMap::new();
    params.insert("phase".to_string(), 1.57);

    let bundle = BundleBuilder::new(ir.clone(), ArtifactType::Run)
        .with_initial_parameters(params.clone())
        .with_results(serde_json::json!({"output": [1.0, 2.0, 3.0]}))
        .with_seed(42)
        .add_tag("test".to_string())
        .with_notes("Test run".to_string())
        .build()
        .unwrap();

    assert!(bundle.artifact_id.starts_with("awen_"));
    assert_eq!(bundle.artifact_type, ArtifactType::Run);
    assert!(bundle.seed == Some(42));

    // Export
    let exported_path = export_bundle(&bundle, temp_dir.path(), ExportFormat::Directory).unwrap();
    assert!(exported_path.exists());
    assert!(exported_path.join("manifest.json").exists());
    assert!(exported_path.join("ir/original.json").exists());
    assert!(exported_path.join("parameters/initial.json").exists());
    assert!(exported_path.join("results/outputs.json").exists());
    assert!(exported_path.join("checksums.json").exists());
}

#[test]
fn test_bundle_export_import_roundtrip() {
    let temp_dir = tempdir().unwrap();

    let ir = Graph {
        nodes: vec![],
        edges: vec![],
        metadata: HashMap::new(),
    };
    let mut params = HashMap::new();
    params.insert("phase".to_string(), 1.57);

    let original_bundle = BundleBuilder::new(ir.clone(), ArtifactType::Run)
        .with_initial_parameters(params.clone())
        .with_results(serde_json::json!({"output": [1.0, 2.0, 3.0]}))
        .with_seed(42)
        .build()
        .unwrap();

    // Export
    let exported_path =
        export_bundle(&original_bundle, temp_dir.path(), ExportFormat::Directory).unwrap();

    // Import
    let imported_bundle = import_bundle(&exported_path).unwrap();

    // Verify
    assert_eq!(original_bundle.artifact_id, imported_bundle.artifact_id);
    assert_eq!(original_bundle.seed, imported_bundle.seed);
    assert_eq!(
        original_bundle.parameters_initial,
        imported_bundle.parameters_initial
    );
}

#[test]
fn test_bundle_validation() {
    let ir = Graph {
        nodes: vec![],
        edges: vec![],
        metadata: HashMap::new(),
    };
    let mut params = HashMap::new();
    params.insert("phase".to_string(), 1.57);

    let bundle = BundleBuilder::new(ir.clone(), ArtifactType::Run)
        .with_initial_parameters(params.clone())
        .with_results(serde_json::json!({"output": 42}))
        .build()
        .unwrap();

    // Should pass validation
    assert!(validate_bundle(&bundle).is_ok());
}

#[test]
fn test_citation_generation() {
    let ir = Graph {
        nodes: vec![],
        edges: vec![],
        metadata: HashMap::new(),
    };
    let params = HashMap::new();

    let bundle = BundleBuilder::new(ir.clone(), ArtifactType::Run)
        .with_initial_parameters(params.clone())
        .with_results(serde_json::json!({"output": 42}))
        .with_citation_metadata(
            "Silicon Photonics Inference Benchmark".to_string(),
            vec!["Alice".to_string(), "Bob".to_string()],
            "Example Lab".to_string(),
        )
        .build()
        .unwrap();

    // Export and check citation file
    let temp_dir = tempdir().unwrap();
    let exported_path = export_bundle(&bundle, temp_dir.path(), ExportFormat::Directory).unwrap();

    let citation_path = exported_path.join("provenance/citation.txt");
    assert!(citation_path.exists());

    let citation = std::fs::read_to_string(&citation_path).unwrap();
    assert!(citation.contains("Alice"));
    assert!(citation.contains("Bob"));
    assert!(citation.contains("Example Lab"));
    assert!(citation.contains(short_id(&bundle.artifact_id)));
}

#[test]
fn test_lineage_tracking() {
    let ir = Graph {
        nodes: vec![],
        edges: vec![],
        metadata: HashMap::new(),
    };
    let params = HashMap::new();

    // Create parent bundle
    let parent = BundleBuilder::new(ir.clone(), ArtifactType::Run)
        .with_initial_parameters(params.clone())
        .with_results(serde_json::json!({"output": 42}))
        .build()
        .unwrap();

    // Create child bundle with lineage
    let child = BundleBuilder::new(ir.clone(), ArtifactType::Gradient)
        .with_initial_parameters(params.clone())
        .with_results(serde_json::json!({"gradients": [0.1, 0.2]}))
        .add_parent_artifact(parent.artifact_id.clone())
        .build()
        .unwrap();

    assert_eq!(child.provenance.parent_artifacts.len(), 1);
    assert_eq!(child.provenance.parent_artifacts[0], parent.artifact_id);
}

#[test]
fn test_observability_integration() {
    let temp_dir = tempdir().unwrap();
    let obs_dir = temp_dir.path().join("obs");
    std::fs::create_dir_all(&obs_dir).unwrap();

    // Create mock observability files
    std::fs::write(
        obs_dir.join("traces.jsonl"),
        r#"{"span_id":"span1","name":"test"}"#,
    )
    .unwrap();
    std::fs::write(
        obs_dir.join("metrics.json"),
        r#"{"counters":[],"gauges":[],"histograms":[]}"#,
    )
    .unwrap();

    let ir = Graph {
        nodes: vec![],
        edges: vec![],
        metadata: HashMap::new(),
    };
    let params = HashMap::new();

    let bundle = BundleBuilder::new(ir.clone(), ArtifactType::Run)
        .with_initial_parameters(params.clone())
        .with_results(serde_json::json!({"output": 42}))
        .with_observability_dir(&obs_dir)
        .build()
        .unwrap();

    assert!(bundle.observability.is_some());
    let obs = bundle.observability.unwrap();
    assert!(obs.traces.is_some());
    assert!(obs.metrics.is_some());
}

#[test]
fn test_checksum_validation() {
    let temp_dir = tempdir().unwrap();

    let ir = Graph {
        nodes: vec![],
        edges: vec![],
        metadata: HashMap::new(),
    };
    let mut params = HashMap::new();
    params.insert("phase".to_string(), 1.57);

    let bundle = BundleBuilder::new(ir.clone(), ArtifactType::Run)
        .with_initial_parameters(params.clone())
        .with_results(serde_json::json!({"output": 42}))
        .build()
        .unwrap();

    // Export
    let exported_path = export_bundle(&bundle, temp_dir.path(), ExportFormat::Directory).unwrap();

    // Import should validate checksums
    let result = import_bundle(&exported_path);
    assert!(result.is_ok(), "Checksum validation should pass");

    // Corrupt a file
    std::fs::write(exported_path.join("results/outputs.json"), "corrupted").unwrap();

    // Import should fail
    let result = import_bundle(&exported_path);
    assert!(
        result.is_err(),
        "Checksum validation should fail on corrupted file"
    );
}
