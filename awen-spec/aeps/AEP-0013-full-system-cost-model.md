# AEP-0013: Full-system cost model and deterministic autotuning

Status: Accepted and implemented

## Summary

AWEN defines `awen.cost-model.v1` as the versioned contract for comparing
digital and photonic lowerings. A comparison is valid only when it includes the
complete execution path: scheduling and queueing, host/link transfer, memory,
layout conversion, optical/electrical boundaries, reconfiguration, calibration,
DAC, modulation, propagation, detection, ADC, accumulation, laser power, and
support-system power. Optical propagation time alone is never a placement cost.

Every estimate carries an expected value, lower and upper uncertainty bounds,
a component breakdown in explicit units, and parameter provenance. The
autotuner deterministically selects a legal tiling, bit-slicing, wavelength,
accumulation, batching, and fusion plan for a fixed graph, device, calibration
snapshot, model, objective, and seed.

## Motivation

Photonic arithmetic can be much faster than its surrounding conversion and
movement path while still losing end-to-end. It can also satisfy a numerical
contract only through extra bit slices or digital accumulation. The compiler
therefore needs a cost contract that makes physical assumptions inspectable,
calibratable, reproducible, and comparable with a digital baseline.

## Inputs

The operation profile contains tensor shape, dtype, layouts, structured or
unstructured sparsity, input error, requested absolute error, and requested
relative error. The tuning context contains batch size, queue depth, overlap,
input residency, and whether boundary fusion is permitted.

The device and health snapshot supplies matrix dimensions, supported precision
and bit slicing, wavelengths and available channels, conversion and modulation
rates, host/link bandwidths, boundary and reconfiguration latency, DAC/ADC
energy, detector bandwidth, laser and total power, calibration uncertainty,
drift, and disabled-channel fraction. Cost-model parameters supply memory,
modulator, detector, accumulation, optical propagation, support-system,
calibration-amortization, SNR, insertion-loss, and fitted correction values.

If a required model value is absent, non-finite, dimensionally invalid, outside
its declared fraction range, or lacks provenance, `auto` and explicit digital
placement conservatively select CPU. Forced photonic placement fails with a
diagnostic; it must never bypass an incomplete comparison.

## Units and equations

Latencies are nanoseconds, bandwidths are gigabits per second, rates are
gigasamples/gigabaud/gigaoperations per second, power is milliwatts, per-event
energy is picojoules, and aggregate energy is microjoules. At those scales:

```text
transfer_ns = bytes * 8 / bandwidth_gbps
conversion_ns = samples / rate_gsps
power_energy_uJ = power_mW * latency_ns / 1,000,000
event_energy_uJ = events * energy_pJ / 1,000,000
throughput_GOPS = operations / latency_ns
```

Structured sparsity may reduce transferred elements and MACs. Unstructured
sparsity remains dense unless a backend advertises a legal structured lowering.
Resident input bytes are removed from host transfer. The declared overlap
fraction reduces only components that can overlap; it does not erase scheduling,
reconfiguration, propagation, or accumulation. Queue depth contributes explicit
scheduling delay. Batching amortizes one-time scheduling, reconfiguration, and
calibration costs, while an unfused batch retains two conversion boundaries per
batch.

Insertion loss raises the laser-energy requirement and lowers effective SNR.
Estimated numerical error includes the larger of quantization or SNR error,
calibration uncertainty, drift, input error, fitted error, and an
accumulation-mode penalty. Bit slicing raises effective precision and adds
conversion and modulation work. Estimates are clamped to physically meaningful
non-negative ranges.

## Provenance and uncertainty

Each parameter group declares one of:

- `measured`: obtained from immutable benchmark artifacts;
- `vendor_specified`: copied from a named device specification;
- `simulated`: produced by a named simulator and configuration;
- `assumed`: a documented conservative compiler default.

Each entry identifies its reference and uncertainty fraction. Estimate bounds
use the maximum applicable uncertainty so an artifact never presents an
assumption as an exact value. Reference backends use simulated provenance;
unknown backends use assumed provenance until supplied with better data.

## Autotuning

The candidate space is the legal cross-product of full and reduced matrix tiles,
available wavelength counts, advertised accumulation modes, the minimum number
of precision slices, batch size, and permitted boundary-fusion states. Each
candidate has latency, energy, error, throughput, and a complete breakdown.

`optimize_for` selects the comparison field:

- `latency`: minimum expected end-to-end latency;
- `energy`: minimum expected full-system microjoules;
- `accuracy`: minimum estimated numerical error;
- `throughput`: maximum end-to-end GOPS.

Equal scores use a deterministic seed-dependent plan hash followed by a stable
lexicographic key. The result records the selected candidate, ranked alternatives,
their scores, and an explanation. Lowering must consume the selected plan; the
plan is not advisory metadata.

## Cache identity and invalidation

The decision fingerprint covers cost-model version and values, operation shape,
dtype, layouts, sparsity and error contract, backend and capability version,
calibration identity, effective bits, available channel count, objective, seed,
batch size, fusion permission, queue depth, overlap, and residency. Any change
produces a different fingerprint. Cache users retain only entries whose complete
fingerprint matches the current graph/device/calibration context.

## Benchmark fitting and model error

A versioned `awen.cost-observations.v1` hardware or external simulator
observation names its operation and immutable
artifact, source class, observed latency, observed energy, and observed numerical
error. A benchmark report compares it with the selected prediction and records
relative latency error, relative energy error, and numerical-error delta.

`CostModelInputs::calibrated_from_reports` fits dimensionless latency and energy
correction factors and a non-negative numerical-error offset from one or more
reports. The resulting model adds measured provenance naming every source
artifact and remains subject to normal validation. Raw observations and fitted
models must be retained for reproducibility.

## Backwards compatibility

The schema is new and independently versioned. Compilation artifacts retain
`awen.compilation.v1`; added tuning and cost fields are additive for serde-based
consumers. `CompileOptions` supplies defaults when older serialized options omit
new controls. Classical Photonic IR adds required `bit_slices` within its v1
precision plan because the executable semantics were previously incomplete;
consumers must update their v1 validator.

## Test plan

- Verify every total equals the sum of its named components.
- Verify nanosecond, picojoule, microjoule, and GOPS boundaries.
- Reject NaN, infinity, zero denominators, invalid fractions, empty provenance,
  zero tiles, zero slices, zero wavelengths, and zero batches.
- Verify layout, sparsity, queueing, overlap, residency, loss, drift, disabled
  components, batching, and fusion move only the intended terms.
- Verify all four objectives and deterministic replay under a fixed seed.
- Verify selected plans change emitted tile size, wavelengths, precision slices,
  accumulation, and timing.
- Verify graph/device/calibration/model/profile changes invalidate fingerprints.
- Verify predicted-versus-observed reports and measured model calibration.
- Verify incomplete model inputs force CPU in `auto` and fail forced photonic.
