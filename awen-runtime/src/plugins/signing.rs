//! Ed25519 plugin-manifest signing and trust-pinned verification.
//!
//! [`PluginRegistry::verify_manifest`](super::PluginRegistry::verify_manifest)
//! only proves that a manifest matches the key embedded in it, so anyone can
//! produce a "valid" self-signed manifest. This module adds the missing pieces:
//!
//! * [`sign_manifest`] signs the canonical manifest bytes (the manifest
//!   serialized with `signature` and `public_key` cleared, exactly the bytes
//!   that `verify_manifest` checks) and embeds the signature and key.
//! * [`TrustedKeys`] is an explicit allowlist of signer keys. A manifest is
//!   trusted only if its signature verifies AND its embedded key is pinned.
//! * [`discover_trusted`] loads a plugin directory, registering only trusted
//!   manifests and reporting a diagnostic for every rejected file.
//!
//! Key files are 32 raw bytes encoded as standard Base64 on a single line.
//! Secret key files must never be committed; `awenctl plugin keygen` writes
//! them with owner-only permissions on Unix.

use super::registry::{DiscoveryDiagnostic, PluginManifest, PluginRegistry};
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::RngCore;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Canonical bytes covered by a manifest signature.
pub fn canonical_manifest_bytes(manifest: &PluginManifest) -> Result<Vec<u8>> {
    let mut clone = manifest.clone();
    clone.signature = None;
    clone.public_key = None;
    Ok(serde_json::to_vec(&clone)?)
}

/// Generate a new signing key from the operating-system CSPRNG.
pub fn generate_signing_key() -> SigningKey {
    let mut seed = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    SigningKey::from_bytes(&seed)
}

/// Encode a 32-byte key as single-line standard Base64.
pub fn encode_key(bytes: &[u8; 32]) -> String {
    general_purpose::STANDARD.encode(bytes)
}

fn decode_32(text: &str, what: &str) -> Result<[u8; 32]> {
    let bytes = general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|e| anyhow!("invalid {what} base64: {e}"))?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("{what} must contain exactly 32 bytes"))
}

/// Parse a Base64 secret (signing) key.
pub fn parse_signing_key(text: &str) -> Result<SigningKey> {
    Ok(SigningKey::from_bytes(&decode_32(text, "signing key")?))
}

/// Parse a Base64 public (verifying) key.
pub fn parse_verifying_key(text: &str) -> Result<VerifyingKey> {
    VerifyingKey::from_bytes(&decode_32(text, "public key")?)
        .map_err(|e| anyhow!("invalid public key: {e}"))
}

/// Sign `manifest` in place, embedding the signature and the signer's public key.
pub fn sign_manifest(manifest: &mut PluginManifest, key: &SigningKey) -> Result<()> {
    let data = canonical_manifest_bytes(manifest)?;
    let signature = key.sign(&data);
    manifest.signature = Some(general_purpose::STANDARD.encode(signature.to_bytes()));
    manifest.public_key = Some(encode_key(&key.verifying_key().to_bytes()));
    Ok(())
}

/// Explicit allowlist of accepted plugin signer keys.
#[derive(Debug, Clone, Default)]
pub struct TrustedKeys {
    keys: BTreeSet<[u8; 32]>,
}

impl TrustedKeys {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: VerifyingKey) {
        self.keys.insert(key.to_bytes());
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn contains(&self, key: &VerifyingKey) -> bool {
        self.keys.contains(&key.to_bytes())
    }

    /// Parse a trust file: one Base64 public key per line; blank lines and
    /// lines starting with `#` are ignored.
    pub fn parse(text: &str) -> Result<Self> {
        let mut trusted = Self::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let key = parse_verifying_key(line)
                .with_context(|| format!("trust file line {}", index + 1))?;
            trusted.insert(key);
        }
        Ok(trusted)
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        Self::parse(
            &fs::read_to_string(path)
                .with_context(|| format!("reading trust file {}", path.display()))?,
        )
    }

    /// Verify the signature and require the embedded key to be pinned.
    /// Returns a human-readable reason on rejection.
    pub fn verify(&self, manifest: &PluginManifest) -> Result<()> {
        let Some(public_key) = &manifest.public_key else {
            bail!("manifest is unsigned (no public_key)");
        };
        if manifest.signature.is_none() {
            bail!("manifest is unsigned (no signature)");
        }
        let key = parse_verifying_key(public_key)?;
        if !self.contains(&key) {
            bail!("signer key {public_key} is not in the trusted key set");
        }
        match PluginRegistry::new().verify_manifest(manifest)? {
            true => Ok(()),
            false => bail!("signature verification failed"),
        }
    }
}

/// Result of a trust-pinned directory discovery.
#[derive(Debug, Default)]
pub struct TrustedDiscovery {
    pub registry: PluginRegistry,
    pub rejected: Vec<DiscoveryDiagnostic>,
}

/// Discover `*.json` manifests in `dir`, registering only manifests that pass
/// envelope validation, signature verification, and the trust allowlist.
/// Files are visited in sorted order so results are deterministic.
pub fn discover_trusted<P: AsRef<Path>>(dir: P, trusted: &TrustedKeys) -> Result<TrustedDiscovery> {
    let mut out = TrustedDiscovery::default();
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(out);
    }
    let mut paths: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case("json"))
        })
        .collect();
    paths.sort();
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let manifest = match serde_json::from_slice::<PluginManifest>(&fs::read(&path)?) {
            Ok(m) => m,
            Err(_) => continue, // not a manifest (for example a health source)
        };
        let id = if manifest.id.is_empty() {
            name
        } else {
            manifest.id.clone()
        };
        let verdict = out
            .registry
            .validate_manifest_envelope(&manifest)
            .and_then(|_| trusted.verify(&manifest));
        match verdict {
            Ok(()) => out.registry.register(manifest),
            Err(error) => out.rejected.push(DiscoveryDiagnostic {
                plugin_id: id,
                message: format!("{error:#}"),
            }),
        }
    }
    Ok(out)
}
