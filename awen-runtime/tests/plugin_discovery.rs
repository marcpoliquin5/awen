use awen_runtime::plugins::registry::PluginManifest;
use awen_runtime::plugins::PluginRegistry;
use std::fs;
use std::path::PathBuf;

#[test]
fn discover_and_register_unverified_manifest() {
    // Create a temporary plugin dir under the system temp dir
    let mut dir = std::env::temp_dir();
    dir.push(format!("awen_plugin_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp plugin dir");

    let manifest = PluginManifest {
        id: "discover-test".into(),
        version: "0.1".into(),
        capabilities: vec!["execute".into()],
        signature: None,
        public_key: None,
        path: None,
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
