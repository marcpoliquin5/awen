use anyhow::Result;
use awen_compiler::{
    benchmark, benchmark_kernel, benchmark_with_observations, compile_with_backend,
    compile_with_cost_model, execute_kernel_reference, execute_kernel_simulator, select_kernel,
    BackendHealth, BackendSnapshot, CompileOptions, CostModelInputs, DeviceCapabilities,
    KernelBackendProfile, KernelRequest, KernelSimulatorOptions, ObservationSet,
    OptimizationObjective, TargetBackend, TensorProgram,
};
use awen_runtime::benchmark::{
    claims_markdown, generate_public_claims, run_benchmark_suite, write_benchmark_artifact_set,
    BenchmarkArtifact, BenchmarkRunContext, BenchmarkSuite as HilBenchmarkSuite,
    VerificationStatus,
};
use awen_runtime::engine::Engine;
use awen_runtime::gradients;
use awen_runtime::gradients::{GradientOptions, NoiseModel};
use awen_runtime::ir;
use clap::Parser;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Compile typed tensor operations into AWEN Photonic IR and Device IR.
    Compile {
        /// Path to an awen.tensor.v1 JSON program.
        input: String,
        /// Optional device-capability JSON. Uses the reference 128x128 backend when omitted.
        #[clap(long)]
        capabilities: Option<String>,
        /// Optional live awen.backend-health.v1 JSON snapshot.
        #[clap(long)]
        health: Option<String>,
        /// Optional awen.cost-model.v1 JSON, including calibrated parameters and provenance.
        #[clap(long)]
        cost_model: Option<String>,
        /// Compilation artifact output path.
        #[clap(long, default_value = "awen_compilation.json")]
        output: String,
        /// Optimization objective: latency, energy, accuracy, or throughput.
        #[clap(long, default_value = "latency")]
        optimize_for: String,
        /// Target selection: auto, cpu, gpu, or photonic.
        #[clap(long, default_value = "auto")]
        target: String,
        /// Deterministic autotuner seed used to break equal-cost ties.
        #[clap(long, default_value_t = 0)]
        autotune_seed: u64,
        /// Number of identical operations amortized as one batch.
        #[clap(long, default_value_t = 1)]
        batch_size: usize,
        /// Number of ranked non-winning plans retained in the artifact.
        #[clap(long, default_value_t = 3)]
        alternative_plans: usize,
        /// Permit optical/electrical conversion boundaries to be fused across a batch.
        #[clap(long)]
        fuse_boundaries: bool,
        /// Queue depth included in the end-to-end scheduling estimate.
        #[clap(long, default_value_t = 0)]
        queue_depth: usize,
        /// Fraction of transfer work overlapped with execution, within [0, 1].
        #[clap(long, default_value_t = 0.0)]
        overlap_fraction: f64,
        /// Fraction of input tensor bytes already resident on the target, within [0, 1].
        #[clap(long, default_value_t = 0.0)]
        resident_input_fraction: f64,
        /// Effective inter-device tensor-transfer bandwidth in GB/s.
        #[clap(long, default_value_t = 128.0)]
        transfer_bandwidth_gbps: f64,
        /// Fixed latency charged to each deduplicated inter-device transfer.
        #[clap(long, default_value_t = 100.0)]
        transfer_latency_ns: f64,
        /// Additional latency charged at every optical/electrical boundary.
        #[clap(long, default_value_t = 500.0)]
        crossing_penalty_ns: f64,
        /// Additional energy charged at every optical/electrical boundary.
        #[clap(long, default_value_t = 0.001)]
        crossing_penalty_uj: f64,
        /// Maximum live CPU tensor residency in bytes.
        #[clap(long, default_value_t = u64::MAX)]
        cpu_memory_budget_bytes: u64,
        /// Maximum live GPU tensor residency in bytes.
        #[clap(long, default_value_t = u64::MAX)]
        gpu_memory_budget_bytes: u64,
        /// Maximum live photonic tensor residency in bytes.
        #[clap(long, default_value_t = u64::MAX)]
        photonic_memory_budget_bytes: u64,
    },
    /// Compile and execute literal tensor data in the calibrated reference simulator.
    Benchmark {
        /// Path to an awen.tensor.v1 JSON program containing literal input data.
        input: String,
        /// Optional device-capability JSON. Uses the reference 128x128 backend when omitted.
        #[clap(long)]
        capabilities: Option<String>,
        /// Optional live awen.backend-health.v1 JSON snapshot.
        #[clap(long)]
        health: Option<String>,
        /// Optional awen.cost-model.v1 JSON, including calibrated parameters and provenance.
        #[clap(long)]
        cost_model: Option<String>,
        /// Optional JSON array of measured observations to compare with predictions.
        #[clap(long)]
        observations: Option<String>,
        /// Benchmark report output path.
        #[clap(long, default_value = "awen_benchmark.json")]
        output: String,
        /// Optimization objective: latency, energy, accuracy, or throughput.
        #[clap(long, default_value = "latency")]
        optimize_for: String,
        /// Target selection: auto, cpu, gpu, or photonic.
        #[clap(long, default_value = "auto")]
        target: String,
        /// Deterministic autotuner seed used to break equal-cost ties.
        #[clap(long, default_value_t = 0)]
        autotune_seed: u64,
        /// Number of identical operations amortized as one batch.
        #[clap(long, default_value_t = 1)]
        batch_size: usize,
        /// Number of ranked non-winning plans retained in the artifact.
        #[clap(long, default_value_t = 3)]
        alternative_plans: usize,
        /// Permit optical/electrical conversion boundaries to be fused across a batch.
        #[clap(long)]
        fuse_boundaries: bool,
        /// Queue depth included in the end-to-end scheduling estimate.
        #[clap(long, default_value_t = 0)]
        queue_depth: usize,
        /// Fraction of transfer work overlapped with execution, within [0, 1].
        #[clap(long, default_value_t = 0.0)]
        overlap_fraction: f64,
        /// Fraction of input tensor bytes already resident on the target, within [0, 1].
        #[clap(long, default_value_t = 0.0)]
        resident_input_fraction: f64,
        /// Effective inter-device tensor-transfer bandwidth in GB/s.
        #[clap(long, default_value_t = 128.0)]
        transfer_bandwidth_gbps: f64,
        /// Fixed latency charged to each deduplicated inter-device transfer.
        #[clap(long, default_value_t = 100.0)]
        transfer_latency_ns: f64,
        /// Additional latency charged at every optical/electrical boundary.
        #[clap(long, default_value_t = 500.0)]
        crossing_penalty_ns: f64,
        /// Additional energy charged at every optical/electrical boundary.
        #[clap(long, default_value_t = 0.001)]
        crossing_penalty_uj: f64,
        /// Maximum live CPU tensor residency in bytes.
        #[clap(long, default_value_t = u64::MAX)]
        cpu_memory_budget_bytes: u64,
        /// Maximum live GPU tensor residency in bytes.
        #[clap(long, default_value_t = u64::MAX)]
        gpu_memory_budget_bytes: u64,
        /// Maximum live photonic tensor residency in bytes.
        #[clap(long, default_value_t = u64::MAX)]
        photonic_memory_budget_bytes: u64,
    },
    /// Execute one versioned awenBLAS request on the CPU reference or deterministic simulator.
    Kernel {
        /// Path to an awen.blas.v1 JSON request.
        input: String,
        /// Kernel result output path.
        #[clap(long, default_value = "awen_kernel_result.json")]
        output: String,
        /// Concrete execution target: cpu, gpu, or photonic.
        #[clap(long, default_value = "cpu")]
        target: String,
        /// Simulator effective precision. Ignored for CPU reference execution.
        #[clap(long, default_value_t = 8)]
        effective_bits: u8,
        /// Deterministic simulator noise fraction.
        #[clap(long, default_value_t = 0.0)]
        noise_fraction: f64,
        /// Deterministic simulator seed.
        #[clap(long, default_value_t = 0)]
        seed: u64,
    },
    /// Measure end-to-end CPU-reference and simulator execution for one awenBLAS request.
    KernelBenchmark {
        /// Path to an awen.blas.v1 JSON request.
        input: String,
        /// Benchmark report output path.
        #[clap(long, default_value = "awen_kernel_benchmark.json")]
        output: String,
        /// Simulator target: gpu or photonic.
        #[clap(long, default_value = "photonic")]
        target: String,
        /// Simulator effective precision.
        #[clap(long, default_value_t = 8)]
        effective_bits: u8,
        /// Deterministic simulator noise fraction.
        #[clap(long, default_value_t = 0.0)]
        noise_fraction: f64,
        /// Deterministic simulator seed.
        #[clap(long, default_value_t = 0)]
        seed: u64,
        /// Number of complete request executions measured per path.
        #[clap(long, default_value_t = 10)]
        repetitions: usize,
    },
    /// Run one reproducible full-system suite across every configured available backend.
    BenchmarkSuite {
        /// Path to an awen.hil-suite.v1 JSON manifest.
        manifest: String,
        /// New or empty directory receiving the immutable artifact set.
        #[clap(long, default_value = "awen_hil_artifacts")]
        output_dir: String,
        /// Commit SHA recorded in every backend environment. Auto-detected when omitted.
        #[clap(long)]
        commit_sha: Option<String>,
        /// Stable runner identity. Uses CI or host environment metadata when omitted.
        #[clap(long)]
        runner_id: Option<String>,
    },
    /// Generate publishable claims from one verified, immutable hardware benchmark artifact.
    BenchmarkClaims {
        /// Path to an awen.hil-artifact.v1 JSON artifact.
        artifact: String,
        /// Immutable HTTPS URL with the artifact digest in its final path segment.
        #[clap(long)]
        artifact_url: String,
        /// Measured baseline backend id.
        #[clap(long)]
        baseline: String,
        /// Measured lab-rig or hardware-accelerator backend id.
        #[clap(long)]
        candidate: String,
        /// Versioned machine-readable claims output.
        #[clap(long, default_value = "awen_benchmark_claims.json")]
        output: String,
        /// Markdown document generated only from the verified artifact.
        #[clap(long, default_value = "awen_benchmark_claims.md")]
        markdown_output: String,
    },
    /// Select an awenBLAS backend from explicit capability and cost profiles.
    KernelPlan {
        /// Path to an awen.blas.v1 JSON request.
        input: String,
        /// Path to a JSON array of kernel backend profiles.
        profiles: String,
        /// Selection plan output path.
        #[clap(long, default_value = "awen_kernel_plan.json")]
        output: String,
        /// Optimization objective: latency, energy, accuracy, or throughput.
        #[clap(long, default_value = "latency")]
        optimize_for: String,
    },
    /// Load a binary AWEN executable and prepare its device dispatches.
    Execute {
        /// Path to an AWENEXE binary emitted by awen-compile.
        artifact: String,
    },
    /// Discover typed backend plugins and query their current health sources.
    Backends {
        /// Directory containing plugin manifests and their health sources.
        plugin_dir: String,
        /// Permit unsigned manifests for local development and simulation only.
        #[clap(long)]
        allow_unverified: bool,
    },
    Run {
        /// Path to IR JSON file
        ir: String,
        /// Optional RNG seed for deterministic replay
        #[clap(long)]
        seed: Option<u64>,
    },
    Gradient {
        /// Path to IR JSON file
        ir: String,
        /// Comma-separated parameter list, e.g. "mzi_0:phase,mzi_1:phase"
        params: String,
        /// Gradient strategy
        #[clap(long, default_value = "auto")]
        strategy: String,
        /// RNG seed
        #[clap(long)]
        seed: Option<u64>,
        /// Samples for stochastic estimators
        #[clap(long, default_value_t = 1u32)]
        samples: u32,
    },
}

