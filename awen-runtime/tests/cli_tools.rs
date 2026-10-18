//! `awenctl caps` (capability explorer) and `awenctl profile` (kernel profiler).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
}

fn awenctl(dir: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_awenctl"))
        .current_dir(dir)
        .args(arguments)
        .output()
        .expect("awenctl starts")
}

fn ok_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON stdout")
}

#[test]
fn caps_explores_capability_manifest_profiles_and_hardware_descriptions() {
    let dir = tempfile::tempdir().unwrap();
    let capability = repo("awen-compiler/capabilities/reference_2x2.json");
    let summary = ok_json(&awenctl(
        dir.path(),
        &["caps", capability.to_str().unwrap(), "--json"],
    ));
    assert_eq!(summary["backend_id"], "reference-photonic-2x2");
    assert_eq!(summary["operations"], json!(["gemm"]));
    assert_eq!(summary["matrix_core"]["m"], 2);

    let text = awenctl(dir.path(), &["caps", capability.to_str().unwrap()]);
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("matrix_core.k: 2\n"));
    assert!(text.contains("calibration_required: true\n"));

    let manifest = repo("awen-runtime/plugins/reference_sim/backend-manifest.json");
    let summary = ok_json(&awenctl(
        dir.path(),
        &["caps", manifest.to_str().unwrap(), "--json"],
    ));
    assert_eq!(summary["plugin_id"], "awen.reference-photonic-2x2");
    assert_eq!(summary["signed"], false);
    assert_eq!(summary["backend"]["backend_id"], "reference-photonic-2x2");

    let profiles = repo("awen-compiler/kernels/reference_kernel_backends.json");
    let summary = ok_json(&awenctl(
        dir.path(),
        &["caps", profiles.to_str().unwrap(), "--json"],
    ));
    let backends = summary["backends"].as_array().unwrap();
    assert_eq!(backends.len(), 2);
    assert_eq!(backends[1]["parameter_source"], "simulated");

    let hardware = dir.path().join("hw.json");
    std::fs::write(
        &hardware,
        json!({
            "version": "awen.hardware-description.v1",
            "device_id": "sim",
            "provenance": {"source": "reference_simulator"},
            "components": [
                {"class": "adc", "id": "a"}, {"class": "adc", "id": "b"},
                {"class": "mzi_mesh", "id": "m"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let summary = ok_json(&awenctl(
        dir.path(),
        &["caps", hardware.to_str().unwrap(), "--json"],
    ));
    assert_eq!(summary["provenance"], "reference_simulator");
    assert_eq!(
        summary["component_classes"],
        json!({"adc": 2, "mzi_mesh": 1})
    );

    let unknown = dir.path().join("unknown.json");
    std::fs::write(&unknown, "{\"hello\": 1}").unwrap();
    assert!(!awenctl(dir.path(), &["caps", unknown.to_str().unwrap()])
        .status
        .success());
}

#[test]
fn profile_writes_summary_and_chrome_trace_for_reference_and_simulator() {
    let dir = tempfile::tempdir().unwrap();
    let request = repo("awen-compiler/kernels/transformer_qkv.json");
    for (mode, simulated, device) in [
        ("reference", false, "cpu-reference"),
        ("simulator", true, "photonic-simulator"),
    ] {
        let trace = dir.path().join(format!("{mode}.trace.json"));
        let summary = ok_json(&awenctl(
            dir.path(),
            &[
                "profile",
                request.to_str().unwrap(),
                "--mode",
                mode,
                "--repeat",
                "4",
                "--trace",
                trace.to_str().unwrap(),
            ],
        ));
        assert_eq!(summary["simulated"], simulated);
        assert_eq!(summary["device"], device);
        assert_eq!(summary["spans"]["TransformerQkv"]["count"], 4);
        assert!(summary["timing_source"]
            .as_str()
            .unwrap()
            .contains("not a device measurement"));
        let chrome: Value = serde_json::from_str(&std::fs::read_to_string(trace).unwrap()).unwrap();
        let events = chrome["traceEvents"].as_array().unwrap();
        assert_eq!(events.len(), 4);
        assert!(events.iter().all(|e| e["ph"] == "X" && e["tid"] == device));
    }
    assert!(!awenctl(
        dir.path(),
        &["profile", request.to_str().unwrap(), "--repeat", "0"]
    )
    .status
    .success());
    assert!(!awenctl(
        dir.path(),
        &["profile", request.to_str().unwrap(), "--mode", "hardware"]
    )
    .status
    .success());
}
