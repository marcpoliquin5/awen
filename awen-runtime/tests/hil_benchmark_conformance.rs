use awen_compiler::execute_kernel_reference;
use awen_runtime::benchmark::{
    run_benchmark_suite, BenchmarkArtifact, BenchmarkDriverRequest, BenchmarkDriverResponse,
    BenchmarkRunContext, BenchmarkSuite, DriverOutputSample, EvidenceKind, MetricSources,
    VerificationStatus, HIL_ARTIFACT_VERSION, HIL_DRIVER_VERSION, HIL_SUITE_VERSION,
};
use jsonschema::JSONSchema;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const BLAS_ID: &str = "https://awen.dev/schemas/awen_blas.v1.json";
const SUITE_ID: &str = "https://awen.dev/schemas/awen_hil_suite.v1.json";
const ARTIFACT_ID: &str = "https://awen.dev/schemas/awen_hil_artifact.v1.json";
const DRIVER_ID: &str = "https://awen.dev/schemas/awen_hil_driver.v1.json";
const CLAIMS_ID: &str = "https://awen.dev/schemas/awen_benchmark_claims.v1.json";

fn schema(source: &str) -> Value {
    serde_json::from_str(source).expect("published HIL schema must be JSON")
}

fn schemas() -> Vec<(&'static str, Value)> {
    vec![
        (
            BLAS_ID,
            schema(include_str!("../../awen-spec/schemas/awen_blas.v1.json")),
        ),
        (
            SUITE_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_hil_suite.v1.json"
            )),
        ),
        (
            ARTIFACT_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_hil_artifact.v1.json"
            )),
        ),
        (
            DRIVER_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_hil_driver.v1.json"
            )),
        ),
        (
            CLAIMS_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_benchmark_claims.v1.json"
            )),
        ),
    ]
}

fn validator(schema_id: &str) -> JSONSchema {
    let documents = schemas();
    let selected = documents
        .iter()
        .find_map(|(id, document)| (*id == schema_id).then_some(document))
        .expect("registered HIL schema");
    let mut options = JSONSchema::options();
    for (id, document) in &documents {
        options.with_document((*id).to_string(), document.clone());
    }
    options.compile(selected).expect("HIL schema compiles")
}

fn assert_valid(schema_id: &str, value: &Value) {
    if let Err(errors) = validator(schema_id).validate(value) {
        panic!(
            "{schema_id} rejected HIL value: {:?}",
            errors.map(|error| error.to_string()).collect::<Vec<_>>()
        );
    }
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../benchmarks/reference_hil_suite.json")
}

fn fixture() -> BenchmarkSuite {
    serde_json::from_slice(&std::fs::read(fixture_path()).expect("reference suite file"))
        .expect("reference HIL suite")
}

fn run_reference_suite() -> BenchmarkArtifact {
    run_benchmark_suite(
        &fixture(),
        &BenchmarkRunContext {
            commit_sha: "0123456789012345678901234567890123456789".to_string(),
            runner_id: "schema-conformance".to_string(),
        },
    )
    .expect("reference HIL execution")
}

#[test]
fn reference_suite_driver_and_artifact_match_published_schemas() {
    let suite = fixture();
    let artifact = run_reference_suite();
    assert_eq!(suite.version, HIL_SUITE_VERSION);
    assert_eq!(artifact.version, HIL_ARTIFACT_VERSION);
    assert_eq!(artifact.verification.status, VerificationStatus::Verified);
    assert_valid(
        SUITE_ID,
        &serde_json::to_value(&suite).expect("suite value"),
    );
    assert_valid(
        ARTIFACT_ID,
        &serde_json::to_value(&artifact).expect("artifact value"),
    );

    let request = BenchmarkDriverRequest {
        version: HIL_DRIVER_VERSION.to_string(),
        suite_id: suite.id.clone(),
        backend_id: "external-test-driver".to_string(),
        fixture: suite.fixture.clone(),
        warmup: suite.warmup,
        repetitions: suite.repetitions,
        seed: suite.seed,
        commit_sha: artifact.commit_sha.clone(),
        runner_id: "schema-conformance".to_string(),
    };
    assert_valid(
        DRIVER_ID,
        &serde_json::to_value(&request).expect("driver request value"),
    );

    let reference = execute_kernel_reference(&suite.fixture).expect("reference outputs");
    let result = &artifact.results[0];
    let response = BenchmarkDriverResponse {
        version: HIL_DRIVER_VERSION.to_string(),
        backend_id: "external-test-driver".to_string(),
        sources: MetricSources {
            execution: EvidenceKind::Measured,
            latency: EvidenceKind::Measured,
            energy: EvidenceKind::Measured,
            power: EvidenceKind::Measured,
            accuracy: EvidenceKind::Measured,
            calibration: EvidenceKind::Measured,
            environment: EvidenceKind::Measured,
        },
        environment: result.environment.clone(),
        calibration_duration_ns: 0.0,
        samples: result.raw_samples.clone(),
        output_samples: (0..suite.repetitions)
            .map(|iteration| DriverOutputSample {
                iteration,
                outputs: reference.outputs.clone(),
            })
            .collect(),
        raw_data: serde_json::from_value(serde_json::json!({"instrument": "test"}))
            .expect("raw data map"),
    };
    assert_valid(
        DRIVER_ID,
        &serde_json::to_value(&response).expect("driver response value"),
    );
}

