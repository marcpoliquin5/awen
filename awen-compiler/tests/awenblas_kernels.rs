use awen_compiler::{
    benchmark_kernel, execute_kernel_reference, execute_kernel_simulator, select_kernel,
    AccumulationMode, CalibrationInput, ComplexValue, DType, KernelAttributes,
    KernelBackendProfile, KernelData, KernelKind, KernelRequest, KernelSimulatorOptions,
    KernelStructure, KernelTensor, Layout, OptimizationObjective, ParameterSource, PhaseConvention,
    TargetBackend, AWENBLAS_VERSION,
};
use std::collections::BTreeSet;

fn request(kind: KernelKind, inputs: Vec<KernelTensor>) -> KernelRequest {
    KernelRequest {
        version: AWENBLAS_VERSION.to_string(),
        id: format!("conformance.{kind:?}").to_ascii_lowercase(),
        kind,
        inputs,
        attributes: KernelAttributes::default(),
        accuracy: awen_compiler::ir::AccuracyContract {
            max_abs_error: 0.05,
            max_rel_error: 0.05,
            minimum_effective_bits: Some(8),
        },
        calibration_inputs: Vec::new(),
    }
}

fn real(shape: &[usize], values: &[f64]) -> KernelTensor {
    KernelTensor::real("input", shape.to_vec(), values.to_vec())
}

fn complex(shape: &[usize], values: &[(f64, f64)]) -> KernelTensor {
    KernelTensor::complex(
        "input",
        shape.to_vec(),
        values
            .iter()
            .map(|(real, imaginary)| ComplexValue::new(*real, *imaginary))
            .collect(),
    )
}

fn real_output(result: &awen_compiler::KernelResult, index: usize) -> &[f64] {
    match &result.outputs[index].data {
        KernelData::Real(values) => values,
        KernelData::Complex(_) => panic!("expected real output"),
    }
}

fn complex_output(result: &awen_compiler::KernelResult, index: usize) -> &[ComplexValue] {
    match &result.outputs[index].data {
        KernelData::Complex(values) => values,
        KernelData::Real(_) => panic!("expected complex output"),
    }
}

fn close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!((actual - expected).abs() < 1.0e-9, "{actual} != {expected}");
    }
}