#[derive(Clone, Copy)]
struct CompilerControls<'a> {
    optimize_for: &'a str,
    target: &'a str,
    autotune_seed: u64,
    batch_size: usize,
    alternative_plans: usize,
    fuse_boundaries: bool,
    queue_depth: usize,
    overlap_fraction: f64,
    resident_input_fraction: f64,
    transfer_bandwidth_gbps: f64,
    transfer_latency_ns: f64,
    crossing_penalty_ns: f64,
    crossing_penalty_uj: f64,
    cpu_memory_budget_bytes: u64,
    gpu_memory_budget_bytes: u64,
    photonic_memory_budget_bytes: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Compile {
            input,
            capabilities,
            health,
            cost_model,
            output,
            optimize_for,
            target,
            autotune_seed,
            batch_size,
            alternative_plans,
            fuse_boundaries,
            queue_depth,
            overlap_fraction,
            resident_input_fraction,
            transfer_bandwidth_gbps,
            transfer_latency_ns,
            crossing_penalty_ns,
            crossing_penalty_uj,
            cpu_memory_budget_bytes,
            gpu_memory_budget_bytes,
            photonic_memory_budget_bytes,
        } => {
            let controls = CompilerControls {
                optimize_for: &optimize_for,
                target: &target,
                autotune_seed,
                batch_size,
                alternative_plans,
                fuse_boundaries,
                queue_depth,
                overlap_fraction,
                resident_input_fraction,
                transfer_bandwidth_gbps,
                transfer_latency_ns,
                crossing_penalty_ns,
                crossing_penalty_uj,
                cpu_memory_budget_bytes,
                gpu_memory_budget_bytes,
                photonic_memory_budget_bytes,
            };
            compile_command(
                &input,
                capabilities.as_deref(),
                health.as_deref(),
                cost_model.as_deref(),
                &output,
                controls,
            )?;
        }
        Command::Benchmark {
            input,
            capabilities,
            health,
            cost_model,
            observations,
            output,
            optimize_for,
            target,
            autotune_seed,
            batch_size,
            alternative_plans,
            fuse_boundaries,
            queue_depth,
            overlap_fraction,
            resident_input_fraction,
            transfer_bandwidth_gbps,
            transfer_latency_ns,
            crossing_penalty_ns,
            crossing_penalty_uj,
            cpu_memory_budget_bytes,
            gpu_memory_budget_bytes,
            photonic_memory_budget_bytes,
        } => {
            let controls = CompilerControls {
                optimize_for: &optimize_for,
                target: &target,
                autotune_seed,
                batch_size,
                alternative_plans,
                fuse_boundaries,
                queue_depth,
                overlap_fraction,
                resident_input_fraction,
                transfer_bandwidth_gbps,
                transfer_latency_ns,
                crossing_penalty_ns,
                crossing_penalty_uj,
                cpu_memory_budget_bytes,
                gpu_memory_budget_bytes,
                photonic_memory_budget_bytes,
            };
            benchmark_command(
                &input,
                capabilities.as_deref(),
                health.as_deref(),
                cost_model.as_deref(),
                observations.as_deref(),
                &output,
                controls,
            )?;
        }
        Command::Kernel {
            input,
            output,
            target,
            effective_bits,
            noise_fraction,
            seed,
        } => kernel_command(
            &input,
            &output,
            &target,
            effective_bits,
            noise_fraction,
            seed,
        )?,
        Command::KernelBenchmark {
            input,
            output,
            target,
            effective_bits,
            noise_fraction,
            seed,
            repetitions,
        } => kernel_benchmark_command(
            &input,
            &output,
            &target,
            effective_bits,
            noise_fraction,
            seed,
            repetitions,
        )?,
        Command::BenchmarkSuite {
            manifest,
            output_dir,
            commit_sha,
            runner_id,
        } => benchmark_suite_command(
            &manifest,
            &output_dir,
            commit_sha.as_deref(),
            runner_id.as_deref(),
        )?,
        Command::BenchmarkClaims {
            artifact,
            artifact_url,
            baseline,
            candidate,
            output,
            markdown_output,
        } => benchmark_claims_command(
            &artifact,
            &artifact_url,
            &baseline,
            &candidate,
            &output,
            &markdown_output,
        )?,
        Command::KernelPlan {
            input,
            profiles,
            output,
            optimize_for,
        } => kernel_plan_command(&input, &profiles, &output, &optimize_for)?,
        Command::Execute { artifact } => execute_command(&artifact)?,
        Command::Backends {
            plugin_dir,
            allow_unverified,
        } => backends_command(&plugin_dir, allow_unverified)?,
        Command::Run { ir, seed } => run_command(&ir, seed)?,
        Command::Gradient {
            ir,
            params,
            strategy,
            seed,
            samples,
        } => gradient_command(&ir, &params, &strategy, seed, samples)?,
    }
    Ok(())
}

