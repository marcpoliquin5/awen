//! Reproducible full-system and hardware-in-the-loop benchmarking.

use anyhow::{bail, Context, Result};
use awen_compiler::{
    execute_kernel_reference, execute_kernel_simulator, KernelData, KernelRequest, KernelResult,
    KernelSimulatorOptions, KernelTensor, TargetBackend,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

pub const HIL_SUITE_VERSION: &str = "awen.hil-suite.v1";
pub const HIL_DRIVER_VERSION: &str = "awen.hil-driver.v1";
pub const HIL_ARTIFACT_VERSION: &str = "awen.hil-artifact.v1";
pub const BENCHMARK_CLAIMS_VERSION: &str = "awen.benchmark-claims.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Measured,
    Simulated,
    VendorSpecified,
    Estimated,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkBackendClass {
    Cpu,
    CudaGpu,
    Simulator,
    LabRig,
    HardwareAccelerator,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MetricSources {
    pub execution: EvidenceKind,
    pub latency: EvidenceKind,
    pub energy: EvidenceKind,
    pub power: EvidenceKind,
    pub accuracy: EvidenceKind,
    pub calibration: EvidenceKind,
    pub environment: EvidenceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct LatencyBreakdownNs {
    pub host_transfer: f64,
    pub memory: f64,
    pub scheduling: f64,
    pub reconfiguration: f64,
    pub calibration_amortization: f64,
    pub dac: f64,
    pub modulation: f64,
    pub optical_device: f64,
    pub detection: f64,
    pub adc: f64,
    pub digital_postprocessing: f64,
    pub cooling_support: f64,
}

impl LatencyBreakdownNs {
    pub fn total(&self) -> f64 {
        self.host_transfer
            + self.memory
            + self.scheduling
            + self.reconfiguration
            + self.calibration_amortization
            + self.dac
            + self.modulation
            + self.optical_device
            + self.detection
            + self.adc
            + self.digital_postprocessing
            + self.cooling_support
    }

    fn scaled(&self, value: f64) -> Self {
        Self {
            host_transfer: self.host_transfer * value,
            memory: self.memory * value,
            scheduling: self.scheduling * value,
            reconfiguration: self.reconfiguration * value,
            calibration_amortization: self.calibration_amortization * value,
            dac: self.dac * value,
            modulation: self.modulation * value,
            optical_device: self.optical_device * value,
            detection: self.detection * value,
            adc: self.adc * value,
            digital_postprocessing: self.digital_postprocessing * value,
            cooling_support: self.cooling_support * value,
        }
    }

    fn values(&self) -> [f64; 12] {
        [
            self.host_transfer,
            self.memory,
            self.scheduling,
            self.reconfiguration,
            self.calibration_amortization,
            self.dac,
            self.modulation,
            self.optical_device,
            self.detection,
            self.adc,
            self.digital_postprocessing,
            self.cooling_support,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct EnergyBreakdownJ {
    pub host_transfer: f64,
    pub memory: f64,
    pub scheduling: f64,
    pub reconfiguration: f64,
    pub calibration_amortization: f64,
    pub laser: f64,
    pub modulation: f64,
    pub dac: f64,
    pub optical_device: f64,
    pub detector: f64,
    pub adc: f64,
    pub digital_postprocessing: f64,
    pub cooling_support: f64,
}

impl EnergyBreakdownJ {
    pub fn total(&self) -> f64 {
        self.host_transfer
            + self.memory
            + self.scheduling
            + self.reconfiguration
            + self.calibration_amortization
            + self.laser
            + self.modulation
            + self.dac
            + self.optical_device
            + self.detector
            + self.adc
            + self.digital_postprocessing
            + self.cooling_support
    }

    fn scaled(&self, value: f64) -> Self {
        Self {
            host_transfer: self.host_transfer * value,
            memory: self.memory * value,
            scheduling: self.scheduling * value,
            reconfiguration: self.reconfiguration * value,
            calibration_amortization: self.calibration_amortization * value,
            laser: self.laser * value,
            modulation: self.modulation * value,
            dac: self.dac * value,
            optical_device: self.optical_device * value,
            detector: self.detector * value,
            adc: self.adc * value,
            digital_postprocessing: self.digital_postprocessing * value,
            cooling_support: self.cooling_support * value,
        }
    }

    fn values(&self) -> [f64; 13] {
        [
            self.host_transfer,
            self.memory,
            self.scheduling,
            self.reconfiguration,
            self.calibration_amortization,
            self.laser,
            self.modulation,
            self.dac,
            self.optical_device,
            self.detector,
            self.adc,
            self.digital_postprocessing,
            self.cooling_support,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FullSystemAccountingModel {
    pub steady_power_w: f64,
    pub peak_power_w: f64,
    pub latency_shares: LatencyBreakdownNs,
    pub energy_shares: EnergyBreakdownJ,
}

impl FullSystemAccountingModel {
    fn validate(&self) -> Result<()> {
        positive(self.steady_power_w, "steady power")?;
        positive(self.peak_power_w, "peak power")?;
        if self.peak_power_w < self.steady_power_w {
            bail!("peak power must be greater than or equal to steady power");
        }
        validate_non_negative(self.latency_shares.values(), "latency accounting shares")?;
        validate_non_negative(self.energy_shares.values(), "energy accounting shares")?;
        approximately_one(self.latency_shares.total(), "latency accounting shares")?;
        approximately_one(self.energy_shares.total(), "energy accounting shares")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "runner", rename_all = "snake_case", deny_unknown_fields)]
pub enum BenchmarkRunner {
    CpuReference {
        accounting: FullSystemAccountingModel,
    },
    Simulator {
        target: TargetBackend,
        effective_bits: u8,
        noise_fraction: f64,
        accounting: FullSystemAccountingModel,
    },
    ExternalCommand {
        executable: String,
        args: Vec<String>,
        timeout_seconds: u64,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegressionEnforcement {
    RequiredReference,
    AdvisoryHardware,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RegressionPolicy {
    pub enforcement: RegressionEnforcement,
    pub reference_artifact: String,
    pub max_p95_latency_ns: Option<f64>,
    pub max_p95_energy_j: Option<f64>,
    pub min_throughput_gops: Option<f64>,
    pub max_p99_absolute_error: Option<f64>,
    pub max_p99_relative_error: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkBackendSpec {
    pub id: String,
    pub class: BenchmarkBackendClass,
    pub runner: BenchmarkRunner,
    pub regression: Option<RegressionPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkSuite {
    pub version: String,
    pub id: String,
    pub description: String,
    pub fixture: KernelRequest,
    pub warmup: usize,
    pub repetitions: usize,
    pub seed: u64,
    pub backends: Vec<BenchmarkBackendSpec>,
}

impl BenchmarkSuite {
    pub fn validate(&self) -> Result<()> {
        if self.version != HIL_SUITE_VERSION {
            bail!(
                "unsupported HIL suite version '{}'; expected '{}'",
                self.version,
                HIL_SUITE_VERSION
            );
        }
        validate_identifier(&self.id, "suite id")?;
        if self.description.trim().is_empty() || self.repetitions == 0 || self.backends.is_empty() {
            bail!("benchmark suites require a description, repetitions, and backends");
        }
        self.fixture.validate()?;
        let mut ids = BTreeSet::new();
        for backend in &self.backends {
            validate_identifier(&backend.id, "backend id")?;
            if !ids.insert(&backend.id) {
                bail!("duplicate benchmark backend id '{}'", backend.id);
            }
            match &backend.runner {
                BenchmarkRunner::CpuReference { accounting } => {
                    if backend.class != BenchmarkBackendClass::Cpu {
                        bail!("CPU reference runners must use the cpu backend class");
                    }
                    accounting.validate()?;
                }
                BenchmarkRunner::Simulator {
                    target,
                    effective_bits,
                    noise_fraction,
                    accounting,
                } => {
                    if backend.class != BenchmarkBackendClass::Simulator
                        || *target == TargetBackend::Auto
                        || *target == TargetBackend::Cpu
                        || *effective_bits == 0
                        || !noise_fraction.is_finite()
                        || *noise_fraction < 0.0
                    {
                        bail!("simulator runners require a concrete accelerator target and valid numerical controls");
                    }
                    accounting.validate()?;
                }
                BenchmarkRunner::ExternalCommand {
                    executable,
                    timeout_seconds,
                    ..
                } => {
                    if executable.trim().is_empty() || *timeout_seconds == 0 {
                        bail!("external benchmark drivers require an executable and timeout");
                    }
                }
            }
            if let Some(policy) = &backend.regression {
                validate_regression_policy(policy)?;
                if matches!(
                    backend.class,
                    BenchmarkBackendClass::LabRig | BenchmarkBackendClass::HardwareAccelerator
                ) && policy.enforcement == RegressionEnforcement::RequiredReference
                {
                    bail!("noisy lab and hardware regression thresholds must be advisory");
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkRunContext {
    pub commit_sha: String,
    pub runner_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HardwareEnvironment {
    pub hardware_vendor: String,
    pub hardware_model: String,
    pub topology: String,
    pub clock_summary: String,
    pub temperature_c: Option<f64>,
    pub software_versions: BTreeMap<String, String>,
    pub operating_system: String,
    pub commit_sha: String,
    pub runner_id: String,
    pub calibration_snapshot_id: Option<String>,
    pub calibration_fingerprint: Option<String>,
    pub observed_at: String,
    pub unavailable_fields: Vec<String>,
}

impl HardwareEnvironment {
    fn validate(&self) -> Result<()> {
        for (value, label) in [
            (&self.hardware_vendor, "hardware vendor"),
            (&self.hardware_model, "hardware model"),
            (&self.topology, "topology"),
            (&self.clock_summary, "clock summary"),
            (&self.operating_system, "operating system"),
            (&self.commit_sha, "commit SHA"),
            (&self.runner_id, "runner id"),
            (&self.observed_at, "observation time"),
        ] {
            if value.trim().is_empty() {
                bail!("benchmark environment requires {label}");
            }
        }
        if self.software_versions.is_empty()
            || self
                .software_versions
                .iter()
                .any(|(key, value)| key.trim().is_empty() || value.trim().is_empty())
        {
            bail!("benchmark environment requires explicit software versions");
        }
        if self.temperature_c.is_some_and(|value| !value.is_finite()) {
            bail!("environment temperature must be finite when available");
        }
        chrono::DateTime::parse_from_rfc3339(&self.observed_at)
            .context("benchmark environment observed_at must be RFC 3339")?;
        match (
            self.calibration_snapshot_id.as_ref(),
            self.calibration_fingerprint.as_ref(),
        ) {
            (Some(id), Some(fingerprint))
                if !id.trim().is_empty() && valid_fingerprint(fingerprint) => {}
            (None, None) => {}
            _ => bail!("calibration snapshot id and fingerprint must be provided together"),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawBenchmarkSample {
    pub iteration: usize,
    pub latency_ns: f64,
    pub energy_j: f64,
    pub peak_power_w: f64,
    pub steady_power_w: f64,
    pub temperature_c: Option<f64>,
    pub latency_breakdown_ns: LatencyBreakdownNs,
    pub energy_breakdown_j: EnergyBreakdownJ,
    pub raw_counters: BTreeMap<String, f64>,
}

impl RawBenchmarkSample {
    fn validate(&self) -> Result<()> {
        positive(self.latency_ns, "sample latency")?;
        non_negative(self.energy_j, "sample energy")?;
        positive(self.peak_power_w, "sample peak power")?;
        positive(self.steady_power_w, "sample steady power")?;
        if self.peak_power_w < self.steady_power_w {
            bail!("sample peak power must be at least steady power");
        }
        if self.temperature_c.is_some_and(|value| !value.is_finite()) {
            bail!("sample temperature must be finite when present");
        }
        validate_non_negative(self.latency_breakdown_ns.values(), "latency breakdown")?;
        validate_non_negative(self.energy_breakdown_j.values(), "energy breakdown")?;
        approximately_equal(
            self.latency_ns,
            self.latency_breakdown_ns.total(),
            "full-system latency breakdown",
        )?;
        approximately_equal(
            self.energy_j,
            self.energy_breakdown_j.total(),
            "full-system energy breakdown",
        )?;
        if self
            .raw_counters
            .iter()
            .any(|(key, value)| key.trim().is_empty() || !value.is_finite())
        {
            bail!("raw benchmark counters require names and finite values");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DriverOutputSample {
    pub iteration: usize,
    pub outputs: Vec<KernelTensor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkDriverRequest {
    pub version: String,
    pub suite_id: String,
    pub backend_id: String,
    pub fixture: KernelRequest,
    pub warmup: usize,
    pub repetitions: usize,
    pub seed: u64,
    pub commit_sha: String,
    pub runner_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkDriverResponse {
    pub version: String,
    pub backend_id: String,
    pub sources: MetricSources,
    pub environment: HardwareEnvironment,
    pub calibration_duration_ns: f64,
    pub samples: Vec<RawBenchmarkSample>,
    pub output_samples: Vec<DriverOutputSample>,
    pub raw_data: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Distribution {
    pub minimum: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub maximum: f64,
    pub mean: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkMetrics {
    pub latency_ns: Distribution,
    pub throughput_gops: Distribution,
    pub energy_j: Distribution,
    pub peak_power_w: Distribution,
    pub steady_power_w: Distribution,
    pub absolute_error: Distribution,
    pub relative_error: Distribution,
    pub calibration_duration_ns: f64,
    pub conversion_latency_share: f64,
    pub conversion_energy_share: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RegressionFinding {
    pub metric: String,
    pub observed: f64,
    pub threshold: f64,
    pub passed: bool,
    pub enforcement: RegressionEnforcement,
    pub reference_artifact: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BackendBenchmarkResult {
    pub backend_id: String,
    pub class: BenchmarkBackendClass,
    pub sources: MetricSources,
    pub environment: HardwareEnvironment,
    pub metrics: BenchmarkMetrics,
    pub accuracy_within_contract: bool,
    pub regression_findings: Vec<RegressionFinding>,
    pub raw_samples: Vec<RawBenchmarkSample>,
    pub raw_absolute_errors: Vec<f64>,
    pub raw_relative_errors: Vec<f64>,
    pub output_checksums: Vec<String>,
    pub raw_data: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BackendBenchmarkFailure {
    pub backend_id: String,
    pub class: BenchmarkBackendClass,
    pub error: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Verified,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkVerification {
    pub status: VerificationStatus,
    pub checks: Vec<String>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkArtifact {
    pub version: String,
    pub suite_id: String,
    pub suite_fingerprint: String,
    pub fixture_fingerprint: String,
    pub generated_at: String,
    pub commit_sha: String,
    pub warmup: usize,
    pub repetitions: usize,
    pub seed: u64,
    pub suite: BenchmarkSuite,
    pub fixture: KernelRequest,
    pub results: Vec<BackendBenchmarkResult>,
    pub backend_failures: Vec<BackendBenchmarkFailure>,
    pub verification: BenchmarkVerification,
    pub artifact_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkClaim {
    pub metric: String,
    pub value: f64,
    pub unit: String,
    pub statement: String,
    pub baseline_source: EvidenceKind,
    pub candidate_source: EvidenceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkClaims {
    pub version: String,
    pub suite_id: String,
    pub artifact_url: String,
    pub artifact_fingerprint: String,
    pub baseline_backend_id: String,
    pub candidate_backend_id: String,
    pub generated_at: String,
    pub claims: Vec<BenchmarkClaim>,
    pub verification: VerificationStatus,
    pub claims_fingerprint: String,
}

pub fn run_benchmark_suite(
    suite: &BenchmarkSuite,
    context: &BenchmarkRunContext,
) -> Result<BenchmarkArtifact> {
    suite.validate()?;
    validate_run_context(context)?;
    let reference = execute_kernel_reference(&suite.fixture)?;
    let suite_fingerprint = sha256_json(suite)?;
    let fixture_fingerprint = sha256_json(&suite.fixture)?;
    let mut results = Vec::new();
    let mut backend_failures = Vec::new();
    for backend in &suite.backends {
        match execute_backend(suite, backend, context)
            .and_then(|response| result_from_response(suite, backend, response, &reference))
        {
            Ok(result) => results.push(result),
            Err(error) => backend_failures.push(BackendBenchmarkFailure {
                backend_id: backend.id.clone(),
                class: backend.class,
                error: format!("{error:#}"),
            }),
        }
    }
    results.sort_by(|left, right| left.backend_id.cmp(&right.backend_id));
    backend_failures.sort_by(|left, right| left.backend_id.cmp(&right.backend_id));
    let verification = verify_artifact_results(suite, &results, &backend_failures);
    let mut artifact = BenchmarkArtifact {
        version: HIL_ARTIFACT_VERSION.to_string(),
        suite_id: suite.id.clone(),
        suite_fingerprint,
        fixture_fingerprint,
        generated_at: now(),
        commit_sha: context.commit_sha.clone(),
        warmup: suite.warmup,
        repetitions: suite.repetitions,
        seed: suite.seed,
        suite: suite.clone(),
        fixture: suite.fixture.clone(),
        results,
        backend_failures,
        verification,
        artifact_fingerprint: String::new(),
    };
    artifact.artifact_fingerprint = sha256_json(&artifact)?;
    Ok(artifact)
}

pub fn validate_benchmark_artifact(artifact: &BenchmarkArtifact) -> Result<()> {
    if artifact.version != HIL_ARTIFACT_VERSION
        || artifact.suite_id.trim().is_empty()
        || artifact.commit_sha.trim().is_empty()
        || !valid_fingerprint(&artifact.suite_fingerprint)
        || !valid_fingerprint(&artifact.fixture_fingerprint)
        || !valid_fingerprint(&artifact.artifact_fingerprint)
        || artifact.repetitions == 0
    {
        bail!("benchmark artifact identity or protocol fields are invalid");
    }
    chrono::DateTime::parse_from_rfc3339(&artifact.generated_at)
        .context("benchmark artifact generated_at must be RFC 3339")?;
    artifact.fixture.validate()?;
    artifact.suite.validate()?;
    if artifact.suite.id != artifact.suite_id
        || artifact.suite.fixture != artifact.fixture
        || artifact.suite.warmup != artifact.warmup
        || artifact.suite.repetitions != artifact.repetitions
        || artifact.suite.seed != artifact.seed
        || sha256_json(&artifact.suite)? != artifact.suite_fingerprint
    {
        bail!("benchmark artifact does not match its embedded suite");
    }
    if sha256_json(&artifact.fixture)? != artifact.fixture_fingerprint {
        bail!("benchmark artifact fixture fingerprint does not match its content");
    }
    let mut copy = artifact.clone();
    copy.artifact_fingerprint.clear();
    if sha256_json(&copy)? != artifact.artifact_fingerprint {
        bail!("benchmark artifact fingerprint does not match its content");
    }
    let mut ids = BTreeSet::new();
    for result in &artifact.results {
        if !ids.insert(&result.backend_id) {
            bail!("benchmark artifact contains duplicate backend results");
        }
        result.environment.validate()?;
        if result.raw_samples.len() != artifact.repetitions
            || result.output_checksums.len() != artifact.repetitions
            || result.raw_absolute_errors.is_empty()
            || result.raw_absolute_errors.len() != result.raw_relative_errors.len()
        {
            bail!("benchmark result raw sample or error counts are incomplete");
        }
        let expected_iterations = (0..artifact.repetitions).collect::<BTreeSet<_>>();
        let actual_iterations = result
            .raw_samples
            .iter()
            .map(|sample| sample.iteration)
            .collect::<BTreeSet<_>>();
        if actual_iterations != expected_iterations
            || result
                .output_checksums
                .iter()
                .any(|checksum| !valid_fingerprint(checksum))
        {
            bail!("benchmark result raw iterations or output checksums are invalid");
        }
        for sample in &result.raw_samples {
            sample.validate()?;
        }
        if result.environment.commit_sha != artifact.commit_sha {
            bail!("benchmark result environment commit differs from the artifact commit");
        }
        validate_derived_metrics(artifact, result)?;
    }
    let expected_backends = artifact
        .suite
        .backends
        .iter()
        .map(|backend| (&backend.id, backend.class))
        .collect::<BTreeMap<_, _>>();
    for result in &artifact.results {
        if expected_backends.get(&result.backend_id) != Some(&result.class) {
            bail!("benchmark result identity or class differs from the embedded suite");
        }
        let backend = artifact
            .suite
            .backends
            .iter()
            .find(|backend| backend.id == result.backend_id)
            .context("benchmark result has no embedded-suite backend")?;
        let expected_findings = backend
            .regression
            .as_ref()
            .map(|policy| evaluate_regressions(policy, &result.metrics))
            .unwrap_or_default();
        if result.regression_findings != expected_findings {
            bail!("benchmark regression findings do not match the embedded policy and metrics");
        }
    }
    for failure in &artifact.backend_failures {
        if failure.error.trim().is_empty() {
            bail!("benchmark failure diagnostics must not be empty");
        }
        if !ids.insert(&failure.backend_id) {
            bail!("benchmark artifact contains duplicate backend evidence");
        }
        if expected_backends.get(&failure.backend_id) != Some(&failure.class) {
            bail!("benchmark failure identity or class differs from the embedded suite");
        }
    }
    if artifact.results.len() + artifact.backend_failures.len() != expected_backends.len() {
        bail!("benchmark artifact does not account for every embedded-suite backend exactly once");
    }
    let expected_verification = verify_artifact_results(
        &artifact.suite,
        &artifact.results,
        &artifact.backend_failures,
    );
    if artifact.verification != expected_verification {
        bail!("benchmark verification does not match the embedded evidence");
    }
    Ok(())
}

fn validate_derived_metrics(
    artifact: &BenchmarkArtifact,
    result: &BackendBenchmarkResult,
) -> Result<()> {
    let latency = result
        .raw_samples
        .iter()
        .map(|sample| sample.latency_ns)
        .collect::<Vec<_>>();
    let energy = result
        .raw_samples
        .iter()
        .map(|sample| sample.energy_j)
        .collect::<Vec<_>>();
    let peak_power = result
        .raw_samples
        .iter()
        .map(|sample| sample.peak_power_w)
        .collect::<Vec<_>>();
    let steady_power = result
        .raw_samples
        .iter()
        .map(|sample| sample.steady_power_w)
        .collect::<Vec<_>>();
    let operations = artifact.fixture.descriptor()?.operations;
    let throughput = latency
        .iter()
        .map(|latency| operations / latency)
        .collect::<Vec<_>>();
    let expected = [
        (
            "latency",
            distribution(&latency)?,
            &result.metrics.latency_ns,
        ),
        (
            "throughput",
            distribution(&throughput)?,
            &result.metrics.throughput_gops,
        ),
        ("energy", distribution(&energy)?, &result.metrics.energy_j),
        (
            "peak power",
            distribution(&peak_power)?,
            &result.metrics.peak_power_w,
        ),
        (
            "steady power",
            distribution(&steady_power)?,
            &result.metrics.steady_power_w,
        ),
        (
            "absolute error",
            distribution(&result.raw_absolute_errors)?,
            &result.metrics.absolute_error,
        ),
        (
            "relative error",
            distribution(&result.raw_relative_errors)?,
            &result.metrics.relative_error,
        ),
    ];
    if let Some((label, _, _)) = expected
        .iter()
        .find(|(_, recomputed, recorded)| !distributions_match(recomputed, recorded))
    {
        bail!("benchmark result {label} distribution does not match raw samples");
    }
    let conversion_latency = result
        .raw_samples
        .iter()
        .map(|sample| {
            let value = &sample.latency_breakdown_ns;
            value.host_transfer + value.dac + value.modulation + value.detection + value.adc
        })
        .sum::<f64>()
        / latency.iter().sum::<f64>();
    let total_energy = energy.iter().sum::<f64>();
    let conversion_energy = if total_energy == 0.0 {
        0.0
    } else {
        result
            .raw_samples
            .iter()
            .map(|sample| {
                let value = &sample.energy_breakdown_j;
                value.host_transfer + value.dac + value.modulation + value.detector + value.adc
            })
            .sum::<f64>()
            / total_energy
    };
    approximately_equal(
        result.metrics.conversion_latency_share,
        conversion_latency,
        "conversion latency share",
    )?;
    approximately_equal(
        result.metrics.conversion_energy_share,
        conversion_energy,
        "conversion energy share",
    )?;
    let accuracy_within_contract = result.metrics.absolute_error.maximum
        <= artifact.fixture.accuracy.max_abs_error
        || result.metrics.relative_error.maximum <= artifact.fixture.accuracy.max_rel_error;
    if accuracy_within_contract != result.accuracy_within_contract {
        bail!("benchmark accuracy verdict does not match raw error distributions");
    }
    Ok(())
}

pub fn write_benchmark_artifact_set(
    output_dir: &Path,
    suite: &BenchmarkSuite,
    artifact: &BenchmarkArtifact,
) -> Result<Vec<PathBuf>> {
    validate_benchmark_artifact(artifact)?;
    if output_dir.exists()
        && output_dir
            .read_dir()
            .with_context(|| format!("read {}", output_dir.display()))?
            .next()
            .is_some()
    {
        bail!("benchmark output directory must be absent or empty");
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("create {}", output_dir.display()))?;
    let suite_path = output_dir.join("suite.json");
    let digest = artifact
        .artifact_fingerprint
        .strip_prefix("sha256:")
        .context("artifact fingerprint prefix")?;
    let artifact_name = format!("benchmark-{digest}.json");
    let artifact_path = output_dir.join(&artifact_name);
    let suite_bytes = serde_json::to_vec_pretty(suite)?;
    let artifact_bytes = serde_json::to_vec_pretty(artifact)?;
    std::fs::write(&suite_path, &suite_bytes)?;
    std::fs::write(&artifact_path, &artifact_bytes)?;
    let sums = format!(
        "{}  suite.json\n{}  {}\n",
        sha256_bytes(&suite_bytes),
        sha256_bytes(&artifact_bytes),
        artifact_name
    );
    let sums_path = output_dir.join("SHA256SUMS");
    std::fs::write(&sums_path, sums)?;
    Ok(vec![suite_path, artifact_path, sums_path])
}

pub fn generate_public_claims(
    artifact: &BenchmarkArtifact,
    artifact_url: &str,
    baseline_backend_id: &str,
    candidate_backend_id: &str,
) -> Result<BenchmarkClaims> {
    validate_benchmark_artifact(artifact)?;
    if artifact.verification.status != VerificationStatus::Verified {
        bail!("public claims require a verified benchmark artifact");
    }
    let digest = artifact
        .artifact_fingerprint
        .strip_prefix("sha256:")
        .context("artifact fingerprint prefix")?;
    validate_immutable_artifact_url(artifact_url, digest)?;
    let baseline = find_result(artifact, baseline_backend_id)?;
    let candidate = find_result(artifact, candidate_backend_id)?;
    if !matches!(
        candidate.class,
        BenchmarkBackendClass::LabRig | BenchmarkBackendClass::HardwareAccelerator
    ) {
        bail!("public acceleration claims require a lab-rig or hardware-accelerator candidate");
    }
    for (source, label) in [
        (baseline.sources.latency, "baseline latency"),
        (baseline.sources.energy, "baseline energy"),
        (baseline.sources.accuracy, "baseline accuracy"),
        (candidate.sources.latency, "candidate latency"),
        (candidate.sources.energy, "candidate energy"),
        (candidate.sources.accuracy, "candidate accuracy"),
    ] {
        if source != EvidenceKind::Measured {
            bail!("public hardware claims require measured {label}");
        }
    }
    if !baseline.accuracy_within_contract || !candidate.accuracy_within_contract {
        bail!("public claims require both compared backends to pass the accuracy contract");
    }
    let latency_ratio = baseline.metrics.latency_ns.p50 / candidate.metrics.latency_ns.p50;
    let energy_ratio = baseline.metrics.energy_j.p50 / candidate.metrics.energy_j.p50;
    positive(latency_ratio, "latency ratio")?;
    positive(energy_ratio, "energy ratio")?;
    if latency_ratio <= 1.0 || energy_ratio <= 1.0 {
        bail!(
            "public lower-latency and lower-energy claims require measured ratios greater than one"
        );
    }
    if !artifact.fixture.calibration_inputs.is_empty()
        && (candidate.environment.calibration_snapshot_id.is_none()
            || candidate.environment.calibration_fingerprint.is_none())
    {
        bail!("calibrated hardware claims require an immutable calibration snapshot in the environment");
    }
    let claims = vec![
        BenchmarkClaim {
            metric: "p50_full_system_latency_ratio".to_string(),
            value: latency_ratio,
            unit: "x".to_string(),
            statement: format!(
                "{} measured {:.3}x lower p50 full-system application latency than {} for suite '{}'; the boundary includes transfers, memory, scheduling, reconfiguration, conversion, optical-device execution, and digital post-processing.",
                candidate.backend_id, latency_ratio, baseline.backend_id, artifact.suite_id
            ),
            baseline_source: baseline.sources.latency,
            candidate_source: candidate.sources.latency,
        },
        BenchmarkClaim {
            metric: "p50_full_system_energy_ratio".to_string(),
            value: energy_ratio,
            unit: "x".to_string(),
            statement: format!(
                "{} measured {:.3}x lower p50 full-system energy than {} for suite '{}'; the boundary includes conversion, lasers, optical devices, digital processing, calibration amortization, and support power.",
                candidate.backend_id, energy_ratio, baseline.backend_id, artifact.suite_id
            ),
            baseline_source: baseline.sources.energy,
            candidate_source: candidate.sources.energy,
        },
    ];
    let mut report = BenchmarkClaims {
        version: BENCHMARK_CLAIMS_VERSION.to_string(),
        suite_id: artifact.suite_id.clone(),
        artifact_url: artifact_url.to_string(),
        artifact_fingerprint: artifact.artifact_fingerprint.clone(),
        baseline_backend_id: baseline.backend_id.clone(),
        candidate_backend_id: candidate.backend_id.clone(),
        generated_at: now(),
        claims,
        verification: VerificationStatus::Verified,
        claims_fingerprint: String::new(),
    };
    report.claims_fingerprint = sha256_json(&report)?;
    Ok(report)
}

pub fn claims_markdown(claims: &BenchmarkClaims) -> String {
    let mut output = format!(
        "# Verified AWEN benchmark claims\n\nEvidence: [{}]({}) (`{}`)\n\n",
        claims.artifact_fingerprint, claims.artifact_url, claims.artifact_fingerprint
    );
    for claim in &claims.claims {
        output.push_str(&format!("- {}\n", claim.statement));
    }
    output.push_str("\nThese claims use end-to-end application boundaries; optical propagation time alone is never reported as application latency.\n");
    output
}

fn execute_backend(
    suite: &BenchmarkSuite,
    backend: &BenchmarkBackendSpec,
    context: &BenchmarkRunContext,
) -> Result<BenchmarkDriverResponse> {
    let request = BenchmarkDriverRequest {
        version: HIL_DRIVER_VERSION.to_string(),
        suite_id: suite.id.clone(),
        backend_id: backend.id.clone(),
        fixture: suite.fixture.clone(),
        warmup: suite.warmup,
        repetitions: suite.repetitions,
        seed: suite.seed,
        commit_sha: context.commit_sha.clone(),
        runner_id: context.runner_id.clone(),
    };
    let response = match &backend.runner {
        BenchmarkRunner::CpuReference { accounting } => execute_builtin(&request, accounting, None),
        BenchmarkRunner::Simulator {
            target,
            effective_bits,
            noise_fraction,
            accounting,
        } => execute_builtin(
            &request,
            accounting,
            Some(KernelSimulatorOptions {
                target: *target,
                effective_bits: *effective_bits,
                noise_fraction: *noise_fraction,
                seed: suite.seed,
            }),
        ),
        BenchmarkRunner::ExternalCommand {
            executable,
            args,
            timeout_seconds,
        } => execute_external(&request, executable, args, *timeout_seconds),
    }?;
    if response.environment.commit_sha != context.commit_sha
        || response.environment.runner_id != context.runner_id
    {
        bail!(
            "benchmark driver environment does not match the requested commit and runner identity"
        );
    }
    Ok(response)
}

fn execute_builtin(
    request: &BenchmarkDriverRequest,
    accounting: &FullSystemAccountingModel,
    simulator: Option<KernelSimulatorOptions>,
) -> Result<BenchmarkDriverResponse> {
    let execute = |iteration: usize| -> Result<KernelResult> {
        match simulator {
            Some(mut options) => {
                options.seed = request.seed.wrapping_add(iteration as u64);
                execute_kernel_simulator(&request.fixture, options)
            }
            None => execute_kernel_reference(&request.fixture),
        }
    };
    for iteration in 0..request.warmup {
        let _ = execute(iteration)?;
    }
    let mut samples = Vec::with_capacity(request.repetitions);
    let mut output_samples = Vec::with_capacity(request.repetitions);
    for iteration in 0..request.repetitions {
        let start = Instant::now();
        let result = execute(iteration)?;
        let latency_ns = start.elapsed().as_nanos().max(1) as f64;
        let energy_j = accounting.steady_power_w * latency_ns / 1_000_000_000.0;
        samples.push(RawBenchmarkSample {
            iteration,
            latency_ns,
            energy_j,
            peak_power_w: accounting.peak_power_w,
            steady_power_w: accounting.steady_power_w,
            temperature_c: None,
            latency_breakdown_ns: accounting.latency_shares.scaled(latency_ns),
            energy_breakdown_j: accounting.energy_shares.scaled(energy_j),
            raw_counters: BTreeMap::from([("host_wall_clock_ns".to_string(), latency_ns)]),
        });
        output_samples.push(DriverOutputSample {
            iteration,
            outputs: result.outputs,
        });
    }
    let simulated = simulator.is_some();
    Ok(BenchmarkDriverResponse {
        version: HIL_DRIVER_VERSION.to_string(),
        backend_id: request.backend_id.clone(),
        sources: MetricSources {
            execution: if simulated {
                EvidenceKind::Simulated
            } else {
                EvidenceKind::Measured
            },
            latency: EvidenceKind::Measured,
            energy: EvidenceKind::Estimated,
            power: EvidenceKind::Estimated,
            accuracy: if simulated {
                EvidenceKind::Simulated
            } else {
                EvidenceKind::Measured
            },
            calibration: if simulated {
                EvidenceKind::Simulated
            } else {
                EvidenceKind::Measured
            },
            environment: EvidenceKind::Measured,
        },
        environment: builtin_environment(request),
        calibration_duration_ns: 0.0,
        samples,
        output_samples,
        raw_data: BTreeMap::from([
            (
                "timer".to_string(),
                Value::String("std::time::Instant".to_string()),
            ),
            (
                "energy_model".to_string(),
                Value::String("steady_power_w multiplied by measured wall clock".to_string()),
            ),
        ]),
    })
}

fn execute_external(
    request: &BenchmarkDriverRequest,
    executable: &str,
    args: &[String],
    timeout_seconds: u64,
) -> Result<BenchmarkDriverResponse> {
    let request_bytes = serde_json::to_vec(request)?;
    let mut child = Command::new(executable)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("start external benchmark driver '{executable}'"))?;
    let mut stdin = child.stdin.take().context("external driver stdin")?;
    let mut stdout = child.stdout.take().context("external driver stdout")?;
    let mut stderr = child.stderr.take().context("external driver stderr")?;
    let stdin_writer = thread::spawn(move || stdin.write_all(&request_bytes));
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let (status, timed_out) = match child.wait_timeout(Duration::from_secs(timeout_seconds))? {
        Some(status) => (status, false),
        None => {
            child.kill()?;
            (child.wait()?, true)
        }
    };
    let stdin_result = stdin_writer
        .join()
        .map_err(|_| anyhow::anyhow!("external driver stdin writer panicked"))?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow::anyhow!("external driver stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("external driver stderr reader panicked"))??;
    if timed_out {
        bail!("external benchmark driver exceeded {timeout_seconds} second timeout");
    }
    if !status.success() {
        bail!(
            "external benchmark driver exited with {status}: {}",
            String::from_utf8_lossy(&stderr)
        );
    }
    stdin_result.context("write benchmark request to external driver")?;
    serde_json::from_slice(&stdout).with_context(|| {
        format!(
            "parse external benchmark response; stderr: {}",
            String::from_utf8_lossy(&stderr)
        )
    })
}

fn result_from_response(
    suite: &BenchmarkSuite,
    backend: &BenchmarkBackendSpec,
    response: BenchmarkDriverResponse,
    reference: &KernelResult,
) -> Result<BackendBenchmarkResult> {
    validate_driver_response(suite, backend, &response)?;
    let latency_values = response
        .samples
        .iter()
        .map(|sample| sample.latency_ns)
        .collect::<Vec<_>>();
    let descriptor = suite.fixture.descriptor()?;
    let throughput = latency_values
        .iter()
        .map(|latency| descriptor.operations / latency)
        .collect::<Vec<_>>();
    let energy = response
        .samples
        .iter()
        .map(|sample| sample.energy_j)
        .collect::<Vec<_>>();
    let peak_power = response
        .samples
        .iter()
        .map(|sample| sample.peak_power_w)
        .collect::<Vec<_>>();
    let steady_power = response
        .samples
        .iter()
        .map(|sample| sample.steady_power_w)
        .collect::<Vec<_>>();
    let mut absolute_errors = Vec::new();
    let mut relative_errors = Vec::new();
    let mut output_checksums = Vec::new();
    for output_sample in &response.output_samples {
        compare_kernel_outputs(
            &reference.outputs,
            &output_sample.outputs,
            &mut absolute_errors,
            &mut relative_errors,
        )?;
        output_checksums.push(sha256_json(&output_sample.outputs)?);
    }
    if absolute_errors.is_empty() {
        absolute_errors.push(0.0);
        relative_errors.push(0.0);
    }
    let absolute_error = distribution(&absolute_errors)?;
    let relative_error = distribution(&relative_errors)?;
    let accuracy_within_contract = absolute_error.maximum <= suite.fixture.accuracy.max_abs_error
        || relative_error.maximum <= suite.fixture.accuracy.max_rel_error;
    let conversion_latency = response
        .samples
        .iter()
        .map(|sample| {
            let value = &sample.latency_breakdown_ns;
            value.host_transfer + value.dac + value.modulation + value.detection + value.adc
        })
        .sum::<f64>();
    let total_latency = latency_values.iter().sum::<f64>();
    let conversion_energy = response
        .samples
        .iter()
        .map(|sample| {
            let value = &sample.energy_breakdown_j;
            value.host_transfer + value.dac + value.modulation + value.detector + value.adc
        })
        .sum::<f64>();
    let total_energy = energy.iter().sum::<f64>();
    let metrics = BenchmarkMetrics {
        latency_ns: distribution(&latency_values)?,
        throughput_gops: distribution(&throughput)?,
        energy_j: distribution(&energy)?,
        peak_power_w: distribution(&peak_power)?,
        steady_power_w: distribution(&steady_power)?,
        absolute_error,
        relative_error,
        calibration_duration_ns: response.calibration_duration_ns,
        conversion_latency_share: conversion_latency / total_latency,
        conversion_energy_share: if total_energy == 0.0 {
            0.0
        } else {
            conversion_energy / total_energy
        },
    };
    let regression_findings = backend
        .regression
        .as_ref()
        .map(|policy| evaluate_regressions(policy, &metrics))
        .unwrap_or_default();
    Ok(BackendBenchmarkResult {
        backend_id: backend.id.clone(),
        class: backend.class,
        sources: response.sources,
        environment: response.environment,
        metrics,
        accuracy_within_contract,
        regression_findings,
        raw_samples: response.samples,
        raw_absolute_errors: absolute_errors,
        raw_relative_errors: relative_errors,
        output_checksums,
        raw_data: response.raw_data,
    })
}

fn validate_driver_response(
    suite: &BenchmarkSuite,
    backend: &BenchmarkBackendSpec,
    response: &BenchmarkDriverResponse,
) -> Result<()> {
    if response.version != HIL_DRIVER_VERSION || response.backend_id != backend.id {
        bail!("benchmark driver returned the wrong protocol version or backend id");
    }
    response.environment.validate()?;
    non_negative(response.calibration_duration_ns, "calibration duration")?;
    if response.samples.len() != suite.repetitions
        || response.output_samples.len() != suite.repetitions
    {
        bail!("benchmark driver must return one raw and output sample per repetition");
    }
    let expected = (0..suite.repetitions).collect::<BTreeSet<_>>();
    let sample_iterations = response
        .samples
        .iter()
        .map(|sample| sample.iteration)
        .collect::<BTreeSet<_>>();
    let output_iterations = response
        .output_samples
        .iter()
        .map(|sample| sample.iteration)
        .collect::<BTreeSet<_>>();
    if sample_iterations != expected || output_iterations != expected {
        bail!("benchmark driver sample iterations must be complete and unique");
    }
    for sample in &response.samples {
        sample.validate()?;
    }
    Ok(())
}

fn verify_artifact_results(
    suite: &BenchmarkSuite,
    results: &[BackendBenchmarkResult],
    backend_failures: &[BackendBenchmarkFailure],
) -> BenchmarkVerification {
    let mut checks = vec![
        "suite, fixture, warmup, repetitions, seed, and backend identities are versioned"
            .to_string(),
        "full-system latency and energy components sum to every raw sample total".to_string(),
        "latency, throughput, energy, power, accuracy, calibration, environment, and raw data are recorded"
            .to_string(),
        "every metric is tagged measured, simulated, vendor-specified, or estimated".to_string(),
    ];
    let mut failures = backend_failures
        .iter()
        .map(|failure| format!("backend '{}': {}", failure.backend_id, failure.error))
        .collect::<Vec<_>>();
    for result in results {
        if result.accuracy_within_contract {
            checks.push(format!(
                "backend '{}' passed the declared absolute-or-relative accuracy contract",
                result.backend_id
            ));
        } else {
            failures.push(format!(
                "backend '{}' exceeded both declared accuracy tolerances",
                result.backend_id
            ));
        }
        for finding in &result.regression_findings {
            if !finding.passed && finding.enforcement == RegressionEnforcement::RequiredReference {
                failures.push(format!(
                    "backend '{}' failed required regression '{}'",
                    result.backend_id, finding.metric
                ));
            }
        }
    }
    if results.len() != suite.backends.len() {
        failures.push("not every configured available backend produced a result".to_string());
    }
    BenchmarkVerification {
        status: if failures.is_empty() {
            VerificationStatus::Verified
        } else {
            VerificationStatus::Rejected
        },
        checks,
        failures,
    }
}

fn evaluate_regressions(
    policy: &RegressionPolicy,
    metrics: &BenchmarkMetrics,
) -> Vec<RegressionFinding> {
    let mut findings = Vec::new();
    let mut maximum = |metric: &str, observed: f64, threshold: Option<f64>| {
        if let Some(threshold) = threshold {
            findings.push(RegressionFinding {
                metric: metric.to_string(),
                observed,
                threshold,
                passed: observed <= threshold,
                enforcement: policy.enforcement,
                reference_artifact: policy.reference_artifact.clone(),
            });
        }
    };
    maximum(
        "p95_latency_ns",
        metrics.latency_ns.p95,
        policy.max_p95_latency_ns,
    );
    maximum(
        "p95_energy_j",
        metrics.energy_j.p95,
        policy.max_p95_energy_j,
    );
    maximum(
        "p99_absolute_error",
        metrics.absolute_error.p99,
        policy.max_p99_absolute_error,
    );
    maximum(
        "p99_relative_error",
        metrics.relative_error.p99,
        policy.max_p99_relative_error,
    );
    if let Some(threshold) = policy.min_throughput_gops {
        findings.push(RegressionFinding {
            metric: "p50_throughput_gops".to_string(),
            observed: metrics.throughput_gops.p50,
            threshold,
            passed: metrics.throughput_gops.p50 >= threshold,
            enforcement: policy.enforcement,
            reference_artifact: policy.reference_artifact.clone(),
        });
    }
    findings
}

fn compare_kernel_outputs(
    reference: &[KernelTensor],
    candidate: &[KernelTensor],
    absolute_errors: &mut Vec<f64>,
    relative_errors: &mut Vec<f64>,
) -> Result<()> {
    if reference.len() != candidate.len() {
        bail!("benchmark driver output count differs from the reference");
    }
    for (expected, observed) in reference.iter().zip(candidate) {
        if expected.shape != observed.shape
            || expected.dtype != observed.dtype
            || expected.layout != observed.layout
        {
            bail!("benchmark driver output metadata differs from the reference");
        }
        match (&expected.data, &observed.data) {
            (KernelData::Real(left), KernelData::Real(right)) => {
                if left.len() != right.len() {
                    bail!("benchmark driver output data length differs from the reference");
                }
                for (left, right) in left.iter().zip(right) {
                    record_error(*left, *right, absolute_errors, relative_errors)?;
                }
            }
            (KernelData::Complex(left), KernelData::Complex(right)) => {
                if left.len() != right.len() {
                    bail!("benchmark driver complex output length differs from the reference");
                }
                for (left, right) in left.iter().zip(right) {
                    let real = left.real - right.real;
                    let imaginary = left.imaginary - right.imaginary;
                    let absolute = real.hypot(imaginary);
                    let denominator = left.real.hypot(left.imaginary).max(f64::EPSILON);
                    absolute_errors.push(absolute);
                    relative_errors.push(absolute / denominator);
                }
            }
            _ => bail!("benchmark driver output representation differs from the reference"),
        }
    }
    Ok(())
}

fn record_error(
    expected: f64,
    observed: f64,
    absolute_errors: &mut Vec<f64>,
    relative_errors: &mut Vec<f64>,
) -> Result<()> {
    if !expected.is_finite() || !observed.is_finite() {
        bail!("benchmark outputs must contain finite values");
    }
    let absolute = (expected - observed).abs();
    absolute_errors.push(absolute);
    relative_errors.push(absolute / expected.abs().max(f64::EPSILON));
    Ok(())
}

fn distribution(values: &[f64]) -> Result<Distribution> {
    if values.is_empty()
        || values
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        bail!("metric distributions require finite non-negative samples");
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    Ok(Distribution {
        minimum: sorted[0],
        p50: percentile(&sorted, 0.50),
        p95: percentile(&sorted, 0.95),
        p99: percentile(&sorted, 0.99),
        maximum: *sorted.last().expect("non-empty distribution"),
        mean: sorted.iter().sum::<f64>() / sorted.len() as f64,
    })
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let position = percentile * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f64;
    sorted[lower] + (sorted[upper] - sorted[lower]) * fraction
}

fn builtin_environment(request: &BenchmarkDriverRequest) -> HardwareEnvironment {
    let available_threads = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    HardwareEnvironment {
        hardware_vendor: "host".to_string(),
        hardware_model: format!("{}-cpu", std::env::consts::ARCH),
        topology: format!("{available_threads} logical thread(s)"),
        clock_summary: "unavailable from portable Rust runtime".to_string(),
        temperature_c: None,
        software_versions: BTreeMap::from([
            (
                "awen_runtime".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            ),
            (
                "awen_hil_protocol".to_string(),
                HIL_DRIVER_VERSION.to_string(),
            ),
        ]),
        operating_system: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        commit_sha: request.commit_sha.clone(),
        runner_id: request.runner_id.clone(),
        calibration_snapshot_id: None,
        calibration_fingerprint: None,
        observed_at: now(),
        unavailable_fields: vec![
            "clock_hz".to_string(),
            "temperature_c".to_string(),
            "hardware_serial".to_string(),
            "calibration_snapshot".to_string(),
        ],
    }
}

fn validate_regression_policy(policy: &RegressionPolicy) -> Result<()> {
    if policy.reference_artifact.trim().is_empty()
        || [
            policy.max_p95_latency_ns,
            policy.max_p95_energy_j,
            policy.min_throughput_gops,
            policy.max_p99_absolute_error,
            policy.max_p99_relative_error,
        ]
        .into_iter()
        .flatten()
        .any(|value| !value.is_finite() || value < 0.0)
    {
        bail!("regression policies require a reference and non-negative finite thresholds");
    }
    if policy.max_p95_latency_ns.is_none()
        && policy.max_p95_energy_j.is_none()
        && policy.min_throughput_gops.is_none()
        && policy.max_p99_absolute_error.is_none()
        && policy.max_p99_relative_error.is_none()
    {
        bail!("regression policies require at least one threshold");
    }
    Ok(())
}

fn validate_run_context(context: &BenchmarkRunContext) -> Result<()> {
    if context.commit_sha.trim().is_empty() || context.runner_id.trim().is_empty() {
        bail!("benchmark run context requires commit SHA and runner id");
    }
    Ok(())
}

fn find_result<'a>(
    artifact: &'a BenchmarkArtifact,
    backend_id: &str,
) -> Result<&'a BackendBenchmarkResult> {
    artifact
        .results
        .iter()
        .find(|result| result.backend_id == backend_id)
        .with_context(|| format!("benchmark artifact has no backend '{backend_id}'"))
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        bail!("{label} must use 1-128 alphanumeric, dot, underscore, or hyphen characters");
    }
    Ok(())
}

fn validate_non_negative<const N: usize>(values: [f64; N], label: &str) -> Result<()> {
    if values
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        bail!("{label} must contain finite non-negative values");
    }
    Ok(())
}

fn positive(value: f64, label: &str) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        bail!("{label} must be finite and positive");
    }
    Ok(())
}

fn non_negative(value: f64, label: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        bail!("{label} must be finite and non-negative");
    }
    Ok(())
}

fn approximately_one(value: f64, label: &str) -> Result<()> {
    if (value - 1.0).abs() > 1e-12 {
        bail!("{label} must sum to exactly one within numerical tolerance");
    }
    Ok(())
}

fn approximately_equal(left: f64, right: f64, label: &str) -> Result<()> {
    let tolerance = left.abs().max(right.abs()) * 1e-12;
    if (left - right).abs() > tolerance {
        bail!("{label} components must sum to the reported total");
    }
    Ok(())
}

fn distributions_match(left: &Distribution, right: &Distribution) -> bool {
    [
        (left.minimum, right.minimum),
        (left.p50, right.p50),
        (left.p95, right.p95),
        (left.p99, right.p99),
        (left.maximum, right.maximum),
        (left.mean, right.mean),
    ]
    .into_iter()
    .all(|(left, right)| (left - right).abs() <= left.abs().max(right.abs()) * 1e-12)
}

fn valid_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn validate_immutable_artifact_url(artifact_url: &str, digest: &str) -> Result<()> {
    let remainder = artifact_url
        .strip_prefix("https://")
        .context("public claims require an HTTPS artifact URL")?;
    if remainder.contains('?') || remainder.contains('#') {
        bail!("immutable artifact URLs must not use a query string or fragment");
    }
    let (authority, path) = remainder
        .split_once('/')
        .context("immutable artifact URLs require an authority and content-addressed path")?;
    let filename = path.rsplit('/').next().unwrap_or_default();
    if authority.trim().is_empty() || filename.trim().is_empty() || !filename.contains(digest) {
        bail!("public claims require the artifact digest in the final HTTPS path segment");
    }
    Ok(())
}

fn sha256_json(value: &impl Serialize) -> Result<String> {
    // Hash the normalized JSON representation rather than serde's first-pass
    // float representation. This makes the content identity stable after the
    // artifact is written to JSON and parsed back into the typed model.
    let serialized = serde_json::to_vec(value)?;
    let normalized: Value = serde_json::from_slice(&serialized)?;
    Ok(format!(
        "sha256:{}",
        sha256_bytes(&serde_json::to_vec(&normalized)?)
    ))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonschema::JSONSchema;
    use serde_json::json;

    fn request() -> KernelRequest {
        serde_json::from_value(json!({
            "version": "awen.blas.v1",
            "id": "hil.gemm",
            "kind": "gemm",
            "inputs": [
                {"id":"lhs","shape":[2,2],"dtype":"f32","layout":"row_major","data":{"representation":"real","values":[1.0,2.0,3.0,4.0]}},
                {"id":"rhs","shape":[2,2],"dtype":"f32","layout":"row_major","data":{"representation":"real","values":[1.0,0.0,0.0,1.0]}}
            ],
            "accuracy": {"max_abs_error":0.05,"max_rel_error":0.05,"minimum_effective_bits":8}
        }))
        .expect("request")
    }

    fn accounting() -> FullSystemAccountingModel {
        FullSystemAccountingModel {
            steady_power_w: 20.0,
            peak_power_w: 30.0,
            latency_shares: LatencyBreakdownNs {
                memory: 0.3,
                scheduling: 0.1,
                digital_postprocessing: 0.6,
                ..LatencyBreakdownNs::default()
            },
            energy_shares: EnergyBreakdownJ {
                memory: 0.3,
                digital_postprocessing: 0.5,
                cooling_support: 0.2,
                ..EnergyBreakdownJ::default()
            },
        }
    }

    fn suite() -> BenchmarkSuite {
        BenchmarkSuite {
            version: HIL_SUITE_VERSION.to_string(),
            id: "reference-hil".to_string(),
            description: "reference CPU and simulator HIL contract".to_string(),
            fixture: request(),
            warmup: 1,
            repetitions: 3,
            seed: 17,
            backends: vec![
                BenchmarkBackendSpec {
                    id: "cpu".to_string(),
                    class: BenchmarkBackendClass::Cpu,
                    runner: BenchmarkRunner::CpuReference {
                        accounting: accounting(),
                    },
                    regression: Some(RegressionPolicy {
                        enforcement: RegressionEnforcement::RequiredReference,
                        reference_artifact: "reference:v1".to_string(),
                        max_p95_latency_ns: None,
                        max_p95_energy_j: None,
                        min_throughput_gops: None,
                        max_p99_absolute_error: Some(0.0),
                        max_p99_relative_error: Some(0.0),
                    }),
                },
                BenchmarkBackendSpec {
                    id: "photonic-simulator".to_string(),
                    class: BenchmarkBackendClass::Simulator,
                    runner: BenchmarkRunner::Simulator {
                        target: TargetBackend::Photonic,
                        effective_bits: 12,
                        noise_fraction: 0.0,
                        accounting: accounting(),
                    },
                    regression: None,
                },
            ],
        }
    }

    #[test]
    fn reference_suite_records_raw_full_system_distributions() {
        let artifact = run_benchmark_suite(
            &suite(),
            &BenchmarkRunContext {
                commit_sha: "0123456789012345678901234567890123456789".to_string(),
                runner_id: "unit-test".to_string(),
            },
        )
        .expect("benchmark artifact");
        assert_eq!(artifact.verification.status, VerificationStatus::Verified);
        assert_eq!(artifact.results.len(), 2);
        assert!(artifact.results.iter().all(|result| {
            result.raw_samples.len() == 3
                && result.metrics.latency_ns.p99 >= result.metrics.latency_ns.p50
                && result.metrics.energy_j.p99 >= result.metrics.energy_j.p50
        }));
        validate_benchmark_artifact(&artifact).expect("valid artifact");
    }

    #[test]
    fn noisy_hardware_thresholds_cannot_be_required() {
        let mut suite = suite();
        suite.backends[0].class = BenchmarkBackendClass::HardwareAccelerator;
        assert!(suite.validate().is_err());
    }

    #[test]
    fn tampered_summary_is_rejected_even_with_a_recomputed_content_digest() {
        let mut artifact = run_benchmark_suite(
            &suite(),
            &BenchmarkRunContext {
                commit_sha: "0123456789012345678901234567890123456789".to_string(),
                runner_id: "unit-test".to_string(),
            },
        )
        .expect("benchmark artifact");
        artifact.results[0].metrics.latency_ns.p50 += 1.0;
        artifact.artifact_fingerprint.clear();
        artifact.artifact_fingerprint = sha256_json(&artifact).expect("tampered digest");
        let error = validate_benchmark_artifact(&artifact).expect_err("tampering rejected");
        assert!(error.to_string().contains("latency distribution"));
    }

    #[test]
    fn tampered_verification_is_rejected_even_with_a_recomputed_content_digest() {
        let mut artifact = run_benchmark_suite(
            &suite(),
            &BenchmarkRunContext {
                commit_sha: "0123456789012345678901234567890123456789".to_string(),
                runner_id: "unit-test".to_string(),
            },
        )
        .expect("benchmark artifact");
        artifact.verification.status = VerificationStatus::Rejected;
        artifact.artifact_fingerprint.clear();
        artifact.artifact_fingerprint = sha256_json(&artifact).expect("tampered digest");
        let error = validate_benchmark_artifact(&artifact).expect_err("tampering rejected");
        assert!(error.to_string().contains("verification"));
    }

    #[test]
    fn persisted_artifact_retains_its_content_digest() {
        let artifact = run_benchmark_suite(
            &suite(),
            &BenchmarkRunContext {
                commit_sha: "0123456789012345678901234567890123456789".to_string(),
                runner_id: "unit-test".to_string(),
            },
        )
        .expect("benchmark artifact");
        let persisted = serde_json::to_vec_pretty(&artifact).expect("artifact JSON");
        let reloaded: BenchmarkArtifact =
            serde_json::from_slice(&persisted).expect("reloaded artifact");
        let mut original_without_digest = artifact.clone();
        original_without_digest.artifact_fingerprint.clear();
        let mut reloaded_without_digest = reloaded.clone();
        reloaded_without_digest.artifact_fingerprint.clear();
        assert_eq!(
            sha256_json(&original_without_digest).expect("original digest"),
            sha256_json(&reloaded_without_digest).expect("reloaded digest")
        );
        validate_benchmark_artifact(&reloaded).expect("persisted artifact remains valid");
    }

    #[test]
    fn mutable_or_simulated_claim_evidence_is_rejected() {
        let artifact = run_benchmark_suite(
            &suite(),
            &BenchmarkRunContext {
                commit_sha: "0123456789012345678901234567890123456789".to_string(),
                runner_id: "unit-test".to_string(),
            },
        )
        .expect("benchmark artifact");
        assert!(generate_public_claims(
            &artifact,
            "https://example.com/latest.json",
            "cpu",
            "photonic-simulator"
        )
        .is_err());
        let digest = artifact.artifact_fingerprint.trim_start_matches("sha256:");
        assert!(generate_public_claims(
            &artifact,
            &format!("https://example.com/latest.json?digest={digest}"),
            "cpu",
            "photonic-simulator"
        )
        .is_err());
        assert!(generate_public_claims(
            &artifact,
            &format!("https://example.com/benchmark-{digest}.json"),
            "cpu",
            "photonic-simulator"
        )
        .is_err());
    }

    #[test]
    fn verified_measured_hardware_generates_content_bound_claims() {
        let mut artifact = run_benchmark_suite(
            &suite(),
            &BenchmarkRunContext {
                commit_sha: "0123456789012345678901234567890123456789".to_string(),
                runner_id: "unit-test".to_string(),
            },
        )
        .expect("benchmark artifact");
        let baseline = artifact
            .results
            .iter_mut()
            .find(|result| result.backend_id == "cpu")
            .expect("baseline");
        baseline.sources.energy = EvidenceKind::Measured;
        baseline.sources.power = EvidenceKind::Measured;
        let mut candidate = baseline.clone();
        candidate.backend_id = "measured-hardware".to_string();
        candidate.class = BenchmarkBackendClass::HardwareAccelerator;
        candidate.sources = MetricSources {
            execution: EvidenceKind::Measured,
            latency: EvidenceKind::Measured,
            energy: EvidenceKind::Measured,
            power: EvidenceKind::Measured,
            accuracy: EvidenceKind::Measured,
            calibration: EvidenceKind::Measured,
            environment: EvidenceKind::Measured,
        };
        candidate.environment.hardware_vendor = "test-vendor".to_string();
        candidate.environment.hardware_model = "test-accelerator".to_string();
        candidate.environment.calibration_snapshot_id = Some("calibration-1".to_string());
        candidate.environment.calibration_fingerprint = Some(format!("sha256:{}", "a".repeat(64)));
        candidate.environment.unavailable_fields.clear();
        for sample in &mut candidate.raw_samples {
            sample.latency_ns *= 0.5;
            sample.latency_breakdown_ns = sample.latency_breakdown_ns.scaled(0.5);
            sample.energy_j *= 0.5;
            sample.energy_breakdown_j = sample.energy_breakdown_j.scaled(0.5);
        }
        let latency = candidate
            .raw_samples
            .iter()
            .map(|sample| sample.latency_ns)
            .collect::<Vec<_>>();
        let energy = candidate
            .raw_samples
            .iter()
            .map(|sample| sample.energy_j)
            .collect::<Vec<_>>();
        let operations = artifact
            .fixture
            .descriptor()
            .expect("descriptor")
            .operations;
        candidate.metrics.latency_ns = distribution(&latency).expect("latency distribution");
        candidate.metrics.throughput_gops = distribution(
            &latency
                .iter()
                .map(|latency| operations / latency)
                .collect::<Vec<_>>(),
        )
        .expect("throughput distribution");
        candidate.metrics.energy_j = distribution(&energy).expect("energy distribution");
        candidate.regression_findings.clear();
        let simulator_index = artifact
            .results
            .iter()
            .position(|result| result.backend_id == "photonic-simulator")
            .expect("simulator candidate");
        artifact.results[simulator_index] = candidate;
        let suite_candidate = artifact
            .suite
            .backends
            .iter_mut()
            .find(|backend| backend.id == "photonic-simulator")
            .expect("suite candidate");
        suite_candidate.id = "measured-hardware".to_string();
        suite_candidate.class = BenchmarkBackendClass::HardwareAccelerator;
        suite_candidate.runner = BenchmarkRunner::ExternalCommand {
            executable: "/opt/awen/bin/test-driver".to_string(),
            args: Vec::new(),
            timeout_seconds: 60,
        };
        suite_candidate.regression = None;
        artifact.suite_fingerprint = sha256_json(&artifact.suite).expect("suite digest");
        artifact.verification = verify_artifact_results(
            &artifact.suite,
            &artifact.results,
            &artifact.backend_failures,
        );
        artifact.artifact_fingerprint.clear();
        artifact.artifact_fingerprint = sha256_json(&artifact).expect("artifact digest");
        let digest = artifact.artifact_fingerprint.trim_start_matches("sha256:");
        let claims = generate_public_claims(
            &artifact,
            &format!("https://benchmarks.awen.dev/benchmark-{digest}.json"),
            "cpu",
            "measured-hardware",
        )
        .expect("verified claims");
        assert_eq!(claims.claims.len(), 2);
        assert!(claims
            .claims
            .iter()
            .all(|claim| claim.statement.contains("full-system")));
        assert!(claims_markdown(&claims).contains(&artifact.artifact_fingerprint));
        let schema: Value = serde_json::from_str(include_str!(
            "../../awen-spec/schemas/awen_benchmark_claims.v1.json"
        ))
        .expect("claims schema JSON");
        let validator = JSONSchema::options()
            .compile(&schema)
            .expect("claims schema compiles");
        assert!(validator.is_valid(&serde_json::to_value(claims).expect("claims value")));
    }
}
