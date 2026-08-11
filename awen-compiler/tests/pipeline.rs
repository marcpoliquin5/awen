use awen_compiler::{
    benchmark, compile, compile_with_backend, BackendHealth, BackendSnapshot, BitSlicingMode,
    CompileOptions, DeviceCapabilities, HealthStatus, OptimizationObjective, TargetBackend,
    TensorProgram,
};

fn program(name: &str) -> TensorProgram {
    serde_json::from_str(match name {
        "4x4" => include_str!("../examples/gemm_4x4.json"),
        "256" => include_str!("../examples/gemm_256.json"),
        _ => panic!("unknown fixture"),
    })
    .expect("fixture must parse")
}

fn capabilities(name: &str) -> DeviceCapabilities {
    serde_json::from_str(match name {
        "2x2" => include_str!("../capabilities/reference_2x2.json"),
        "128" => include_str!("../capabilities/pace_like_128.json"),
        _ => panic!("unknown fixture"),
    })
    .expect("capability fixture must parse")
}

fn snapshot(name: &str) -> BackendSnapshot {
    let capabilities = capabilities(name);
    let health: BackendHealth = serde_json::from_str(match name {
        "2x2" => include_str!("../capabilities/reference_2x2.health.json"),
        "128" => include_str!("../capabilities/pace_like_128.health.json"),
        _ => panic!("unknown fixture"),
    })
    .expect("health fixture must parse");
    BackendSnapshot::new(capabilities, health).expect("snapshot must validate")
}

