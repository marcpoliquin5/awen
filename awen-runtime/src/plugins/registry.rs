use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{PublicKey, Signature, Verifier};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Basic plugin manifest describing capability and a signing handle.
/// Implementations must provide a `public_key` and `signature` (both Base64).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub version: String,
    pub capabilities: Vec<String>,
    /// Base64-encoded ed25519 signature over the manifest content (excluding `signature` and `public_key` fields)
    pub signature: Option<String>,
    /// Base64-encoded ed25519 public key corresponding to the signer
    pub public_key: Option<String>,
    /// Optional path to plugin binary / adapter
    pub path: Option<PathBuf>,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn register_and_find_manifest_without_signature() {
        let mut reg = PluginRegistry::new();

        let manifest = PluginManifest {
            id: "test-plugin".into(),
            version: "0.1".into(),
            capabilities: vec!["execute".into()],
            signature: None,
            public_key: None,
            path: None,
        };

        // No signature/public_key present — verify_manifest should return false
        assert!(!reg.verify_manifest(&manifest).unwrap());

        reg.register(manifest.clone());
        let found = reg.find_by_capability("execute").unwrap();
        assert_eq!(found.id, "test-plugin");
    }
}
