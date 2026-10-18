//! Trust-pinned Ed25519 plugin signing (X1b).
use awen_runtime::plugins::signing::{
    canonical_manifest_bytes, discover_trusted, encode_key, generate_signing_key,
    parse_signing_key, sign_manifest, TrustedKeys,
};
use awen_runtime::plugins::{PluginManifest, PluginRegistry, PLUGIN_MANIFEST_VERSION};
use std::fs;
use std::process::Command;

fn manifest(id: &str) -> PluginManifest {
    PluginManifest {
        manifest_version: PLUGIN_MANIFEST_VERSION.into(),
        id: id.into(),
        version: "1.0.0".into(),
        capabilities: vec!["execute".into()],
        signature: None,
        public_key: None,
        path: None,
        backend: None,
        physical_design_adapters: Vec::new(),
    }
}

fn trust(keys: &[&ed25519_dalek::SigningKey]) -> TrustedKeys {
    let mut t = TrustedKeys::new();
    for k in keys {
        t.insert(k.verifying_key());
    }
    t
}

#[test]
fn signed_by_pinned_key_is_trusted_and_matches_registry_verification() {
    let key = generate_signing_key();
    let mut m = manifest("vendor.sim");
    sign_manifest(&mut m, &key).unwrap();
    assert!(PluginRegistry::new().verify_manifest(&m).unwrap());
    trust(&[&key]).verify(&m).unwrap();
}

#[test]
fn self_signed_manifest_from_unpinned_key_is_rejected() {
    let pinned = generate_signing_key();
    let attacker = generate_signing_key();
    let mut m = manifest("vendor.sim");
    sign_manifest(&mut m, &attacker).unwrap();
    // Integrity alone passes: the embedded key matches.
    assert!(PluginRegistry::new().verify_manifest(&m).unwrap());
    // Trust fails: the signer is not pinned.
    let err = trust(&[&pinned]).verify(&m).unwrap_err().to_string();
    assert!(err.contains("not in the trusted key set"), "{err}");
}

#[test]
fn tampering_after_signing_is_rejected() {
    let key = generate_signing_key();
    let mut m = manifest("vendor.sim");
    sign_manifest(&mut m, &key).unwrap();
    m.capabilities.push("admin".into());
    assert!(trust(&[&key]).verify(&m).is_err());
}

#[test]
fn swapping_in_a_pinned_key_without_its_signature_is_rejected() {
    let pinned = generate_signing_key();
    let attacker = generate_signing_key();
    let mut m = manifest("vendor.sim");
    sign_manifest(&mut m, &attacker).unwrap();
    m.public_key = Some(encode_key(&pinned.verifying_key().to_bytes()));
    assert!(trust(&[&pinned]).verify(&m).is_err());
}

#[test]
fn unsigned_manifest_is_rejected() {
    let key = generate_signing_key();
    let err = trust(&[&key])
        .verify(&manifest("x"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("unsigned"), "{err}");
}

#[test]
fn canonical_bytes_ignore_signature_fields() {
    let key = generate_signing_key();
    let plain = manifest("vendor.sim");
    let mut signed = plain.clone();
    sign_manifest(&mut signed, &key).unwrap();
    assert_eq!(
        canonical_manifest_bytes(&plain).unwrap(),
        canonical_manifest_bytes(&signed).unwrap()
    );
}

#[test]
fn trust_file_parsing_skips_comments_and_rejects_bad_keys() {
    let key = generate_signing_key();
    let text = format!(
        "# pinned vendor keys\n\n{}\n",
        encode_key(&key.verifying_key().to_bytes())
    );
    let t = TrustedKeys::parse(&text).unwrap();
    assert_eq!(t.len(), 1);
    assert!(t.contains(&key.verifying_key()));
    assert!(TrustedKeys::parse("not-base64!!\n").is_err());
    assert!(TrustedKeys::parse(&encode_key(&[0; 32])[..20]).is_err());
}

#[test]
fn secret_key_round_trips_through_base64() {
    let key = generate_signing_key();
    let back = parse_signing_key(&encode_key(&key.to_bytes())).unwrap();
    assert_eq!(back.to_bytes(), key.to_bytes());
}

#[test]
fn directory_discovery_registers_only_trusted_manifests() {
    let dir = tempfile::tempdir().unwrap();
    let good = generate_signing_key();
    let evil = generate_signing_key();
    let mut a = manifest("a.trusted");
    sign_manifest(&mut a, &good).unwrap();
    let mut b = manifest("b.untrusted");
    sign_manifest(&mut b, &evil).unwrap();
    let c = manifest("c.unsigned");
    for (name, m) in [("a.json", &a), ("b.json", &b), ("c.json", &c)] {
        fs::write(
            dir.path().join(name),
            serde_json::to_string_pretty(m).unwrap(),
        )
        .unwrap();
    }
    fs::write(dir.path().join("health.json"), "{\"not\":\"a manifest\"}").unwrap();
    let found = discover_trusted(dir.path(), &trust(&[&good])).unwrap();
    let ids: Vec<_> = found
        .registry
        .plugins
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(ids, ["a.trusted"]);
    let rejected: Vec<_> = found
        .rejected
        .iter()
        .map(|d| d.plugin_id.as_str())
        .collect();
    assert_eq!(rejected, ["b.untrusted", "c.unsigned"]);
}

#[test]
fn awenctl_keygen_sign_verify_round_trip() {
    let bin = env!("CARGO_BIN_EXE_awenctl");
    let dir = tempfile::tempdir().unwrap();
    let prefix = dir.path().join("vendor");
    let run = |args: &[&str]| Command::new(bin).args(args).output().unwrap();
    let out = run(&["plugin", "keygen", prefix.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(prefix.with_extension("key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "secret key must be owner-only");
    }
    // keygen refuses to overwrite an existing secret key
    assert!(!run(&["plugin", "keygen", prefix.to_str().unwrap()])
        .status
        .success());

    let mpath = dir.path().join("m.json");
    fs::write(&mpath, serde_json::to_string(&manifest("cli.sim")).unwrap()).unwrap();
    let key = prefix.with_extension("key");
    let public = prefix.with_extension("pub");
    let out = run(&[
        "plugin",
        "sign",
        mpath.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run(&[
        "plugin",
        "verify",
        mpath.to_str().unwrap(),
        "--trusted",
        public.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("trusted: cli.sim"));

    let other = dir.path().join("other.pub");
    fs::write(
        &other,
        encode_key(&generate_signing_key().verifying_key().to_bytes()),
    )
    .unwrap();
    let out = run(&[
        "plugin",
        "verify",
        mpath.to_str().unwrap(),
        "--trusted",
        other.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
}
