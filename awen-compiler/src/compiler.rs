use crate::calibration::{artifact_record, derated_matrix_core, CalibrationArtifactRecord};
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
use crate::partition::{
    partition_graph, GraphNode, GraphOpKind, GraphTensor, NodeCandidate, PartitionCost,
    PartitionGraph, PartitionOptions, PartitionRequest, PartitionTrace, PARTITION_GRAPH_VERSION,
};
use crate::physical_design::PhysicalDesignProvenance;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const ARTIFACT_REFRESH_VERSION: &str = "awen.artifact-refresh.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CompileOptions {
    pub optimize_for: OptimizationObjective,
    pub target: TargetBackend,
    pub cpu_throughput_tops: f64,
    pub cpu_energy_pj_per_mac: f64,
    pub cpu_launch_latency_ns: f64,
    pub gpu_throughput_tops: f64,
    pub gpu_energy_pj_per_mac: f64,
    pub gpu_launch_latency_ns: f64,
    pub autotune_seed: u64,
    pub batch_size: usize,
    pub allow_boundary_fusion: bool,
    pub alternative_plans: usize,
    pub queue_depth: usize,
    pub overlap_fraction: f64,
    pub resident_input_fraction: f64,
    pub transfer_bandwidth_gbps: f64,
    pub transfer_latency_ns: f64,
    pub transfer_energy_pj_per_byte: f64,
    pub crossing_penalty_ns: f64,
    pub crossing_penalty_uj: f64,
    pub crossing_error_fraction: f64,
    pub cpu_memory_budget_bytes: u64,
    pub gpu_memory_budget_bytes: u64,
    pub photonic_memory_budget_bytes: u64,
    pub partition_alternatives: usize,
    pub partition_max_search_states: usize,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            optimize_for: OptimizationObjective::Latency,
            target: TargetBackend::Auto,
            cpu_throughput_tops: 25.0,
            cpu_energy_pj_per_mac: 20.0,
            cpu_launch_latency_ns: 2_500.0,
            gpu_throughput_tops: 100.0,
            gpu_energy_pj_per_mac: 10.0,
            gpu_launch_latency_ns: 5_000.0,
            autotune_seed: 0,
            batch_size: 1,
            allow_boundary_fusion: false,
            alternative_plans: 3,
            queue_depth: 0,
            overlap_fraction: 0.0,
            resident_input_fraction: 0.0,
            transfer_bandwidth_gbps: 128.0,
            transfer_latency_ns: 100.0,
            transfer_energy_pj_per_byte: 1.0,
            crossing_penalty_ns: 500.0,
            crossing_penalty_uj: 0.001,
            crossing_error_fraction: 0.0,
            cpu_memory_budget_bytes: u64::MAX,
            gpu_memory_budget_bytes: u64::MAX,
            photonic_memory_budget_bytes: u64::MAX,
            partition_alternatives: 3,
            partition_max_search_states: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompilationArtifact {
    pub artifact_version: String,
    pub source_graph_fingerprint: String,
    pub backend_snapshot_fingerprint: String,
    pub source_ir_version: String,
    pub capability_version: String,
    pub backend_id: String,
    pub physical_design_provenance: PhysicalDesignProvenance,
    pub health_snapshot: BackendHealth,
    pub options: CompileOptions,
    pub capability_negotiations: Vec<CapabilityNegotiation>,
    pub placement: Vec<PlacementDecision>,
    pub partition_trace: PartitionTrace,
    pub photonic_ir: ClassicalPhotonicProgram,
    pub device_ir: DeviceProgram,
    pub calibration_record: Option<CalibrationArtifactRecord>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRefreshAction {
    Reused,
    Recompiled,
    FellBack,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactRefresh {
    pub refresh_version: String,
    pub action: ArtifactRefreshAction,
    pub reasons: Vec<String>,
    pub artifact: CompilationArtifact,
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
    effective_capabilities.matrix_core =
        derated_matrix_core(&snapshot.capabilities, &snapshot.health);
    effective_capabilities.simultaneous_channels = snapshot
        .health
        .available_channels
        .min(snapshot.capabilities.simultaneous_channels);
    let capabilities = &effective_capabilities;
    if options.cpu_throughput_tops <= 0.0
        || options.cpu_energy_pj_per_mac <= 0.0
        || options.cpu_launch_latency_ns < 0.0
        || options.gpu_throughput_tops <= 0.0
        || options.gpu_energy_pj_per_mac <= 0.0
        || options.gpu_launch_latency_ns < 0.0
    {
        bail!("CPU and GPU cost-model parameters must be positive");
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
    let gpu = DigitalBaseline {
        throughput_tops: options.gpu_throughput_tops,
        energy_pj_per_mac: options.gpu_energy_pj_per_mac,
        launch_latency_ns: options.gpu_launch_latency_ns,
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
                gemm.precision_policy
                    .map_or(gemm.lhs.dtype, |policy| policy.compute_dtype),
                gemm.op.accuracy().minimum_effective_bits,
                *transpose_lhs,
                *transpose_rhs,
            )
        })
        .collect();
    let mut placement: Vec<_> = validated
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
                estimated_output_magnitude: estimate_output_magnitude(gemm),
                maximum_absolute_error: Some(accuracy.max_abs_error),
                maximum_relative_error: Some(accuracy.max_rel_error),
                requested_compute_dtype: gemm.precision_policy.map(|policy| policy.compute_dtype),
                requested_accumulator_dtype: gemm
                    .precision_policy
                    .map(|policy| policy.accumulator_dtype),
                allowed_bit_slicing_mode_mask: gemm.precision_policy.map(|policy| {
                    policy
                        .allowed_bit_slicing_modes
                        .iter()
                        .fold(0_u8, |mask, mode| {
                            mask | 1_u8
                                << match mode {
                                    crate::capability::BitSlicingMode::None => 0,
                                    crate::capability::BitSlicingMode::TwosComplement => 1,
                                    crate::capability::BitSlicingMode::SignedMagnitude => 2,
                                }
                        })
                }),
                noise_seed: gemm.precision_policy.map(|policy| policy.stochastic_seed),
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
                gpu,
                tuning,
            )?;
            if !negotiation.eligible {
                if decision.selected_backend == TargetBackend::Photonic {
                    decision.selected_backend = match options.target {
                        TargetBackend::Gpu => TargetBackend::Gpu,
                        _ => TargetBackend::Cpu,
                    };
                }
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

    let partition_request = compiler_partition_request(program, &validated, &placement, options)?;
    let partition_trace = partition_graph(&partition_request)?;
    let placement_by_id = partition_trace
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<HashMap<_, _>>();
    for decision in &mut placement {
        let trace = placement_by_id
            .get(decision.op_id.as_str())
            .expect("partition trace covers every validated operation");
        decision.selected_backend = trace.selected_device;
        if trace.selected_device != TargetBackend::Photonic {
            decision.optical_electrical_boundary_crossings = 0;
        }
        decision.rationale = format!(
            "{}; graph partition: {}",
            decision.rationale, trace.rationale
        );
    }

    let (photonic_ir, device_ir) = lower(
        program,
        &validated,
        &placement,
        capabilities,
        &snapshot.health,
    )?;
    let calibration_record = capabilities.calibration_profile.as_ref().map(|profile| {
        let impacts = photonic_ir
            .ops
            .iter()
            .map(|op| {
                (
                    op.calibration_impact.op_id.clone(),
                    op.calibration_impact.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .collect();
        artifact_record(profile, &snapshot.health, impacts)
    });
    let mut diagnostics = vec![partition_trace.rationale.clone()];
    diagnostics.extend(
        placement
            .iter()
            .map(|decision| {
                format!(
                    "{} -> {:?}: {}",
                    decision.op_id, decision.selected_backend, decision.rationale
                )
            })
            .collect::<Vec<_>>(),
    );
    Ok(CompilationArtifact {
        artifact_version: "awen.compilation.v1".to_string(),
        source_graph_fingerprint: fingerprint_json(program)?,
        backend_snapshot_fingerprint: fingerprint_json(snapshot)?,
        source_ir_version: program.ir_version.clone(),
        capability_version: capabilities.capability_version.clone(),
        backend_id: capabilities.backend_id.clone(),
        physical_design_provenance: capabilities.physical_design.provenance()?,
        health_snapshot: snapshot.health.clone(),
        options,
        capability_negotiations,
        placement,
        partition_trace,
        photonic_ir,
        device_ir,
        calibration_record,
        diagnostics,
    })
}

pub fn refresh_for_backend(
    program: &TensorProgram,
    artifact: &CompilationArtifact,
    current_snapshot: &BackendSnapshot,
) -> Result<ArtifactRefresh> {
    current_snapshot.capabilities.validate()?;
    current_snapshot
        .health
        .validate(&current_snapshot.capabilities)?;
    let source_graph_fingerprint = fingerprint_json(program)?;
    if source_graph_fingerprint != artifact.source_graph_fingerprint {
        bail!("artifact refresh requires the exact source graph used for compilation");
    }
    let current_snapshot_fingerprint = fingerprint_json(current_snapshot)?;
    if current_snapshot_fingerprint == artifact.backend_snapshot_fingerprint {
        return Ok(ArtifactRefresh {
            refresh_version: ARTIFACT_REFRESH_VERSION.to_string(),
            action: ArtifactRefreshAction::Reused,
            reasons: Vec::new(),
            artifact: artifact.clone(),
        });
    }

    let mut reasons = Vec::new();
    if artifact.backend_id != current_snapshot.capabilities.backend_id {
        reasons.push(format!(
            "backend identity changed from '{}' to '{}'",
            artifact.backend_id, current_snapshot.capabilities.backend_id
        ));
    }
    let current_physical_design = current_snapshot.capabilities.physical_design.provenance()?;
    if artifact.physical_design_provenance.pdk_manifest.digest
        != current_physical_design.pdk_manifest.digest
        || artifact.physical_design_provenance.pdk_version != current_physical_design.pdk_version
    {
        reasons.push("PDK identity or version changed".to_string());
    }
    if artifact
        .physical_design_provenance
        .process_corner_fingerprint
        != current_physical_design.process_corner_fingerprint
        || artifact.physical_design_provenance.process_corner_id
            != current_physical_design.process_corner_id
    {
        reasons.push("physical-design process corner changed".to_string());
    }
    if artifact.physical_design_provenance.binding_fingerprint
        != current_physical_design.binding_fingerprint
        && !reasons
            .iter()
            .any(|reason| reason.contains("PDK") || reason.contains("process corner"))
    {
        reasons.push("physical-design binding changed".to_string());
    }
    match (
        artifact.calibration_record.as_ref(),
        current_snapshot.capabilities.calibration_profile.as_ref(),
    ) {
        (Some(previous), Some(current)) => {
            if previous.snapshot_id != current.id {
                reasons.push(format!(
                    "calibration snapshot changed from '{}' to '{}'",
                    previous.snapshot_id, current.id
                ));
            }
            if previous.fingerprint != current.fingerprint {
                reasons.push("calibration fingerprint changed".to_string());
            }
            if previous.topology_fingerprint != current.topology_fingerprint {
                reasons.push("calibration topology fingerprint changed".to_string());
            }
        }
        (Some(_), None) => {
            reasons.push("the current backend has no calibration snapshot".to_string())
        }
        (None, Some(_)) => reasons.push("a calibration snapshot became available".to_string()),
        (None, None) => {}
    }
    if artifact.health_snapshot.observed_at != current_snapshot.health.observed_at {
        reasons.push("backend health observation changed".to_string());
    }
    if artifact.health_snapshot.status != current_snapshot.health.status {
        reasons.push("backend health status changed".to_string());
    }
    if artifact.health_snapshot.drift.to_bits() != current_snapshot.health.drift.to_bits() {
        reasons.push("measured hardware drift changed".to_string());
    }
    if artifact.health_snapshot.temperature_c.to_bits()
        != current_snapshot.health.temperature_c.to_bits()
    {
        reasons.push("measured hardware temperature changed".to_string());
    }
    if artifact.health_snapshot.disabled_components != current_snapshot.health.disabled_components {
        reasons.push("disabled component set changed".to_string());
    }
    if artifact.health_snapshot.unavailable_resources
        != current_snapshot.health.unavailable_resources
    {
        reasons.push("unavailable resource set changed".to_string());
    }
    if reasons.is_empty() {
        reasons.push("backend snapshot fingerprint changed".to_string());
    }

    let mut options = artifact.options;
    options.target = TargetBackend::Auto;
    let mut refreshed = compile_with_backend(program, current_snapshot, options)?;
    refreshed.diagnostics.splice(
        0..0,
        reasons
            .iter()
            .map(|reason| format!("artifact invalidated: {reason}")),
    );
    let fell_back = artifact
        .placement
        .iter()
        .any(|decision| decision.selected_backend == TargetBackend::Photonic)
        && refreshed
            .placement
            .iter()
            .all(|decision| decision.selected_backend != TargetBackend::Photonic);
    Ok(ArtifactRefresh {
        refresh_version: ARTIFACT_REFRESH_VERSION.to_string(),
        action: if fell_back {
            ArtifactRefreshAction::FellBack
        } else {
            ArtifactRefreshAction::Recompiled
        },
        reasons,
        artifact: refreshed,
    })
}

fn fingerprint_json(value: &impl Serialize) -> Result<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(format!("fnv1a64:{:016x}", stable_fingerprint_bytes(&bytes)))
}

fn compiler_partition_request(
    program: &TensorProgram,
    validated: &[crate::ir::ValidatedGemm<'_>],
    placement: &[PlacementDecision],
    options: CompileOptions,
) -> Result<PartitionRequest> {
    let produced = validated
        .iter()
        .map(|gemm| gemm.output.id.as_str())
        .collect::<BTreeSet<_>>();
    let consumed = validated
        .iter()
        .flat_map(|gemm| [gemm.lhs.id.as_str(), gemm.rhs.id.as_str()])
        .collect::<BTreeSet<_>>();
    let tensors = program
        .tensors
        .iter()
        .map(|tensor| {
            let elements = tensor
                .shape
                .iter()
                .try_fold(1_u64, |total, dimension| {
                    total.checked_mul(*dimension as u64)
                })
                .ok_or_else(|| anyhow::anyhow!("tensor '{}' byte size overflows u64", tensor.id))?;
            let bytes = elements
                .checked_mul(u64::from(tensor.dtype.bits()))
                .and_then(|bits| bits.checked_add(7))
                .map(|bits| bits / 8)
                .ok_or_else(|| anyhow::anyhow!("tensor '{}' byte size overflows u64", tensor.id))?;
            Ok(GraphTensor {
                id: tensor.id.clone(),
                bytes,
                initial_device: (!produced.contains(tensor.id.as_str()))
                    .then_some(TargetBackend::Cpu),
                required_device: (!consumed.contains(tensor.id.as_str()))
                    .then_some(TargetBackend::Cpu),
                persistent: !produced.contains(tensor.id.as_str()),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let nodes = validated
        .iter()
        .zip(placement)
        .map(|(gemm, decision)| {
            let allows = |device| options.target == TargetBackend::Auto || options.target == device;
            let operations = 2.0
                * gemm.shape.m as f64
                * gemm.shape.n as f64
                * gemm.shape.k as f64
                * options.batch_size as f64
                * if gemm.op.cost_hints().structured_sparsity {
                    1.0 - gemm.op.cost_hints().sparsity_fraction
                } else {
                    1.0
                };
            let mut candidates = vec![
                NodeCandidate {
                    device: TargetBackend::Cpu,
                    eligible: allows(TargetBackend::Cpu),
                    cost: allows(TargetBackend::Cpu)
                        .then(|| graph_internal_cost(&decision.cpu, operations, false)),
                    reason: if allows(TargetBackend::Cpu) {
                        "CPU reference implementation satisfies the numerical contract".to_string()
                    } else {
                        "excluded by the explicit compilation target".to_string()
                    },
                },
                NodeCandidate {
                    device: TargetBackend::Gpu,
                    eligible: allows(TargetBackend::Gpu),
                    cost: allows(TargetBackend::Gpu)
                        .then(|| graph_internal_cost(&decision.gpu, operations, false)),
                    reason: if allows(TargetBackend::Gpu) {
                        "GPU digital implementation satisfies the numerical contract".to_string()
                    } else {
                        "excluded by the explicit compilation target".to_string()
                    },
                },
            ];
            let photonic_eligible = allows(TargetBackend::Photonic) && decision.photonic.is_some();
            candidates.push(NodeCandidate {
                device: TargetBackend::Photonic,
                eligible: photonic_eligible,
                cost: decision
                    .photonic
                    .as_ref()
                    .filter(|_| photonic_eligible)
                    .map(|estimate| graph_internal_cost(estimate, operations, true)),
                reason: if photonic_eligible {
                    "capability negotiation and the numerical contract admit a photonic plan"
                        .to_string()
                } else if options.target != TargetBackend::Auto
                    && options.target != TargetBackend::Photonic
                {
                    "excluded by the explicit compilation target".to_string()
                } else {
                    "photonic capability or numerical-contract requirements are not satisfied"
                        .to_string()
                },
            });
            let crate::ir::TensorOp::Gemm {
                lhs, rhs, output, ..
            } = gemm.op;
            GraphNode {
                id: gemm.op.id().to_string(),
                kind: GraphOpKind::Gemm,
                inputs: vec![lhs.clone(), rhs.clone()],
                outputs: vec![output.clone()],
                dynamic_shape: false,
                control_flow_barrier: false,
                candidates,
            }
        })
        .collect();
    Ok(PartitionRequest {
        graph: PartitionGraph {
            graph_version: PARTITION_GRAPH_VERSION.to_string(),
            tensors,
            nodes,
        },
        options: PartitionOptions {
            objective: options.optimize_for,
            seed: options.autotune_seed,
            transfer_bandwidth_gbps: options.transfer_bandwidth_gbps,
            transfer_latency_ns: options.transfer_latency_ns,
            transfer_energy_pj_per_byte: options.transfer_energy_pj_per_byte,
            crossing_penalty_ns: options.crossing_penalty_ns,
            crossing_penalty_uj: options.crossing_penalty_uj,
            crossing_error_fraction: options.crossing_error_fraction,
            cpu_memory_budget_bytes: options.cpu_memory_budget_bytes,
            gpu_memory_budget_bytes: options.gpu_memory_budget_bytes,
            photonic_memory_budget_bytes: options.photonic_memory_budget_bytes,
            alternatives: options.partition_alternatives,
            max_search_states: options.partition_max_search_states,
        },
    })
}

fn estimate_output_magnitude(gemm: &crate::ir::ValidatedGemm<'_>) -> Option<f64> {
    let crate::ir::TensorOp::Gemm {
        transpose_lhs,
        transpose_rhs,
        ..
    } = gemm.op;
    crate::awenblas::reference_gemm(
        gemm.lhs,
        gemm.rhs,
        *transpose_lhs,
        *transpose_rhs,
        gemm.shape.m,
        gemm.shape.n,
        gemm.shape.k,
    )
    .ok()
    .map(|values| {
        values
            .iter()
            .fold(0.0_f64, |maximum, value| maximum.max(value.abs()))
    })
}

fn graph_internal_cost(
    estimate: &crate::cost::CostEstimate,
    operations: f64,
    photonic: bool,
) -> PartitionCost {
    let excluded_latency = estimate.latency_breakdown_ns.host_transfer
        + if photonic {
            estimate.latency_breakdown_ns.boundary_conversion
                + estimate.latency_breakdown_ns.dac
                + estimate.latency_breakdown_ns.adc
        } else {
            0.0
        };
    let excluded_energy = estimate.energy_breakdown_uj.host_transfer
        + if photonic {
            estimate.energy_breakdown_uj.dac + estimate.energy_breakdown_uj.adc
        } else {
            0.0
        };
    let source = estimate
        .provenance
        .iter()
        .map(|item| item.source)
        .max_by_key(|source| match source {
            ParameterSource::Measured => 0,
            ParameterSource::VendorSpecified => 1,
            ParameterSource::Simulated => 2,
            ParameterSource::Assumed => 3,
        })
        .unwrap_or(ParameterSource::Assumed);
    PartitionCost {
        latency_ns: (estimate.latency_ns - excluded_latency).max(f64::EPSILON),
        energy_uj: (estimate.energy_uj - excluded_energy).max(0.0),
        error_fraction: estimate.estimated_error_fraction,
        operations,
        source,
    }
}
