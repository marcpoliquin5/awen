use anyhow::Result;
use awen_compiler::{BackendHealth, BackendSnapshot, DeviceCapabilities};
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{PublicKey, Signature, Verifier};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub const PLUGIN_MANIFEST_VERSION: &str = "awen.plugin-manifest.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HealthQuery {
    File { path: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendPluginContract {
    pub capabilities: DeviceCapabilities,
    pub health_query: HealthQuery,
}

/// Basic plugin manifest describing capability and a signing handle.
/// Implementations must provide a `public_key` and `signature` (both Base64).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub manifest_version: String,
    pub id: String,
    pub version: String,
    pub capabilities: Vec<String>,
    /// Base64-encoded ed25519 signature over the manifest content (excluding `signature` and `public_key` fields)
    pub signature: Option<String>,
    /// Base64-encoded ed25519 public key corresponding to the signer
    pub public_key: Option<String>,
    /// Optional path to plugin binary / adapter
    pub path: Option<PathBuf>,
    /// Typed compiler/runtime contract for hardware or simulator backends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<BackendPluginContract>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredBackend {
    pub plugin_id: String,
    pub snapshot: BackendSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryDiagnostic {
    pub plugin_id: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct BackendDiscoveryReport {
    pub backends: Vec<DiscoveredBackend>,
    pub diagnostics: Vec<DiscoveryDiagnostic>,
}

/// Registry that holds discovered plugins and performs manifest enforcement.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    pub plugins: Vec<PluginManifest>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin manifest into the registry (discovery step)
    pub fn register(&mut self, manifest: PluginManifest) {
        self.plugins.push(manifest);
    }

    pub fn validate_manifest(&self, manifest: &PluginManifest) -> Result<()> {
        self.validate_manifest_envelope(manifest)?;
        if let Some(backend) = &manifest.backend {
            backend.capabilities.validate()?;
        }
        Ok(())
    }

    fn validate_manifest_envelope(&self, manifest: &PluginManifest) -> Result<()> {
        if manifest.manifest_version != PLUGIN_MANIFEST_VERSION {
            anyhow::bail!(
                "unsupported plugin manifest version '{}'; expected '{}'",
                manifest.manifest_version,
                PLUGIN_MANIFEST_VERSION
            );
        }
        if manifest.id.trim().is_empty() || manifest.version.trim().is_empty() {
            anyhow::bail!("plugin id and version must not be empty");
        }
        if manifest
            .capabilities
            .iter()
            .any(|value| value.trim().is_empty())
        {
            anyhow::bail!("plugin capability names must not be empty");
        }
        Ok(())
    }

    /// Verify manifest signing and policy. Real implementation should verify
    /// signature against an organizational trust root and check manifest contents.
    pub fn verify_manifest(&self, manifest: &PluginManifest) -> Result<bool> {
        match (&manifest.signature, &manifest.public_key) {
            (Some(sig_b64), Some(pk_b64)) => {
                let sig_bytes = general_purpose::STANDARD
                    .decode(sig_b64)
                    .map_err(|e| anyhow::anyhow!("invalid signature base64: {}", e))?;
                let pk_bytes = general_purpose::STANDARD
                    .decode(pk_b64)
                    .map_err(|e| anyhow::anyhow!("invalid public_key base64: {}", e))?;

                let pk = PublicKey::from_bytes(&pk_bytes)
                    .map_err(|e| anyhow::anyhow!("invalid public key: {}", e))?;
                let sig = Signature::from_bytes(&sig_bytes)
                    .map_err(|e| anyhow::anyhow!("invalid signature bytes: {}", e))?;

                // Serialize manifest to canonical JSON excluding signature and public_key fields
                let mut clone = manifest.clone();
                clone.signature = None;
                clone.public_key = None;
                let data = serde_json::to_vec(&clone)?;

                pk.verify(&data, &sig)
                    .map(|_| true)
                    .map_err(|e| anyhow::anyhow!("signature verification failed: {}", e))
            }
            _ => Ok(false),
        }
    }

    /// Lookup plugin by capability name.
    pub fn find_by_capability(&self, cap: &str) -> Option<PluginManifest> {
        for p in &self.plugins {
            if p.capabilities.iter().any(|c| c == cap) {
                return Some(p.clone());
            }
        }
        None
    }

    /// Query the current health source for every typed backend plugin and
    /// combine it with the validated static capability contract. File queries
    /// are resolved inside the manifest directory and re-read on every call.
    pub fn query_backend_snapshots<P: AsRef<Path>>(
        &self,
        manifest_dir: P,
    ) -> BackendDiscoveryReport {
        let mut report = BackendDiscoveryReport::default();
        for manifest in &self.plugins {
            let Some(backend) = &manifest.backend else {
                continue;
            };
            let result = (|| -> Result<BackendSnapshot> {
                self.validate_manifest(manifest)?;
                let health = match &backend.health_query {
                    HealthQuery::File { path } => {
                        let path = resolve_inside(manifest_dir.as_ref(), path)?;
                        let bytes = fs::read(&path)?;
                        serde_json::from_slice::<BackendHealth>(&bytes)?
                    }
                };
                BackendSnapshot::new(backend.capabilities.clone(), health)
            })();
            match result {
                Ok(snapshot) => report.backends.push(DiscoveredBackend {
                    plugin_id: manifest.id.clone(),
                    snapshot,
                }),
                Err(error) => report.diagnostics.push(DiscoveryDiagnostic {
                    plugin_id: manifest.id.clone(),
                    message: format!("{error:#}"),
                }),
            }
        }
        report
    }

    /// Discover plugin manifests from a directory. Files ending with `.json` will be
    /// parsed as `PluginManifest` and registered only if `verify_manifest` passes.
    pub fn discover_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let mut reg = PluginRegistry::new();
        let dirp = dir.as_ref();
        if !dirp.exists() {
            return Ok(reg);
        }

        for entry in fs::read_dir(dirp)? {
            let e = entry?;
            let path = e.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if ext.eq_ignore_ascii_case("json") {
                        let data = fs::read(&path)?;
                        match serde_json::from_slice::<PluginManifest>(&data) {
                            Ok(manifest) => {
                                if reg.validate_manifest_envelope(&manifest).is_err() {
                                    continue;
                                }
                                match reg.verify_manifest(&manifest) {
                                    Ok(true) => {
                                        reg.register(manifest);
                                    }
                                    Ok(false) => {
                                        // signature missing or verification false — skip
                                    }
                                    Err(_) => {
                                        // verification errored — skip
                                    }
                                }
                            }
                            Err(_) => {
                                // not a manifest file — ignore
                            }
                        }
                    }
                }
            }
        }

        Ok(reg)
    }

    /// Discover plugin manifests from a directory, optionally allowing unverified manifests.
    /// When `allow_unverified` is true any parseable manifest will be registered even if
    /// signature verification fails or is absent. This is intended for test or developer
    /// flows only.
    pub fn discover_from_dir_allow_unverified<P: AsRef<Path>>(
        dir: P,
        allow_unverified: bool,
    ) -> Result<Self> {
        let mut reg = PluginRegistry::new();
        let dirp = dir.as_ref();
        if !dirp.exists() {
            return Ok(reg);
        }

        for entry in fs::read_dir(dirp)? {
            let e = entry?;
            let path = e.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if ext.eq_ignore_ascii_case("json") {
                        let data = fs::read(&path)?;
                        if let Ok(manifest) = serde_json::from_slice::<PluginManifest>(&data) {
                            if reg.validate_manifest_envelope(&manifest).is_err() {
                                continue;
                            }
                            match reg.verify_manifest(&manifest) {
                                Ok(true) => reg.register(manifest),
                                Ok(false) => {
                                    if allow_unverified {
                                        reg.register(manifest);
                                    }
                                }
                                Err(_) => {
                                    if allow_unverified {
                                        reg.register(manifest);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(reg)
    }
}

fn resolve_inside(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        anyhow::bail!("health query path must stay inside the plugin manifest directory");
    }
    let root = root.canonicalize()?;
    let candidate = root.join(relative).canonicalize()?;
    if !candidate.starts_with(&root) {
        anyhow::bail!("health query path escapes the plugin manifest directory");
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn register_and_find_manifest_without_signature() {
        let mut reg = PluginRegistry::new();

        let manifest = PluginManifest {
            manifest_version: PLUGIN_MANIFEST_VERSION.into(),
            id: "test-plugin".into(),
            version: "0.1".into(),
            capabilities: vec!["execute".into()],
            signature: None,
            public_key: None,
            path: None,
            backend: None,
        };

        // No signature/public_key present — verify_manifest should return false
        assert!(!reg.verify_manifest(&manifest).unwrap());

        reg.register(manifest.clone());
        let found = reg.find_by_capability("execute").unwrap();
        assert_eq!(found.id, "test-plugin");
    }
}
