use awen_compiler::{
    autotune_with_profile, benchmark_with_observations, compile, decide_placement_with_model,
    AutotuneOptions, CostModelInputs, DType, DeviceCapabilities, DigitalBaseline, GemmShape,
    Layout, Observation, ObservationSet, OperationCostProfile, OptimizationObjective,
    ParameterSource, TargetBackend, TensorProgram,
};

fn capabilities() -> DeviceCapabilities {
    serde_json::from_str(include_str!(
        "../../awen-compiler/capabilities/pace_like_128.json"
    ))
    .expect("reference capability")
}

fn model() -> CostModelInputs {
    serde_json::from_str(include_str!(
        "../../awen-compiler/cost_models/reference_full_system.json"
    ))
    .expect("reference cost model")
}

fn program() -> TensorProgram {
    serde_json::from_str(include_str!("../../awen-compiler/examples/gemm_4x4.json"))
        .expect("reference program")
}

#[test]
fn reference_cost_model_conforms_to_schema_and_rust_contract() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-spec/schemas/awen_cost_model.v1.json"
    ))
    .expect("cost-model schema");
    let instance: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-compiler/cost_models/reference_full_system.json"
    ))
    .expect("cost-model fixture");
    let compiled = jsonschema::JSONSchema::compile(&schema).expect("compile cost-model schema");
    assert!(compiled.is_valid(&instance));
    model().validate().expect("Rust cost-model validation");

    let observation_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-spec/schemas/awen_cost_observations.v1.json"
    ))
    .expect("observation schema");
    let observation_instance: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-compiler/cost_models/reference_observations.json"
    ))
    .expect("observation fixture");
    let compiled =
        jsonschema::JSONSchema::compile(&observation_schema).expect("compile observation schema");
    assert!(compiled.is_valid(&observation_instance));
    serde_json::from_value::<ObservationSet>(observation_instance)
        .expect("Rust observation decoding")
        .validate()
        .expect("Rust observation validation");
}

#[test]
fn estimate_is_full_system_dimensioned_and_uncertain() {
    let result = autotune_with_profile(
        GemmShape {
            m: 256,
            n: 256,
            k: 256,
        },
        DType::F16,
        Some(8),
        &capabilities(),
        &model(),
        OperationCostProfile {
            lhs_layout: Layout::ColumnMajor,
            rhs_layout: Layout::RowMajor,
            output_layout: Layout::RowMajor,
            sparsity_fraction: 0.5,
            structured_sparsity: true,
            input_error_fraction: 0.0001,
            maximum_input_magnitude: Some(1.0),
            estimated_output_magnitude: None,
            maximum_absolute_error: Some(0.02),
            maximum_relative_error: Some(0.02),
            requested_compute_dtype: None,
            requested_accumulator_dtype: None,
            allowed_bit_slicing_mode_mask: None,
            noise_seed: None,
        },
        OptimizationObjective::Latency,
        AutotuneOptions {
            graph_fingerprint: 0x17,
            seed: 17,
            batch_size: 4,
            allow_boundary_fusion: true,
            alternatives: 3,
            queue_depth: 2,
            overlap_fraction: 0.5,
            resident_input_fraction: 0.5,
        },
    )
    .expect("autotune");
    let estimate = &result.selected.estimate;

    assert!(estimate.latency_breakdown_ns.scheduling > 0.0);
    assert!(estimate.latency_breakdown_ns.host_transfer > 0.0);
    assert!(estimate.latency_breakdown_ns.memory > 0.0);
    assert!(estimate.latency_breakdown_ns.boundary_conversion > 0.0);
    assert!(estimate.latency_breakdown_ns.reconfiguration > 0.0);
    assert!(estimate.latency_breakdown_ns.dac > 0.0);
    assert!(estimate.latency_breakdown_ns.modulation > 0.0);
    assert!(estimate.latency_breakdown_ns.optical_propagation > 0.0);
    assert!(estimate.latency_breakdown_ns.detection > 0.0);
    assert!(estimate.latency_breakdown_ns.adc > 0.0);
    assert!(estimate.energy_breakdown_uj.laser > 0.0);
    assert!(estimate.energy_breakdown_uj.support_system > 0.0);
    assert!(estimate.latency_ns > estimate.latency_breakdown_ns.optical_propagation);
    assert!(estimate.energy_uj > 0.0);
    assert!(estimate.throughput_gops > 0.0);
    assert!(estimate.latency_interval_ns.lower <= estimate.latency_ns);
    assert!(estimate.latency_interval_ns.upper >= estimate.latency_ns);
    assert!(estimate.energy_interval_uj.lower <= estimate.energy_uj);
    assert!(estimate.energy_interval_uj.upper >= estimate.energy_uj);
    assert!(!estimate.provenance.is_empty());
}

