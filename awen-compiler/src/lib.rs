//! AWEN's hardware-aware tensor-to-photonic compiler.
//!
//! The executable vertical slice accepts a typed tensor GEMM program, partitions
//! the whole graph across CPU, GPU, and photonic regions, tiles supported GEMMs
//! for a matrix-core backend, emits classical Photonic IR and Device IR, and
//! validates numerical behavior with the calibrated reference simulator.

pub mod awenblas;
pub mod capability;
pub mod compiler;
pub mod cost;
pub mod executable;
pub mod ir;
pub mod lowering;
pub mod partition;
pub mod simulator;

pub use capability::{
    BackendHealth, BackendSnapshot, BitSlicingMode, CalibrationProfile, CapabilityNegotiation,
    DeviceCapabilities, DynamicRange, HealthStatus, MatrixCore, OperationCapability, OperationKind,
    SaturationMode, CAPABILITY_VERSION, HEALTH_VERSION, PLUGIN_ABI_VERSION, RUNTIME_ABI_VERSION,
};
pub use compiler::{
    compile, compile_with_backend, compile_with_cost_model, CompilationArtifact, CompileOptions,
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
pub use simulator::{benchmark, benchmark_with_observations, BenchmarkReport};
