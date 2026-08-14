// AWEN Runtime crate root
pub mod benchmark;
pub mod calibration;
pub mod chokepoint;
pub mod engine;
pub mod engine_v2;
pub mod executable;
pub mod ffi;
pub mod gradients;
pub mod hal;
pub mod hal_v0;
pub mod ir;
pub mod observability;
pub mod photonic;
pub mod plugins;
pub mod quantum;
pub mod scheduler;
pub mod state;
pub mod storage;

pub use chokepoint::*;