fn kernel_command(
    input_path: &str,
    output_path: &str,
    target_name: &str,
    effective_bits: u8,
    noise_fraction: f64,
    seed: u64,
) -> Result<()> {
    let request: KernelRequest = serde_json::from_str(&std::fs::read_to_string(input_path)?)?;
    let target = concrete_kernel_target(target_name)?;
    let result = if target == TargetBackend::Cpu {
        execute_kernel_reference(&request)?
    } else {
        execute_kernel_simulator(
            &request,
            KernelSimulatorOptions {
                target,
                effective_bits,
                noise_fraction,
                seed,
            },
        )?
    };
    std::fs::write(output_path, serde_json::to_string_pretty(&result)?)?;
    println!(
        "Executed {:?} kernel '{}' on {:?}. Result: {}",
        result.kind, result.request_id, result.execution_target, output_path
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn kernel_benchmark_command(
    input_path: &str,
    output_path: &str,
    target_name: &str,
    effective_bits: u8,
    noise_fraction: f64,
    seed: u64,
    repetitions: usize,
) -> Result<()> {
    let request: KernelRequest = serde_json::from_str(&std::fs::read_to_string(input_path)?)?;
    let target = concrete_kernel_target(target_name)?;
    if target == TargetBackend::Cpu {
        anyhow::bail!("kernel benchmark simulator target must be gpu or photonic");
    }
    let report = benchmark_kernel(
        &request,
        KernelSimulatorOptions {
            target,
            effective_bits,
            noise_fraction,
            seed,
        },
        repetitions,
    )?;
    std::fs::write(output_path, serde_json::to_string_pretty(&report)?)?;
    println!(
        "Benchmarked {:?} kernel '{}' for {} repetition(s). Report: {}",
        report.kind, report.request_id, report.repetitions, output_path
    );
    if !report.within_contract {
        anyhow::bail!("kernel simulator output exceeded the request numerical contract");
    }
    Ok(())
}

fn benchmark_suite_command(
    manifest_path: &str,
    output_dir: &str,
    commit_sha: Option<&str>,
    runner_id: Option<&str>,
) -> Result<()> {
    let suite: HilBenchmarkSuite = serde_json::from_slice(&std::fs::read(manifest_path)?)?;
    let context = BenchmarkRunContext {
        commit_sha: commit_sha
            .map(str::to_string)
            .map(Ok)
            .unwrap_or_else(detect_commit_sha)?,
        runner_id: runner_id
            .map(str::to_string)
            .unwrap_or_else(detect_runner_id),
    };
    let artifact = run_benchmark_suite(&suite, &context)?;
    let paths = write_benchmark_artifact_set(std::path::Path::new(output_dir), &suite, &artifact)?;
    println!(
        "Benchmark suite '{}' produced {} backend result(s), {} backend failure(s), and {} artifact file(s) in {}. Verification: {:?}",
        artifact.suite_id,
        artifact.results.len(),
        artifact.backend_failures.len(),
        paths.len(),
        output_dir,
        artifact.verification.status
    );
    if artifact.verification.status != VerificationStatus::Verified {
        anyhow::bail!(
            "benchmark artifact was preserved but verification rejected it: {}",
            artifact.verification.failures.join("; ")
        );
    }
    Ok(())
}

fn benchmark_claims_command(
    artifact_path: &str,
    artifact_url: &str,
    baseline: &str,
    candidate: &str,
    output_path: &str,
    markdown_output_path: &str,
) -> Result<()> {
    let artifact: BenchmarkArtifact = serde_json::from_slice(&std::fs::read(artifact_path)?)?;
    let claims = generate_public_claims(&artifact, artifact_url, baseline, candidate)?;
    std::fs::write(output_path, serde_json::to_vec_pretty(&claims)?)?;
    std::fs::write(markdown_output_path, claims_markdown(&claims))?;
    println!(
        "Generated {} verified public claim(s) from immutable artifact {}. JSON: {}; Markdown: {}",
        claims.claims.len(),
        claims.artifact_fingerprint,
        output_path,
        markdown_output_path
    );
    Ok(())
}

fn detect_commit_sha() -> Result<String> {
    for key in ["AWEN_COMMIT_SHA", "GITHUB_SHA"] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                return Ok(value);
            }
        }
    }
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "could not detect commit SHA; pass --commit-sha explicitly: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let value = String::from_utf8(output.stdout)?.trim().to_string();
    if value.is_empty() {
        anyhow::bail!("detected an empty commit SHA; pass --commit-sha explicitly");
    }
    Ok(value)
}

