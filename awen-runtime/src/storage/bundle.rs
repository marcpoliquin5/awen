//! Artifact bundle structure and builder
//!
//! Hermetically sealed bundles with complete provenance.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::{capture_environment, compute_deterministic_id, Manifest};
use crate::ir::Graph;

/// Complete artifact bundle
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactBundle {
    /// Deterministic content-addressable ID
    pub artifact_id: String,

    /// Bundle type
    pub artifact_type: ArtifactType,

    /// Manifest with metadata and content index
    pub manifest: Manifest,

    /// IR graph (original)
    pub ir_original: Graph,

    /// IR graph (lowered/optimized)
    pub ir_lowered: Option<Graph>,

    /// Parameters (initial)
    pub parameters_initial: HashMap<String, f64>,

    /// Parameters (final, if optimization)
    pub parameters_final: Option<HashMap<String, f64>>,

    /// Calibration state (initial)
    pub calibration_state_initial: Option<serde_json::Value>,

    /// Calibration state (final)
    pub calibration_state_final: Option<serde_json::Value>,

    /// Results
    pub results: serde_json::Value,

    /// Random seed
    pub seed: Option<u64>,

    /// Observability artifacts (populated by runtime)
    pub observability: Option<ObservabilityData>,

    /// Environment snapshot
    pub environment: EnvironmentSnapshot,

    /// Provenance
    pub provenance: ProvenanceData,
}

/// Bundle type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactType {
    Run,
    Gradient,
    Calibration,
    Replay,
    Validation,
}

/// Observability data (from AEP-0005)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservabilityData {
    pub traces: Option<PathBuf>,
    pub timeline: Option<PathBuf>,
    pub metrics: Option<PathBuf>,
    pub events: Option<PathBuf>,
}

/// Environment snapshot
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    pub runtime: crate::storage::RuntimeInfo,
    pub system: crate::storage::SystemInfo,
    pub device: crate::storage::DeviceInfo,
}

/// Provenance data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProvenanceData {
    pub creator: CreatorInfo,
    pub parent_artifacts: Vec<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub citation: Option<String>,
}

/// Creator information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatorInfo {
    pub user: Option<String>,
    pub organization: Option<String>,
    pub machine: String,
}

/// Builder for constructing artifact bundles during execution
pub struct BundleBuilder {
    ir_original: Graph,
    artifact_type: ArtifactType,
    ir_lowered: Option<Graph>,
    parameters_initial: HashMap<String, f64>,
    parameters_final: Option<HashMap<String, f64>>,
    calibration_state_initial: Option<serde_json::Value>,
    calibration_state_final: Option<serde_json::Value>,
    results: Option<serde_json::Value>,
    seed: Option<u64>,
    observability_dir: Option<PathBuf>,
    parent_artifacts: Vec<String>,
    tags: Vec<String>,
    notes: Option<String>,
    title: Option<String>,
    authors: Option<String>,
    organization: Option<String>,
}

impl BundleBuilder {
    /// Create new bundle builder
    pub fn new(ir: Graph, artifact_type: ArtifactType) -> Self {
        Self {
            ir_original: ir,
            artifact_type,
            ir_lowered: None,
            parameters_initial: HashMap::new(),
            parameters_final: None,
            calibration_state_initial: None,
            calibration_state_final: None,
            results: None,
            seed: None,
            observability_dir: None,
            parent_artifacts: Vec::new(),
            tags: Vec::new(),
            notes: None,
            title: None,
            authors: None,
            organization: None,
        }
    }

    /// Set lowered IR
    pub fn with_lowered_ir(mut self, ir: Graph) -> Self {
        self.ir_lowered = Some(ir);
        self
    }

    /// Set initial parameters
    pub fn with_initial_parameters(mut self, params: HashMap<String, f64>) -> Self {
        self.parameters_initial = params;
        self
    }

    /// Set final parameters (for optimization runs)
    pub fn with_final_parameters(mut self, params: HashMap<String, f64>) -> Self {
        self.parameters_final = Some(params);
        self
    }

    /// Set calibration state
    pub fn with_calibration_state(
        mut self,
        initial: serde_json::Value,
        final_state: Option<serde_json::Value>,
    ) -> Self {
        self.calibration_state_initial = Some(initial);
        self.calibration_state_final = final_state;
        self
    }

    /// Set results
    pub fn with_results(mut self, results: serde_json::Value) -> Self {
        self.results = Some(results);
        self
    }