fn every_kernel_request() -> Vec<KernelRequest> {
    let matrix = || real(&[2, 2], &[1.0, 2.0, 3.0, 4.0]);
    let identity = || real(&[2, 2], &[1.0, 0.0, 0.0, 1.0]);
    let complex_matrix = || complex(&[2, 2], &[(1.0, 0.0), (0.0, 1.0), (1.0, -1.0), (2.0, 0.0)]);
    let complex_identity = || complex(&[2, 2], &[(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)]);
    let mut random_projection = request(KernelKind::RandomProjection, vec![matrix()]);
    random_projection.attributes.output_size = 2;
    let mut low_rank = request(
        KernelKind::LowRankGemm,
        vec![
            matrix(),
            real(&[2, 1], &[1.0, 2.0]),
            real(&[2, 1], &[3.0, 4.0]),
        ],
    );
    low_rank.attributes.rank = 1;
    let mut block_circulant = request(
        KernelKind::BlockCirculant,
        vec![real(&[2, 1, 1], &[1.0, 2.0]), real(&[2], &[3.0, 4.0])],
    );
    block_circulant.attributes.block_size = 1;
    let mut reservoir = request(
        KernelKind::ReservoirStep,
        vec![
            real(&[2], &[0.0, 0.0]),
            identity(),
            real(&[2, 1], &[1.0, -1.0]),
        ],
    );
    reservoir.attributes.scale = 0.25;
    vec![
        request(KernelKind::Gemm, vec![matrix(), identity()]),
        request(
            KernelKind::BatchedGemm,
            vec![
                real(&[1, 2, 2], &[1.0, 2.0, 3.0, 4.0]),
                real(&[1, 2, 2], &[1.0, 0.0, 0.0, 1.0]),
            ],
        ),
        request(
            KernelKind::ComplexGemm,
            vec![complex_matrix(), complex_identity()],
        ),
        request(
            KernelKind::Linear,
            vec![matrix(), identity(), real(&[2], &[1.0, 1.0])],
        ),
        request(
            KernelKind::TransformerQkv,
            vec![matrix(), identity(), identity(), identity()],
        ),
        request(KernelKind::AttentionScores, vec![matrix(), identity()]),
        request(KernelKind::AttentionValue, vec![matrix(), identity()]),
        request(KernelKind::MlpProjection, vec![matrix(), identity()]),
        request(
            KernelKind::Convolution1d,
            vec![real(&[4], &[1.0, 2.0, 3.0, 4.0]), real(&[2], &[1.0, 2.0])],
        ),
        request(
            KernelKind::Correlation1d,
            vec![real(&[4], &[1.0, 2.0, 3.0, 4.0]), real(&[2], &[1.0, 2.0])],
        ),
        request(
            KernelKind::Dft,
            vec![complex(&[2], &[(1.0, 0.0), (0.0, 0.0)])],
        ),
        request(
            KernelKind::Fft,
            vec![complex(&[2], &[(1.0, 0.0), (0.0, 0.0)])],
        ),
        request(
            KernelKind::FourierFilter,
            vec![
                complex(&[2], &[(1.0, 0.0), (0.0, 0.0)]),
                complex(&[2], &[(1.0, 0.0), (1.0, 0.0)]),
            ],
        ),
        low_rank,
        random_projection,
        request(
            KernelKind::Toeplitz,
            vec![
                real(&[2], &[1.0, 2.0]),
                real(&[2], &[1.0, 3.0]),
                real(&[2], &[4.0, 5.0]),
            ],
        ),
        request(
            KernelKind::Circulant,
            vec![real(&[2], &[1.0, 2.0]), real(&[2], &[3.0, 4.0])],
        ),
        block_circulant,
        request(
            KernelKind::Beamforming,
            vec![complex_matrix(), complex_identity()],
        ),
        request(
            KernelKind::RfFir,
            vec![real(&[4], &[1.0, 2.0, 3.0, 4.0]), real(&[2], &[1.0, 2.0])],
        ),
        reservoir,
        request(
            KernelKind::Propagation,
            vec![complex_matrix(), complex_identity()],
        ),
    ]
}

#[test]
fn every_kernel_has_reference_simulator_and_measured_benchmark_execution() {
    let requests = every_kernel_request();
    assert_eq!(requests.len(), 22);
    for request in requests {
        let reference = execute_kernel_reference(&request).expect("reference kernel");
        let options = KernelSimulatorOptions {
            target: TargetBackend::Photonic,
            effective_bits: 16,
            noise_fraction: 0.0,
            seed: 99,
        };
        let simulated = execute_kernel_simulator(&request, options).expect("simulator kernel");
        assert_eq!(reference.outputs.len(), simulated.outputs.len());
        assert_eq!(
            reference.descriptor.structure,
            simulated.descriptor.structure
        );
        let report = benchmark_kernel(&request, options, 1).expect("end-to-end benchmark");
        assert_eq!(report.kind, request.kind);
        assert_eq!(report.source, ParameterSource::Measured);
        assert!(
            report.within_contract,
            "{:?} exceeded contract",
            request.kind
        );
    }
}

