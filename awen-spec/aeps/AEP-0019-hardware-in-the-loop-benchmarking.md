# AEP-0019: Reproducible full-system and hardware-in-the-loop benchmarking

Status: Accepted and implemented

## Summary

AWEN benchmark evidence is produced from one versioned suite manifest applied to
every configured available backend. A suite fixes the exact tensor fixture,
accuracy contract, warmup, repetitions, seed, backend identities, driver
protocol, power/accounting inputs, and regression policy. The orchestrator
records raw repetition data, recomputes distributions and accuracy against the
same CPU reference, verifies complete accounting, and writes a content-addressed
artifact set with SHA-256 checksums.

CPU reference and deterministic simulator runners are built in. CUDA devices,
lab rigs, and physical accelerators use a timeout-bounded external-driver
protocol over JSON on standard input and output. Drivers execute without a
shell, must return the requested commit and runner identity, and must identify
the source of execution, latency, energy, power, accuracy, calibration, and
environment data independently as measured, simulated, vendor-specified, or
estimated.

Reference conformance runs in ordinary CI. Physical measurements run only in a
manual workflow on a self-hosted runner labeled `awen-hardware`. Required
thresholds are permitted for deterministic reference evidence. Lab-rig and
hardware thresholds are advisory, so noise and unavailable equipment cannot
make ordinary pull requests flaky.

Public claim generation is fail-closed. It accepts only a verified artifact,
an immutable HTTPS URL with that artifact's SHA-256 digest in its final path
segment and no query or fragment, a measured
baseline, and a measured lab-rig or hardware-accelerator result. Both backends
must pass the accuracy contract. Calibrated hardware must identify an immutable
calibration snapshot. Lower-latency and lower-energy claims are emitted only
when both recomputed ratios are greater than one.

## Motivation

Optical propagation time is not application latency. An acceleration claim that
omits host transfer, memory, scheduling, reconfiguration, calibration,
conversion, digital accumulation, or support power is not comparable with a CPU
or GPU application measurement. Likewise, a simulator result, a vendor data
sheet value, and a physical measurement are not interchangeable evidence.

Before this proposal, AWEN had a full-system cost model and software-conformance
benchmarks, but it had no common physical-driver protocol, no raw per-repetition
HIL artifact, no percentile/power/error distribution contract, no manual lab
workflow, and no mechanism preventing public prose from being generated from
mutable, simulated, or unverified inputs.

## Normative contracts

This proposal defines:

- `awen.hil-suite.v1`, serialized by `awen_hil_suite.v1.json`;
- `awen.hil-driver.v1`, serialized by `awen_hil_driver.v1.json`;
- `awen.hil-artifact.v1`, serialized by `awen_hil_artifact.v1.json`; and
- `awen.benchmark-claims.v1`, serialized by
  `awen_benchmark_claims.v1.json`.

Unknown fields are rejected by Rust deserialization and JSON Schema. The tensor
fixture is an `awen.blas.v1` request, so its shape, layout, dtype, numerical
contract, operation semantics, and literal data are shared by every backend.

## Suite identity and comparability

An `awen.hil-suite.v1` manifest contains a stable suite ID and description,
complete literal kernel fixture, warmup count, measured repetition count,
deterministic seed, and one or more uniquely identified backends.

Each backend declares exactly one runner:

- `cpu_reference` for portable CPU reference execution;
- `simulator` for a concrete GPU or photonic simulation target; or
- `external_command` for CUDA, a lab rig, or a physical accelerator.

The orchestrator sends the same fixture, warmup, repetitions, seed, commit SHA,
and runner ID to every backend. An artifact is verified only if every configured
available backend produces a result and every result passes the declared
absolute-or-relative accuracy contract.

The canonical `benchmarks/reference_hil_suite.json` is software evidence only.
Its wall-clock latency is measured, while its built-in power and energy are
explicitly tagged estimated. Its photonic execution and numerical accuracy are
tagged simulated. It cannot produce a physical hardware acceleration claim.

## Required full-system boundary

Every raw latency sample records the following non-negative components, whose
sum must equal reported application latency:

1. host transfer;
2. memory;
3. scheduling;
4. reconfiguration;
5. calibration amortization;
6. DAC;
7. modulation;
8. optical-device execution;
9. detection;
10. ADC;
11. digital post-processing; and
12. cooling/support overhead when material.

Every raw energy sample records host transfer, memory, scheduling,
reconfiguration, calibration amortization, laser, modulation, DAC, optical
device, detector, ADC, digital post-processing, and cooling/support energy.

The built-in runners accept normalized accounting shares and multiply them by
measured host wall-clock latency and the explicitly estimated steady-power
input. External physical drivers return actual component values and identify
each metric's evidence source. Component sums are validated for every
repetition. Optical-device time is one component and can never substitute for
application latency.

## Raw evidence and derived metrics

Every backend result preserves one raw sample per repetition with iteration
number, full-system latency and component breakdown, joules and component
breakdown, peak and steady power, optional measured temperature, and named raw
instrument counters.

The artifact also preserves every per-element absolute and relative error and
one SHA-256 output checksum per repetition. From raw samples, the orchestrator
derives minimum, p50, p95, p99, maximum, and mean for latency, throughput,
energy, peak power, steady power, absolute error, and relative error. It also
records calibration duration and latency/energy conversion shares.

