//! Export bundles to various formats

use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;

use super::{ArtifactBundle, compute_checksum_sha256};

#[derive(Clone, Debug)]
pub enum ExportFormat {
    Directory,
    TarGz,
}

/// Export artifact bundle to filesystem
pub fn export_bundle(bundle: &ArtifactBundle, output_dir: &Path, format: ExportFormat) -> Result<PathBuf> {
    match format {
        ExportFormat::Directory => export_to_directory(bundle, output_dir),
        ExportFormat::TarGz => export_to_tarball(bundle, output_dir),
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
    fs::create_dir_all(bundle_dir.join("execution"))?;
    fs::create_dir_all(bundle_dir.join("results"))?;
    fs::create_dir_all(bundle_dir.join("provenance"))?;
    
    // Write IR
    write_json(&bundle_dir.join("ir/original.json"), &bundle.ir_original)?;
    if let Some(ref lowered) = bundle.ir_lowered {
        write_json(&bundle_dir.join("ir/lowered.json"), lowered)?;
    }
    
    // Write parameters
    write_json(&bundle_dir.join("parameters/initial.json"), &bundle.parameters_initial)?;
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
    write_json(&bundle_dir.join("environment/snapshot.json"), &bundle.environment)?;
    if let Some(seed) = bundle.seed {
        write_text(&bundle_dir.join("environment/seed.txt"), &seed.to_string())?;
    }
    
    // Write execution traces (observability)
    if let Some(ref obs) = bundle.observability {
        if let Some(ref traces) = obs.traces {
            write_json(&bundle_dir.join("execution/traces.jsonl"), traces)?;
        }
        if let Some(ref metrics) = obs.metrics {
            write_json(&bundle_dir.join("execution/metrics.json"), metrics)?;
        }
        if let Some(ref events) = obs.events {
            write_json(&bundle_dir.join("execution/events.jsonl"), events)?;
        }
        if let Some(ref timeline) = obs.timeline {
            write_json(&bundle_dir.join("execution/timeline.json"), timeline)?;
        }
    }
    
    // Write results
    write_json(&bundle_dir.join("results/outputs.json"), &bundle.results)?;
    
    // Write provenance
    write_json(&bundle_dir.join("provenance/lineage.json"), &bundle.provenance)?;
    
    // Generate citation
    if let Some(ref metadata) = bundle.provenance.citation_metadata {
        let citation = super::generate_citation(
            &bundle.artifact_id,
            &metadata.title,
            &metadata.authors,
            &metadata.organization,
            &bundle.environment.runtime.version,
        );
        write_text(&bundle_dir.join("provenance/citation.txt"), &citation)?;
    }
    
    // Compute checksums
    let checksums = compute_bundle_checksums(&bundle_dir)?;
    write_json(&bundle_dir.join("checksums.json"), &checksums)?;
    
    // Write manifest (last, after all files created)
    write_json(&bundle_dir.join("manifest.json"), &bundle.manifest)?;
    
    Ok(bundle_dir)
}

/// Export to tarball (.tar.gz)
fn export_to_tarball(bundle: &ArtifactBundle, output_dir: &Path) -> Result<PathBuf> {
    // First export to temporary directory
    let temp_dir = tempfile::tempdir()?;
    let bundle_dir = export_to_directory(bundle, temp_dir.path())?;
    
    // Create tarball
    let tarball_path = output_dir.join(format!("{}.tar.gz", bundle.artifact_id));
    let tar_gz = fs::File::create(&tarball_path)?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);
    
    tar.append_dir_all(&bundle.artifact_id, &bundle_dir)?;
    tar.finish()?;
    
    Ok(tarball_path)
}

/// Write JSON to file
fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, json)?;
    Ok(())
}

/// Write text to file
fn write_text(path: &Path, text: &str) -> Result<()> {
    fs::write(path, text)?;
    Ok(())
}

/// Compute checksums for all files in bundle
fn compute_bundle_checksums(bundle_dir: &Path) -> Result<serde_json::Value> {
    let mut checksums = serde_json::Map::new();
    
    for entry in walkdir::WalkDir::new(bundle_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.path();
            let relative_path = path.strip_prefix(bundle_dir)?;
            let checksum = compute_checksum_sha256(path)?;
            checksums.insert(
                relative_path.to_string_lossy().to_string(),
                serde_json::Value::String(checksum),
            );
        }
    }
    
    Ok(serde_json::Value::Object(checksums))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[test]
    fn test_export_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        
        let bundle = create_test_bundle();
        let result = export_bundle(&bundle, temp_dir.path(), ExportFormat::Directory);
        
        assert!(result.is_ok());
        let bundle_dir = result.unwrap();
        
        // Verify structure
        assert!(bundle_dir.join("manifest.json").exists());
        assert!(bundle_dir.join("ir/original.json").exists());
        assert!(bundle_dir.join("parameters/initial.json").exists());
        assert!(bundle_dir.join("environment/snapshot.json").exists());
        assert!(bundle_dir.join("results/outputs.json").exists());
        assert!(bundle_dir.join("checksums.json").exists());
    }
    
    fn create_test_bundle() -> ArtifactBundle {
        use crate::ir::Graph;
        use super::super::{ArtifactType, Manifest, EnvironmentSnapshot, ProvenanceData};
        
        ArtifactBundle {
            artifact_id: "awen_test123".to_string(),
            artifact_type: ArtifactType::Run,
            manifest: Manifest::new("awen_test123".to_string(), ArtifactType::Run, "0.5.0".to_string()),
            ir_original: Graph { nodes: vec![], edges: vec![] },
            ir_lowered: None,
            parameters_initial: HashMap::new(),
            parameters_final: None,
            calibration_state_initial: None,
            calibration_state_final: None,
            results: serde_json::json!({"output": 42}),
            seed: Some(42),
            observability: None,
            environment: EnvironmentSnapshot {
                runtime: super::super::environment::RuntimeInfo {
                    runtime_name: "awen-runtime".to_string(),
                    version: "0.5.0".to_string(),
                    build_timestamp: "2024-01-01".to_string(),
                    build_profile: "release".to_string(),
                    rust_version: "1.75".to_string(),
                    features: vec![],
                    plugins: vec![],
                },
                system: super::super::environment::SystemInfo {
                    os: "linux".to_string(),
                    os_version: "Ubuntu 24.04".to_string(),
                    arch: "x86_64".to_string(),
                    cpu_model: "Test CPU".to_string(),
                    cpu_cores: 8,
                    memory_gb: 16,
                    hostname: "test".to_string(),
                },
                device: super::super::environment::DeviceInfo {
                    device_type: "simulated".to_string(),
                    device_id: "sim_0".to_string(),
                    capabilities: super::super::environment::DeviceCapabilities {
                        channels: 64,
                        max_frequency_hz: 1e15,
                        wavelength_range_nm: [1530.0, 1570.0],
                        phase_resolution_rad: 0.001,
                        power_range_dbm: [-30.0, 10.0],
                    },
                    firmware_version: None,
                    calibration_date: None,
                },
            },
            provenance: ProvenanceData {
                parent_artifacts: vec![],
                tags: vec![],
                notes: None,
                citation_metadata: None,
            },
        }
    }
}
