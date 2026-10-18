# Changelog

## Unreleased

- Added `docs/AUDIT.md` (implementation truth table at `020be48`),
  `docs/ROADMAP.md` (dependency-aware milestones with acceptance tests and
  hardware needs) and `docs/DIRECTIVE_TRACKER.md`; opened labeled issues
  #55–#79 for every missing module.
- Removed about 1,060 lines of never-compiled runtime code (`control_v0`, the
  old simulator module, a duplicate `awenctl`) and 65 tests that only exercised
  mocks; added real plugin-loader, HAL and `awenctl` tests. Runtime line
  coverage rose from 81.2% to 85.6% (#54).
- Updated Rust dependencies: `sha2` 0.11, `ed25519-dalek` 3.0, `serde_json`,
  `flate2`, `log`, `time`; `uuid` held at 1.24.0 because newer releases need
  Rust 1.89 (#53). Superseded Dependabot PRs were closed with explanations.
- Supported PyTorch 2.10 through 2.14 in `awen_py`; CI tests 2.10.0 and 2.14.1
  (#52, supersedes Dependabot #49).
- Pinned the Rust toolchain (1.99.1) in `rust-toolchain.toml`, declared and
  CI-checked a minimum supported Rust version of 1.88, added a weekly scheduled
  quality-gate run, Rust build caching, a prebuilt `cargo-audit`, and upgraded
  GitHub Actions off the deprecated Node 20 runtime.
- Added strict normalized equal-batch rank-three StableHLO `dot_general`
  lowering through typed MLIR dialects, deterministic AWENEXE emission, and
  direct Rust runtime preparation.
- Consolidated repository policy, security, CI, licensing, and release evidence.
- Removed obsolete phase sign-offs and nonfunctional product directories.

## Historical development snapshot

The tag `phase-2.5-gated-2026-01-05` is a historical source snapshot. It is not
a supported product release, does not establish hardware-performance evidence,
and predates the current verified-release policy.