fn detect_runner_id() -> String {
    for key in ["AWEN_RUNNER_ID", "RUNNER_NAME", "HOSTNAME", "COMPUTERNAME"] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                return value;
            }
        }
    }
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn kernel_plan_command(
    input_path: &str,
    profiles_path: &str,
    output_path: &str,
    objective_name: &str,
) -> Result<()> {
    let request: KernelRequest = serde_json::from_str(&std::fs::read_to_string(input_path)?)?;
    let profiles: Vec<KernelBackendProfile> =
        serde_json::from_str(&std::fs::read_to_string(profiles_path)?)?;
    let objective = OptimizationObjective::parse(objective_name).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown optimization objective '{objective_name}'; use latency, energy, accuracy, or throughput"
        )
    })?;
    let plan = select_kernel(&request, &profiles, objective)?;
    std::fs::write(output_path, serde_json::to_string_pretty(&plan)?)?;
    println!(
        "Selected '{}' on {:?} for {:?}. Plan: {}",
        plan.selected_backend_id, plan.selected_target, request.kind, output_path
    );
    Ok(())
}

fn concrete_kernel_target(value: &str) -> Result<TargetBackend> {
    let target = TargetBackend::parse(value).ok_or_else(|| {
        anyhow::anyhow!("unknown kernel target '{value}'; use cpu, gpu, or photonic")
    })?;
    if target == TargetBackend::Auto {
        anyhow::bail!("kernel execution requires a concrete cpu, gpu, or photonic target");
    }
    Ok(target)
}

