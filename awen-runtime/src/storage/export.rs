//! Export bundles to various formats

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use super::ArtifactBundle;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub enum ExportFormat {
    Directory,
    TarGz,
}

/// Export artifact bundle to filesystem
pub fn export_bundle(
    bundle: &ArtifactBundle,
    output_dir: &Path,
    format: ExportFormat,
) -> Result<PathBuf> {
    match format {
        ExportFormat::Directory => export_to_directory(bundle, output_dir),
        ExportFormat::TarGz => export_to_directory(bundle, output_dir),
    }
}

/// Export to directory structure
fn export_to_directory(bundle: &ArtifactBundle, output_dir: &Path) -> Result<PathBuf> {
    let bundle_dir = output_dir.join(&bundle.artifact_id);

    // Create directory structure
    fs::create_dir_all(&bundle_dir)?;
    fs::create_dir_all(bundle_dir.join("ir"))?;
    fs::create_dir_all(bundle_dir.join("parameters"))?;
    fs::create_dir_all(bundle_dir.join("calibration"))?;
    fs::create_dir_all(bundle_dir.join("environment"))?;
    fs::create_dir_all(bundle_dir.join("results"))?;
    fs::create_dir_all(bundle_dir.join("provenance"))?;

    // Write IR
    write_json(&bundle_dir.join("ir/original.json"), &bundle.ir_original)?;
    if let Some(ref lowered) = bundle.ir_lowered {
        write_json(&bundle_dir.join("ir/lowered.json"), lowered)?;
    }

    // Write parameters
    write_json(
        &bundle_dir.join("parameters/initial.json"),
        &bundle.parameters_initial,
    )?;
    if let Some(ref final_params) = bundle.parameters_final {
        write_json(&bundle_dir.join("parameters/final.json"), final_params)?;
    }

    // Write calibration
    if let Some(ref initial_calib) = bundle.calibration_state_initial {
        write_json(&bundle_dir.join("calibration/initial.json"), initial_calib)?;
    }
    if let Some(ref final_calib) = bundle.calibration_state_final {
        write_json(&bundle_dir.join("calibration/final.json"), final_calib)?;
    }

    // Write environment
    write_json(
        &bundle_dir.join("environment/snapshot.json"),
        &bundle.environment,
    )?;
    if let Some(seed) = bundle.seed {
        fs::write(bundle_dir.join("environment/seed.txt"), seed.to_string())?;
    }

    // Write results
    write_json(&bundle_dir.join("results/outputs.json"), &bundle.results)?;

    // Write provenance
    write_json(
        &bundle_dir.join("provenance/lineage.json"),
        &bundle.provenance,
    )?;

    // Write manifest
    write_json(&bundle_dir.join("manifest.json"), &bundle.manifest)?;

    // Write citation if present
    if let Some(ref citation) = bundle.provenance.citation {
        std::fs::write(bundle_dir.join("provenance/citation.txt"), citation)?;
    }

    // Compute checksums for all files (exclude checksums.json itself)
    let mut checksums: HashMap<String, String> = HashMap::new();
    for entry in WalkDir::new(&bundle_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Ok(rel) = path.strip_prefix(&bundle_dir) {
                if rel == std::path::Path::new("checksums.json") {
                    continue;
                }
                let mut file = std::fs::File::open(path)?;
                let mut hasher = Sha256::new();
                std::io::copy(&mut file, &mut hasher)?;
                let digest = hasher.finalize();
                let hex = hex::encode(digest);
                checksums.insert(rel.to_string_lossy().to_string(), hex);
            }
        }
    }

    // Write checksums.json
    write_json(&bundle_dir.join("checksums.json"), &checksums)?;

    Ok(bundle_dir)
}

/// Write JSON to file
fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, json)?;
    Ok(())
}
