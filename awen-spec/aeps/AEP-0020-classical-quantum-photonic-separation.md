# AEP-0020: Separate classical and quantum-photonic semantics

Status: Accepted and implemented

## Decision

AWEN uses independent `awen.photonic` and `awen.qphotonic` contracts. They may
share tensor storage, device discovery, scheduling primitives, observability,
artifact storage, plugin loading, and runtime transport, but those shared
layers do not own or weaken dialect correctness.

The mixed `photonic_ir.v5.json` document remains a legacy migration input. It
is not the execution contract for new programs. The typed runtime chokepoint no
longer accepts an arbitrary operation type string and does not validate both
semantic families through the mixed V5 schema.

## Classical photonic contract

`awen.photonic.program.v1` describes deterministic or explicitly noisy
classical analog signal processing. Its closed operation set is GEMM, analog
transform, modulation, and detection. Every operation carries:

- declared classical signal inputs and outputs;
- precision, DAC, ADC, and accumulator widths;
- absolute and relative numerical tolerances;
- an explicit seeded analog-noise model;
- an immutable calibration snapshot and fingerprint;
- a calibrated transfer-function model and residual error; and
- dependency-aware timing.

Classical validation requires non-zero precision, calibrated transfer identity,
valid signal references, operation-specific arity and parameters, and an
explicit capability set. There are no shots, quantum modes, state collapse,
Fock cutoffs, coherence budgets, or statistical correctness fields in this
contract.

## Quantum-photonic contract

`awen.qphotonic.program.v1` describes quantum modes, state preparation, gates,
measurement, sampling, feed-forward, coherence, capabilities, and statistical
correctness. State spaces are explicit:

- Fock/discrete-variable modes have a finite cutoff and occupation vector; and
- Gaussian continuous-variable modes have finite displacement and symmetric
  positive-semidefinite covariance data with positive diagonal.

The gate set is closed to beam splitter, phase shift, squeeze, displace,
controlled-X, and Fourier operations. Fock-only and Gaussian-only gates are
verified against their mode state space. Measurements are photon counting for
Fock modes and homodyne/heterodyne for Gaussian CV modes.

Feed-forward must consume a preceding named measurement, target a later gate,
name one supported control parameter, match that gate parameter, and declare a
positive latency limit. The sum of operation coherence costs must fit the
program coherence budget.

Quantum execution fixes shots, seed, and deterministic replay. Correctness is
statistical: expected outcome distribution, total-variation limit, optional
expected means, mean-error limit, fidelity floor, and confidence floor.
`awen.qphotonic.result.v1` validates shot totals, distribution distance, means,
fidelity, confidence, coherence elapsed, and a SHA-256 replay fingerprint bound
to the complete program fingerprint, program ID, seed, shot count, counts,
means, fidelity, confidence, and elapsed coherence.

## Narrow interoperability

`awen.photonic-interop.v1` contains only two boundary operations:

- `measurement_readout`, which exports a named quantum measurement into a
  named classical output; and
- `classical_control`, which applies a named classical input to a named later
  quantum gate parameter with scale, offset, and maximum latency.

No generic cast converts a classical signal into quantum state, quantum state
into an analog tensor, or optical amplitude into measurement outcome. A
compiler or runtime must preserve the explicit boundary operation in plans,
artifacts, profiling, and capability negotiation.

## Runtime execution boundary

The runtime `PhotonicProgram` is a closed Rust enum over classical, quantum,
and interop programs. `NonBypassableGateway` dispatches on that enum, runs the
dialect's Rust verifier, validates the independent JSON Schema, records the
entire typed contract and fingerprint in the artifact bundle, and requests a
dialect-specific plugin capability such as `execute:awen.qphotonic`.

The gateway does not inject classical calibration into quantum programs. It
does not erase a quantum program into the legacy node type string before
validation. Shared artifacts record the exact typed program alongside the
dialect identity.

## MLIR dialects

The textual namespaces remain `awen_photonic` and `awen_qphotonic` because
MLIR dialect names end at the first period.

`awen_photonic` provides a classical signal marker plus calibrated transform,
modulation, detection, and GEMM tile operations. `awen_qphotonic` provides
distinct Fock state, Gaussian state, and sample-stream types. State-space-
specific operations make illegal crossings fail MLIR verification; for
example, a Fock state cannot be an operand of the Gaussian `squeeze` operation.

Quantum MLIR operations carry seed, shots, coherence cost/budget, confidence,
and statistical tolerance properties rather than reusing classical precision
or calibration attributes. The existing classical StableHLO GEMM lowering
never passes through `awen_qphotonic`.

The MLIR surface covers both Q- and P-quadrature homodyne measurement and uses
separate typed feed-forward operations for phase, Q displacement, P
displacement, and squeezing controls. Each operation has an independent
verifier for numeric ranges, finite parameters, coherence cost, sampling, and
latency.

## V5 migration

`awenctl migrate-photonic-v5` reads only V5 JSON and writes
`awen.photonic-v5-migration.v1`. The migrator has an allowlist of dialect-
prefixed legacy operation names. It classifies recognized operations but does
not invent missing precision, calibration, quantum state, sampling, or
correctness semantics.

Before classification, the complete source document must validate against the
frozen legacy V5 schema. A malformed or non-V5 document is not presented as a
migration report; a structurally valid V5 document always preserves its
classified operations and ambiguity diagnostics in the report.

Unprefixed names such as `measurement`, `splitter`, `phase_shift`, or
`beam_splitter` are ambiguous and produce an error diagnostic. Unknown names
also produce an error. A document containing both recognized semantic families
emits an explicit-interoperability warning. The report is preserved even when
migration is rejected, and the command exits unsuccessfully until all error
diagnostics are resolved.

Migration classification is deliberately not a complete executable program.
Operators must supply the missing dialect-specific contracts and explicit
interop operations after reviewing the report.

## Dependency policy

The historical review inspected the archived upstream
[XanaduAI/strawberryfields](https://github.com/XanaduAI/strawberryfields)
implementation, specifically its `Program`, operation base classes, and
`Result` model. Useful prior-art boundaries were:

- programs track live subsystem references, reject duplicate subsystem use,
  and preserve command order/DAG dependencies;
- gates, channels, and deferred measurements have distinct operation classes;
- measured-register dependencies make feed-forward explicit;
- compilation checks a program against a target device instead of erasing the
  program into backend strings; and
- results distinguish shot-indexed samples from optional simulator-only state.

AWEN adopts those architectural lessons only. Its owned contract adds the
classical/quantum dialect split, explicit interoperability, calibration and
precision rules, statistical acceptance thresholds, coherence accounting,
content-bound replay evidence, independent schemas, and Rust/MLIR verifiers.
Strawberry Fields is not a build, runtime, schema, source-code, or test
dependency.

## Acceptance evidence

- Rust and JSON Schema reject cross-dialect operation substitution.
- Classical tests cover precision, noise, calibrated transfer functions, and
  the typed runtime boundary.
- Quantum tests cover Fock sampling, Gaussian CV measurement/feed-forward,
  coherence, statistical correctness, and seeded replay.
- Interop tests validate the only two permitted cross-dialect operations.
- V5 tests preserve error diagnostics for ambiguous operations and verify both
  rejected and successful CLI reports.
- MLIR text and bytecode round trips retain classical operations, Fock and
  Gaussian states, sample streams, measurement, and feed-forward.
- MLIR diagnostics reject a Fock state passed to a Gaussian gate and reject
  invalid numeric contracts such as a zero-shot measurement.
