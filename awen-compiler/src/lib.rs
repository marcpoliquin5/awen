//! AWEN's hardware-aware tensor-to-photonic compiler.
//!
//! The first executable vertical slice accepts a typed tensor GEMM program,
//! selects CPU or photonic placement, tiles supported GEMMs for a matrix-core
//! backend, emits classical Photonic IR and Device IR, and validates numerical
//! behavior with the calibrated reference simulator.

pub mod awenblas;
pub mod capability;
pub mod compiler;
pub mod cost;
pub mod executable;
pub mod ir;
pub mod lowering;
pub mod simulator;

pub use capability::{
    BackendHealth, BackendSnapshot, BitSlicingMode, CalibrationProfile, CapabilityNegotiation,
    DeviceCapabilities, DynamicRange, HealthStatus, MatrixCore, OperationCapability, OperationKind,
    SaturationMode, CAPABILITY_VERSION, HEALTH_VERSION, PLUGIN_ABI_VERSION, RUNTIME_ABI_VERSION,
};
pub use compiler::{compile, compile_with_backend, CompilationArtifact, CompileOptions};
pub use cost::{OptimizationObjective, TargetBackend};
pub use executable::{ExecutableCommand, ExecutablePackage};
pub use ir::{DType, Layout, Tensor, TensorOp, TensorProgram};
pub use simulator::{benchmark, BenchmarkReport};