fn execute_command(artifact_path: &str) -> Result<()> {
    let bytes = std::fs::read(artifact_path)?;
    let executable = awen_runtime::executable::prepare_executable(&bytes)?;
    println!(
        "Prepared AWEN executable ABI {}.{} for {}: {} dispatch(es), {} MLIR bytecode bytes",
        executable.abi_major,
        executable.abi_minor,
        executable.backend_id,
        executable.dispatches.len(),
        executable.mlir_bytecode_bytes
    );
    for (index, dispatch) in executable.dispatches.iter().enumerate() {
        println!(
            "  dispatch[{index}] GEMM tile={}x{}x{}, effective_bits={}, calibration={}, layout={}, result_shape={:?}",
            dispatch.tile[0],
            dispatch.tile[1],
            dispatch.tile[2],
            dispatch.minimum_effective_bits,
            dispatch.calibration,
            dispatch.layout,
            dispatch.result_shape
        );
    }
    Ok(())
}

fn compile_command(
    input_path: &str,
    capabilities_path: Option<&str>,
    health_path: Option<&str>,
    cost_model_path: Option<&str>,
    output_path: &str,
    controls: CompilerControls<'_>,
) -> Result<()> {
    let (program, snapshot, options) =
        compiler_inputs(input_path, capabilities_path, health_path, controls)?;
    let artifact = compile_with_optional_cost_model(&program, &snapshot, options, cost_model_path)?;
    std::fs::write(output_path, serde_json::to_string_pretty(&artifact)?)?;
    println!(
        "Compiled {} operation(s) for {}. Artifact: {}",
        artifact.placement.len(),
        artifact.backend_id,
        output_path
    );
    for diagnostic in &artifact.diagnostics {
        println!("  {diagnostic}");
    }
    Ok(())
}

