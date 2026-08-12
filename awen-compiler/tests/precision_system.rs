use awen_compiler::lowering::DeviceCommand;
use awen_compiler::{
    autotune_with_profile, benchmark, compile, default_quantization, AccumulatorDType,
    AutotuneOptions, BitSlicingMode, CompileOptions, CostModelInputs, DType, DeviceCapabilities,
    DynamicRange, OperationCostProfile, OptimizationObjective, OverflowMode, PrecisionEncoding,
    QuantizationSpec, RoundingMode, ScaleGranularity, TargetBackend, TensorProgram,
};
use serde_json::json;

fn capabilities() -> DeviceCapabilities {
    serde_json::from_str(include_str!("../capabilities/reference_2x2.json"))
        .expect("reference capability")
}

fn mixed_program(
    max_abs_error: f64,
    minimum_effective_bits: u8,
    compute_dtype: &str,
    accumulator_dtype: &str,
    minimum_accumulator_bits: u8,
    allowed_bit_slicing_modes: &[&str],
    seed: u64,
) -> TensorProgram {
    serde_json::from_value(json!({
        "ir_version": "awen.tensor.v1",
        "tensors": [
            {
                "id": "lhs", "shape": [2, 2], "dtype": "f32", "layout": "row_major",
                "data": [0.25, -0.5, 0.75, 1.0]
            },
            {
                "id": "rhs", "shape": [2, 2], "dtype": "bf16", "layout": "row_major",
                "data": [1.0, 0.5, -0.25, 0.75]
            },
            {"id": "out", "shape": [2, 2], "dtype": "f32", "layout": "row_major"}
        ],
        "ops": [{
            "op": "gemm", "id": "mixed", "lhs": "lhs", "rhs": "rhs", "output": "out",
            "accuracy": {
                "max_abs_error": max_abs_error,
                "max_rel_error": max_abs_error,
                "minimum_effective_bits": minimum_effective_bits
            }
        }],
        "precision": {
            "version": "awen.precision.v1",
            "tensors": [],
            "operations": [{
                "op_id": "mixed",
                "compute_dtype": compute_dtype,
                "output_dtype": "f32",
                "accumulator_dtype": accumulator_dtype,
                "minimum_accumulator_bits": minimum_accumulator_bits,
                "allowed_bit_slicing_modes": allowed_bit_slicing_modes,
                "stochastic_seed": seed
            }]
        }
    }))
    .expect("mixed precision program")
}

#[test]
fn every_precision_encoding_and_declared_dtype_is_validatable() {
    let range = DynamicRange {
        minimum: -1.0,
        maximum: 1.0,
    };
    for dtype in [
        DType::F32,
        DType::F16,
        DType::Bf16,
        DType::Int8,
        DType::Int4,
        DType::ComplexF32,
    ] {
        let spec = default_quantization(dtype, dtype.bits(), range, OverflowMode::Saturate)
            .expect("default precision");
        spec.validate(&[2, 2]).expect("valid default precision");
    }

    let base = QuantizationSpec {
        encoding: PrecisionEncoding::OpticalEffectiveBits,
        bits: 8,
        signed: true,
        granularity: ScaleGranularity::PerTensor,
        axis: None,
        block_size: None,
        scales: vec![1.0 / 127.0],
        zero_points: vec![0],
        clipping_min: -1.0,
        clipping_max: 1.0,
        rounding: RoundingMode::NearestEven,
        overflow: OverflowMode::Saturate,
        backend_encoding: None,
    };
    base.validate(&[4]).expect("optical effective bits");

    let block = QuantizationSpec {
        encoding: PrecisionEncoding::BlockFloatingPoint,
        bits: 8,
        granularity: ScaleGranularity::PerBlock,
        block_size: Some(2),
        scales: vec![0.5, 1.0],
        zero_points: vec![0, 0],
        ..base.clone()
    };
    block.validate(&[4]).expect("block floating point");

    let native = QuantizationSpec {
        encoding: PrecisionEncoding::BackendNative,
        backend_encoding: Some("reference.fixed-point.q1.7".to_string()),
        ..base
    };
    native.validate(&[4]).expect("backend-native encoding");
}