#[test]
fn gemm_batched_gemm_and_complex_gemm_conformance_vectors() {
    let gemm = request(
        KernelKind::Gemm,
        vec![
            real(&[2, 2], &[1.0, 2.0, 3.0, 4.0]),
            real(&[2, 2], &[5.0, 6.0, 7.0, 8.0]),
        ],
    );
    close(
        real_output(&execute_kernel_reference(&gemm).expect("GEMM"), 0),
        &[19.0, 22.0, 43.0, 50.0],
    );

    let batched = request(
        KernelKind::BatchedGemm,
        vec![
            real(&[2, 1, 2], &[1.0, 2.0, 3.0, 4.0]),
            real(&[2, 2, 1], &[5.0, 6.0, 7.0, 8.0]),
        ],
    );
    close(
        real_output(
            &execute_kernel_reference(&batched).expect("batched GEMM"),
            0,
        ),
        &[17.0, 53.0],
    );

    let complex_gemm = request(
        KernelKind::ComplexGemm,
        vec![
            complex(&[1, 2], &[(1.0, 1.0), (2.0, -1.0)]),
            complex(&[2, 1], &[(1.0, -1.0), (0.5, 2.0)]),
        ],
    );
    let output = execute_kernel_reference(&complex_gemm).expect("complex GEMM");
    assert_eq!(complex_output(&output, 0), &[ComplexValue::new(5.0, 3.5)]);
    assert_eq!(
        output.descriptor.phase_convention,
        Some(PhaseConvention::NegativeExponent)
    );
}

#[test]
fn transformer_linear_attention_and_mlp_conformance_vectors() {
    let x = real(&[1, 2], &[1.0, 2.0]);
    let identity = real(&[2, 2], &[1.0, 0.0, 0.0, 1.0]);
    let qkv = request(
        KernelKind::TransformerQkv,
        vec![
            x.clone(),
            identity.clone(),
            identity.clone(),
            identity.clone(),
        ],
    );
    let qkv_result = execute_kernel_reference(&qkv).expect("QKV");
    assert_eq!(qkv_result.outputs.len(), 3);
    for index in 0..3 {
        close(real_output(&qkv_result, index), &[1.0, 2.0]);
    }

    let mut scores = request(
        KernelKind::AttentionScores,
        vec![
            real(&[2, 2], &[1.0, 0.0, 0.0, 1.0]),
            real(&[2, 2], &[1.0, 0.0, 0.0, 1.0]),
        ],
    );
    scores.attributes.scale = 0.5;
    close(
        real_output(&execute_kernel_reference(&scores).expect("scores"), 0),
        &[0.5, 0.0, 0.0, 0.5],
    );

    let attention_value = request(
        KernelKind::AttentionValue,
        vec![
            real(&[1, 2], &[0.25, 0.75]),
            real(&[2, 2], &[4.0, 0.0, 0.0, 8.0]),
        ],
    );
    close(
        real_output(
            &execute_kernel_reference(&attention_value).expect("attention value"),
            0,
        ),
        &[1.0, 6.0],
    );

    for kind in [KernelKind::Linear, KernelKind::MlpProjection] {
        let linear = request(
            kind,
            vec![x.clone(), identity.clone(), real(&[2], &[3.0, 4.0])],
        );
        close(
            real_output(&execute_kernel_reference(&linear).expect("linear"), 0),
            &[4.0, 6.0],
        );
    }
}

#[test]
fn convolution_correlation_and_rf_fir_conformance_vectors() {
    let signal = real(&[4], &[1.0, 2.0, 3.0, 4.0]);
    let taps = real(&[2], &[1.0, 2.0]);
    for (kind, expected) in [
        (KernelKind::Convolution1d, vec![4.0, 7.0, 10.0]),
        (KernelKind::Correlation1d, vec![5.0, 8.0, 11.0]),
        (KernelKind::RfFir, vec![4.0, 7.0, 10.0]),
    ] {
        let request = request(kind, vec![signal.clone(), taps.clone()]);
        let result = execute_kernel_reference(&request).expect("convolution family");
        close(real_output(&result, 0), &expected);
        assert_eq!(result.descriptor.structure, KernelStructure::Convolutional);
    }
}

