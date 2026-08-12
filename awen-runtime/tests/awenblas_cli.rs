use awen_compiler::{KernelBenchmarkReport, KernelExecutionPlan, KernelResult, TargetBackend};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("awenblas-cli-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).expect("create isolated CLI test directory");
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../awen-compiler/kernels")
        .join(name)
}

fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_awenctl"))
        .args(arguments)
        .output()
        .expect("run awenctl")
}

#[test]
fn cli_executes_plans_and_benchmarks_versioned_awenblas_requests() {
    let directory = TestDirectory::new();
    let request = fixture("transformer_qkv.json");
    let profiles = fixture("reference_kernel_backends.json");
    let result_path = directory.path("result.json");
    let plan_path = directory.path("plan.json");
    let benchmark_path = directory.path("benchmark.json");

    let output = run(&[
        "kernel",
        request.to_str().expect("request path"),
        "--target",
        "photonic",
        "--effective-bits",
        "12",
        "--seed",
        "17",
        "--output",
        result_path.to_str().expect("result path"),
    ]);
    assert!(
        output.status.success(),
        "kernel stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: KernelResult =
        serde_json::from_slice(&std::fs::read(result_path).expect("result file"))
            .expect("result contract");
    assert_eq!(result.execution_target, TargetBackend::Photonic);
    assert!(result.simulated);
    assert_eq!(result.outputs.len(), 3);

    let output = run(&[
        "kernel-plan",
        request.to_str().expect("request path"),
        profiles.to_str().expect("profiles path"),
        "--optimize-for",
        "latency",
        "--output",
        plan_path.to_str().expect("plan path"),
    ]);
    assert!(
        output.status.success(),
        "plan stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan: KernelExecutionPlan =
        serde_json::from_slice(&std::fs::read(plan_path).expect("plan file"))
            .expect("plan contract");
    assert_eq!(plan.selected_target, TargetBackend::Photonic);
    assert!(!plan.fallback);

    let output = run(&[
        "kernel-benchmark",
        request.to_str().expect("request path"),
        "--target",
        "photonic",
        "--effective-bits",
        "12",
        "--repetitions",
        "2",
        "--output",
        benchmark_path.to_str().expect("benchmark path"),
    ]);
    assert!(
        output.status.success(),
        "benchmark stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: KernelBenchmarkReport =
        serde_json::from_slice(&std::fs::read(benchmark_path).expect("benchmark file"))
            .expect("benchmark contract");
    assert_eq!(report.repetitions, 2);
    assert!(report.within_contract);
}

#[test]
fn cli_rejects_auto_execution_and_cpu_simulator_benchmark_targets() {
    let directory = TestDirectory::new();
    let request = fixture("transformer_qkv.json");
    let output_path = directory.path("invalid.json");

    let auto = run(&[
        "kernel",
        request.to_str().expect("request path"),
        "--target",
        "auto",
        "--output",
        output_path.to_str().expect("output path"),
    ]);
    assert!(!auto.status.success());
    assert!(String::from_utf8_lossy(&auto.stderr).contains("concrete"));

    let cpu = run(&[
        "kernel-benchmark",
        request.to_str().expect("request path"),
        "--target",
        "cpu",
        "--output",
        output_path.to_str().expect("output path"),
    ]);
    assert!(!cpu.status.success());
    assert!(String::from_utf8_lossy(&cpu.stderr).contains("gpu or photonic"));
}
