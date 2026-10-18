# Integrating photonic hardware with AWEN

This guide is for teams that own a photonic, hybrid electronic-photonic or
quantum-photonic device and want AWEN to compile for it and run on it. It
covers what AWEN already provides, what a hardware owner writes, and the
checks a backend must pass before anyone should trust its results.

AWEN ships **no vendor drivers**. Everything in this repository that looks
like a device is a reference simulator, and it is labelled as one. A real
driver can only be written against the real device and its SDK, by someone
who has both.

## 1. The pieces

| Piece | Where | What it is for |
| --- | --- | --- |
| Hardware description | `awen-spec/schemas/awen_hardware_description.v1.json` | Physical components behind a backend: MZI meshes, microring banks, WDM, diffractive layers, detectors, ADCs, DACs, lasers, reservoirs, memory, quantum sources and detectors. Every document states its provenance. |
| Device capability | `awen-spec/schemas/awen_device_capability.v1.json` | What the compiler may target: matrix core shape, operations, dtypes, effective bits, noise, calibration requirements. |
| Backend health | `awen-spec/schemas/awen_backend_health.v1.json` | Live status, temperature, drift, disabled components. Re-read on every discovery query. |
| Plugin manifest | `awen-spec/schemas/awen_plugin_manifest.v1.json`, `awen-runtime/src/plugins/registry.rs` | Identity, version, capability names, the embedded capability + health query, and an optional Ed25519 signature. |
| Plugin process boundary | `awen-runtime/src/plugins/loader.rs` | Plugins run as separate executables that exchange JSON over stdin/stdout. No dynamic linking of vendor code into the runtime. |
| Device control traits | `awen-runtime/src/hal/mod.rs` (`Device`, `LabDevice`) | `set_param` / `read_sensor`, calibration application under `SafetyLimits`, and health reports. |
| Reference simulator drivers | `awen-runtime/src/hal/hardware.rs` | Worked examples of `Device` built from a hardware description: Clements MZI mesh, all-pass microring bank, detector + ADC readout chain. |
| Numerical contracts | `awen-spec/schemas/awen_photonic_noise_contract.v1.json`, `awen_precision.v1.json` | How noise, precision and error bounds are declared and checked. |

The reference plugin at `awen-runtime/plugins/reference_sim/` is the smallest
complete example of a manifest plus a live health file.

## 2. Describe the hardware honestly

Start with an `awen.hardware-description.v1` document. The fixture
`awen-spec/fixtures/hardware_reference_simulator.v1.json` uses every component
class and is checked in CI by `scripts/validate_json.py` and by
`awen-runtime/tests/hardware_description.rs`.

`provenance.source` is required and must be one of:

- `reference_simulator`: illustrative values for a simulator. Not a product.
- `vendor_datasheet`: values copied from a datasheet. Put the document and
  revision in `provenance.reference`.
- `measured`: values measured on a specific unit. Give `measured_at` and
  describe the method in `notes`.

Units are part of the field names (`_db`, `_nm`, `_ghz`, `_mw`, `_v`,
`_na`, `_a_per_w`). Unknown fields are rejected, so a typo cannot hide a
missing value. When a component does not fit an existing class, propose a
schema change through an AEP (`awen-spec/aeps/`) rather than stuffing extra
fields into an existing class.

## 3. Write the driver

A driver implements `Device` (and `LabDevice` if it controls lab hardware):

- `id()` and `capabilities()` return stable descriptors.
- `set_param(name, value)` applies a named control (for example
  `mzi_3:theta`, `ring_1:phase`, `pd_0:power_w` in the reference drivers).
  Reject non-finite values and unknown names with an error. Never clamp
  silently.
- `read_sensor(name)` returns a named observable.
- `LabDevice::apply_calibration` must enforce `SafetyLimits` before touching
  hardware.

The reference drivers in `hal/hardware.rs` show the expected shape. Each one
is built from its hardware-description component, rejects configurations it
does not model (the MZI driver accepts the Clements topology only, the ring
driver all-pass rings only) and is tested against closed-form physics:

- MZI mesh: N(N-1)/2 MZIs, unitary to 1e-12 when lossless, single-MZI
  splitting `cos²(θ/2)` / `sin²(θ/2)` scaled by the insertion loss.
- Microring: all-pass transmission
  `(a² − 2ar cos φ + r²) / (1 − 2ar cos φ + (ar)²)`, zero at critical
  coupling, unity when lossless.
- Detector + ADC: `I = R·P + I_dark`, linear quantization with saturation,
  monotone codes.

A vendor driver has to meet the same standard against its own device model
and then against measurements.

Package the driver as a plugin executable plus a manifest. The runtime
discovers manifests with `PluginRegistry::discover_from_dir` and invokes the
executable through `PluginLoader`. Production deployments should sign
manifests and pin the accepted signer keys; unsigned manifests are accepted
only with the explicit `--allow-unverified` development flag.

## 4. Acceptance checklist

A backend is ready for others to use when **all** of the following hold. Items
marked *hardware* cannot be satisfied by a simulator.

1. The hardware description validates against
   `awen_hardware_description.v1.json`, and its provenance is accurate.
2. The device capability and health documents validate, and
   `awenctl backends <dir>` discovers the backend without diagnostics.
3. The manifest is signed and the signer key is pinned by the deployment.
4. The driver rejects unknown parameters, non-finite values and
   out-of-range indices, and enforces `SafetyLimits` (unit tests).
5. The driver's device model matches closed-form or independently simulated
   physics for every component it exposes (unit tests, as in
   `tests/hardware_description.rs`).
6. GEMM and every claimed awenBLAS kind pass the conformance suites
   (`awen-compiler/tests/awenblas_kernels.rs`, the runtime awenBLAS
   conformance tests) within the declared accuracy contract on the
   simulator.
7. *hardware*: the same suites pass on the device, and the measured error is
   reported next to the simulator prediction.
8. *hardware*: calibration snapshots are produced, versioned and checked for
   age and temperature as the capability requires.
9. *hardware*: latency and energy figures are measured with a documented
   method and marked `measured`. Anything estimated stays `estimated` or
   `simulated`.
10. Health transitions (healthy, degraded, unavailable) are exercised, and
    the runtime stops placing work on unhealthy resources.

## 5. What is not provided

- No driver for any commercial device or SDK.
- No measured performance, energy or accuracy figure for any hardware. The
  reference descriptions are simulator inputs.
- Quantum components are described by the schema, but the runtime executes
  quantum-photonic programs only in simulation.