#[test]
fn dft_fft_and_fourier_filter_use_explicit_phase_conventions() {
    let impulse = complex(&[4], &[(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]);
    for kind in [KernelKind::Dft, KernelKind::Fft] {
        let mut transform = request(kind, vec![impulse.clone()]);
        transform.attributes.phase_convention = PhaseConvention::NegativeExponent;
        let result = execute_kernel_reference(&transform).expect("transform");
        assert!(complex_output(&result, 0)
            .iter()
            .all(|value| (value.real - 1.0).abs() < 1.0e-9 && value.imaginary.abs() < 1.0e-9));

        let mut inverse = request(kind, vec![result.outputs[0].clone()]);
        inverse.attributes.inverse = true;
        let recovered = execute_kernel_reference(&inverse).expect("inverse transform");
        assert!((complex_output(&recovered, 0)[0].real - 1.0).abs() < 1.0e-9);
    }

    let filter = request(
        KernelKind::FourierFilter,
        vec![
            impulse,
            complex(&[4], &[(1.0, 0.0), (1.0, 0.0), (1.0, 0.0), (1.0, 0.0)]),
        ],
    );
    let filtered = execute_kernel_reference(&filter).expect("Fourier filter");
    assert!((complex_output(&filtered, 0)[0].real - 1.0).abs() < 1.0e-9);
}

#[test]
fn low_rank_random_projection_and_structured_transforms_preserve_structure() {
    let mut low_rank = request(
        KernelKind::LowRankGemm,
        vec![
            real(&[1, 2], &[1.0, 2.0]),
            real(&[2, 1], &[3.0, 4.0]),
            real(&[2, 1], &[5.0, 6.0]),
        ],
    );
    low_rank.attributes.rank = 1;
    let result = execute_kernel_reference(&low_rank).expect("low-rank GEMM");
    close(real_output(&result, 0), &[55.0, 66.0]);
    assert_eq!(result.descriptor.structure, KernelStructure::LowRank);

    let mut projection = request(
        KernelKind::RandomProjection,
        vec![real(&[2, 3], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])],
    );
    projection.attributes.output_size = 2;
    projection.attributes.seed = 42;
    let first = execute_kernel_reference(&projection).expect("projection");
    let second = execute_kernel_reference(&projection).expect("projection replay");
    assert_eq!(first, second);
    assert_eq!(
        first.descriptor.structure,
        KernelStructure::RandomProjection
    );

    let toeplitz = request(
        KernelKind::Toeplitz,
        vec![
            real(&[2], &[1.0, 2.0]),
            real(&[2], &[1.0, 3.0]),
            real(&[2], &[4.0, 5.0]),
        ],
    );
    close(
        real_output(&execute_kernel_reference(&toeplitz).expect("Toeplitz"), 0),
        &[19.0, 13.0],
    );

    let circulant = request(
        KernelKind::Circulant,
        vec![real(&[3], &[1.0, 2.0, 3.0]), real(&[3], &[1.0, 0.0, 0.0])],
    );
    close(
        real_output(&execute_kernel_reference(&circulant).expect("circulant"), 0),
        &[1.0, 3.0, 2.0],
    );

    let mut block = request(
        KernelKind::BlockCirculant,
        vec![real(&[2, 1, 1], &[1.0, 2.0]), real(&[2], &[3.0, 4.0])],
    );
    block.attributes.block_size = 1;
    let block_result = execute_kernel_reference(&block).expect("block circulant");
    close(real_output(&block_result, 0), &[11.0, 10.0]);
    assert_eq!(
        block_result.descriptor.structure,
        KernelStructure::BlockCirculant
    );
}

#[test]
fn beamforming_propagation_and_reservoir_conformance_vectors() {
    let weights = complex(&[1, 2], &[(1.0, 0.0), (0.0, 1.0)]);
    let samples = complex(&[2, 1], &[(2.0, 0.0), (3.0, 0.0)]);
    for kind in [KernelKind::Beamforming, KernelKind::Propagation] {
        let request = request(kind, vec![weights.clone(), samples.clone()]);
        let result = execute_kernel_reference(&request).expect("complex physical kernel");
        assert_eq!(complex_output(&result, 0), &[ComplexValue::new(2.0, 3.0)]);
    }

    let mut reservoir = request(
        KernelKind::ReservoirStep,
        vec![
            real(&[2], &[0.0, 0.0]),
            real(&[2, 2], &[0.0, 0.0, 0.0, 0.0]),
            real(&[2, 1], &[1.0, -1.0]),
        ],
    );
    reservoir.attributes.scale = 0.5;
    reservoir.attributes.leakage = 1.0;
    let output = execute_kernel_reference(&reservoir).expect("reservoir");
    close(
        real_output(&output, 0),
        &[0.5_f64.tanh(), (-0.5_f64).tanh()],
    );
}