#[test]
fn incomplete_inputs_fallback_in_auto_and_fail_when_forced() {
    let capabilities = capabilities();
    let mut incomplete = model();
    incomplete.provenance.clear();
    let auto = decide_placement_with_model(
        "gemm",
        GemmShape { m: 4, n: 4, k: 4 },
        DType::F16,
        Some(8),
        &capabilities,
        &incomplete,
        OperationCostProfile::default(),
        OptimizationObjective::Latency,
        TargetBackend::Auto,
        DigitalBaseline::default(),
        DigitalBaseline::default(),
        AutotuneOptions::default(),
    )
    .expect("auto fallback");
    assert_eq!(auto.selected_backend, TargetBackend::Cpu);
    assert!(auto.rationale.contains("required cost-model inputs"));

    let error = decide_placement_with_model(
        "gemm",
        GemmShape { m: 4, n: 4, k: 4 },
        DType::F16,
        Some(8),
        &capabilities,
        &incomplete,
        OperationCostProfile::default(),
        OptimizationObjective::Latency,
        TargetBackend::Photonic,
        DigitalBaseline::default(),
        DigitalBaseline::default(),
        AutotuneOptions::default(),
    )
    .expect_err("forced photonic must reject incomplete comparisons");
    assert!(error.to_string().contains("complete cost inputs"));
}

#[test]
fn benchmark_artifacts_track_error_and_fit_measured_parameters() {
    let program = program();
    let artifact = compile(
        &program,
        &capabilities(),
        awen_compiler::CompileOptions {
            target: TargetBackend::Photonic,
            ..awen_compiler::CompileOptions::default()
        },
    )
    .expect("compile");
    let predicted = artifact.placement[0]
        .photonic
        .as_ref()
        .expect("photonic estimate");
    let report = benchmark_with_observations(
        &program,
        &artifact,
        &[Observation {
            op_id: artifact.placement[0].op_id.clone(),
            latency_ns: predicted.latency_ns * 1.2,
            energy_uj: predicted.energy_uj * 1.1,
            error_fraction: predicted.estimated_error_fraction + 0.0005,
            source: ParameterSource::Measured,
            artifact_id: "sha256:hardware-run-001".to_string(),
        }],
    )
    .expect("benchmark with observation");

    assert_eq!(report.predicted_vs_observed.len(), 1);
    assert!(report.predicted_vs_observed[0].latency_error_fraction > 0.16);
    let calibrated = model()
        .calibrated_from_reports(&report.predicted_vs_observed)
        .expect("fit model");
    assert!(calibrated.latency_calibration_factor > 1.19);
    assert!(calibrated.energy_calibration_factor > 1.09);
    assert!(calibrated
        .provenance
        .iter()
        .any(|entry| entry.source == ParameterSource::Measured
            && entry.reference.contains("sha256:hardware-run-001")));
}

#[test]
fn model_profile_device_and_calibration_changes_invalidate_fingerprints() {
    let capabilities = capabilities();
    let base = autotune_with_profile(
        GemmShape {
            m: 256,
            n: 256,
            k: 256,
        },
        DType::F16,
        Some(8),
        &capabilities,
        &model(),
        OperationCostProfile::default(),
        OptimizationObjective::Energy,
        AutotuneOptions::default(),
    )
    .expect("base tune");

    let mut changed_model = model();
    changed_model.memory_energy_pj_per_byte *= 2.0;
    let model_change = autotune_with_profile(
        GemmShape {
            m: 256,
            n: 256,
            k: 256,
        },
        DType::F16,
        Some(8),
        &capabilities,
        &changed_model,
        OperationCostProfile::default(),
        OptimizationObjective::Energy,
        AutotuneOptions::default(),
    )
    .expect("model change tune");
    assert_ne!(base.fingerprint, model_change.fingerprint);

    let profile_change = autotune_with_profile(
        GemmShape {
            m: 256,
            n: 256,
            k: 256,
        },
        DType::F16,
        Some(8),
        &capabilities,
        &model(),
        OperationCostProfile {
            lhs_layout: Layout::ColumnMajor,
            ..OperationCostProfile::default()
        },
        OptimizationObjective::Energy,
        AutotuneOptions::default(),
    )
    .expect("profile change tune");
    assert_ne!(base.fingerprint, profile_change.fingerprint);

    let mut calibration_change = capabilities;
    let calibration_profile = calibration_change
        .calibration_profile
        .as_mut()
        .expect("profile");
    calibration_profile.id.push_str("-new");
    calibration_profile.fingerprint =
        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string();
    let calibration_change = autotune_with_profile(
        GemmShape {
            m: 256,
            n: 256,
            k: 256,
        },
        DType::F16,
        Some(8),
        &calibration_change,
        &model(),
        OperationCostProfile::default(),
        OptimizationObjective::Energy,
        AutotuneOptions::default(),
    )
    .expect("calibration change tune");
    assert_ne!(base.fingerprint, calibration_change.fingerprint);
}