#[test]
fn mixed_precision_is_explicit_in_photonic_and_device_ir() {
    let program = mixed_program(0.2, 8, "f16", "f32", 32, &["none"], 0x5eed);
    let artifact = compile(
        &program,
        &capabilities(),
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("explicit mixed precision compilation");

    let plan = artifact.placement[0].selected_plan.expect("selected plan");
    assert_eq!(plan.compute_dtype, DType::F16);
    assert_eq!(plan.accumulator_dtype, AccumulatorDType::F32);
    assert_eq!(plan.noise_seed, 0x5eed);
    assert!(artifact.photonic_ir.ops.iter().all(|operation| {
        operation.precision.source_dtype == DType::F32
            && operation.precision.rhs_source_dtype == DType::Bf16
            && operation.precision.compute_dtype == DType::F16
            && operation.precision.output_dtype == DType::F32
            && operation.precision.accumulator_dtype == AccumulatorDType::F32
            && operation.precision.calibration_compensation.is_some()
    }));
    assert!(artifact.device_ir.commands.iter().any(|command| matches!(
        command,
        DeviceCommand::ConvertTensor {
            tensor,
            source_dtype: DType::F32,
            target_dtype: DType::F16,
            stochastic_seed: 0x5eed,
            ..
        } if tensor == "lhs"
    )));
    assert!(artifact.device_ir.commands.iter().any(|command| matches!(
        command,
        DeviceCommand::Accumulate {
            accumulator_dtype: AccumulatorDType::F32,
            minimum_accumulator_bits: 32,
            overflow: OverflowMode::Saturate,
            ..
        }
    )));
    assert!(artifact
        .device_ir
        .commands
        .iter()
        .any(|command| matches!(command, DeviceCommand::Rescale { .. })));
}

#[test]
fn bit_slicing_passes_and_signed_mode_are_executable_ir() {
    let program = mixed_program(0.2, 12, "f16", "f32", 32, &["twos_complement"], 9);
    let artifact = compile(
        &program,
        &capabilities(),
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("bit-sliced mixed precision compilation");
    let plan = artifact.placement[0].selected_plan.expect("selected plan");
    assert_eq!(plan.bit_slices, 2);
    assert_eq!(plan.bit_slicing_mode, BitSlicingMode::TwosComplement);
    let slices = artifact
        .device_ir
        .commands
        .iter()
        .filter(|command| matches!(command, DeviceCommand::BitSlice { .. }))
        .count();
    assert_eq!(slices, 2);
    assert!(artifact.device_ir.commands.iter().all(|command| {
        !matches!(
            command,
            DeviceCommand::BitSlice {
                passes: 2,
                mode: BitSlicingMode::TwosComplement,
                ..
            }
        ) || matches!(
            command,
            DeviceCommand::BitSlice {
                total_bits: 16,
                slice_bits: 8,
                overflow: OverflowMode::Saturate,
                ..
            }
        )
    }));
}

#[test]
fn impossible_contract_rejects_forced_photonic_and_falls_back_in_auto() {
    let program = mixed_program(1.0e-12, 4, "int4", "i32", 32, &["none"], 1);
    let error = compile(
        &program,
        &capabilities(),
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect_err("forced backend must reject an impossible precision contract");
    assert!(
        error.to_string().contains("photonic"),
        "unexpected rejection: {error:#}"
    );

    let artifact = compile(&program, &capabilities(), CompileOptions::default())
        .expect("automatic placement must fall back");
    assert_ne!(
        artifact.placement[0].selected_backend,
        TargetBackend::Photonic
    );
    assert!(
        artifact.placement[0].rationale.contains("photonic"),
        "unexpected fallback rationale: {}",
        artifact.placement[0].rationale
    );
}

#[test]
fn error_reports_are_componentized_provenanced_and_seed_deterministic() {
    let program = mixed_program(0.2, 8, "f16", "f32", 32, &["none"], 77);
    let artifact = compile(
        &program,
        &capabilities(),
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("precision compilation");
    let first = benchmark(&program, &artifact).expect("first benchmark");
    let second = benchmark(&program, &artifact).expect("second benchmark");
    assert_eq!(first, second);
    let report = &first.outputs[0].error_report;
    assert_eq!(report.seed, 77);
    assert!(report.static_fraction.quantization > 0.0);
    assert!(report.static_fraction.calibration_residual > 0.0);
    assert!(report.observed_absolute.quantization > 0.0);
    assert!(report.observed_absolute.shot_noise > 0.0);
    assert!(report.observed_absolute.thermal_noise > 0.0);
    assert!(report.observed_absolute.phase_noise > 0.0);
    assert!(report.observed_absolute.detector_noise > 0.0);
    assert!(report
        .provenance
        .iter()
        .any(|item| item.contains("seed 77")));
    assert!(report
        .provenance
        .iter()
        .any(|item| item.contains("calibration")));
}

#[test]
fn empirical_report_separates_clipping_from_quantization() {
    let mut value = serde_json::to_value(mixed_program(10.0, 8, "f16", "f32", 32, &["none"], 31))
        .expect("program value");
    value["precision"]["tensors"] = json!([{
        "tensor_id": "lhs",
        "storage_dtype": "f32",
        "quantization": {
            "encoding": "affine_integer",
            "bits": 8,
            "signed": true,
            "granularity": "per_tensor",
            "scales": [0.002362204724409449],
            "zero_points": [0],
            "clipping_min": -0.3,
            "clipping_max": 0.3,
            "rounding": "nearest_even",
            "overflow": "saturate"
        }
    }]);
    let program: TensorProgram = serde_json::from_value(value).expect("clipping program");
    let artifact = compile(
        &program,
        &capabilities(),
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("clipping compilation");
    let report = benchmark(&program, &artifact).expect("clipping benchmark");
    let observed = report.outputs[0].error_report.observed_absolute;
    assert!(observed.clipping > 0.0);
    assert!(observed.quantization > 0.0);
    assert_ne!(observed.clipping, observed.quantization);
}

#[test]
fn error_contract_changes_the_autotuned_compute_precision() {
    let capabilities = capabilities();
    let model = CostModelInputs::from_capabilities(
        &capabilities,
        awen_compiler::ParameterSource::Simulated,
    );
    let options = AutotuneOptions {
        seed: 11,
        ..AutotuneOptions::default()
    };
    let loose = autotune_with_profile(
        awen_compiler::GemmShape { m: 4, n: 4, k: 4 },
        DType::F32,
        Some(8),
        &capabilities,
        &model,
        OperationCostProfile::default(),
        OptimizationObjective::Latency,
        options,
    )
    .expect("loose contract tune");
    let strict = autotune_with_profile(
        awen_compiler::GemmShape { m: 4, n: 4, k: 4 },
        DType::F32,
        Some(8),
        &capabilities,
        &model,
        OperationCostProfile {
            estimated_output_magnitude: Some(10.0),
            maximum_absolute_error: Some(0.005),
            maximum_relative_error: Some(0.005),
            ..OperationCostProfile::default()
        },
        OptimizationObjective::Latency,
        options,
    )
    .expect("strict contract tune");
    assert_eq!(loose.selected.plan.compute_dtype, DType::Int8);
    assert_eq!(strict.selected.plan.compute_dtype, DType::F16);
    assert_eq!(loose.selected.plan.accumulator_dtype, AccumulatorDType::I32);
    assert_eq!(
        strict.selected.plan.accumulator_dtype,
        AccumulatorDType::F32
    );
}