#[test]
fn simulator_is_deterministic_and_benchmark_covers_end_to_end_boundary() {
    let mut gemm = request(
        KernelKind::Gemm,
        vec![real(&[1, 2], &[1.0, 2.0]), real(&[2, 1], &[3.0, 4.0])],
    );
    gemm.calibration_inputs.push(CalibrationInput {
        id: "calibration-1".to_string(),
        gain: 1.0,
        bias: 0.0,
        uncertainty_fraction: 0.001,
    });
    let options = KernelSimulatorOptions {
        target: TargetBackend::Photonic,
        effective_bits: 12,
        noise_fraction: 0.0001,
        seed: 7,
    };
    let first = execute_kernel_simulator(&gemm, options).expect("simulation");
    let second = execute_kernel_simulator(&gemm, options).expect("simulation replay");
    assert_eq!(first, second);

    let report = benchmark_kernel(&gemm, options, 2).expect("benchmark");
    assert!(report.reference_wall_clock_ns > 0);
    assert!(report.simulator_wall_clock_ns > 0);
    assert!(report.within_contract);
    assert_eq!(report.source, ParameterSource::Measured);
    assert!(report
        .measurement_boundary
        .contains(&"input quantization".to_string()));
    assert!(report
        .measurement_boundary
        .contains(&"output materialization".to_string()));
}

#[test]
fn capability_and_cost_selection_preserves_structure_and_falls_back_safely() {
    let mut request = request(
        KernelKind::Toeplitz,
        vec![
            real(&[2], &[1.0, 2.0]),
            real(&[2], &[1.0, 3.0]),
            real(&[2], &[4.0, 5.0]),
        ],
    );
    request.calibration_inputs.push(CalibrationInput {
        id: "calibration-1".to_string(),
        gain: 1.0,
        bias: 0.0,
        uncertainty_fraction: 0.001,
    });
    let photonic = KernelBackendProfile {
        version: AWENBLAS_VERSION.to_string(),
        backend_id: "photonic-structured".to_string(),
        target: TargetBackend::Photonic,
        supported_kinds: [KernelKind::Toeplitz].into_iter().collect(),
        supported_dtypes: [DType::F32].into_iter().collect(),
        supported_structures: [KernelStructure::Toeplitz].into_iter().collect(),
        supports_complex: false,
        maximum_tensor_elements: 1024,
        effective_bits: 12,
        requires_calibration: true,
        launch_latency_ns: 1.0,
        throughput_tops: 100.0,
        energy_pj_per_operation: 0.01,
        estimated_error_fraction: 0.001,
        source: ParameterSource::Simulated,
    };
    let plan = select_kernel(
        &request,
        std::slice::from_ref(&photonic),
        OptimizationObjective::Latency,
    )
    .expect("selection");
    assert_eq!(plan.selected_target, TargetBackend::Photonic);
    assert_eq!(plan.descriptor.structure, KernelStructure::Toeplitz);
    assert!(!plan.fallback);

    let mut unsupported = photonic;
    unsupported.supported_structures = [KernelStructure::Dense].into_iter().collect();
    let fallback =
        select_kernel(&request, &[unsupported], OptimizationObjective::Latency).expect("fallback");
    assert_eq!(fallback.selected_target, TargetBackend::Cpu);
    assert!(fallback.fallback);
    assert!(fallback
        .candidates
        .iter()
        .any(|candidate| candidate.reason.contains("must not be densified")));
}

