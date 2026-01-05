//! Import and validate artifact bundles

use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;

use super::{ArtifactBundle, ArtifactType, Manifest, EnvironmentSnapshot, ProvenanceData, ObservabilityData};
use crate::ir::Graph;

/// Import artifact bundle from directory or tarball
pub fn import_bundle(path: &Path) -> Result<ArtifactBundle> {
    if path.is_dir() {
        import_from_directory(path)
    } else if path.extension().map_or(false, |ext| ext == "gz" || ext == "tar") {
        import_from_tarball(path)
    } else {
        bail!("Unsupported bundle format: {:?}", path);
    }
}

/// Import from directory
fn import_from_directory(bundle_dir: &Path) -> Result<ArtifactBundle> {
    // Load manifest first
    let manifest: Manifest = read_json(&bundle_dir.join("manifest.json"))?;
    
    // Validate checksums
    if bundle_dir.join("checksums.json").exists() {
        validate_checksums(bundle_dir)?;
    }
    
    // Load IR
    let ir_original: Graph = read_json(&bundle_dir.join("ir/original.json"))?;
    let ir_lowered: Option<Graph> = read_json_optional(&bundle_dir.join("ir/lowered.json"))?;
    
    // Load parameters
    let parameters_initial: HashMap<String, f64> = read_json(&bundle_dir.join("parameters/initial.json"))?;
    let parameters_final: Option<HashMap<String, f64>> = read_json_optional(&bundle_dir.join("parameters/final.json"))?;
    
    // Load calibration
    let calibration_state_initial: Option<serde_json::Value> = read_json_optional(&bundle_dir.join("calibration/initial.json"))?;
    let calibration_state_final: Option<serde_json::Value> = read_json_optional(&bundle_dir.join("calibration/final.json"))?;
    
    // Load environment
    let environment: EnvironmentSnapshot = read_json(&bundle_dir.join("environment/snapshot.json"))?;
    let seed: Option<u64> = if bundle_dir.join("environment/seed.txt").exists() {
        let seed_str = fs::read_to_string(bundle_dir.join("environment/seed.txt"))?;
        Some(seed_str.trim().parse()?)
    } else {
        None
    };
    
    // Load observability (optional)
    let observability = load_observability(bundle_dir)?;
    
    // Load results
    let results: serde_json::Value = read_json(&bundle_dir.join("results/outputs.json"))?;
    
    // Load provenance
    let provenance: ProvenanceData = read_json(&bundle_dir.join("provenance/lineage.json"))?;
    
    // Parse artifact type
    let artifact_type = match manifest.artifact_type.as_str() {
        "run" => ArtifactType::Run,
        "gradient" => ArtifactType::Gradient,
        "calibration" => ArtifactType::Calibration,
        "replay" => ArtifactType::Replay,
        "validation" => ArtifactType::Validation,
        _ => bail!("Unknown artifact type: {}", manifest.artifact_type),
    };
    
    let bundle = ArtifactBundle {
        artifact_id: manifest.artifact_id.clone(),
        artifact_type,
        manifest,
        ir_original,
        ir_lowered,
        parameters_initial,
        parameters_final,
        calibration_state_initial,
        calibration_state_final,
        results,
        seed,
        observability,
        environment,
        provenance,
    };
    
    // Final validation
    validate_bundle(&bundle)?;
    
    Ok(bundle)
}

/// Import from tarball
fn import_from_tarball(tarball_path: &Path) -> Result<ArtifactBundle> {
    // Extract to temporary directory
    let temp_dir = tempfile::tempdir()?;
    
    let tar_gz = fs::File::open(tarball_path)?;
    let dec = flate2::read::GzDecoder::new(tar_gz);
    let mut tar = tar::Archive::new(dec);
    tar.unpack(temp_dir.path())?;
    
    // Find bundle directory (should be single top-level dir)
    let entries: Vec<_> = fs::read_dir(temp_dir.path())?.collect();
    if entries.len() != 1 {
        bail!("Tarball should contain exactly one top-level directory");
    }
    
    let bundle_dir = entries[0].as_ref()?.path();
    import_from_directory(&bundle_dir)
}

/// Validate artifact bundle
pub fn validate_bundle(bundle: &ArtifactBundle) -> Result<()> {
    // Check manifest schema version
    if !bundle.manifest.schema_version.starts_with("awen_artifact") {
        bail!("Invalid manifest schema version: {}", bundle.manifest.schema_version);
    }
    
    // Check artifact ID matches deterministic computation
    let computed_id = super::compute_deterministic_id(
        &bundle.ir_original,
        &bundle.parameters_initial,
        bundle.calibration_state_initial.as_ref(),
        bundle.seed,
        &bundle.environment.runtime.version,
    )?;
    
    if computed_id != bundle.artifact_id {
        bail!(
            "Artifact ID mismatch: manifest says {}, computed {}",
            bundle.artifact_id, computed_id
        );
    }
    
    // Check required files
    if bundle.parameters_initial.is_empty() {
        bail!("parameters_initial is required");
    }
    
    Ok(())
}

/// Validate checksums
fn validate_checksums(bundle_dir: &Path) -> Result<()> {
    let checksums: serde_json::Value = read_json(&bundle_dir.join("checksums.json"))?;
    let checksums_map = checksums.as_object()
        .context("checksums.json must be an object")?;
    
    for (relative_path, expected_hash) in checksums_map {
        let file_path = bundle_dir.join(relative_path);
        if !file_path.exists() {
            continue; // Skip checksums.json and manifest.json themselves
        }
        
        let actual_hash = super::compute_checksum_sha256(&file_path)?;
        let expected_str = expected_hash.as_str()
            .context("Checksum must be a string")?;
        
        if actual_hash != expected_str {
            bail!(
                "Checksum mismatch for {}: expected {}, got {}",
                relative_path, expected_str, actual_hash
            );
        }
    }
    
    Ok(())
}

/// Load observability data
fn load_observability(bundle_dir: &Path) -> Result<Option<ObservabilityData>> {
    let exec_dir = bundle_dir.join("execution");
    if !exec_dir.exists() {
        return Ok(None);
    }
    
    let traces = read_json_optional(&exec_dir.join("traces.jsonl"))?;
    let metrics = read_json_optional(&exec_dir.join("metrics.json"))?;
    let events = read_json_optional(&exec_dir.join("events.jsonl"))?;
    let timeline = read_json_optional(&exec_dir.join("timeline.json"))?;
    
    if traces.is_none() && metrics.is_none() && events.is_none() && timeline.is_none() {
        return Ok(None);
    }
    
    Ok(Some(ObservabilityData {
        traces,
        metrics,
        events,
        timeline,
    }))
}

/// Read JSON from file
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read {:?}", path))?;
    let value = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse JSON from {:?}", path))?;
    Ok(value)
}

/// Read JSON from file (optional)
fn read_json_optional<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(read_json(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_bundle() {
        // Test will be expanded with actual bundle creation
        // For now, just test basic validation logic
    }
}
