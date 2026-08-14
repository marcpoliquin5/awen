//! AWEN's hardware-aware tensor-to-photonic compiler.
//!
//! The compiler accepts typed tensor graphs, partitions them across CPU, GPU,
//! and photonic regions, tiles supported GEMMs for matrix-core backends, emits
//! classical Photonic IR and Device IR, and validates numerical behavior with
//! calibrated reference simulation. The crate also exposes the versioned
//! awenBLAS registry with executable CPU references, deterministic accelerator
//! simulation, capability/cost selection, and conformance measurement.

pub mod awenblas;
pub mod calibration;
pub mod capability;
pub mod compiler;
pub mod cost;
pub mod executable;
pub mod ir;
pub mod lowering;
pub mod partition;
pub mod physical_design;
pub mod precision;
pub mod simulator;

pub use awenblas::kernels::{
    benchmark_kernel, execute_reference as execute_kernel_reference,
    execute_simulator as execute_kernel_simulator, select_kernel, CalibrationInput, ComplexValue,
    KernelAttributes, KernelBackendProfile, KernelBenchmarkReport, KernelCandidateTrace,
    KernelCostEstimate, KernelData, KernelDescriptor, KernelExecutionPlan, KernelKind,
    KernelRequest, KernelResult, KernelSimulatorOptions, KernelStructure, KernelTensor,
    PhaseConvention, AWENBLAS_BENCHMARK_VERSION, AWENBLAS_VERSION,
};
pub use calibration::{
    CalibrationArtifactRecord, CalibrationDecisionImpact, CellRemap, EffectiveTransfer,
    CALIBRATION_DECISION_VERSION,
};
pub use capability::{
    AccumulationMode, BackendHealth, BackendSnapshot, BitSlicingMode, CalibrationCell,
    CalibrationChannel, CalibrationEnvironment, CalibrationProfile, CalibrationSpareCell,
    CapabilityNegotiation, DeviceCapabilities, DynamicRange, HealthStatus, MatrixCore,
    OperationCapability, OperationKind, SaturationMode, CALIBRATION_SNAPSHOT_VERSION,
    CAPABILITY_VERSION, HEALTH_VERSION, PLUGIN_ABI_VERSION, RUNTIME_ABI_VERSION,
};
pub use compiler::{
    compile, compile_with_backend, compile_with_cost_model, refresh_for_backend, ArtifactRefresh,
    ArtifactRefreshAction, CompilationArtifact, CompileOptions, ARTIFACT_REFRESH_VERSION,
};
pub use cost::{
    autotune, autotune_with_profile, decide_placement_with_model, estimate_digital_with_context,
    estimate_photonic_plan, estimate_photonic_plan_with_profile, stable_fingerprint_bytes,
    AutotuneOptions, AutotuneResult, CostEstimate, CostModelInputs, DecisionCache, DigitalBaseline,
    EnergyBreakdownUj, EstimateInterval, LatencyBreakdownNs, ModelErrorReport, Observation,
    ObservationSet, OperationCostProfile, OptimizationObjective, ParameterProvenance,
    ParameterSource, PlacementDecision, TargetBackend, TuningCandidate, TuningPlan,
    COST_MODEL_VERSION, OBSERVATION_SET_VERSION,
};
pub use executable::{ExecutableCommand, ExecutablePackage};
pub use ir::{CostHints, DType, GemmShape, Layout, Tensor, TensorOp, TensorProgram};
pub use partition::{
    partition_graph, GraphNode, GraphOpKind, GraphTensor, MemoryPeak, NodeCandidate,
    NodePlacementTrace, PartitionAlternative, PartitionCost, PartitionGraph, PartitionOptions,
    PartitionProfilerEvent, PartitionRegion, PartitionRequest, PartitionTotals, PartitionTrace,
    ProfilerEventKind, TransferRecord, VisualizationEdge, PARTITION_GRAPH_VERSION,
    PARTITION_TRACE_VERSION,
};
pub use physical_design::{
    import_mapping_response, AdapterKind, ArtifactReference, CircuitFramework,
    CircuitModelReference, DataClassification, EvidenceKind, EvidenceStatus, LayoutConstraints,
    LengthUnit, LogicalOperation, LogicalOperationKind, MappingRequest, MappingResponse,
    PdkReference, PhysicalDesignAdapter, PhysicalDesignBinding, PhysicalDesignProvenance,
    PortContract, PortKind, ProcessCorner, ToolIdentity, TopologyConnection, TopologyContract,
    TopologyEndpoint, TopologyNode, VerificationEvidence, WavelengthBand, PHYSICAL_DESIGN_VERSION,
};
pub use precision::{
    accumulate_integer_products, apply_noise, bit_slice_signed, default_quantization, quantize,
    reconstruct_bit_slices, AccumulationResult, AccumulatorDType, AnalogNoiseModel, BitSlicedValue,
    EmpiricalErrorReport, ErrorAttribution, NoiseApplication, OperationPrecisionPolicy,
    OverflowMode, PrecisionConfiguration, PrecisionEncoding, QuantizationSpec, QuantizedTensor,
    RoundingMode, ScaleGranularity, TensorPrecisionPolicy, ERROR_REPORT_VERSION, PRECISION_VERSION,
};
pub use simulator::{benchmark, benchmark_with_observations, BenchmarkReport};
