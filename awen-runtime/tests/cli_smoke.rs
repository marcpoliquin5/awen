//! End-to-end smoke tests for every `awenctl` subcommand that runs without
//! external hardware. Each test runs in an isolated temporary directory so
//! run artifacts never land in the repository.

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

fn ok(output: &Output) -> String {
    assert!(
        output.status.success(),
        "awenctl failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn help_lists_every_subcommand_with_a_description() {
    let dir = tempfile::tempdir().unwrap();
    let help = ok(&awenctl(dir.path(), &["--help"]));
    for command in [
        "compile",
        "benchmark",
        "kernel",
        "execute",
        "backends",
        "run",
        "gradient",
    ] {
        let line = help
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("{command} ")))
            .unwrap_or_else(|| panic!("help is missing {command}"));
        assert!(
            line.split_whitespace().count() > 1,
            "{command} has no description"
        );
    }
}

#[test]
fn compile_writes_a_versioned_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("compilation.json");
    let stdout = ok(&awenctl(
        dir.path(),
        &[
            "compile",
            repo("awen-compiler/examples/gemm_256.json")
                .to_str()
                .unwrap(),
            "--capabilities",
            repo("awen-compiler/capabilities/pace_like_128.json")
                .to_str()
                .unwrap(),
            "--health",
            repo("awen-compiler/capabilities/pace_like_128.health.json")
                .to_str()
                .unwrap(),
            "--target",
            "photonic",
            "--output",
            output_path.to_str().unwrap(),
        ],
    ));
    assert!(stdout.contains("Compiled 1 operation(s)"));
    let artifact: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output_path).unwrap()).unwrap();
    assert!(artifact.get("photonic_ir").is_some());
    assert!(artifact.get("device_ir").is_some());
}

#[test]
fn benchmark_executes_literal_data_in_the_simulator() {
    let dir = tempfile::tempdir().unwrap();
    let report_path = dir.path().join("benchmark.json");
    ok(&awenctl(
        dir.path(),
        &[
            "benchmark",
            repo("awen-compiler/examples/gemm_4x4.json")
                .to_str()
                .unwrap(),
            "--output",
            report_path.to_str().unwrap(),
        ],
    ));
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report_path).unwrap()).unwrap();
    assert!(report.is_object());
}

#[test]
fn execute_rejects_a_corrupt_executable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.awenx");
    std::fs::write(&path, b"not an AWEN executable").unwrap();
    let output = awenctl(dir.path(), &["execute", path.to_str().unwrap()]);
    assert!(!output.status.success());
}

#[test]
fn run_and_gradient_execute_the_legacy_graph_in_isolation() {
    let dir = tempfile::tempdir().unwrap();
    let ir = repo("awen-runtime/example_ir.json");
    ok(&awenctl(
        dir.path(),
        &["run", ir.to_str().unwrap(), "--seed", "7"],
    ));
    ok(&awenctl(
        dir.path(),
        &[
            "gradient",
            ir.to_str().unwrap(),
            "mzi_0:phase",
            "--strategy",
            "finite_difference",
            "--seed",
            "7",
        ],
    ));
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().any(|name| name.starts_with("awen_run_")));
    assert!(names.iter().any(|name| name.starts_with("awen_grad_")));
}

#[test]
fn backends_reports_an_empty_plugin_directory() {
    let dir = tempfile::tempdir().unwrap();
    let plugins = dir.path().join("plugins");
    std::fs::create_dir(&plugins).unwrap();
    let output = awenctl(dir.path(), &["backends", plugins.to_str().unwrap()]);
    ok(&output);
}

fn write_gemm_executable(dir: &Path) -> PathBuf {
    use awen_compiler::executable::{EXECUTABLE_ABI_MAJOR, EXECUTABLE_ABI_MINOR};
    use awen_compiler::{ExecutableCommand, ExecutablePackage};
    let package = ExecutablePackage {
        abi_major: EXECUTABLE_ABI_MAJOR,
        abi_minor: EXECUTABLE_ABI_MINOR,
        backend_id: "awen.reference.v1".to_string(),
        commands: vec![ExecutableCommand::ExecuteGemm {
            tile_m: 4,
            tile_n: 4,
            tile_k: 2,
            minimum_effective_bits: 4,
            calibration: "required".to_string(),
            layout: "row_major".to_string(),
            result_shape: vec![2, 2],
        }],
        mlir_bytecode: b"ML\xefRtest".to_vec(),
    };
    let path = dir.join("gemm.awenx");
    std::fs::write(&path, package.encode().unwrap()).unwrap();
    path
}

#[test]
fn execute_runs_gemm_arithmetic_on_the_noise_model() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = write_gemm_executable(dir.path());
    std::fs::write(
        dir.path().join("inputs.json"),
        r#"{"version":"awen.execution-inputs.v1","dispatches":[
            {"lhs":{"shape":[2,3],"data":[1,2,3,4,5,6]},
             "rhs":{"shape":[3,2],"data":[1,0,0,1,1,1]}}]}"#,
    )
    .unwrap();
    let artifact = artifact.to_str().unwrap();

    let prepared = ok(&awenctl(dir.path(), &["execute", artifact]));
    assert!(prepared.contains("no arithmetic performed"));

    let ideal = ok(&awenctl(
        dir.path(),
        &[
            "execute",
            artifact,
            "--inputs",
            "inputs.json",
            "--ideal",
            "--output",
            "ideal.json",
        ],
    ));
    assert!(ideal.contains("not hardware"));
    let result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("ideal.json")).unwrap())
            .unwrap();
    assert_eq!(result["fidelity"], "compact_model");
    let values: Vec<f64> = result["dispatches"][0]["result"]["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    for (got, want) in values.iter().zip([4.0, 5.0, 10.0, 11.0]) {
        assert!((got - want).abs() < 1e-12, "{got} vs {want}");
    }

    let noisy = ok(&awenctl(
        dir.path(),
        &[
            "execute",
            artifact,
            "--inputs",
            "inputs.json",
            "--seed",
            "3",
            "--output",
            "noisy.json",
        ],
    ));
    assert!(noisy.contains("relative_rms_error"));
    let result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("noisy.json")).unwrap())
            .unwrap();
    assert!(result["dispatches"][0]["rms_error"].as_f64().unwrap() > 0.0);

    let missing = awenctl(
        dir.path(),
        &["execute", artifact, "--inputs", "absent.json"],
    );
    assert!(!missing.status.success());
}