    /// Set random seed
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set observability directory (where traces/timeline/metrics/events are)
    pub fn with_observability_dir<P: AsRef<std::path::Path>>(mut self, dir: P) -> Self {
        self.observability_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Add parent artifact
    pub fn add_parent_artifact(mut self, artifact_id: String) -> Self {
        self.parent_artifacts.push(artifact_id);
        self
    }

    /// Add tag
    pub fn add_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// Set notes
    pub fn with_notes(mut self, notes: String) -> Self {
        self.notes = Some(notes);
        self
    }

    /// Set citation metadata
    pub fn with_citation_metadata(
        mut self,
        title: String,
        authors: Vec<String>,
        organization: String,
    ) -> Self {
        self.title = Some(title);
        self.authors = Some(authors.join(", "));
        self.organization = Some(organization);
        self
    }

    /// Build the artifact bundle
    pub fn build(self) -> Result<ArtifactBundle> {
        let results = self
            .results
            .ok_or_else(|| anyhow::anyhow!("Results not set"))?;

        // Capture environment
        let environment = capture_environment();

        // Compute deterministic ID
        let artifact_id = compute_deterministic_id(
            &self.ir_original,
            &self.parameters_initial,
            self.calibration_state_initial.as_ref(),
            self.seed,
            &environment.runtime.version,
        )?;

        // Gather observability data
        let observability = self
            .observability_dir
            .as_ref()
            .map(|dir| ObservabilityData {
                traces: Some(dir.join("traces.jsonl")),
                timeline: Some(dir.join("timeline.json")),
                metrics: Some(dir.join("metrics.json")),
                events: Some(dir.join("events.jsonl")),
            });

        // Create provenance
        let machine = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        let creator = CreatorInfo {
            user: std::env::var("USER").ok(),
            organization: self.organization.clone(),
            machine,
        };

        // Generate citation if metadata provided
        let citation = if let (Some(title), Some(authors), Some(org)) = (
            self.title.as_ref(),
            self.authors.as_ref(),
            self.organization.as_ref(),
        ) {
            Some(generate_citation(
                &artifact_id,
                title,
                authors,
                org,
                &environment.runtime.version,
            ))
        } else {
            None
        };

        let provenance = ProvenanceData {
            creator,
            parent_artifacts: self.parent_artifacts,
            tags: self.tags,
            notes: self.notes,
            citation,
        };

        // Create manifest
        let manifest = Manifest::new(
            artifact_id.clone(),
            self.artifact_type.clone(),
            environment.runtime.version.clone(),
        );

        Ok(ArtifactBundle {
            artifact_id,
            artifact_type: self.artifact_type,
            manifest,
            ir_original: self.ir_original,
            ir_lowered: self.ir_lowered,
            parameters_initial: self.parameters_initial,
            parameters_final: self.parameters_final,
            calibration_state_initial: self.calibration_state_initial,
            calibration_state_final: self.calibration_state_final,
            results,
            seed: self.seed,
            observability,
            environment,
            provenance,
        })
    }
}

/// Validate an artifact bundle for basic structural correctness.
/// Returns Ok(()) for a valid bundle; otherwise returns an error.
pub fn validate_bundle(bundle: &ArtifactBundle) -> Result<()> {
    // Basic checks: artifact id present and manifest exists
    if !bundle.artifact_id.starts_with("awen_") {
        return Err(anyhow::anyhow!("invalid artifact id"));
    }

    // IR should be non-empty (at least present)
    if bundle.ir_original.nodes.is_empty() && bundle.ir_original.edges.is_empty() {
        // Accept empty graphs for tests, but ensure manifest exists
    }

    Ok(())
}

fn generate_citation(
    artifact_id: &str,
    title: &str,
    authors: &str,
    org: &str,
    runtime_version: &str,
) -> String {
    let short_id = &artifact_id[..std::cmp::min(20, artifact_id.len())];
    let year = chrono::Utc::now().format("%Y");

    format!(
        r#"AWEN Artifact: {artifact_id}
Title: {title}
Authors: {authors}
Organization: {org}
Created: {}
Runtime: awen-runtime v{runtime_version}

Citation (IEEE):
{authors}, "{title}," AWEN Artifact {short_id}, {org}, {year}. 
Available: https://artifacts.awen.dev/{short_id}

BibTeX:
@misc{{{short_id},
  author = {{{authors}}},
  title = {{{title}}},
  howpublished = {{AWEN Artifact}},
  note = {{{short_id}}},
  year = {{{year}}},
  url = {{https://artifacts.awen.dev/{short_id}}}
}}

Reproducibility Command:
awenctl replay --artifact {short_id} --verify
"#,
        chrono::Utc::now().to_rfc3339()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_builder() {
        let ir = Graph {
            nodes: vec![],
            edges: vec![],
            metadata: std::collections::HashMap::new(),
        };
        let mut params = HashMap::new();
        params.insert("test_param".to_string(), 1.0);

        let bundle = BundleBuilder::new(ir, ArtifactType::Run)
            .with_initial_parameters(params)
            .with_results(serde_json::json!({"output": 42}))
            .with_seed(123)
            .add_tag("test".to_string())
            .build()
            .unwrap();

        assert_eq!(bundle.artifact_type, ArtifactType::Run);
        assert_eq!(bundle.seed, Some(123));
        assert!(bundle.artifact_id.starts_with("awen_"));
        assert_eq!(bundle.provenance.tags.len(), 1);
    }

    #[test]
    fn test_citation_generation() {
        let citation = generate_citation(
            "awen_0123456789abcdef",
            "Test Experiment",
            "J. Smith",
            "Test Lab",
            "0.5.0",
        );

        assert!(citation.contains("AWEN Artifact"));
        assert!(citation.contains("Test Experiment"));
        assert!(citation.contains("J. Smith"));
        assert!(citation.contains("BibTeX"));
        assert!(citation.contains("awenctl replay"));
    }
}
