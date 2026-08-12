use crate::awenblas::{accumulate_tile, reference_gemm};
use crate::compiler::CompilationArtifact;
use crate::cost::{ModelErrorReport, Observation, TargetBackend, COST_MODEL_VERSION};
use crate::ir::{validate_program, Tensor, TensorOp, TensorProgram};
use crate::precision::{
    apply_noise, maximum_absolute, quantize, EmpiricalErrorReport, ErrorAttribution,
    ERROR_REPORT_VERSION,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputComparison {
    pub tensor: String,
    pub backend: TargetBackend,
    pub values: Vec<f64>,
    pub reference_values: Vec<f64>,
    pub max_abs_error: f64,
    pub max_rel_error: f64,
    pub passed: bool,
    pub error_report: EmpiricalErrorReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkReport {
    pub report_version: String,
    pub backend_id: String,
    pub outputs: Vec<OutputComparison>,
    pub all_outputs_within_tolerance: bool,
    pub estimated_total_latency_ns: f64,
    pub estimated_total_energy_uj: f64,
    pub optical_electrical_boundary_crossings: u32,
    pub cost_model_version: String,
    pub predicted_vs_observed: Vec<ModelErrorReport>,
}

pub fn benchmark(
    program: &TensorProgram,
    artifact: &CompilationArtifact,
) -> Result<BenchmarkReport> {
    benchmark_with_observations(program, artifact, &[])
}

pub fn benchmark_with_observations(
    program: &TensorProgram,
    artifact: &CompilationArtifact,
    observations: &[Observation],
) -> Result<BenchmarkReport> {
    let validated = validate_program(program)?;
    let tensors: HashMap<&str, &Tensor> = program
        .tensors
        .iter()
        .map(|tensor| (tensor.id.as_str(), tensor))
        .collect();
    let decisions: HashMap<&str, _> = artifact
        .placement
        .iter()
        .map(|decision| (decision.op_id.as_str(), decision))
        .collect();
    let mut outputs = Vec::with_capacity(validated.len());
    let mut produced = BTreeMap::new();

    for gemm in validated {
        let decision = decisions
            .get(gemm.op.id())
            .copied()
            .with_context(|| format!("artifact has no placement for '{}'", gemm.op.id()))?;
        let TensorOp::Gemm {
            transpose_lhs,
            transpose_rhs,
            output,
            accuracy,
            ..
        } = gemm.op;
        let reference = reference_gemm(
            gemm.lhs,
            gemm.rhs,
            *transpose_lhs,
            *transpose_rhs,
            gemm.shape.m,
            gemm.shape.n,
            gemm.shape.k,
        )?;
        let simulation = match decision.selected_backend {
            TargetBackend::Photonic => simulate_photonic_gemm(
                gemm.op.id(),
                gemm.lhs,
                gemm.rhs,
                *transpose_lhs,
                *transpose_rhs,
                gemm.shape.n,
                &reference,
                artifact,
            )?,
            TargetBackend::Cpu | TargetBackend::Gpu => SimulatedOutput {
                values: reference.clone(),
                error_attribution: ErrorAttribution::default().checked()?,
                seed: 0,
                provenance: vec!["exact deterministic digital reference kernel".to_string()],
            },
            TargetBackend::Auto => bail!("compiled artifacts must not contain auto placement"),
        };
        let values = simulation.values;
        let (max_abs_error, max_rel_error) = compare(&values, &reference);
        let passed = values.iter().zip(&reference).all(|(actual, expected)| {
            (actual - expected).abs()
                <= accuracy.max_abs_error + accuracy.max_rel_error * expected.abs()
        });
        let static_fraction = match decision.selected_backend {
            TargetBackend::Photonic => decision
                .photonic
                .as_ref()
                .map_or(decision.cpu.error_attribution, |estimate| {
                    estimate.error_attribution
                }),
            TargetBackend::Gpu => decision.gpu.error_attribution,
            _ => decision.cpu.error_attribution,
        };
        let error_report = EmpiricalErrorReport {
            version: ERROR_REPORT_VERSION.to_string(),
            operation_id: gemm.op.id().to_string(),
            seed: simulation.seed,
            static_fraction,
            observed_absolute: simulation.error_attribution,
            maximum_absolute_error: max_abs_error,
            maximum_relative_error: max_rel_error,
            passed,
            provenance: simulation.provenance,
        };
        produced.insert(output.clone(), values.clone());
        outputs.push(OutputComparison {
            tensor: output.clone(),
            backend: decision.selected_backend,
            values,
            reference_values: reference,
            max_abs_error,
            max_rel_error,
            passed,
            error_report,
        });
    }

    for tensor_id in produced.keys() {
        if !tensors.contains_key(tensor_id.as_str()) {
            bail!("benchmark produced undeclared tensor '{tensor_id}'");
        }
    }
    let all_outputs_within_tolerance = outputs.iter().all(|output| output.passed);
    let mut predicted_vs_observed = Vec::with_capacity(observations.len());
    for observation in observations {
        let decision = decisions
            .get(observation.op_id.as_str())
            .copied()
            .with_context(|| {
                format!(
                    "observation references unknown operation '{}'",
                    observation.op_id
                )
            })?;
        let predicted = match decision.selected_backend {
            TargetBackend::Photonic => decision
                .photonic
                .clone()
                .unwrap_or_else(|| decision.cpu.clone()),
            TargetBackend::Cpu => decision.cpu.clone(),
            TargetBackend::Gpu => decision.gpu.clone(),
            TargetBackend::Auto => bail!("compiled artifacts must not contain auto placement"),
        };
        predicted_vs_observed.push(ModelErrorReport::compare(
            decision.decision_fingerprint.clone(),
            predicted,
            observation.clone(),
        )?);
    }
    let estimated_total_latency_ns = artifact
        .placement
        .iter()
        .map(|decision| match decision.selected_backend {
            TargetBackend::Photonic => decision
                .photonic
                .as_ref()
                .map(|estimate| estimate.latency_ns)
                .unwrap_or(decision.cpu.latency_ns),
            TargetBackend::Gpu => decision.gpu.latency_ns,
            _ => decision.cpu.latency_ns,
        })
        .sum();
    let estimated_total_energy_uj = artifact
        .placement
        .iter()
        .map(|decision| match decision.selected_backend {
            TargetBackend::Photonic => decision
                .photonic
                .as_ref()
                .map(|estimate| estimate.energy_uj)
                .unwrap_or(decision.cpu.energy_uj),
            TargetBackend::Gpu => decision.gpu.energy_uj,
            _ => decision.cpu.energy_uj,
        })
        .sum();
    Ok(BenchmarkReport {
        report_version: "awen.benchmark.v1".to_string(),
        backend_id: artifact.backend_id.clone(),
        outputs,
        all_outputs_within_tolerance,
        estimated_total_latency_ns,
        estimated_total_energy_uj,
        optical_electrical_boundary_crossings: artifact
            .placement
            .iter()
            .map(|decision| decision.optical_electrical_boundary_crossings)
            .sum(),
        cost_model_version: COST_MODEL_VERSION.to_string(),
        predicted_vs_observed,
    })
}

struct SimulatedOutput {
    values: Vec<f64>,
    error_attribution: ErrorAttribution,
    seed: u64,
    provenance: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
fn simulate_photonic_gemm(
    op_id: &str,
    lhs: &Tensor,
    rhs: &Tensor,
    transpose_lhs: bool,
    transpose_rhs: bool,
    output_columns: usize,
    reference: &[f64],
    artifact: &CompilationArtifact,
) -> Result<SimulatedOutput> {
    let lhs_data = lhs
        .data
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("tensor '{}' has no literal data", lhs.id))?;
    let rhs_data = rhs
        .data
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("tensor '{}' has no literal data", rhs.id))?;
    let matching_tiles: Vec<_> = artifact
        .photonic_ir
        .ops
        .iter()
        .filter(|tile| tile.op_id.starts_with(&format!("{op_id}__")))
        .collect();
    if matching_tiles.is_empty() {
        bail!("photonic placement for '{op_id}' emitted no tiles");
    }
    let rows = matching_tiles
        .iter()
        .map(|tile| tile.tile.m_offset + tile.tile.m)
        .max()
        .unwrap_or(0);
    let precision = &matching_tiles[0].precision;
    let quantized_lhs = quantize(
        lhs_data,
        &lhs.shape,
        &precision.lhs_quantization,
        precision.noise_seed,
    )?;
    let quantized_rhs = quantize(
        rhs_data,
        &rhs.shape,
        &precision.rhs_quantization,
        precision.noise_seed.wrapping_add(1),
    )?;
    let clamped_lhs = lhs_data
        .iter()
        .map(|value| {
            value.clamp(
                precision.lhs_quantization.clipping_min,
                precision.lhs_quantization.clipping_max,
            )
        })
        .collect::<Vec<_>>();
    let clamped_rhs = rhs_data
        .iter()
        .map(|value| {
            value.clamp(
                precision.rhs_quantization.clipping_min,
                precision.rhs_quantization.clipping_max,
            )
        })
        .collect::<Vec<_>>();
    let inner = if transpose_lhs {
        lhs.shape[0]
    } else {
        lhs.shape[1]
    };
    let mut output = vec![0.0; rows * output_columns];
    for compiled_tile in &matching_tiles {
        accumulate_tile(
            &mut output,
            lhs,
            rhs,
            &quantized_lhs.dequantized,
            &quantized_rhs.dequantized,
            transpose_lhs,
            transpose_rhs,
            compiled_tile.tile,
            output_columns,
        )?;
    }
    let mut full_precision_accumulation = vec![0.0; rows * output_columns];
    accumulate_tile(
        &mut full_precision_accumulation,
        lhs,
        rhs,
        &quantized_lhs.dequantized,
        &quantized_rhs.dequantized,
        transpose_lhs,
        transpose_rhs,
        crate::lowering::Tile {
            m_offset: 0,
            n_offset: 0,
            k_offset: 0,
            m: rows,
            n: output_columns,
            k: inner,
        },
        output_columns,
    )?;
    let mut clamped_reference = vec![0.0; rows * output_columns];
    accumulate_tile(
        &mut clamped_reference,
        lhs,
        rhs,
        &clamped_lhs,
        &clamped_rhs,
        transpose_lhs,
        transpose_rhs,
        crate::lowering::Tile {
            m_offset: 0,
            n_offset: 0,
            k_offset: 0,
            m: rows,
            n: output_columns,
            k: inner,
        },
        output_columns,
    )?;
    let floating_point_accumulation = compare(&output, &full_precision_accumulation).0;
    let input_quantization = compare(&full_precision_accumulation, &clamped_reference).0;
    let input_clipping = compare(&clamped_reference, reference).0;

    let noise = apply_noise(&output, precision.analog_noise, precision.noise_seed)?;
    output = noise.values;

    let mut calibration_residual = 0.0_f64;
    if let Some(calibration) = &precision.calibration_compensation {
        for value in &mut output {
            let original = *value;
            let measured = original * calibration.measured_gain + calibration.measured_offset;
            *value = measured * calibration.rescale + calibration.rebias;
            calibration_residual = calibration_residual.max((*value - original).abs());
        }
    }

    let output_quantized = quantize(
        &output,
        &[rows, output_columns],
        &precision.output_quantization,
        precision.noise_seed.wrapping_add(2),
    )?;
    let clamped_output = output
        .iter()
        .map(|value| {
            value.clamp(
                precision.output_quantization.clipping_min,
                precision.output_quantization.clipping_max,
            )
        })
        .collect::<Vec<_>>();
    let output_quantization = compare(&output_quantized.dequantized, &clamped_output).0;
    let clipping = input_clipping + compare(&clamped_output, &output).0;
    let error_attribution = ErrorAttribution {
        quantization: input_quantization + output_quantization,
        shot_noise: maximum_absolute(&noise.shot_noise),
        thermal_noise: maximum_absolute(&noise.thermal_noise),
        phase_noise: maximum_absolute(&noise.phase_noise),
        detector_noise: maximum_absolute(&noise.detector_noise),
        calibration_residual,
        floating_point_accumulation,
        integer_overflow: 0.0,
        clipping,
        propagated_input: 0.0,
        total: 0.0,
    }
    .checked_absolute()?;
    let mut provenance = vec![
        format!(
            "quantization: lhs={} bits, rhs={} bits, output={} bits",
            precision.lhs_quantization.bits,
            precision.rhs_quantization.bits,
            precision.output_quantization.bits
        ),
        format!("analog noise: deterministic seed {}", precision.noise_seed),
        format!(
            "accumulation: {:?} with {:?}",
            matching_tiles[0].accumulation_mode, precision.accumulator_dtype
        ),
    ];
    if let Some(calibration) = &precision.calibration_compensation {
        provenance.push(format!(
            "calibration: measured profile {} with inverse transfer compensation",
            calibration.profile_id
        ));
    }
    Ok(SimulatedOutput {
        values: output_quantized.dequantized,
        error_attribution,
        seed: precision.noise_seed,
        provenance,
    })
}

fn compare(actual: &[f64], expected: &[f64]) -> (f64, f64) {
    actual.iter().zip(expected).fold(
        (0.0_f64, 0.0_f64),
        |(max_abs, max_rel), (actual, expected)| {
            let absolute = (actual - expected).abs();
            let relative = absolute / expected.abs().max(1.0e-12);
            (max_abs.max(absolute), max_rel.max(relative))
        },
    )
}
