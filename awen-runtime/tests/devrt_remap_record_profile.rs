use awen_compiler::{
    execute_kernel_reference, KernelAttributes, KernelData, KernelKind, KernelRequest,
    KernelTensor, AWENBLAS_VERSION,
};
use awen_runtime::devrt::profile::Profiler;
use awen_runtime::devrt::record::{
    output_digest, DeterministicSeeds, ExperimentRecord, ExperimentRecorder,
};
use awen_runtime::devrt::remap::{
    DevicePool, Health, PlacementPolicy, SimulatedDevice, CPU_DEVICE,
};
use std::collections::BTreeSet;

fn request(id: &str, kind: KernelKind, inputs: Vec<KernelTensor>) -> KernelRequest {
    KernelRequest {
        version: AWENBLAS_VERSION.to_string(),
        id: id.into(),
        kind,
        inputs,
        attributes: KernelAttributes::default(),
        accuracy: awen_compiler::ir::AccuracyContract {
            max_abs_error: 0.5,
            max_rel_error: 0.5,
            minimum_effective_bits: None,
        },
        calibration_inputs: Vec::new(),
    }
}

fn gemm(id: &str, seed: f64) -> KernelRequest {
    let a: Vec<f64> = (0..24).map(|i| (i as f64 * 0.7 + seed).sin()).collect();
    let b: Vec<f64> = (0..24).map(|i| (i as f64 * 0.3 - seed).cos()).collect();
    request(
        id,
        KernelKind::Gemm,
        vec![
            KernelTensor::real("a", vec![4, 6], a),
            KernelTensor::real("b", vec![6, 4], b),
        ],
    )
}

fn conv(id: &str) -> KernelRequest {
    request(
        id,
        KernelKind::Convolution1d,
        vec![
            KernelTensor::real("x", vec![8], (0..8).map(f64::from).collect()),
            KernelTensor::real("w", vec![3], vec![1.0, -1.0, 0.5]),
        ],
    )
}

fn device(id: &str, kinds: &[KernelKind], noise: f64) -> SimulatedDevice {
    SimulatedDevice {
        id: id.into(),
        supported: kinds.iter().copied().collect::<BTreeSet<_>>(),
        effective_bits: 8,
        noise_fraction: noise,
        health: Health::Healthy,
    }
}

fn pool() -> DevicePool {
    let mut pool = DevicePool::new();
    pool.add(device("sim0", &[KernelKind::Gemm], 0.01));
    pool.add(device("sim1", &[KernelKind::Gemm], 0.01));
    pool
}

fn values(result: &awen_compiler::KernelResult) -> Vec<f64> {
    match &result.outputs[0].data {
        KernelData::Real(values) => values.clone(),
        KernelData::Complex(_) => panic!("real"),
    }
}

