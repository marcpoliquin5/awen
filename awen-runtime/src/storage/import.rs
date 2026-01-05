//! Import bundles from various formats

use anyhow::Result;
use std::path::Path;

use super::ArtifactBundle;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Import artifact bundle from filesystem
pub fn import_bundle(path: &Path) -> Result<ArtifactBundle> {
    // Read manifest
    let manifest_path = path.join("manifest.json");
    let manifest_content = std::fs::read_to_string(&manifest_path)?;
    let manifest: super::Manifest = serde_json::from_str(&manifest_content)?;

    // If checksums.json exists, validate checksums
    let checksums_path = path.join("checksums.json");
    if checksums_path.exists() {
        let checksums_content = std::fs::read_to_string(&checksums_path)?;
        let expected: HashMap<String, String> = serde_json::from_str(&checksums_content)?;

        for (rel, expected_hex) in expected.iter() {
            let file_path = path.join(rel);
            let mut file = std::fs::File::open(&file_path)?;
            let mut hasher = Sha256::new();
            std::io::copy(&mut file, &mut hasher)?;
            let digest = hasher.finalize();
            let hex_actual = hex::encode(digest);
            if &hex_actual != expected_hex {
                return Err(anyhow::anyhow!("Checksum mismatch for {}", rel));
            }
        }
    }

    // Load core files
    let ir_path = path.join("ir/original.json");
    let ir_content = std::fs::read_to_string(&ir_path)?;
    let ir: crate::ir::Graph = serde_json::from_str(&ir_content)?;

    let params_path = path.join("parameters/initial.json");
    let params_content = std::fs::read_to_string(&params_path)?;
    let parameters_initial: std::collections::HashMap<String, f64> =
        serde_json::from_str(&params_content)?;

    let results_path = path.join("results/outputs.json");
    let results_content = std::fs::read_to_string(&results_path)?;
    let results: serde_json::Value = serde_json::from_str(&results_content)?;

    // Environment snapshot
    let environment_path = path.join("environment/snapshot.json");
    let environment: super::EnvironmentSnapshot = if environment_path.exists() {
        let env_c = std::fs::read_to_string(&environment_path)?;
        serde_json::from_str(&env_c)?
    } else {
        // If not present, capture current environment as a best-effort snapshot
        super::capture_environment()
    };

    // Provenance
    let prov_path = path.join("provenance/lineage.json");
    let provenance: super::ProvenanceData = if prov_path.exists() {
        let p = std::fs::read_to_string(&prov_path)?;
        serde_json::from_str(&p)?
    } else {
        super::ProvenanceData {
            creator: super::CreatorInfo {
                user: None,
                organization: None,
                machine: "unknown".to_string(),
            },
            parent_artifacts: vec![],
            tags: vec![],
            notes: None,
            citation: None,
        }
    };

    // Seed
    let seed_path = path.join("environment/seed.txt");
    let seed = if seed_path.exists() {
        let s = std::fs::read_to_string(&seed_path)?;
        s.trim().parse::<u64>().ok()
    } else {
        None
    };

    // Observability (best-effort)
    let observability = if path.join("provenance/traces.jsonl").exists()
        || path.join("provenance/timeline.json").exists()
    {
        Some(super::ObservabilityData {
            traces: if path.join("provenance/traces.jsonl").exists() {
                Some(path.join("provenance/traces.jsonl"))
            } else {
                None
            },
            timeline: if path.join("provenance/timeline.json").exists() {
                Some(path.join("provenance/timeline.json"))
            } else {
                None
            },
            metrics: if path.join("provenance/metrics.json").exists() {
                Some(path.join("provenance/metrics.json"))
            } else {
                None
            },
            events: if path.join("provenance/events.jsonl").exists() {
                Some(path.join("provenance/events.jsonl"))
            } else {
                None
            },
        })
    } else {
        None
    };

    // Build ArtifactBundle
    let artifact_type = match manifest.artifact_type.as_str() {
        "run" => super::ArtifactType::Run,
        "gradient" => super::ArtifactType::Gradient,
        "calibration" => super::ArtifactType::Calibration,
        "replay" => super::ArtifactType::Replay,
        "validation" => super::ArtifactType::Validation,
        other => return Err(anyhow::anyhow!("Unknown artifact type: {}", other)),
    };

    let bundle = super::ArtifactBundle {
        artifact_id: manifest.artifact_id.clone(),
        artifact_type,
        manifest,
        ir_original: ir,
        ir_lowered: None,
        parameters_initial,
        parameters_final: None,
        calibration_state_initial: None,
        calibration_state_final: None,
        results,
        seed,
        observability,
        environment,
        provenance,
    };

    Ok(bundle)
}
