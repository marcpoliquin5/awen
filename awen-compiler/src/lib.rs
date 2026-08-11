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
pub mod ir;
pub mod lowering;
pub mod simulator;

pub use capability::{CalibrationProfile, DeviceCapabilities, MatrixCore};
pub use compiler::{compile, CompilationArtifact, CompileOptions};
pub use cost::{OptimizationObjective, TargetBackend};
pub use ir::{DType, Layout, Tensor, TensorOp, TensorProgram};
pub use simulator::{benchmark, BenchmarkReport};