Artifact validation recomputes these distributions, conversion shares, the
accuracy verdict, the fixture digest, every component sum, and the complete
artifact digest. A recorded summary that differs from raw evidence is rejected.

## Hardware and software environment

Every backend environment records hardware vendor and model, topology, clock
summary, temperature when available, software versions, operating system, exact
commit SHA, stable runner ID, calibration snapshot ID and SHA-256 fingerprint
when applicable, RFC 3339 observation time, and an explicit list of unavailable
fields.

An external response whose commit or runner identity differs from the driver
request is rejected. Calibration ID and fingerprint must appear together.
Unavailable sensors are stated rather than silently represented as zeros.

## External driver protocol

The orchestrator starts the configured executable directly with its argument
array; it does not invoke a shell. It writes one `awen.hil-driver.v1` request to
standard input and closes the stream. The driver writes one JSON response to
standard output. Diagnostic text belongs on standard error. The orchestrator
reads output concurrently to avoid pipe deadlock, enforces the configured
timeout, kills a timed-out process, rejects a non-zero exit, and preserves the
failure in the aggregate artifact.

The request contains the suite/backend IDs, complete fixture, warmup,
repetitions, seed, commit SHA, and runner ID. The response contains metric
sources, environment, calibration duration, raw samples, output samples, and
driver-specific raw data. Output samples are compared with the CPU reference;
drivers do not supply their own pass/fail verdict.

## Artifact set and immutable identity

`awenctl benchmark-suite` writes a new or empty directory containing:

- `suite.json`, the complete canonicalized input suite;
- `benchmark-<sha256>.json`, the complete aggregate artifact; and
- `SHA256SUMS`, covering both JSON files.

The artifact records SHA-256 fingerprints of the suite, fixture, and artifact.
It also embeds the exact suite and binds every result/failure identity and class
to that suite, so a claims consumer can verify the backend contract without a
second mutable lookup.
The artifact digest is computed from normalized JSON with its own digest field
empty, then stored in the filename and object. Normalization makes the content
identity stable across a JSON write/read round trip, including finite
floating-point values. The command preserves a rejected artifact for diagnosis
but exits unsuccessfully when verification fails. It refuses to mix new
evidence into a non-empty output directory.

## Regression policy

Regression thresholds may cover p95 latency, p95 energy, p50 throughput, p99
absolute error, and p99 relative error. Each policy identifies its reference
artifact.

`required_reference` failures reject artifacts and are reserved for
deterministic reference paths. `advisory_hardware` findings are recorded but do
not reject the artifact. A lab-rig or hardware-accelerator backend configured
with required thresholds is invalid. This separation prevents noisy physical
measurements from becoming flaky required pull-request checks while retaining
visible regression evidence.

## Claim generation

`awenctl benchmark-claims` takes an artifact file, immutable HTTPS artifact URL,
baseline backend, and candidate backend. It refuses generation unless:

- the artifact's content digest and all derived metrics validate;
- artifact verification is `verified`;
- the URL has the artifact's SHA-256 digest in its final path segment and has no
  query string or fragment;
- the candidate is a lab rig or hardware accelerator;
- latency, energy, and accuracy are measured for both results;
- both results pass the fixture accuracy contract;
- calibrated hardware records its calibration snapshot; and
- measured p50 latency and energy ratios are both greater than one.

Successful generation writes `awen.benchmark-claims.v1` JSON and Markdown. Each
statement links to immutable evidence and names the full-system boundary.
Simulator, vendor-specified, estimated, mutable, inaccurate, or slower evidence
cannot produce lower-latency/lower-energy language through this path.

## Automation

`HIL benchmark reference conformance` runs on relevant pull requests and main
updates. It formats, lints, tests protocol/artifact/claims integrity,
meta-validates all schemas, runs the canonical suite with one command, and
uploads its content-addressed software-reference artifact.

`Manual physical hardware benchmark` is workflow-dispatch only. It targets a
self-hosted runner labeled `awen-hardware`, verifies a clean exact revision,
requires the main branch, restricts input to a tracked manifest resolving
inside the checkout, builds the orchestrator, runs the selected repository
manifest, and uploads any resulting physical artifact set even when its
verification is rejected. It uses a concurrency group without cancellation so
one lab experiment cannot interrupt another. Real hardware is neither presumed
available nor made a required pull-request dependency.

## Security and limitations

External drivers are trusted device adapters and receive literal benchmark
inputs. Manifests must be reviewed like executable configuration. Direct
process execution avoids shell interpretation, but driver binaries and their
arguments still require repository and lab-operator review.

SHA-256 provides content identity, not an assertion that a physical instrument
was honest. Repository workflow provenance, controlled self-hosted runners,
retained raw counters, environment capture, and independent reruns establish
the evidence chain. The repository ships no measured physical-accelerator
artifact and makes no hardware acceleration claim merely because this protocol
exists.

## Acceptance evidence

- one CLI command runs every backend in the reference manifest and emits one
  comparable content-addressed artifact set;
- protocol tests validate suite, request, response, artifact, and claims
  schemas through cross-schema references;
- boundary tests prove component sums equal application totals and application
  latency exceeds the optical-device component;
- integrity tests recompute percentile, throughput, energy, power, error,
  conversion, accuracy, fixture, and artifact identities from raw evidence;
- regression tests prevent required lab/hardware thresholds;
- claims tests reject mutable and simulated inputs and validate successful
  content-bound measured-hardware output; and
- normal CI and manual self-hosted hardware workflows are separate.
