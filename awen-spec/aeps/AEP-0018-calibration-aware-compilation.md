# AEP-0018: Calibration-aware compilation, fault remapping, and artifact refresh

Status: Accepted and implemented

## Summary

AWEN treats a measured calibration snapshot and its matching live-health
observation as immutable compiler inputs. A snapshot is versioned, identified,
fingerprinted, bound to one backend topology, measured in a recorded
environment, and populated with global, per-cell, spare-cell, and per-channel
transfer data. Compilation uses those measurements to choose wavelength
channels, route disabled logical cells to calibrated spares, estimate capacity
and accuracy loss, compensate the effective transfer function, and record every
decision in the emitted artifact.

Compiled artifacts also fingerprint the source graph and complete backend
snapshot. Before reuse, `refresh_for_backend` compares those identities with a
current snapshot. An unchanged snapshot is reusable. Changed calibration,
health, drift, temperature, disabled components, resources, backend identity,
or topology causes deterministic recompilation. If the current hardware cannot
safely satisfy the operation, automatic refresh produces an explicit digital
fallback instead of executing a stale photonic artifact.

## Motivation

Scalar calibration metadata is insufficient for programmable photonics. Two
nominally identical accelerators may have different transfer functions,
wavelength quality, insertion loss, phase error, spare-cell quality, disabled
components, temperature, and drift. A compiler that ignores those differences
can select an unavailable route, compensate the wrong transfer function,
silently reuse stale state, or report an accuracy estimate that does not match
the hardware used for execution.

Before this proposal, AWEN validated one global calibration ID, timestamp,
temperature, gain, offset, phase error, and uncertainty. It rejected expired or
drifted calibration, but the compiler did not bind that profile to a topology,
did not confirm an exact content fingerprint through health, did not consume
per-cell or per-channel measurements, did not remap faults, and did not expose
an artifact-refresh decision. Device IR could request calibration and rescale a
result, but it could not state the actual cell remap or channel selection.

## Normative versions

This proposal defines or extends the following serialized contracts:

- `awen.calibration-snapshot.v1` for immutable measured calibration input;
- `awen.calibration-decision.v1` for per-operation routing and error impact;
- `awen.compilation.v1` for source/backend fingerprints and calibration
  provenance;
- `awen.artifact-refresh.v1` for reuse, recompilation, and fallback decisions;
- `awen.device-capability.v1` to carry the calibration snapshot;
- `awen.backend-health.v1` to confirm its exact ID and fingerprint;
- `awen.photonic.classical.v1` to carry calibrated channel and cell routing;
- `awen.device.v1` to make calibration, remapping, channel selection, and
  compensated rescaling executable; and
- the cost/error model defined by AEP-0013 and AEP-0017.

Unknown fields remain rejected. Version skew, missing required snapshot data,
invalid transfer values, and identity mismatches fail validation.

## Calibration snapshot

`awen.calibration-snapshot.v1` contains:

- `id`, immutable `fingerprint`, and optional `parent_id` lineage;
- `backend_id` as the device/backend identity;
- a computed `topology_fingerprint`;
- `measured_at`;
- the measured environment, including temperature and laser power;
- global gain, offset, phase error, insertion loss, and uncertainty;
- measured logical matrix cells;
- measured spare cells; and
- measured wavelength channels.

Every transfer record has finite gain, phase, loss, and uncertainty. Gains must
be non-zero. Loss and uncertainty are non-negative. Logical cell coordinates
must be unique and within the advertised matrix topology. Component IDs and
channel IDs are non-empty and unique. A measured channel wavelength must be in
the device capability. The measured laser power must not exceed the device
power budget.

The optional `parent_id` captures recalibration lineage. It must be non-empty,
must differ from the current snapshot ID, and identifies the calibration state
from which the new measurement was derived.

## Topology binding

The topology fingerprint is deterministic FNV-1a over:

1. backend identity;
2. matrix-core M, N, and K dimensions;
3. simultaneous-channel capacity; and
4. the ordered advertised wavelength list as exact floating-point bits.