#[test]
fn full_system_boundaries_are_complete_and_optical_time_is_not_application_latency() {
    let artifact = run_reference_suite();
    for result in &artifact.results {
        for sample in &result.raw_samples {
            let latency = &sample.latency_breakdown_ns;
            assert!((sample.latency_ns - latency.total()).abs() < 1e-6);
            assert!((sample.energy_j - sample.energy_breakdown_j.total()).abs() < 1e-12);
            if result.backend_id == "photonic-simulator" {
                assert!(latency.host_transfer > 0.0);
                assert!(latency.reconfiguration > 0.0);
                assert!(latency.dac > 0.0);
                assert!(latency.optical_device > 0.0);
                assert!(latency.adc > 0.0);
                assert!(latency.digital_postprocessing > 0.0);
                assert!(sample.latency_ns > latency.optical_device);
                assert!(sample.energy_breakdown_j.laser > 0.0);
                assert!(sample.energy_breakdown_j.cooling_support > 0.0);
            }
        }
        assert!(result.metrics.latency_ns.p50.is_finite());
        assert!(result.metrics.latency_ns.p95 >= result.metrics.latency_ns.p50);
        assert!(result.metrics.latency_ns.p99 >= result.metrics.latency_ns.p95);
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("awen-hil-cli-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).expect("create CLI test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn one_cli_command_emits_a_content_addressed_comparable_artifact_set() {
    let directory = TestDirectory::new();
    let output_dir = directory.0.join("artifact-set");
    let output = Command::new(env!("CARGO_BIN_EXE_awenctl"))
        .args([
            "benchmark-suite",
            fixture_path().to_str().expect("suite path"),
            "--output-dir",
            output_dir.to_str().expect("output directory"),
            "--commit-sha",
            "0123456789012345678901234567890123456789",
            "--runner-id",
            "cli-conformance",
        ])
        .output()
        .expect("run benchmark-suite CLI");
    assert!(
        output.status.success(),
        "benchmark-suite stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_dir.join("suite.json").is_file());
    assert!(output_dir.join("SHA256SUMS").is_file());
    let artifact_path = std::fs::read_dir(&output_dir)
        .expect("artifact directory")
        .map(|entry| entry.expect("artifact entry").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("benchmark-") && name.ends_with(".json"))
        })
        .expect("content-addressed artifact");
    let artifact: BenchmarkArtifact =
        serde_json::from_slice(&std::fs::read(&artifact_path).expect("artifact bytes"))
            .expect("artifact contract");
    assert_eq!(artifact.results.len(), fixture().backends.len());
    let filename = artifact_path
        .file_name()
        .expect("artifact filename")
        .to_string_lossy();
    assert!(filename.contains(
        artifact
            .artifact_fingerprint
            .strip_prefix("sha256:")
            .expect("SHA-256 fingerprint")
    ));

    let digest = artifact
        .artifact_fingerprint
        .strip_prefix("sha256:")
        .expect("artifact digest");
    let rejected_claims = Command::new(env!("CARGO_BIN_EXE_awenctl"))
        .args([
            "benchmark-claims",
            artifact_path.to_str().expect("artifact path"),
            "--artifact-url",
            &format!("https://example.com/benchmark-{digest}.json"),
            "--baseline",
            "cpu-reference",
            "--candidate",
            "photonic-simulator",
            "--output",
            directory
                .0
                .join("claims.json")
                .to_str()
                .expect("claims path"),
            "--markdown-output",
            directory
                .0
                .join("claims.md")
                .to_str()
                .expect("claims Markdown path"),
        ])
        .output()
        .expect("run benchmark-claims CLI");
    assert!(!rejected_claims.status.success());
    let rejection = String::from_utf8_lossy(&rejected_claims.stderr);
    assert!(
        rejection.contains("lab-rig or hardware-accelerator"),
        "claims rejection stderr: {rejection}"
    );
}

#[test]
fn schemas_reject_unversioned_or_unattributed_hardware_evidence() {
    let mut suite = serde_json::to_value(fixture()).expect("suite value");
    suite.as_object_mut().expect("suite object").remove("seed");
    assert!(!validator(SUITE_ID).is_valid(&suite));

    let mut artifact = serde_json::to_value(run_reference_suite()).expect("artifact value");
    artifact["results"][0]["sources"]
        .as_object_mut()
        .expect("sources object")
        .remove("energy");
    assert!(!validator(ARTIFACT_ID).is_valid(&artifact));

    let claim = serde_json::json!({
        "version": "awen.benchmark-claims.v1",
        "suite_id": "suite",
        "artifact_url": "https://example.com/latest.json",
        "artifact_fingerprint": format!("sha256:{}", "0".repeat(64)),
        "baseline_backend_id": "cpu",
        "candidate_backend_id": "hardware",
        "generated_at": "2026-08-13T00:00:00Z",
        "claims": [],
        "verification": "verified",
        "claims_fingerprint": format!("sha256:{}", "1".repeat(64))
    });
    assert!(!validator(CLAIMS_ID).is_valid(&claim));
}
