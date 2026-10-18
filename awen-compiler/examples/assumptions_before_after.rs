//! Fixed-config noise-model and cost-estimate outputs, with and without the opt-in
//! hardware assumption set. Run:
//!   cargo run -q --example assumptions_before_after > results/assumptions-wiring/after.json
use awen_compiler::assumptions::{Corner, HardwareAssumptions};
use awen_compiler::capability::{AccumulationMode, BitSlicingMode, DeviceCapabilities};
use awen_compiler::cost::{
    estimate_photonic_plan, CostEstimate, CostModelInputs, ParameterSource, TuningPlan,
};
use awen_compiler::ir::{DType, GemmShape};
use awen_compiler::noise::{photonic_gemm, PhotonicNoiseContract};
use awen_compiler::precision::AccumulatorDType;
use serde_json::{json, Value};

const SEED: u64 = 7;
const TILE_K: usize = 16;

fn operands() -> (Vec<f64>, Vec<f64>, GemmShape) {
    let shape = GemmShape { m: 4, k: 64, n: 8 };
    let x = (0..shape.m * shape.k)
        .map(|i| ((i as f64) * 0.37).sin())
        .collect();
    let w = (0..shape.n * shape.k)
        .map(|i| ((i as f64) * 0.11).cos())
        .collect();
    (x, w, shape)
}

fn noise_output(contract: &PhotonicNoiseContract) -> Value {
    let (x, w, shape) = operands();
    let (y, report) = photonic_gemm(&x, &w, shape, contract, TILE_K, SEED).expect("gemm");
    json!({
        "contract": contract,
        "y": y,
        "max_abs_error_vs_exact": report.max_abs_error_vs_exact,
        "rms_error_vs_exact": report.rms_error_vs_exact,
        "tile_budget": report.tile_budget,
    })
}

fn plan() -> TuningPlan {
    TuningPlan {
        tile_m: 128,
        tile_n: 128,
        tile_k: 128,
        bit_slices: 1,
        bit_slicing_mode: BitSlicingMode::None,
        compute_dtype: DType::F16,
        accumulator_dtype: AccumulatorDType::F32,
        noise_seed: 0,
        wavelength_channels: 16,
        accumulation_mode: AccumulationMode::Digital,
        batch_size: 1,
        fuse_boundaries: false,
    }
}

fn cost_output(model: &CostModelInputs) -> Value {
    let capabilities = DeviceCapabilities::pace_like_128();
    let shape = GemmShape {
        m: 256,
        n: 256,
        k: 256,
    };
    let estimate: CostEstimate =
        estimate_photonic_plan(shape, DType::F16, &capabilities, model, plan()).expect("estimate");
    json!({
        "latency_ns": estimate.latency_ns,
        "latency_interval_ns": estimate.latency_interval_ns,
        "energy_uj": estimate.energy_uj,
        "energy_interval_uj": estimate.energy_interval_uj,
        "throughput_gops": estimate.throughput_gops,
        "effective_bits": estimate.effective_bits,
        "estimated_error_fraction": estimate.estimated_error_fraction,
        "energy_breakdown_uj": estimate.energy_breakdown_uj,
        "provenance": estimate.provenance,
    })
}

fn main() {
    let contract = PhotonicNoiseContract::default();
    let model = CostModelInputs::from_capabilities(
        &DeviceCapabilities::pace_like_128(),
        ParameterSource::Assumed,
    );
    let mut out = json!({
        "config": {"gemm": "X[4,64] = sin(0.37 i), W[8,64] = cos(0.11 i)", "tile_k": TILE_K, "seed": SEED,
                   "cost": "pace_like_128, CostModelInputs::from_capabilities(Assumed), GEMM 256x256x256 F16, tile 128, 16 wavelengths"},
        "default": {"noise": noise_output(&contract), "cost": cost_output(&model)},
    });
    // BEGIN opt-in (removed for the main-branch "before" run; see awen-compiler/results/assumptions-wiring/README.md)
    let assumptions = HardwareAssumptions::bundled().expect("bundled assumptions");
    let mut opt_in = serde_json::Map::new();
    for corner in Corner::ALL {
        let (c, noise_report) = contract
            .with_assumptions(&assumptions, corner)
            .expect("noise");
        let (m, cost_report) = model
            .clone()
            .with_assumptions(&assumptions, corner)
            .expect("cost");
        opt_in.insert(
            format!("{corner:?}").to_lowercase(),
            json!({
                "noise": noise_output(&c), "noise_report": noise_report,
                "cost": cost_output(&m), "cost_report": cost_report,
            }),
        );
    }
    out["opt_in"] = Value::Object(opt_in);
    // END opt-in
    println!("{}", serde_json::to_string_pretty(&out).expect("json"));
}