The compiler recomputes this fingerprint from the capability and rejects a
calibration snapshot whose value differs. A calibration measured for another
backend is rejected independently through `backend_id`. These checks prevent a
snapshot from silently crossing a device or topology boundary even when its
human-readable ID looks plausible.

## Exact health confirmation

When calibration is required, live health must confirm both:

- `calibration_profile_id`; and
- `calibration_fingerprint`.

Matching only the ID is insufficient because content may have changed under an
incorrectly reused name. Capability negotiation rejects a missing profile,
wrong profile ID, wrong fingerprint, future timestamp, expired age,
out-of-range temperature, or excessive drift.

Health also exposes disabled components, unavailable resources, available
channel count, temperature, drift, status, and observation time. The available
channel count may not contradict known disabled calibrated channels.

## Fault-remapping algorithm

For every photonic GEMM, the compiler builds a deterministic calibration
routing plan.

1. Match `disabled_components` against calibrated logical cell IDs.
2. Remove disabled spare cells from the replacement pool.
3. Rank healthy spare cells by gain error, absolute offset, phase error,
   insertion loss, uncertainty, and stable ID tie-break.
4. Sort disabled logical cells by stable ID.
5. Pair each disabled logical cell with the best remaining spare.
6. Reject photonic capability negotiation if healthy spare capacity is
   insufficient.

The resulting `CellRemap` records the disabled cell, replacement cell, logical
row and column, and replacement score. Lowering emits an explicit
`remap_cell` command for every mapping. The simulator retains the original
logical GEMM semantics; the mapping changes physical realization, not the
mathematical operation.

Calibrated cells and the healthy spares required by current faults also feed
tiling. If an active measured cell or required replacement exceeds the
calibration transfer-score threshold, or any logical cell requires remapping,
the compiler derates M, N, and K tile dimensions by half with ceiling. Costing,
autotuning, tile count, lowering, timing, and the recorded decision all consume
that same derated matrix-core shape.

## Wavelength selection

The compiler matches calibrated channels to advertised wavelengths, excludes
disabled channel IDs, ranks the remaining channels by measured transfer score,
and chooses the best deterministic subset permitted by the tuning plan and
live available-channel count. Unmeasured advertised channels remain usable but
rank after measured candidates.

Photonic IR records both stable channel IDs and wavelengths. Device IR emits a
`select_channels` command before matrix execution. Capacity loss is recorded as
the fraction of advertised wavelength channels excluded by current health.

Different valid calibration snapshots can therefore intentionally produce
different channel order, cell remapping, effective transfer compensation,
cost/error estimates, Device IR, and backend-snapshot fingerprint for the same
source graph.

## Effective transfer and numerical impact

The active measured logical cells, selected channels, and replacement cells
are composed with the global calibration transfer. The effective transfer records:

- gain;
- offset;
- phase error;
- insertion loss; and
- uncertainty.

Insertion loss is converted to amplitude attenuation and phase error to its
cosine response before inverse scaling is generated. Photonic IR records the
measured effective gain, offset, phase error, insertion loss, inverse scale,
inverse bias, residual uncertainty, snapshot ID, and exact snapshot
fingerprint. Device IR `rescale` names both the calibration handle and
fingerprint.

The calibration decision estimates error separately from quantization, analog
noise, floating-point accumulation, clipping, and overflow. The full-system
cost model incorporates global and measured-component uncertainty, phase error,
insertion loss, drift, and disabled-channel capacity. Autotuner fingerprints
use the calibration fingerprint, so replacing calibration content invalidates
cached decisions even if a profile ID was improperly reused.

## Calibration decision record

Each logical photonic operation produces one `awen.calibration-decision.v1`
record with:

- operation ID;
- complete disabled-component observation;
- selected and excluded channel IDs;
- cell remaps;
- selected M/N/K tile shape;
- capacity-loss fraction;
- estimated calibration-error fraction; and
- composed effective transfer.

Every lowered tile carries the decision needed to interpret its physical
mapping. The compilation artifact deduplicates tile decisions into one record
per logical operation.

## Compilation artifact provenance

