use awen_compiler::{
    compile_with_backend, BackendHealth, CompileOptions, DeviceCapabilities, HealthStatus,
    TargetBackend, TensorProgram, HEALTH_VERSION,
};
use awen_runtime::plugins::registry::PluginManifest;
use awen_runtime::plugins::{
    BackendPluginContract, HealthQuery, PluginRegistry, PLUGIN_MANIFEST_VERSION,
};
use std::fs;
use std::path::PathBuf;

#[test]
fn discover_and_register_unverified_manifest() {
    // Create a temporary plugin dir under the system temp dir
    let mut dir = std::env::temp_dir();
    dir.push(format!("awen_plugin_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp plugin dir");

    let manifest = PluginManifest {
        manifest_version: PLUGIN_MANIFEST_VERSION.into(),
        id: "discover-test".into(),
        version: "0.1".into(),
        capabilities: vec!["execute".into()],
        signature: None,
        public_key: None,
        path: None,
        backend: None,
        physical_design_adapters: Vec::new(),
    };

    // Write manifest file (no signature)
    let mut path = PathBuf::from(&dir);
    path.push("discover-manifest.json");
    let s = serde_json::to_string_pretty(&manifest).unwrap();
    fs::write(&path, s).expect("write manifest");

    // Run discovery allowing unverified manifests (test/dev flow)
    let reg = PluginRegistry::discover_from_dir_allow_unverified(&dir, true).expect("discover");
    let found = reg.find_by_capability("execute");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "discover-test");

    // cleanup (best effort)
    let _ = fs::remove_file(path);
    let _ = fs::remove_dir(dir);
}

#[test]
fn backend_discovery_requeries_live_health_and_exposes_unavailability() {
    let directory = tempfile::tempdir().expect("temporary plugin directory");
    let capabilities = DeviceCapabilities::pace_like_128();
    let manifest = PluginManifest {
        manifest_version: PLUGIN_MANIFEST_VERSION.into(),
        id: "typed-backend".into(),
        version: "1.0.0".into(),
        capabilities: vec!["backend".into(), "gemm".into(), "health".into()],
        signature: None,
        public_key: None,
        path: None,
        backend: Some(BackendPluginContract {
            capabilities: capabilities.clone(),
            health_query: HealthQuery::File {
                path: PathBuf::from("health.json"),
            },
        }),
        physical_design_adapters: Vec::new(),
    };
    let manifest_path = directory.path().join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");

    let mut health = BackendHealth {
        health_version: HEALTH_VERSION.into(),
        backend_id: capabilities.backend_id.clone(),
        observed_at: "2026-08-11T22:30:00Z".into(),
        status: HealthStatus::Healthy,
        temperature_c: 22.1,
        drift: 0.002,
        available_channels: capabilities.simultaneous_channels,
        disabled_components: Vec::new(),
        unavailable_resources: Vec::new(),
        calibration_profile_id: capabilities
            .calibration_profile
            .as_ref()
            .map(|profile| profile.id.clone()),
        calibration_fingerprint: capabilities
            .calibration_profile
            .as_ref()
            .map(|profile| profile.fingerprint.clone()),
    };
    let health_path = directory.path().join("health.json");
    fs::write(
        &health_path,
        serde_json::to_vec_pretty(&health).expect("serialize health"),
    )
    .expect("write health");

    let registry = PluginRegistry::discover_from_dir_allow_unverified(directory.path(), true)
        .expect("discover typed plugin");
    let first = registry.query_backend_snapshots(directory.path());
    assert_eq!(first.backends.len(), 1);
    assert!(first.diagnostics.is_empty());
    assert_eq!(
        first.backends[0].snapshot.health.status,
        HealthStatus::Healthy
    );
    let program: TensorProgram =
        serde_json::from_str(include_str!("../../awen-compiler/examples/gemm_4x4.json"))
            .expect("compiler fixture");
    let artifact = compile_with_backend(
        &program,
        &first.backends[0].snapshot,
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("discovered backend should compile without compiler source changes");
    assert!(!artifact.photonic_ir.ops.is_empty());

    health.status = HealthStatus::Unavailable;
    health.unavailable_resources.push("matrix_core".to_string());
    fs::write(
        &health_path,
        serde_json::to_vec_pretty(&health).expect("serialize changed health"),
    )
    .expect("update health");
    let second = registry.query_backend_snapshots(directory.path());
    assert_eq!(second.backends.len(), 1);
    assert_eq!(
        second.backends[0].snapshot.health.status,
        HealthStatus::Unavailable
    );
}

#[test]
fn backend_version_skew_produces_a_discovery_diagnostic() {
    let directory = tempfile::tempdir().expect("temporary plugin directory");
    let mut capabilities = DeviceCapabilities::pace_like_128();
    capabilities.capability_version = "awen.device-capability.v2".to_string();
    let manifest = PluginManifest {
        manifest_version: PLUGIN_MANIFEST_VERSION.into(),
        id: "future-backend".into(),
        version: "2.0.0".into(),
        capabilities: vec!["backend".into()],
        signature: None,
        public_key: None,
        path: None,
        backend: Some(BackendPluginContract {
            capabilities: capabilities.clone(),
            health_query: HealthQuery::File {
                path: PathBuf::from("health.json"),
            },
        }),
        physical_design_adapters: Vec::new(),
    };
    fs::write(
        directory.path().join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");
    let health = BackendHealth {
        health_version: HEALTH_VERSION.into(),
        backend_id: capabilities.backend_id.clone(),
        observed_at: "2026-08-11T22:30:00Z".into(),
        status: HealthStatus::Healthy,
        temperature_c: 22.0,
        drift: 0.0,
        available_channels: capabilities.simultaneous_channels,
        disabled_components: Vec::new(),
        unavailable_resources: Vec::new(),
        calibration_profile_id: capabilities
            .calibration_profile
            .as_ref()
            .map(|profile| profile.id.clone()),
        calibration_fingerprint: capabilities
            .calibration_profile
            .as_ref()
            .map(|profile| profile.fingerprint.clone()),
    };
    fs::write(
        directory.path().join("health.json"),
        serde_json::to_vec_pretty(&health).expect("serialize health"),
    )
    .expect("write health");

    let registry = PluginRegistry::discover_from_dir_allow_unverified(directory.path(), true)
        .expect("discover future plugin envelope");
    let report = registry.query_backend_snapshots(directory.path());
    assert!(report.backends.is_empty());
    assert_eq!(report.diagnostics.len(), 1);
    assert!(report.diagnostics[0]
        .message
        .contains("awen.device-capability.v2"));
}
