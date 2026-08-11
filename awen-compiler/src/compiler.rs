use crate::capability::{
    BackendHealth, BackendSnapshot, CapabilityNegotiation, DeviceCapabilities,
};
use crate::cost::{
    decide_placement_with_model, stable_fingerprint_bytes, AutotuneOptions, CostModelInputs,
    DigitalBaseline, OperationCostProfile, OptimizationObjective, ParameterSource,
    PlacementDecision, TargetBackend,
};
use crate::ir::{validate_program, DType, TensorProgram};
use crate::lowering::{lower, ClassicalPhotonicProgram, DeviceProgram};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CompileOptions {
    pub optimize_for: OptimizationObjective,
    pub target: TargetBackend,
    pub cpu_throughput_tops: f64,
    pub cpu_energy_pj_per_mac: f64,
    pub cpu_launch_latency_ns: f64,
    pub autotune_seed: u64,
    pub batch_size: usize,
    pub allow_boundary_fusion: bool,
    pub alternative_plans: usize,
    pub queue_depth: usize,
    pub overlap_fraction: f64,
    pub resident_input_fraction: f64,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            optimize_for: OptimizationObjective::Latency,
            target: TargetBackend::Auto,
            cpu_throughput_tops: 25.0,
            cpu_energy_pj_per_mac: 20.0,
            cpu_launch_latency_ns: 2_500.0,
            autotune_seed: 0,
            batch_size: 1,
            allow_boundary_fusion: false,
            alternative_plans: 3,
            queue_depth: 0,
            overlap_fraction: 0.0,
            resident_input_fraction: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompilationArtifact {
    pub artifact_version: String,
    pub source_ir_version: String,
    pub capability_version: String,
    pub backend_id: String,
    pub health_snapshot: BackendHealth,
    pub options: CompileOptions,
    pub capability_negotiations: Vec<CapabilityNegotiation>,
    pub placement: Vec<PlacementDecision>,
    pub photonic_ir: ClassicalPhotonicProgram,
    pub device_ir: DeviceProgram,
    pub diagnostics: Vec<String>,
}

pub fn compile(
    program: &TensorProgram,
    capabilities: &DeviceCapabilities,
    options: CompileOptions,
) -> Result<CompilationArtifact> {
    let snapshot = BackendSnapshot::offline(capabilities.clone())?;
    compile_with_backend(program, &snapshot, options)
}

pub fn compile_with_backend(
    program: &TensorProgram,
    snapshot: &BackendSnapshot,
    options: CompileOptions,
) -> Result<CompilationArtifact> {
    let source = if snapshot.capabilities.backend_id.starts_with("reference-") {
        ParameterSource::Simulated
    } else {
        ParameterSource::Assumed
    };
    let cost_model =
        CostModelInputs::from_snapshot(&snapshot.capabilities, &snapshot.health, source);
    compile_with_cost_model(program, snapshot, &cost_model, options)
}

pub fn compile_with_cost_model(
    program: &TensorProgram,
    snapshot: &BackendSnapshot,
    cost_model: &CostModelInputs,
    options: CompileOptions,
) -> Result<CompilationArtifact> {
    snapshot.capabilities.validate()?;
    snapshot.health.validate(&snapshot.capabilities)?;
    let mut effective_capabilities = snapshot.capabilities.clone();
    effective_capabilities.simultaneous_channels = snapshot
        .health
        .available_channels
        .min(snapshot.capabilities.simultaneous_channels);
    let capabilities = &effective_capabilities;
    if options.cpu_throughput_tops <= 0.0
        || options.cpu_energy_pj_per_mac <= 0.0
        || options.cpu_launch_latency_ns < 0.0
    {
        bail!("CPU cost-model parameters must be positive");
    }
    if options.batch_size == 0 {
        bail!("autotuner batch size must be non-zero");
    }
    for (value, name) in [
        (options.overlap_fraction, "overlap fraction"),
        (options.resident_input_fraction, "resident input fraction"),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            bail!("{name} must be finite and within [0, 1]");
        }
    }
    let validated = validate_program(program)?;
    if validated
        .iter()
        .any(|gemm| gemm.lhs.dtype == DType::ComplexF32)
    {
        bail!("complex GEMM is represented in Tensor IR but is not executable in compiler v0.1");
    }
    let digital = DigitalBaseline {
        throughput_tops: options.cpu_throughput_tops,
        energy_pj_per_mac: options.cpu_energy_pj_per_mac,
        launch_latency_ns: options.cpu_launch_latency_ns,
        effective_bits: 16,
    };
    let tuning = AutotuneOptions {
        graph_fingerprint: stable_fingerprint_bytes(&serde_json::to_vec(program)?),
        seed: options.autotune_seed,
        batch_size: options.batch_size,
        allow_boundary_fusion: options.allow_boundary_fusion,
        alternatives: options.alternative_plans,
        queue_depth: options.queue_depth,
        overlap_fraction: options.overlap_fraction,
        resident_input_fraction: options.resident_input_fraction,
    };
    let capability_negotiations: Vec<_> = validated
        .iter()
        .map(|gemm| {
            let crate::ir::TensorOp::Gemm {
                transpose_lhs,
                transpose_rhs,
                ..
            } = gemm.op;
            snapshot.negotiate_gemm(
                gemm.shape,
                gemm.lhs.dtype,
                gemm.op.accuracy().minimum_effective_bits,
                *transpose_lhs,
                *transpose_rhs,
            )
        })
        .collect();
    let placement: Vec<_> = validated
        .iter()
        .zip(&capability_negotiations)
        .map(|(gemm, negotiation)| -> Result<PlacementDecision> {
            let accuracy = gemm.op.accuracy();
            let cost_hints = gemm.op.cost_hints();
            let profile = OperationCostProfile {
                lhs_layout: gemm.lhs.layout,
                rhs_layout: gemm.rhs.layout,
                output_layout: gemm.output.layout,
                sparsity_fraction: cost_hints.sparsity_fraction,
                structured_sparsity: cost_hints.structured_sparsity,
                input_error_fraction: cost_hints.input_error_fraction,
                maximum_input_magnitude: cost_hints.maximum_input_magnitude,
                maximum_absolute_error: Some(accuracy.max_abs_error),
                maximum_relative_error: Some(accuracy.max_rel_error),
            };
            let mut decision = decide_placement_with_model(
                gemm.op.id(),
                gemm.shape,
                gemm.lhs.dtype,
                accuracy.minimum_effective_bits,
                capabilities,
                cost_model,
                profile,
                options.optimize_for,
                options.target,
                digital,
                tuning,
            )?;
            if !negotiation.eligible {
                decision.selected_backend = TargetBackend::Cpu;
                decision.photonic = None;
                decision.selected_plan = None;
                decision.alternatives.clear();
                decision.optical_electrical_boundary_crossings = 0;
                decision.tile_count = 0;
                decision.rationale = format!(
                    "photonic capability negotiation rejected the operation: {}",
                    negotiation
                        .diagnostics
                        .iter()
                        .map(|diagnostic| {
                            format!("{} ({})", diagnostic.message, diagnostic.code)
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                );
            }
            Ok(decision)
        })
        .collect::<Result<Vec<_>>>()?;

    if options.target == TargetBackend::Photonic {
        let rejected: Vec<_> = placement
            .iter()
            .filter(|decision| decision.selected_backend != TargetBackend::Photonic)
            .collect();
        if !rejected.is_empty() {
            let reasons = rejected
                .iter()
                .map(|decision| format!("{}: {}", decision.op_id, decision.rationale))
                .collect::<Vec<_>>()
                .join("; ");
            bail!("forced photonic compilation failed: {reasons}");
        }
    }

    let (photonic_ir, device_ir) = lower(program, &validated, &placement, capabilities)?;
    let diagnostics = placement
        .iter()
        .map(|decision| {
            format!(
                "{} -> {:?}: {}",
                decision.op_id, decision.selected_backend, decision.rationale
            )
        })
        .collect();
    Ok(CompilationArtifact {
        artifact_version: "awen.compilation.v1".to_string(),
        source_ir_version: program.ir_version.clone(),
        capability_version: capabilities.capability_version.clone(),
        backend_id: capabilities.backend_id.clone(),
        health_snapshot: snapshot.health.clone(),
        options,
        capability_negotiations,
        placement,
        photonic_ir,
        device_ir,
        diagnostics,
    })
}