`awen.compilation.v1` records:

- deterministic source-graph fingerprint;
- deterministic full backend-snapshot fingerprint;
- capability and backend identity;
- complete health observation;
- compilation options and negotiation diagnostics;
- placement, partition, Photonic IR, and Device IR;
- calibration snapshot version, ID, fingerprint, parent ID, backend identity,
  topology fingerprint, measured time, measured environment, uncertainty;
- health observation time, health status, and health fingerprint; and
- every calibration decision impact.

This record is sufficient to explain why a route was selected, quantify lost
capacity and modeled error, detect stale execution, and distinguish a replay
against exact historical state from a re-execution against current hardware.

## Artifact invalidation and refresh

`refresh_for_backend(program, artifact, current_snapshot)` is the non-bypassable
compiler refresh boundary for serialized compiler artifacts.

It first verifies that the supplied program has the exact recorded source
fingerprint. A source mismatch is an error rather than an implicit recompile.
If the full backend-snapshot fingerprint is unchanged, the artifact is returned
with action `reused`.

If the snapshot changed, the API reports individual reasons for:

- backend identity;
- calibration ID, fingerprint, or topology;
- calibration presence;
- health observation time or status;
- drift or temperature;
- disabled components; and
- unavailable resources.

It then recompiles using current capability, health, cost, precision, and
partition information under automatic placement. If photonic execution remains
legal, the action is `recompiled`. If the original artifact used photonics but
the refreshed artifact cannot, the action is `fell_back` and the emitted
program uses a diagnosed digital backend. A forced-photonic option from the old
artifact never overrides this safety refresh.

Invalidation reasons are prepended to compilation diagnostics, so the result is
observable and reproducible rather than an invisible retry.

## Replay semantics

Exact replay requires the original source graph and the exact backend snapshot
fingerprint. If historical physical state is unavailable, AWEN must not label a
current-hardware recompile as exact replay. It is a new compilation connected
through calibration parent lineage and invalidation diagnostics.

The deterministic simulator can execute the recorded logical program and
calibration transfer for software conformance. This does not assert that old
physical hardware state can be recreated, nor does it constitute hardware
performance evidence.

## Safe failure behavior

The following conditions reject or remove photonic placement:

- missing, stale, future, cross-backend, cross-topology, or fingerprint-mismatched
  calibration;
- temperature or drift outside the advertised tolerance;
- unavailable matrix core or zero channels;
- disabled logical cells exceeding healthy spare capacity;
- no healthy wavelength selection; and
- any existing precision, accuracy, power, memory, or capability failure.

Forced photonic compilation returns an error containing the exact negotiation
diagnostic. Automatic compilation records CPU/GPU fallback. Refresh always
changes an old forced target to automatic placement before handling changed
hardware state.

## Backwards compatibility

This proposal expands v1 JSON objects under strict unknown-field validation.
Producers and consumers must be upgraded together. Reference capabilities,
health fixtures, plugin manifests, Rust types, Python types, Photonic IR, Device
IR, and schemas are updated atomically in this repository.

The runtime calibration-kernel/state subsystem remains available. This AEP
defines the compiler-facing immutable snapshot and refresh boundary; it does
not replace runtime procedures that acquire measurements or generate a new
calibration lineage node.

## Conformance

The implementation verifies:

- the same graph emits different physical routing for different measured
  snapshots;
- a disabled-cell/channel fixture remaps and preserves GEMM semantics;
- exhausted spare capacity falls back automatically and rejects forced
  photonic compilation;
- drift invalidates a compiled artifact end to end and produces safe fallback;
- a fresh child calibration recompiles with preserved parent lineage;
- an identical snapshot reuses the exact artifact;
- cross-topology calibration fails before compilation;
- Rust and Python enforce exact ID/fingerprint health confirmation;
- generated calibration, capability, health, Photonic IR, Device IR,
  compilation artifacts, and artifact-refresh decisions validate against Draft
  2020-12 schemas; and
- compiler format, strict Clippy, full tests, and runtime schema/plugin tests
  run in the dedicated calibration conformance workflow.
