# Security policy

## Supported versions

AWEN has no supported product release. Security fixes are applied to the
default branch. Historical tags and development snapshots receive no fixes.

## Reporting a vulnerability

Do not open a public issue. Use GitHub's private vulnerability-reporting form:

https://github.com/marcpoliquin5/awen/security/advisories/new

Include affected commits, impact, reproduction steps, and any suggested fix.
The maintainer will acknowledge a complete report within three business days,
provide an initial assessment within seven days, and coordinate disclosure
after a fix is available. These are response targets, not a warranty.

## Scope

Reports may cover compiler/runtime input validation, plugin loading and
signatures, path handling, artifact integrity, CI/release provenance, exposed
credentials, or unsafe FFI behavior. Public browser configuration is not a
secret, but authorization must never rely on obscuring it; backend policy and
row-level security remain required.

Never include live credentials, proprietary PDK data, personal data, or exploit
payloads in public issues, pull requests, fixtures, logs, or artifacts.

## Supply-chain controls

Every control below runs in the required quality gate
(`.github/workflows/observability-quality-gate.yml`) unless noted.

| Control | Mechanism | Where |
|---|---|---|
| Secret scanning | gitleaks over full history | quality gate |
| Dependency review | `dependency-review-action`, fails on moderate+ advisories and GPL-3.0/AGPL-3.0/SSPL | pull requests |
| Vulnerability audit | `cargo audit` (three lockfiles), `pip-audit` | quality gate |
| License compliance | `cargo deny check licenses sources` with the permissive allowlist in `deny.toml`; `scripts/check_dependency_licenses.py` as a second, dependency-free scan | quality gate |
| SBOM | CycloneDX 1.5 JSON per Rust crate (`cargo cyclonedx`) plus the CI Python environment (`pip-audit -f cyclonedx-json`), via `scripts/generate_sbom.py`; uploaded as a workflow artifact | quality gate; attached to releases |
| Checksums | `SHA256SUMS` over every release asset | release workflow |
| Plugin signing | Ed25519 manifest signatures with a pinned trust set (below) | runtime, `awenctl plugin` |
| Safe model loading | `scripts/check_safe_model_loading.py` (below) | quality gate |

### Plugin signing and trust

A plugin manifest is signed over its canonical bytes: the manifest serialized
as JSON with `signature` and `public_key` cleared. `PluginRegistry::verify_manifest`
only proves the manifest matches the key embedded in it, so a self-signed
manifest from anyone passes that check. Trust therefore requires a pinned key:
`awen_runtime::plugins::TrustedKeys` holds an explicit allowlist, and
`TrustedKeys::verify` / `discover_trusted` accept a manifest only when its
signature verifies **and** its signer key is pinned. Unsigned, tampered,
key-swapped, and unpinned manifests are rejected with a diagnostic
(`awen-runtime/tests/plugin_signing.rs`).

```bash
awenctl plugin keygen vendor            # vendor.key (secret, 0600, never overwritten) + vendor.pub
awenctl plugin sign manifest.json --key vendor.key
awenctl plugin verify manifest.json --trusted trusted_keys.txt   # one Base64 key per line, '#' comments
awenctl plugin discover plugins/ --trusted trusted_keys.txt
```

Never commit a `.key` file. The legacy `--allow-unverified` discovery flag is
for local simulation only.

### Safe model loading

AWEN never executes code shipped with a model and never unpickles weights:

* Hugging Face loaders always pass `trust_remote_code=False` explicitly and
  load weights with `use_safetensors=True` (`awen_models.hf_adapter`,
  `awen_models.finetune`).
* AWEN checkpoints are safetensors plus JSON (`awen_models.checkpoint`).
* `pickle`/`dill`/`joblib`/`cloudpickle` loads are forbidden in shipped code,
  and `torch.load` is only allowed with `weights_only=True`.

`scripts/check_safe_model_loading.py` enforces these rules over every shipped
Python file with an AST scan, and fails the gate on any violation. Models whose
architecture needs custom remote code are not supported by AWEN loaders; convert
them to a supported architecture and safetensors first.

### Reproducible builds

See `docs/REPRODUCIBLE_BUILDS.md` for what is and is not reproducible today.
