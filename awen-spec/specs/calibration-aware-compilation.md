# Calibration-aware compilation contract

This specification is normative for the compiler-facing calibration path.
Runtime calibration acquisition remains defined in `calibration.md`; this file
defines how immutable measured state affects compilation and artifact reuse.

## Inputs

A calibration-aware compile consumes one `BackendSnapshot` containing:

- a validated `DeviceCapabilities` document;
- an optional `awen.calibration-snapshot.v1` embedded in that capability; and
- a live `awen.backend-health.v1` observation.

When calibration is required, health must confirm the snapshot ID and exact
fingerprint. The snapshot backend identity and computed topology fingerprint
must match the capability.

## Snapshot identity

The tuple below identifies measured state:

```text
(snapshot_version, id, fingerprint, backend_id, topology_fingerprint)
```

`parent_id` links recalibration lineage. `measured_at` and `environment`
describe when and under what temperature/laser-power state measurements were
taken. Fingerprints are immutable identifiers; content must never be changed
in place under an existing fingerprint.

## Transfer measurements

The global transfer and each measured cell, spare, and channel contain gain,
phase error, insertion loss, and uncertainty. Cells and spares also carry
offset. Logical cells carry matrix row and column. Channels carry stable IDs and
advertised wavelengths.

All gains are finite and non-zero. Loss and uncertainty are finite and
non-negative. Cell coordinates and channel wavelengths must belong to the
advertised topology.

## Routing

Disabled logical cells are assigned to healthy spare cells in ascending
transfer-score order with stable-ID tie-breaking. The score is:

```text
abs(gain - 1) + abs(offset) + abs(phase_error)
  + 0.01 * insertion_loss_db + uncertainty
```

Disabled channel IDs are excluded. Healthy measured channels are ranked by the
same score without offset; unmeasured advertised wavelengths rank after
measured candidates. The tuning plan and health channel count cap the selected
set.

Any remapped logical cell, active high-error cell, or high-error spare required
by the current fault set derates all three matrix-core tile dimensions by half
with ceiling. The derated shape is used by cost estimation, autotuning, tiling,
lowering, and the serialized calibration decision.

Insufficient spares or zero selected channels make photonic routing illegal.

## Lowered representation

Classical Photonic IR records stable channel IDs, wavelengths, cell remaps,
effective transfer, capacity loss, error impact, snapshot fingerprint, and
inverse compensation. Device IR emits:

```text
calibrate(profile_id, fingerprint, topology_fingerprint)
remap_cell(op_id, disabled_cell, replacement_cell, logical_row, logical_column)
select_channels(op_id, channel_ids, wavelengths_nm)
configure_matrix(...)
upload_tile(...)
execute_gemm(...)
accumulate(...)
download(...)
rescale(tensor, scale, offset, calibration_handle, calibration_fingerprint)
```

Conversions and precision operations defined by AEP-0017 remain explicit.

## Effective transfer

Active logical-cell, selected-channel, and remapped-spare measurements compose
with the global transfer. Insertion loss contributes amplitude attenuation
`10^(-loss_db / 20)` and phase contributes `cos(phase_error)`. The inverse scale
and bias are emitted explicitly. Residual uncertainty remains attributed as
calibration error; it is never merged into quantization or generic analog
noise.

## Artifact record

Every compilation artifact records source and backend snapshot fingerprints,
the complete health observation, immutable calibration lineage/environment,
and one deduplicated decision-impact record per logical photonic operation.

Exact artifact reuse requires equal source and backend-snapshot fingerprints.
Changed state is handled by `refresh_for_backend`, which emits `reused`,
`recompiled`, or `fell_back` with exact invalidation reasons.

## Safety

Stale, future, cross-device, cross-topology, fingerprint-mismatched,
temperature-invalid, drift-invalid, or unremappable snapshots may not execute
silently. Forced photonic compilation rejects. Automatic compilation and
artifact refresh use a diagnosed digital fallback when current hardware cannot
meet the complete contract.

## Reproducibility

The source graph, capability, calibration, health observation, mapping,
precision plan, cost decision, and refresh reasons are serialized. Recompiling
against current hardware is a new lineage event and must not be described as
exact replay of unavailable historical physical state.

See AEP-0018 and these schemas:

- `awen_calibration_snapshot.v1.json`
- `awen_backend_health.v1.json`
- `awen_device_capability.v1.json`
- `awen_photonic_ir.classical.v1.json`
- `awen_device_ir.v1.json`
- `awen_compilation_artifact.v1.json`
- `awen_artifact_refresh.v1.json`