#[test]
fn health_changes_remap_and_unsupported_kernels_fall_back_to_cpu() {
    let mut pool = pool();
    let policy = PlacementPolicy::default();
    assert_eq!(pool.place(KernelKind::Gemm, policy).unwrap().device, "sim0");

    pool.set_health(
        "sim0",
        Health::Failed {
            reason: "laser interlock".into(),
        },
    )
    .unwrap();
    let placement = pool.place(KernelKind::Gemm, policy).unwrap();
    assert_eq!(placement.device, "sim1");
    assert!(placement.skipped[0].contains("laser interlock"));

    pool.set_health(
        "sim1",
        Health::Degraded {
            reason: "calibration stale".into(),
        },
    )
    .unwrap();
    let fallback = pool.place(KernelKind::Gemm, policy).unwrap();
    assert_eq!(fallback.device, CPU_DEVICE);
    assert!(fallback.fell_back_to_cpu);
    let degraded_ok = pool
        .place(
            KernelKind::Gemm,
            PlacementPolicy {
                allow_degraded: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(degraded_ok.device, "sim1");
    assert!(pool
        .place(
            KernelKind::Gemm,
            PlacementPolicy {
                forbid_cpu_fallback: true,
                ..Default::default()
            }
        )
        .is_err());
    assert!(pool.set_health("nope", Health::Healthy).is_err());

    // CPU fallback is exact; photonic placement is marked simulated.
    let req = conv("conv");
    let (placement, result) = pool.execute(&req, policy, 1).unwrap();
    assert!(placement.fell_back_to_cpu);
    assert!(!result.simulated);
    assert_eq!(
        values(&result),
        values(&execute_kernel_reference(&req).unwrap())
    );

    pool.set_health("sim0", Health::Healthy).unwrap();
    let req = gemm("g", 0.2);
    let (placement, result) = pool.execute(&req, policy, 1).unwrap();
    assert_eq!(placement.device, "sim0");
    assert!(result.simulated);
    let exact = values(&execute_kernel_reference(&req).unwrap());
    let got = values(&result);
    let max_err = got
        .iter()
        .zip(&exact)
        .map(|(g, e)| (g - e).abs())
        .fold(0.0, f64::max);
    assert!(max_err < 0.5, "simulated GEMM error {max_err}");
}

#[test]
fn seed_stream_is_the_documented_splitmix64_sequence() {
    // Reference values of SplitMix64 for seed 0 (Vigna's reference implementation).
    let mut seeds = DeterministicSeeds::new(0);
    assert_eq!(seeds.next_seed(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(seeds.next_seed(), 0x6E78_9E6A_A1B9_65F4);
    assert_eq!(seeds.next_seed(), 0x06C4_5D18_8009_454F);
}

fn run_experiment(pool: &DevicePool, root: u64) -> ExperimentRecord {
    let mut recorder = ExperimentRecorder::new("replay", pool, PlacementPolicy::default(), root);
    for index in 0..6 {
        recorder
            .run(&gemm(&format!("g{index}"), index as f64))
            .unwrap();
    }
    recorder.run(&conv("c")).unwrap();
    recorder.finish()
}

#[test]
fn deterministic_mode_replays_bit_exactly_and_detects_divergence() {
    let pool = pool();
    let first = run_experiment(&pool, 42);
    let replay = run_experiment(&pool, 42);
    assert!(first.verify_replay(&replay).is_empty());
    assert_eq!(first, replay);
    assert_eq!(first.entries.len(), 7);
    assert!(first.entries[0].simulated);
    assert!(!first.entries[6].simulated);
    assert!(first.entries[6].placement.fell_back_to_cpu);

    let reloaded = ExperimentRecord::from_json(&first.to_json().unwrap()).unwrap();
    assert_eq!(reloaded, first);

    let other_seed = run_experiment(&pool, 43);
    let diverged = first.verify_replay(&other_seed);
    // Every entry records a different seed; only the simulated (photonic)
    // outputs actually change, the exact CPU fallback does not.
    assert_eq!(diverged, (0..7).collect::<Vec<_>>());
    for (a, b) in first.entries.iter().zip(&other_seed.entries) {
        assert_eq!(a.output_sha256 == b.output_sha256, !a.simulated);
    }

    let mut truncated = replay.clone();
    truncated.entries.pop();
    assert_eq!(first.verify_replay(&truncated), vec![6]);

    let mut wrong = first.to_json().unwrap();
    wrong = wrong.replace("awen.experiment-record.v1", "awen.experiment-record.v0");
    assert!(ExperimentRecord::from_json(&wrong).is_err());
}

#[test]
fn output_digest_is_sensitive_to_one_ulp() {
    let req = gemm("g", 0.0);
    let mut result = execute_kernel_reference(&req).unwrap();
    let before = output_digest(&result);
    if let KernelData::Real(values) = &mut result.outputs[0].data {
        values[0] = f64::from_bits(values[0].to_bits() + 1);
    }
    assert_ne!(before, output_digest(&result));
}

#[test]
fn profiler_summarises_spans_and_exports_chrome_trace() {
    let mut profiler = Profiler::new();
    let req = gemm("g", 0.5);
    for _ in 0..5 {
        profiler.time("gemm", "cpu", || execute_kernel_reference(&req).unwrap());
    }
    let sum = profiler.time("other", "sim0", || 2 + 2);
    assert_eq!(sum, 4);
    let summary = profiler.summary();
    assert_eq!(summary["gemm"].count, 5);
    assert_eq!(summary["other"].count, 1);
    let total: u128 = profiler
        .spans()
        .iter()
        .filter(|s| s.name == "gemm")
        .map(|s| s.duration_ns)
        .sum();
    assert_eq!(summary["gemm"].total_ns, total);
    assert!(summary["gemm"].max_ns >= summary["gemm"].median_ns);
    let starts: Vec<u128> = profiler.spans().iter().map(|s| s.start_ns).collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]));
    let trace = profiler.chrome_trace();
    let events = trace["traceEvents"].as_array().unwrap();
    assert_eq!(events.len(), 6);
    assert_eq!(events[0]["ph"], "X");
    assert_eq!(events[5]["tid"], "sim0");
}
