//! Fixed-config noise-model and cost-estimate outputs, by default and with the hardware assumption
//! set opted in at each corner. See awen-compiler/results/assumptions-wiring/README.md.
use awen_compiler::assumptions::{Corner, HardwareAssumptions};
use awen_compiler::capability::{AccumulationMode, BitSlicingMode, DeviceCapabilities};
use awen_compiler::cost::{estimate_photonic_plan, CostModelInputs, ParameterSource, TuningPlan};
use awen_compiler::ir::{DType, GemmShape};
use awen_compiler::noise::{photonic_gemm, PhotonicNoiseContract};
use awen_compiler::precision::AccumulatorDType;
use serde_json::{json, Value};

fn noise_output(contract: &PhotonicNoiseContract) -> Value {
    let shape = GemmShape { m: 4, k: 64, n: 8 };
    let x: Vec<f64> = (0..shape.m * shape.k)
        .map(|i| (i as f64 * 0.37).sin())
        .collect();
    let w: Vec<f64> = (0..shape.n * shape.k)
        .map(|i| (i as f64 * 0.11).cos())
        .collect();
    let (y, report) = photonic_gemm(&x, &w, shape, contract, 16, 7).expect("gemm");
    json!({"contract": contract, "y": y, "max_abs_error_vs_exact": report.max_abs_error_vs_exact,
           "rms_error_vs_exact": report.rms_error_vs_exact, "tile_budget": report.tile_budget})
}

fn cost_output(model: &CostModelInputs) -> Value {
    let plan = TuningPlan {
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
    };
    let shape = GemmShape {
        m: 256,
        n: 256,
        k: 256,
    };
    let e = estimate_photonic_plan(
        shape,
        DType::F16,
        &DeviceCapabilities::pace_like_128(),
        model,
        plan,
    )
    .expect("estimate");
    json!({"latency_ns": e.latency_ns, "latency_interval_ns": e.latency_interval_ns, "energy_uj": e.energy_uj,
           "energy_interval_uj": e.energy_interval_uj, "throughput_gops": e.throughput_gops,
           "effective_bits": e.effective_bits, "estimated_error_fraction": e.estimated_error_fraction,
           "energy_breakdown_uj": e.energy_breakdown_uj, "provenance": e.provenance})
}

fn main() {
    let contract = PhotonicNoiseContract::default();
    let model = CostModelInputs::from_capabilities(
        &DeviceCapabilities::pace_like_128(),
        ParameterSource::Assumed,
    );
    let mut out = json!({
        "config": {"gemm": "X[4,64] = sin(0.37 i), W[8,64] = cos(0.11 i)", "tile_k": 16, "seed": 7,
                   "cost": "pace_like_128, CostModelInputs::from_capabilities(Assumed), GEMM 256x256x256 F16, tile 128, 16 wavelengths"},
        "default": {"noise": noise_output(&contract), "cost": cost_output(&model)},
    });
    // BEGIN opt-in (removed for the main-branch "before" run)
    let assumptions = HardwareAssumptions::bundled().expect("bundled assumptions");
    for corner in Corner::ALL {
        let (c, noise_report) = contract
            .with_assumptions(&assumptions, corner)
            .expect("noise");
        let (m, cost_report) = model
            .clone()
            .with_assumptions(&assumptions, corner)
            .expect("cost");
        out["opt_in"][format!("{corner:?}").to_lowercase()] = json!({"noise": noise_output(&c),
            "noise_report": noise_report, "cost": cost_output(&m), "cost_report": cost_report});
    }
    // END opt-in
    println!("{}", serde_json::to_string_pretty(&out).expect("json"));
}