#[test]
fn tiles_256_gemm_into_eight_128_cubed_operations() {
    let artifact = compile(
        &program("256"),
        &capabilities("128"),
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("compilation should succeed");

    assert_eq!(artifact.photonic_ir.ops.len(), 8);
    assert_eq!(artifact.placement[0].tile_count, 8);
    assert_eq!(
        artifact.placement[0].optical_electrical_boundary_crossings,
        2
    );
    assert!(artifact
        .photonic_ir
        .ops
        .iter()
        .all(|op| op.calibration_handle.is_some()));
}

#[test]
fn calibrated_simulator_executes_tiled_gemm_within_contract() {
    let program = program("4x4");
    let artifact = compile(
        &program,
        &capabilities("2x2"),
        CompileOptions {
            optimize_for: OptimizationObjective::Latency,
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("compilation should succeed");
    let report = benchmark(&program, &artifact).expect("benchmark should run");

    assert_eq!(artifact.photonic_ir.ops.len(), 8);
    assert!(report.all_outputs_within_tolerance);
    assert_eq!(report.optical_electrical_boundary_crossings, 2);
    assert_eq!(report.outputs[0].values.len(), 16);
}

#[test]
fn accuracy_contract_rejects_insufficient_photonic_precision() {
    let mut program = program("4x4");
    let awen_compiler::TensorOp::Gemm { accuracy, .. } = &mut program.ops[0];
    accuracy.minimum_effective_bits = Some(12);
    let mut capabilities = capabilities("2x2");
    capabilities.bit_slicing_modes = vec![BitSlicingMode::None];

    let error = compile(
        &program,
        &capabilities,
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect_err("forced photonic placement must reject insufficient precision");
    assert!(error.to_string().contains("effective precision"));
}

#[test]
fn autotuned_bit_slicing_is_emitted_into_photonic_ir() {
    let mut program = program("4x4");
    let awen_compiler::TensorOp::Gemm { accuracy, .. } = &mut program.ops[0];
    accuracy.minimum_effective_bits = Some(12);

    let artifact = compile(
        &program,
        &capabilities("2x2"),
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("bit slicing should satisfy the precision contract");

    let plan = artifact.placement[0].selected_plan.expect("tuning plan");
    assert_eq!(plan.bit_slices, 2);
    assert!(artifact
        .photonic_ir
        .ops
        .iter()
        .all(|op| op.precision.bit_slices == 2 && op.precision.optical_effective_bits == 16));
}

#[test]
fn auto_placement_accounts_for_conversion_boundaries() {
    let artifact = compile(
        &program("4x4"),
        &capabilities("2x2"),
        CompileOptions::default(),
    )
    .expect("compilation should succeed");
    let decision = &artifact.placement[0];

    assert!(
        decision.rationale.contains("conversion")
            || decision.selected_backend == TargetBackend::Cpu
    );
    if decision.selected_backend == TargetBackend::Photonic {
        assert_eq!(decision.optical_electrical_boundary_crossings, 2);
    }
}

#[test]
fn validation_rejects_incompatible_output_shape() {
    let mut program = program("4x4");
    program.tensors[2].shape = vec![4, 3];
    let error = compile(&program, &capabilities("2x2"), CompileOptions::default())
        .expect_err("invalid shape must fail");
    assert!(error.to_string().contains("expected [4, 4]"));
}

#[test]
fn rectangular_gemm_emits_partial_m_n_and_k_tiles() {
    let program: TensorProgram = serde_json::from_str(
        r#"{
          "ir_version": "awen.tensor.v1",
          "tensors": [
            {"id":"lhs","shape":[3,5],"dtype":"f16","layout":"row_major"},
            {"id":"rhs","shape":[5,4],"dtype":"f16","layout":"row_major"},
            {"id":"out","shape":[3,4],"dtype":"f16","layout":"row_major"}
          ],
          "ops": [{"op":"gemm","id":"rect","lhs":"lhs","rhs":"rhs","output":"out"}]
        }"#,
    )
    .expect("program must parse");
    let artifact = compile(
        &program,
        &capabilities("2x2"),
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("rectangular compilation should succeed");

    assert_eq!(artifact.photonic_ir.ops.len(), 12);
    assert!(artifact.photonic_ir.ops.iter().any(|op| op.tile.m == 1));
    assert!(artifact.photonic_ir.ops.iter().any(|op| op.tile.k == 1));
}

#[test]
fn transpose_and_column_major_inputs_execute_correctly() {
    let program: TensorProgram = serde_json::from_str(
        r#"{
          "ir_version": "awen.tensor.v1",
          "tensors": [
            {"id":"lhs","shape":[2,3],"dtype":"f16","layout":"column_major","data":[1,4,2,5,3,6]},
            {"id":"rhs","shape":[2,2],"dtype":"f16","layout":"row_major","data":[1,2,3,4]},
            {"id":"out","shape":[3,2],"dtype":"f16","layout":"row_major"}
          ],
          "ops": [{
            "op":"gemm","id":"transpose","lhs":"lhs","rhs":"rhs","output":"out",
            "transpose_lhs":true,
            "accuracy":{"max_abs_error":0.01,"max_rel_error":0.01,"minimum_effective_bits":8}
          }]
        }"#,
    )
    .expect("program must parse");
    let mut caps = capabilities("2x2");
    caps.effective_bits = 16;
    let artifact = compile(
        &program,
        &caps,
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("transpose compilation should succeed");
    let report = benchmark(&program, &artifact).expect("benchmark should run");
    let expected = [13.0, 18.0, 17.0, 24.0, 21.0, 30.0];

    assert!(report.all_outputs_within_tolerance);
    for (actual, expected) in report.outputs[0].values.iter().zip(expected) {
        assert!((actual - expected).abs() < 0.01);
    }
}

#[test]
fn unavailable_backend_causes_conservative_auto_fallback() {
    let mut snapshot = snapshot("128");
    snapshot.health.status = HealthStatus::Unavailable;
    snapshot
        .health
        .unavailable_resources
        .push("matrix_core".to_string());
    let artifact = compile_with_backend(&program("256"), &snapshot, CompileOptions::default())
        .expect("auto mode must retain a safe CPU path");

    assert_eq!(artifact.placement[0].selected_backend, TargetBackend::Cpu);
    assert!(artifact.placement[0]
        .rationale
        .contains("matrix_core_unavailable"));
    assert!(artifact.photonic_ir.ops.is_empty());
    assert_eq!(artifact.device_ir.commands.len(), 1);
}

#[test]
fn unavailable_backend_rejects_forced_photonic_target() {
    let mut snapshot = snapshot("128");
    snapshot.health.available_channels = 0;
    let error = compile_with_backend(
        &program("256"),
        &snapshot,
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect_err("forced photonic execution cannot ignore live resource loss");

    assert!(error.to_string().contains("no_channels"));
}

#[test]
fn live_channel_availability_limits_emitted_wavelength_allocation() {
    let mut snapshot = snapshot("128");
    snapshot.health.available_channels = 1;
    let artifact = compile_with_backend(
        &program("256"),
        &snapshot,
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("one live channel is sufficient for serialized execution");

    assert!(artifact
        .photonic_ir
        .ops
        .iter()
        .all(|operation| operation.wavelength_channels.len() == 1));
}
