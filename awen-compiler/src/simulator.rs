use crate::awenblas::{accumulate_tile, reference_gemm};
use crate::compiler::CompilationArtifact;
use crate::cost::{ModelErrorReport, Observation, TargetBackend, COST_MODEL_VERSION};
use crate::ir::{validate_program, Tensor, TensorOp, TensorProgram};
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
        let values = match decision.selected_backend {
            TargetBackend::Photonic => simulate_photonic_gemm(
                gemm.op.id(),
                gemm.lhs,
                gemm.rhs,
                *transpose_lhs,
                *transpose_rhs,
                gemm.shape.n,
                artifact,
            )?,
            TargetBackend::Cpu | TargetBackend::Gpu => reference.clone(),
            TargetBackend::Auto => bail!("compiled artifacts must not contain auto placement"),
        };
        let (max_abs_error, max_rel_error) = compare(&values, &reference);
        let passed = values.iter().zip(&reference).all(|(actual, expected)| {
            (actual - expected).abs()
                <= accuracy.max_abs_error + accuracy.max_rel_error * expected.abs()
        });
        produced.insert(output.clone(), values.clone());
        outputs.push(OutputComparison {
            tensor: output.clone(),
            backend: decision.selected_backend,
            values,
            reference_values: reference,
            max_abs_error,
            max_rel_error,
            passed,
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

#[allow(clippy::too_many_arguments)]
fn simulate_photonic_gemm(
    op_id: &str,
    lhs: &Tensor,
    rhs: &Tensor,
    transpose_lhs: bool,
    transpose_rhs: bool,
    output_columns: usize,
    artifact: &CompilationArtifact,
) -> Result<Vec<f64>> {
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
    let effective_bits = matching_tiles[0].precision.optical_effective_bits;
    let quantized_lhs = quantize(lhs_data, effective_bits);
    let quantized_rhs = quantize(rhs_data, effective_bits);
    let mut output = vec![0.0; rows * output_columns];
    for compiled_tile in matching_tiles {
        accumulate_tile(
            &mut output,
            lhs,
            rhs,
            &quantized_lhs,
            &quantized_rhs,
            transpose_lhs,
            transpose_rhs,
            compiled_tile.tile,
            output_columns,
        )?;
    }

    if let Some(calibration) = &artifact.photonic_ir.calibration {
        // Model the measured transfer function and its compiler-provided
        // inverse compensation. Keeping both steps explicit makes calibration
        // part of executable semantics rather than device setup metadata.
        for value in &mut output {
            let measured = *value * calibration.gain + calibration.offset;
            *value = (measured - calibration.offset) / calibration.gain;
        }
    }
    Ok(output)
}

fn quantize(values: &[f64], effective_bits: u8) -> Vec<f64> {
    let max_abs = values
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    if max_abs == 0.0 {
        return values.to_vec();
    }
    let levels = (2_u64
        .pow(u32::from(effective_bits.saturating_sub(1)))
        .saturating_sub(1)
        .max(1)) as f64;
    values
        .iter()
        .map(|value| (value / max_abs * levels).round() / levels * max_abs)
        .collect()
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
