// AWEN Runtime crate root
pub mod calibration;
pub mod chokepoint;
pub mod control;
pub mod engine;
pub mod engine_v2;
pub mod gradients;
pub mod hal;
pub mod hal_v0;
pub mod ir;
pub mod observability;
pub mod plugins;
pub mod quantum;
pub mod scheduler;
pub mod state;
pub mod storage;

pub use chokepoint::*;

// TODO: Implement core engine types: Engine, ExecutionPlan, StateStore, RunContext
