# Reproducible builds

This page records what AWEN can rebuild bit-for-bit today, how to check it,
and what is known not to be reproducible. Only claims with a check behind
them are listed as reproducible.

## Inputs that are pinned

* Rust toolchain: `rust-toolchain.toml` pins the exact compiler; CI installs
  the same version and checks the declared MSRV separately.
* Rust dependencies: every crate commits `Cargo.lock`; CI builds with
  `--locked`.
* Release tags are annotated and must point at a commit with a green quality
  gate (`docs/RELEASING.md`).
* `SOURCE_DATE_EPOCH` is set to the tagged commit time for archives, SBOMs,
  and wheels.

## Reproducible (checked)

| Artifact | How it is checked |
|---|---|
| Source archive (`git archive` of the tag) | Deterministic by construction for a fixed commit and git version; `sha256sum -c SHA256SUMS` verifies a download |
| Rust SBOMs (`scripts/generate_sbom.py`) | Two consecutive runs on the same commit produced byte-identical files for all three crates (checked locally on 2026-10-18) |

## Reproducible with conditions

* `awenctl` release binary: built with `--locked --release`, the pinned
  toolchain, and `--remap-path-prefix` so the build directory does not leak
  into the binary. The release workflow builds it twice in separate target
  directories and records whether the two SHA-256 values match; the result is
  printed in the release notes rather than assumed.
* Python wheels: pure-Python wheels built with `SOURCE_DATE_EPOCH` set have
  stable file timestamps. The native `awen_native` wheel depends on the host
  toolchain and manylinux image and is not claimed reproducible.

## Not reproducible

* The Python-environment SBOM reflects whatever `pip` resolved in CI at that
  time; it is a record, not a reproducible artifact.
* Model training runs: AWEN-Mini training is seeded, but CPU thread
  scheduling and BLAS reductions make bit-identical weights across machines
  unlikely. Releases publish the shipped checkpoint with its SHA-256 in
  `SHA256SUMS`, so results can be tied to one exact file.

## How to verify a release

```bash
sha256sum -c SHA256SUMS          # every asset matches the published checksum
git archive --format=tar.gz --prefix="awen-X.Y.Z/" vX.Y.Z | sha256sum   # compare to the source archive line
```