fn benchmark_command(
    input_path: &str,
    capabilities_path: Option<&str>,
    health_path: Option<&str>,
    cost_model_path: Option<&str>,
    observations_path: Option<&str>,
    output_path: &str,
    controls: CompilerControls<'_>,
) -> Result<()> {
    let (program, snapshot, options) =
        compiler_inputs(input_path, capabilities_path, health_path, controls)?;
    let artifact = compile_with_optional_cost_model(&program, &snapshot, options, cost_model_path)?;
    let report = match observations_path {
        Some(path) => {
            let observations: ObservationSet =
                serde_json::from_str(&std::fs::read_to_string(path)?)?;
            observations.validate()?;
            benchmark_with_observations(&program, &artifact, &observations.observations)?
        }
        None => benchmark(&program, &artifact)?,
    };
    std::fs::write(output_path, serde_json::to_string_pretty(&report)?)?;
    println!(
        "Benchmark complete: {} output(s), tolerance_passed={}, report={}",
        report.outputs.len(),
        report.all_outputs_within_tolerance,
        output_path
    );
    if !report.all_outputs_within_tolerance {
        anyhow::bail!("one or more benchmark outputs exceeded their accuracy contract");
    }
    Ok(())
}

fn compile_with_optional_cost_model(
    program: &TensorProgram,
    snapshot: &BackendSnapshot,
    options: CompileOptions,
    cost_model_path: Option<&str>,
) -> Result<awen_compiler::CompilationArtifact> {
    match cost_model_path {
        Some(path) => {
            let model: CostModelInputs = serde_json::from_str(&std::fs::read_to_string(path)?)?;
            compile_with_cost_model(program, snapshot, &model, options)
        }
        None => compile_with_backend(program, snapshot, options),
    }
}

