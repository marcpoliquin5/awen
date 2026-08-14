# AEP-0006: Reproducibility and artifacts

Status: Implemented experimental contract

## Decision

Artifact bundles carry a deterministic content identity, manifest and content
index, original and lowered IR, inputs and results, seeds, calibration state,
environment, observability paths, provenance, parent artifacts, and checksums.
Import verifies integrity before use.

The normative export/import and replay rules are in
`../specs/reproducibility.md`. The implementation is in
`../../awen-runtime/src/storage`, with integration coverage in
`../../awen-runtime/tests/artifacts_integration.rs` and
`../../awen-runtime/tests/reproducibility_integration.rs`.

Run directories are mutable local artifacts. Only content-addressed bundles and
verified HIL artifacts may be cited as immutable public evidence.
