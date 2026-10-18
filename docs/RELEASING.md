# Verified release procedure

Every AWEN release must satisfy every requirement below. Releases are
simulator-only software releases: no release is a hardware-performance claim.

## Required evidence

1. The release tag is annotated, uses `vMAJOR.MINOR.PATCH` (or
   `vMAJOR.MINOR.PATCH-rc.N` for a release candidate, published as a GitHub
   pre-release), and points to a commit reachable from `main`.
2. The exact commit has a completed, successful run of
   `.github/workflows/observability-quality-gate.yml`; reruns on another commit
   do not count.
3. The release workflow records the immutable commit SHA, quality-gate run ID
   and URL, source-archive SHA-256, toolchain versions, and artifact checksums.
4. `CHANGELOG.md`, compatibility notes, schemas, package versions, and security
   support are consistent with the tag.
5. Any performance statement links to a verified immutable HIL artifact and
   names its complete system boundary. Simulated, estimated, assumed, or vendor
   values cannot become measured product claims.

## Artifacts

`scripts/build_release_artifacts.sh VERSION DIST` builds, into one flat
directory:

| Artifact | Built by |
|---|---|
| `awenctl-VERSION-x86_64-unknown-linux-gnu.tar.gz` | `cargo build --locked --release`, built twice to record whether the binary is bit-for-bit reproducible |
| `awen_native-*.whl` | `maturin build --release --locked` |
| pure-Python wheels (`awen_py`, `awen_models` with the bundled AWEN-Mini checkpoint, `awen_physics`, `awen_qphotonic`, `awen_systems`, `awen_lab`, `awen_studio` with its built frontend) | `python -m build --wheel` |
| `awen-mini-checkpoint-VERSION.tar.gz` | deterministic `tar` of the checkpoint directory |
| `*.cdx.json`, `SBOM-SUMMARY.json` | `scripts/generate_sbom.py` |
| `awen-VERSION.tar.gz` | `git archive` |
| `BUILD-INFO.json`, `SHA256SUMS` | toolchain versions, rebuild check, checksums of every file |

`scripts/verify_release_artifacts.py DIST` then checks every checksum, runs the
`awenctl` binary, confirms the checkpoint tarball and the `awen_models` wheel
hold identical weights, installs every wheel into a fresh virtual environment,
compiles a program with the Rust compiler, runs the photonic simulator,
generates text with AWEN-Mini on CPU and on the simulator, and checks that
the installed Studio finds its bundled frontend.

## Publication

1. Tag a `main` commit whose quality gate is green:
   `git tag -a v0.1.0-rc.1 -m "AWEN 0.1.0-rc.1" <sha> && git push origin v0.1.0-rc.1`.
2. Run the manual `Verified release` workflow with the tag
   (`gh workflow run release.yml -f tag=v0.1.0-rc.1`). It refuses a tag
   without exact green evidence, builds and verifies the artifacts, and creates
   a **draft** release (a pre-release for `-rc.N` tags) whose generated notes
   include the evidence, build information, and checksums.
   Every asset also receives a Sigstore-signed build-provenance attestation
   (`actions/attest-build-provenance`); verify with
   `gh attestation verify <file> -R marcpoliquin5/awen`.
3. A maintainer downloads the draft's assets, runs `sha256sum -c SHA256SUMS`,
   compares the draft with this checklist, and only then publishes it
   (`gh release edit TAG --draft=false`). Draft creation never changes the
   protected quality-gate result.

Milestones, phase documents, local test logs, and branch checks are not release
evidence.