fn compiler_inputs(
    input_path: &str,
    capabilities_path: Option<&str>,
    health_path: Option<&str>,
    controls: CompilerControls<'_>,
) -> Result<(TensorProgram, BackendSnapshot, CompileOptions)> {
    let program: TensorProgram = serde_json::from_str(&std::fs::read_to_string(input_path)?)?;
    let capabilities = match capabilities_path {
        Some(path) => serde_json::from_str(&std::fs::read_to_string(path)?)?,
        None => DeviceCapabilities::default(),
    };
    let snapshot = match health_path {
        Some(path) => {
            let health: BackendHealth = serde_json::from_str(&std::fs::read_to_string(path)?)?;
            BackendSnapshot::new(capabilities, health)?
        }
        None => BackendSnapshot::offline(capabilities)?,
    };
    let optimize_for = OptimizationObjective::parse(controls.optimize_for).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown optimization objective '{}'; use latency, energy, accuracy, or throughput",
            controls.optimize_for
        )
    })?;
    let target = TargetBackend::parse(controls.target).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown target '{}'; use auto, cpu, gpu, or photonic",
            controls.target
        )
    })?;
    Ok((
        program,
        snapshot,
        CompileOptions {
            optimize_for,
            target,
            autotune_seed: controls.autotune_seed,
            batch_size: controls.batch_size,
            alternative_plans: controls.alternative_plans,
            allow_boundary_fusion: controls.fuse_boundaries,
            queue_depth: controls.queue_depth,
            overlap_fraction: controls.overlap_fraction,
            resident_input_fraction: controls.resident_input_fraction,
            transfer_bandwidth_gbps: controls.transfer_bandwidth_gbps,
            transfer_latency_ns: controls.transfer_latency_ns,
            crossing_penalty_ns: controls.crossing_penalty_ns,
            crossing_penalty_uj: controls.crossing_penalty_uj,
            cpu_memory_budget_bytes: controls.cpu_memory_budget_bytes,
            gpu_memory_budget_bytes: controls.gpu_memory_budget_bytes,
            photonic_memory_budget_bytes: controls.photonic_memory_budget_bytes,
            ..CompileOptions::default()
        },
    ))
}

