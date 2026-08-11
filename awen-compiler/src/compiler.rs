use crate::capability::{
    BackendHealth, BackendSnapshot, CapabilityNegotiation, DeviceCapabilities,
};
use crate::cost::{
    decide_placement, DigitalBaseline, OptimizationObjective, PlacementDecision, TargetBackend,
};
use crate::ir::{validate_program, DType, TensorProgram};
use crate::lowering::{lower, ClassicalPhotonicProgram, DeviceProgram};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CompileOptions {
    pub optimize_for: OptimizationObjective,
    pub target: TargetBackend,
    pub cpu_throughput_tops: f64,
    pub cpu_energy_pj_per_mac: f64,
    pub cpu_launch_latency_ns: f64,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            optimize_for: OptimizationObjective::Latency,
            target: TargetBackend::Auto,
            cpu_throughput_tops: 25.0,
            cpu_energy_pj_per_mac: 20.0,
            cpu_launch_latency_ns: 2_500.0,
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
        .map(|(gemm, negotiation)| {
            let mut decision = decide_placement(
                gemm.op.id(),
                gemm.shape,
                gemm.lhs.dtype,
                gemm.op.accuracy().minimum_effective_bits,
                capabilities,
                options.optimize_for,
                options.target,
                digital,
            );
            if !negotiation.eligible {
                decision.selected_backend = TargetBackend::Cpu;
                decision.photonic = None;
                decision.optical_electrical_boundary_crossings = 0;
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
            decision
        })
        .collect();

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
