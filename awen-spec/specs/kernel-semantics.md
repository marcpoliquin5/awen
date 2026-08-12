# Kernel Semantics v1.0

This specification defines the common requirements for every AWEN executable
kernel. AEP-0015 and the `awen_blas*.json` schemas are normative for the
initial awenBLAS registry.

## Required declaration

Every kernel request and result must declare or carry through:

- a version and stable request identifier;
- operation kind and natural operator structure;
- ordered input and output tensor identifiers, shapes, dtypes, layouts, and
  real or explicit complex representation;
- bounded attributes with defaults that are part of the versioned semantics;
- absolute-error, relative-error, and optional effective-bit requirements;
- accumulation mode;
- an explicit phase convention for every complex/Fourier operation;
- all calibration identities consumed by execution;
- an operation-count estimate and provenance-bearing cost inputs;
- the concrete execution target and whether execution was simulated; and
- deterministic request/plan/result fingerprints.

Unknown fields, non-finite numbers, zero dimensions, data-length mismatches,
unsupported ranks, incompatible inner dimensions, dtype/representation
mismatches, and invalid attribute ranges must be rejected before execution.

## Composition and lowering

Kernel calls are pure with respect to their materialized tensor inputs,
attributes, calibration inputs, and simulator options. Equal inputs and seeds
must produce equal CPU-reference and deterministic-simulator outputs. State is
represented as an explicit input/output tensor, including reservoir state; it
must not be hidden in a backend instance.

Natural structure is semantic metadata. Low-rank, random-projection, Toeplitz,
circulant, block-circulant, convolutional, Fourier, beamforming, reservoir, and
propagation calls must retain their structure through capability matching and
planning. Densification, factorization, Fourier conversion, or other changes of
representation are compiler transformations with separately modeled costs;
they are not implicit kernel fallback.

Nonlinear operations such as softmax and activation functions form explicit
graph boundaries unless a registered kernel includes them in its mathematical
definition. In the v1 registry, attention score/value calls exclude masking
and softmax, while `reservoir_step` explicitly includes `tanh`.

## Scheduling and backend selection

Each backend must advertise exact kernel kinds, dtypes, structures, complex
support, tensor capacity, effective precision, calibration requirement, launch
latency, throughput, energy per operation, expected error, and provenance.
Support is conjunctive: a candidate is legal only when all requested
properties are supported. The CPU reference candidate is always present.

Kernel-local dispatch deterministically selects by the requested latency,
energy, accuracy, or throughput objective and records all rejected candidates.
Whole-graph scheduling additionally follows AEP-0013 and AEP-0014 for transfer,
residency, optical/electrical boundary, queueing, overlap, and memory costs.

## Calibration and measurement

Calibration inputs must have stable identities and validated parameters. A
backend declaring calibration mandatory is ineligible without one. A result
must retain the identities it used, and a hardware result must also retain the
device, environment, validity interval, and raw evidence required by the
calibration-aware compiler contract.

A performance report must distinguish measured, vendor-specified, simulated,
and assumed data. Measured reports must state their complete boundary. The v1
software benchmark includes validation, quantization, kernel execution,
calibration transfer and inverse compensation, deterministic noise, and output
materialization. It is software conformance evidence, not measured photonic or
GPU performance.

## Compatibility and conformance

Changing an existing kernel's tensor order, shape rules, layout interpretation,
phase sign, inverse normalization, transpose behavior, random generator,
structured representation, accumulation meaning, calibration behavior, or
error comparison requires a new major contract. Additive kernel kinds require
a schema update, exact semantic text, a CPU reference, simulator coverage,
known-answer vectors, randomized properties where applicable, capability and
fallback tests, and end-to-end benchmark coverage in the same change.