#[test]
fn randomized_gemm_identity_and_fft_round_trip_properties_hold() {
    let mut state = 13_u64;
    for size in 1..=8 {
        let values = (0..size * size)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((state >> 32) as f64 / u32::MAX as f64) * 2.0 - 1.0
            })
            .collect::<Vec<_>>();
        let identity = (0..size * size)
            .map(|index| {
                if index / size == index % size {
                    1.0
                } else {
                    0.0
                }
            })
            .collect::<Vec<_>>();
        let gemm = request(
            KernelKind::Gemm,
            vec![real(&[size, size], &values), real(&[size, size], &identity)],
        );
        close(
            real_output(&execute_kernel_reference(&gemm).expect("identity GEMM"), 0),
            &values,
        );

        let complex_values = values
            .iter()
            .take(size)
            .map(|value| (*value, -*value / 2.0))
            .collect::<Vec<_>>();
        let mut forward = request(KernelKind::Fft, vec![complex(&[size], &complex_values)]);
        forward.attributes.phase_convention = PhaseConvention::PositiveExponent;
        let transformed = execute_kernel_reference(&forward).expect("FFT property");
        let mut inverse = request(KernelKind::Fft, transformed.outputs);
        inverse.attributes.inverse = true;
        inverse.attributes.phase_convention = PhaseConvention::PositiveExponent;
        let recovered = execute_kernel_reference(&inverse).expect("inverse FFT property");
        for (actual, (real, imaginary)) in complex_output(&recovered, 0).iter().zip(complex_values)
        {
            assert!((actual.real - real).abs() < 1.0e-9);
            assert!((actual.imaginary - imaginary).abs() < 1.0e-9);
        }
    }
}

#[test]
fn request_rejects_layout_data_and_numerical_contract_drift() {
    let mut tensor = real(&[2, 2], &[1.0, 2.0, 3.0, 4.0]);
    tensor.layout = Layout::ColumnMajor;
    tensor.dtype = DType::ComplexF32;
    let invalid = request(KernelKind::Gemm, vec![tensor, real(&[2, 2], &[1.0; 4])]);
    assert!(execute_kernel_reference(&invalid).is_err());

    let mut accumulation = KernelAttributes::default();
    assert_eq!(accumulation.accumulation_mode, AccumulationMode::Digital);
    accumulation.accumulation_mode = AccumulationMode::Optical;
    assert_eq!(accumulation.accumulation_mode, AccumulationMode::Optical);

    let kinds = [KernelKind::Gemm, KernelKind::Fft]
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert!(kinds.contains(&KernelKind::Gemm));
}

#[test]
fn calibration_chain_is_finite_invertible_and_compensated() {
    let mut gemm = request(
        KernelKind::Gemm,
        vec![real(&[1, 2], &[1.0, 2.0]), real(&[2, 1], &[3.0, 4.0])],
    );
    gemm.calibration_inputs = vec![
        CalibrationInput {
            id: "modulator-transfer".to_string(),
            gain: 2.0,
            bias: 3.0,
            uncertainty_fraction: 0.001,
        },
        CalibrationInput {
            id: "detector-transfer".to_string(),
            gain: 0.5,
            bias: -1.0,
            uncertainty_fraction: 0.001,
        },
    ];
    let reference = execute_kernel_reference(&gemm).expect("reference");
    let simulated = execute_kernel_simulator(
        &gemm,
        KernelSimulatorOptions {
            target: TargetBackend::Photonic,
            effective_bits: 24,
            noise_fraction: 0.0,
            seed: 0,
        },
    )
    .expect("calibrated simulation");
    assert_eq!(
        simulated.descriptor.calibration_inputs,
        vec!["modulator-transfer", "detector-transfer"]
    );
    assert!((real_output(&simulated, 0)[0] - real_output(&reference, 0)[0]).abs() < 1.0e-5);

    gemm.calibration_inputs[0].gain = 1.0e308;
    gemm.calibration_inputs[1].gain = 1.0e308;
    assert!(gemm.validate().is_err());
}