fn backends_command(plugin_dir: &str, allow_unverified: bool) -> Result<()> {
    let registry = if allow_unverified {
        awen_runtime::plugins::PluginRegistry::discover_from_dir_allow_unverified(plugin_dir, true)?
    } else {
        awen_runtime::plugins::PluginRegistry::discover_from_dir(plugin_dir)?
    };
    let report = registry.query_backend_snapshots(plugin_dir);
    for backend in report.backends {
        println!(
            "{} plugin={} status={:?} channels={}/{} capability={} runtime_abi={} plugin_abi={}",
            backend.snapshot.capabilities.backend_id,
            backend.plugin_id,
            backend.snapshot.health.status,
            backend.snapshot.health.available_channels,
            backend.snapshot.capabilities.simultaneous_channels,
            backend.snapshot.capabilities.capability_version,
            backend.snapshot.capabilities.runtime_abi_version,
            backend.snapshot.capabilities.plugin_abi_version,
        );
    }
    for diagnostic in report.diagnostics {
        eprintln!("{}: {}", diagnostic.plugin_id, diagnostic.message);
    }
    Ok(())
}

fn run_command(ir_path: &str, seed: Option<u64>) -> Result<()> {
    println!("awenctl: running IR {} (seed={:?})", ir_path, seed);
    let graph = ir::load_from_json(ir_path).map_err(|e| anyhow::anyhow!(e))?;
    let engine = Engine::new();
    let out_dir = engine.run_graph(&graph, seed)?;
    println!("Run complete. Artifacts written to: {}", out_dir.display());
    Ok(())
}

fn gradient_command(
    ir_path: &str,
    params_csv: &str,
    strategy: &str,
    seed: Option<u64>,
    samples: u32,
) -> Result<()> {
    println!(
        "awenctl: computing gradients for {} (strategy={}, seed={:?})",
        ir_path, strategy, seed
    );
    let ir_json = std::fs::read_to_string(ir_path)?;

    // Register defaults into the global registry and pick the reference provider
    gradients::register_defaults_to_global();
    // Provider selection logic:
    // - if strategy == "adjoint" -> prefer adjoint provider
    // - if strategy == "finite_difference" -> use fd
    // - if strategy == "auto" -> prefer adjoint if supported, else fd
    let provider: std::sync::Arc<dyn gradients::GradientProvider> = match strategy {
        s if s.eq_ignore_ascii_case("adjoint") => gradients::GLOBAL_GRADIENT_REGISTRY
            .get("reference-adjoint")
            .ok_or_else(|| anyhow::anyhow!("adjoint provider not available"))?,
        s if s.eq_ignore_ascii_case("finite_difference")
            || s.eq_ignore_ascii_case("finite-difference")
            || s.eq_ignore_ascii_case("fd") =>
        {
            gradients::GLOBAL_GRADIENT_REGISTRY
                .get("reference-fd")
                .ok_or_else(|| anyhow::anyhow!("fd provider not available"))?
        }
        _ => {
            // auto
            if let Some(adj) = gradients::GLOBAL_GRADIENT_REGISTRY.get("reference-adjoint") {
                adj
            } else if let Some(fd) = gradients::GLOBAL_GRADIENT_REGISTRY.get("reference-fd") {
                fd
            } else {
                return Err(anyhow::anyhow!("no gradient providers registered"));
            }
        }
    };

    let params: Vec<String> = params_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let noise = NoiseModel {
        shot_noise_std: None,
        thermal_noise_std: None,
        phase_noise_std: None,
        loss_variation: None,
        metadata: None,
    };
    let opts = GradientOptions {
        strategy: strategy.to_string(),
        seed,
        samples: Some(samples),
    };

    let res = provider.compute_gradients(&ir_json, &params, &noise, &opts)?;

    // write artifact
    let run_id = Uuid::new_v4().to_string();
    let out_dir: PathBuf = std::env::current_dir()?.join(format!("awen_grad_{}", run_id));
    std::fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("gradients.json");
    std::fs::write(&out_path, serde_json::to_string_pretty(&res)?)?;

    // write observability artifacts for gradient run
    let node_ids = vec!["gradient_op".to_string()];
    let (spans, events, metrics) =
        awen_runtime::observability::build_basic_observability(&run_id, &node_ids, opts.seed);
    awen_runtime::observability::write_traces(&out_dir, &spans)?;
    awen_runtime::observability::write_timeline(&out_dir, &events)?;
    awen_runtime::observability::write_metrics(&out_dir, &metrics)?;

    println!("Gradients written to: {}", out_path.display());
    Ok(())
}
