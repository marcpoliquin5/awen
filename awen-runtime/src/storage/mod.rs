//! Artifact storage and reproducibility infrastructure
//!
//! Hermetically sealed artifact bundles with complete provenance, deterministic IDs,
//! and round-trip import/export for publication-ready reproducibility.

// Public module structure
pub mod bundle;
pub mod deterministic_id;
pub mod environment;
pub mod export;
pub mod import;
pub mod manifest;

// Re-export key types for ergonomics
pub use bundle::{ArtifactBundle, ArtifactType, BundleBuilder, ObservabilityData, ProvenanceData, CreatorInfo, EnvironmentSnapshot};
pub use deterministic_id::{compute_deterministic_id, short_id};
pub use environment::{capture_environment, RuntimeInfo, SystemInfo, DeviceInfo, DeviceCapabilities};
pub use export::{export_bundle, ExportFormat};
pub use import::import_bundle;
pub use manifest::Manifest;
pub use self::ReplayComponents;

/// Components needed for deterministic replay
#[derive(Clone, Debug)]
pub struct ReplayComponents {
    pub ir: crate::ir::Graph,
    pub parameters: std::collections::HashMap<String, f64>,
    pub seed: Option<u64>,
    pub environment: EnvironmentSnapshot,
}

use std::path::{Path, PathBuf};
use anyhow::Result;

/// Initialize artifact storage
///
/// Creates artifact directory structure if it doesn't exist
pub fn initialize_storage(artifacts_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(artifacts_dir)?;
    Ok(())
}

/// Save artifact bundle to persistent storage
///
/// Exports the bundle to a standardized directory structure,
/// generating all required metadata and checksums.
pub fn save_artifact(bundle: &ArtifactBundle, artifacts_dir: &Path) -> Result<PathBuf> {
    initialize_storage(artifacts_dir)?;
    export_bundle(bundle, artifacts_dir, ExportFormat::Directory)
}

/// Load artifact bundle for deterministic replay
///
/// Loads a previously saved artifact bundle and returns the components
/// needed to replay the execution: IR, parameters, seed, and environment.
pub fn load_artifact_for_replay(artifact_path: &Path) -> Result<ReplayComponents> {
    let bundle = import_bundle(artifact_path)?;
    Ok(ReplayComponents {
        ir: bundle.ir,
        parameters: bundle.parameters,
        seed: bundle.seed,
        environment: bundle.environment,
    })
}
