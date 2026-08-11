# AEP-0004: Versioned plugin and backend-capability system

Status: Implemented foundation

## Decision

AWEN plugins use `awen.plugin-manifest.v1`. Hardware and simulator plugins may
embed one `awen.device-capability.v1` document and a live-health query. Static
capabilities describe what a backend can do; `awen.backend-health.v1` describes
what the particular device can safely do at a specific observation time.

The compiler must never infer missing physical data. A capability document is
accepted only after structural, numerical, cross-field, capability-version,
runtime-ABI, and plugin-ABI validation. A valid but currently unavailable,
uncalibrated, stale, thermally out-of-range, drifted, or resource-constrained
backend remains discoverable but is ineligible for photonic placement. `auto`
therefore emits a diagnosed digital fallback. A forced photonic target fails.

## Version contracts

- Capability schema: `awen.device-capability.v1`
- Live health schema: `awen.backend-health.v1`
- Plugin manifest schema: `awen.plugin-manifest.v1`
- Runtime/backend ABI: `awen.runtime-backend.v1`
- Backend plugin ABI: `awen.backend-plugin.v1`

The strings are compatibility identities, not package versions. This
implementation accepts the exact v1 identities. Unknown or future identities
produce an explicit diagnostic before operation negotiation or execution.
Additive compatibility requires a new documented schema revision; incompatible
meaning requires a new major identity.

## Static capability contract

A backend advertises:

- matrix-core M/N/K dimensions;
- operation legality, transpose support, and partial M/N/K tile rules;
- scalar and complex dtypes;
- optical effective bits, ADC/DAC bits, bit-slicing modes, saturation behavior,
  and accepted input dynamic range;
- wavelengths, simultaneous channels, coherence mode, modulation rate, sample
  rate, and detector bandwidth;
- reconfiguration latency, host bandwidth, link bandwidth, and
  optical/electrical boundary latency;
- insertion-loss budget, laser power, total power budget, and ADC/DAC energy;
- accumulation modes and complex-arithmetic support;
- calibration requirements and an optional typed calibration profile.

Cross-field validation rejects, among other contradictions, zero matrix axes,
duplicate resources, a channel count larger than the wavelength set, complex
dtypes without complex support, effective precision beyond converter precision
without bit slicing, laser power above the total power budget, invalid dynamic
ranges, cross-device calibration, zero calibration gain, and non-finite values.

The reference 2x2 and PACE-like 128x128 profiles are simulator inputs. Their
numbers are neither measured hardware facts nor performance claims.

## Dynamic health contract

A health snapshot records backend identity, observation time, health status,
temperature, drift, currently available channels, disabled components,
unavailable resources, and the active calibration-profile identity. The
snapshot is validated against its static capability document.

Calibration age is computed from `health.observed_at -
calibration.measured_at`. The compiler does not compare against its own wall
clock. This makes compilation, replay, tests, and artifact fingerprints
deterministic for a fixed capability/health pair.

The first live-health query is a file provider. The runtime resolves relative
health paths inside the plugin directory, rejects absolute and parent-traversal
paths, and re-reads the file on every query. The same interface can later gain
authenticated RPC or in-process providers without changing compiler source or
the capability schema.

## Discovery and signing

Production discovery registers only manifests whose Ed25519 signature verifies.
Unsigned discovery is an explicit development/simulator option. Signatures cover
the canonical serialized manifest with `signature` and `public_key` cleared.
Backend discovery returns validated snapshots and per-plugin diagnostics; a bad
backend does not make unrelated plugins disappear.

Manifest capability names are indexing hints. Compiler legality comes only from
the typed backend capability and health contracts.

## Negotiation

GEMM negotiation evaluates:

- live backend and matrix-core availability;
- usable wavelength channels;
- operation, dtype, transpose, and partial-tile support;
- requested effective precision;
- calibration identity, age, temperature tolerance, and drift tolerance.

Every rejection has a stable machine-readable code and human-readable message.
Compilation artifacts retain both the health snapshot and negotiation results.

## Security and failure behavior

- Unknown fields are rejected by Rust deserialization and JSON Schema.
- Non-finite or contradictory numeric values are rejected.
- Health files cannot escape the plugin directory.
- Missing critical capability fields prevent registration.
- Unavailable resources produce conservative fallback, never optimistic
  execution.
- Required calibration cannot be bypassed by omitting a profile or health ID.
- ABI and schema version skew is diagnosed before plugin invocation.

## Schemas and reference implementation

- `awen-spec/schemas/awen_device_capability.v1.json`
- `awen-spec/schemas/awen_backend_health.v1.json`
- `awen-spec/schemas/awen_plugin_manifest.v1.json`
- `awen-compiler/src/capability.rs`
- `awen-runtime/src/plugins/registry.rs`
- `awen-ecosystem/python_awen/awen_py/capabilities.py`
- `awen-runtime/plugins/reference_sim/backend-manifest.json`
