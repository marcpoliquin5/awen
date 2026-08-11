use anyhow::Result;
use awen_compiler::{
    benchmark, compile_with_backend, BackendHealth, BackendSnapshot, CompileOptions,
    DeviceCapabilities, OptimizationObjective, TargetBackend, TensorProgram,
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
        /// Compilation artifact output path.
        #[clap(long, default_value = "awen_compilation.json")]
        output: String,
        /// Optimization objective: latency, energy, accuracy, or throughput.
        #[clap(long, default_value = "latency")]
        optimize_for: String,
        /// Target selection: auto, cpu, or photonic.
        #[clap(long, default_value = "auto")]
        target: String,
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
        /// Benchmark report output path.
        #[clap(long, default_value = "awen_benchmark.json")]
        output: String,
        /// Optimization objective: latency, energy, accuracy, or throughput.
        #[clap(long, default_value = "latency")]
        optimize_for: String,
        /// Target selection: auto, cpu, or photonic.
        #[clap(long, default_value = "auto")]
        target: String,
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

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Compile {
            input,
            capabilities,
            health,
            output,
            optimize_for,
            target,
        } => compile_command(
            &input,
            capabilities.as_deref(),
            health.as_deref(),
            &output,
            &optimize_for,
            &target,
        )?,
        Command::Benchmark {
            input,
            capabilities,
            health,
            output,
            optimize_for,
            target,
        } => benchmark_command(
            &input,
            capabilities.as_deref(),
            health.as_deref(),
            &output,
            &optimize_for,
            &target,
        )?,
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
    output_path: &str,
    optimize_for: &str,
    target: &str,
) -> Result<()> {
    let (program, snapshot, options) = compiler_inputs(
        input_path,
        capabilities_path,
        health_path,
        optimize_for,
        target,
    )?;
    let artifact = compile_with_backend(&program, &snapshot, options)?;
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
    output_path: &str,
    optimize_for: &str,
    target: &str,
) -> Result<()> {
    let (program, snapshot, options) = compiler_inputs(
        input_path,
        capabilities_path,
        health_path,
        optimize_for,
        target,
    )?;
    let artifact = compile_with_backend(&program, &snapshot, options)?;
    let report = benchmark(&program, &artifact)?;
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

fn compiler_inputs(
    input_path: &str,
    capabilities_path: Option<&str>,
    health_path: Option<&str>,
    optimize_for: &str,
    target: &str,
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
    let optimize_for = OptimizationObjective::parse(optimize_for).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown optimization objective '{optimize_for}'; use latency, energy, accuracy, or throughput"
        )
    })?;
    let target = TargetBackend::parse(target)
        .ok_or_else(|| anyhow::anyhow!("unknown target '{target}'; use auto, cpu, or photonic"))?;
    Ok((
        program,
        snapshot,
        CompileOptions {
            optimize_for,
            target,
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
