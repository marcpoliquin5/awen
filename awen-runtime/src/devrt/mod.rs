//! Device runtime services layered on top of the existing HAL, scheduler and
//! simulator (directive item C8). Nothing here replaces existing modules; these
//! types coordinate work that the existing backends execute.
//!
//! * [`stream`]: ordered asynchronous work queues with events for cross-stream
//!   dependencies, one worker thread per stream.
//! * [`residency`]: per-device memory residency tracking with capacity limits,
//!   LRU eviction and transfer accounting.
//! * [`lease`]: exclusive/shared device leases with logical-clock expiry and
//!   resource budgets.
//! * [`remap`]: health-aware placement on simulated devices with CPU fallback.
//! * [`record`]: deterministic seeding, experiment recording, bit-exact replay.
//! * [`profile`]: host wall-clock spans, summaries and Chrome trace export.
//!
//! All quantities are bookkeeping values supplied by callers or by the
//! simulator. They are not hardware measurements.

pub mod lease;
pub mod profile;
pub mod record;
pub mod remap;
pub mod residency;
pub mod stream;