#[test]
fn backend_contract_rejects_complex_drift_and_output_capacity_overflow() {
    let mut contradictory = KernelBackendProfile::cpu_reference();
    contradictory.supports_complex = false;
    assert!(contradictory.validate().is_err());

    let mut projection = request(
        KernelKind::RandomProjection,
        vec![real(&[1, 2], &[1.0, 2.0])],
    );
    projection.attributes.output_size = 16;
    let mut constrained = KernelBackendProfile::cpu_reference();
    constrained.backend_id = "capacity-constrained-photonic".to_string();
    constrained.target = TargetBackend::Photonic;
    constrained.maximum_tensor_elements = 8;
    let first = select_kernel(
        &projection,
        std::slice::from_ref(&constrained),
        OptimizationObjective::Latency,
    )
    .expect("capacity fallback");
    let second = select_kernel(&projection, &[constrained], OptimizationObjective::Latency)
        .expect("deterministic capacity fallback");
    assert_eq!(first, second);
    assert_eq!(first.selected_target, TargetBackend::Cpu);
    assert!(first.fallback);
    assert!(first.candidates.iter().any(|candidate| {
        candidate.backend_id == "capacity-constrained-photonic"
            && candidate.reason.contains("input or output")
    }));

    let mut impossible_precision = projection;
    impossible_precision.accuracy.minimum_effective_bits = Some(33);
    assert!(impossible_precision.validate().is_err());
}

#[test]
fn structured_matrix_kernels_honor_column_major_storage() {
    let column_major = |shape: &[usize], values: &[f64]| {
        let mut tensor = real(shape, values);
        tensor.layout = Layout::ColumnMajor;
        tensor
    };

    let mut low_rank = request(
        KernelKind::LowRankGemm,
        vec![
            column_major(&[2, 2], &[1.0, 3.0, 2.0, 4.0]),
            column_major(&[2, 2], &[1.0, 0.0, 0.0, 1.0]),
            column_major(&[2, 2], &[1.0, 0.0, 0.0, 1.0]),
        ],
    );
    low_rank.attributes.rank = 2;
    close(
        real_output(
            &execute_kernel_reference(&low_rank).expect("column-major low rank"),
            0,
        ),
        &[1.0, 2.0, 3.0, 4.0],
    );

    let mut block = request(
        KernelKind::BlockCirculant,
        vec![
            column_major(&[1, 2, 2], &[1.0, 3.0, 2.0, 4.0]),
            real(&[2], &[5.0, 6.0]),
        ],
    );
    block.attributes.block_size = 2;
    close(
        real_output(
            &execute_kernel_reference(&block).expect("column-major block"),
            0,
        ),
        &[17.0, 39.0],
    );

    let mut reservoir = request(
        KernelKind::ReservoirStep,
        vec![
            real(&[2], &[1.0, 2.0]),
            column_major(&[2, 2], &[1.0, 3.0, 2.0, 4.0]),
            column_major(&[2, 1], &[0.0, 0.0]),
        ],
    );
    reservoir.attributes.scale = 0.0;
    reservoir.attributes.leakage = 1.0;
    close(
        real_output(
            &execute_kernel_reference(&reservoir).expect("column-major reservoir"),
            0,
        ),
        &[5.0_f64.tanh(), 11.0_f64.tanh()],
    );

    let mut row_projection = request(
        KernelKind::RandomProjection,
        vec![real(&[2, 2], &[1.0, 2.0, 3.0, 4.0])],
    );
    row_projection.attributes.output_size = 3;
    row_projection.attributes.seed = 91;
    let mut column_projection = row_projection.clone();
    column_projection.inputs[0] = column_major(&[2, 2], &[1.0, 3.0, 2.0, 4.0]);
    assert_eq!(
        execute_kernel_reference(&row_projection)
            .expect("row-major projection")
            .outputs[0]
            .data,
        execute_kernel_reference(&column_projection)
            .expect("column-major projection")
            .outputs[0]
            .data
    );

    let mut mixed_dtype = request(
        KernelKind::Gemm,
        vec![
            real(&[1, 1], &[1.0]),
            KernelTensor {
                dtype: DType::F16,
                ..real(&[1, 1], &[1.0])
            },
        ],
    );
    assert!(mixed_dtype.validate().is_err());
    mixed_dtype.inputs[0].dtype = DType::F16;
    assert!(mixed_dtype.validate().is_ok());
}
