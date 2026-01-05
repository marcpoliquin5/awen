//! Artifact manifest schema

use serde::{Serialize, Deserialize};

/// Artifact bundle manifest
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: String,
    pub artifact_id: String,
    pub artifact_type: String,
    pub created_at: String,
    pub awen_runtime_version: String,
    pub conformance_level: String,
    pub determinism_guarantee: String,
    pub contents: ContentIndex,
    pub inputs: InputsHash,
    pub outputs: OutputsHash,
    pub provenance: ProvisionInfo,
}

impl Manifest {
    pub fn new(artifact_id: String, artifact_type: super::ArtifactType, runtime_version: String) -> Self {
        Self {
            schema_version: "awen_artifact.v0.2".to_string(),
            artifact_id,
            artifact_type: format!("{:?}", artifact_type).to_lowercase(),
            created_at: chrono::Utc::now().to_rfc3339(),
            awen_runtime_version: runtime_version,
            conformance_level: "full".to_string(),
            determinism_guarantee: "bit-exact".to_string(),
            contents: ContentIndex::default(),
            inputs: InputsHash::default(),
            outputs: OutputsHash::default(),
            provenance: ProvisionInfo::default(),
        }
    }
}

/// Content index (files in bundle)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ContentIndex {
    pub ir: Vec<String>,
    pub parameters: Vec<String>,
    pub calibration: Vec<String>,
    pub environment: Vec<String>,
    pub execution: Vec<String>,
    pub results: Vec<String>,
    pub provenance: Vec<String>,
}

/// Input hashes for deterministic ID
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct InputsHash {
    pub ir_hash: Option<String>,
    pub parameters_hash: Option<String>,
    pub calibration_hash: Option<String>,
    pub seed: Option<u64>,
}

/// Output hashes
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OutputsHash {
    pub results_hash: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

/// Provenance information
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProvisionInfo {
    pub creator: Option<CreatorData>,
    pub parent_artifacts: Vec<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatorData {
    pub user: Option<String>,
    pub organization: Option<String>,
    pub machine: String,
}
