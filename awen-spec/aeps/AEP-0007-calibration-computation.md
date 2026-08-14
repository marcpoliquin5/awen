# AEP-0007: Calibration as computation

Status: Implemented experimental contract; refined by AEP-0018

## Decision

Calibration is a versioned compiler/runtime input with backend, topology,
environment, timestamp, lineage, per-cell/per-spare/per-channel transfer data,
and uncertainty. Backend health binds the active calibration identity and exact
fingerprint. Compilation selects measured channels, remaps disabled cells,
attributes error, and invalidates artifacts when the calibration contract
changes.

The normative documents are `../specs/calibration.md`,
`../specs/control_calibration.md`, and
`../specs/calibration-aware-compilation.md`. Schemas include
`../schemas/awen_calibration_snapshot.v1.json` and
`../schemas/awen_artifact_refresh.v1.json`. Runtime/compiler tests cover drift,
freshness, remapping, fingerprint tampering, environment mismatch, and safe
digital fallback.

The reference implementation does not perform autonomous control of physical
lab hardware. Such control remains behind a reviewed HAL/plugin implementation
with explicit safety constraints.
