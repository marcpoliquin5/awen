use anyhow::Result;
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
