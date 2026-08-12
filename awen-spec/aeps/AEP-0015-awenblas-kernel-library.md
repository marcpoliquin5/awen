# AEP-0015: Executable awenBLAS kernel library

Status: Accepted and implemented

## Summary

AWEN defines the versioned `awen.blas.v1` request, backend, result, and
execution-plan contracts and the `awen.blas-benchmark.v1` measured benchmark
contract. The initial executable library contains CPU references and a
deterministic accelerator simulator for dense, complex, transformer,
convolution/correlation, Fourier, structured-linear-algebra, RF, reservoir,
and propagation kernels. Backend choice is capability- and cost-driven, always
retains a CPU reference candidate, preserves declared operator structure, and
records explicit fallback reasons.

## Motivation

A photonic compiler cannot be useful as a GEMM-only demonstration. Workloads
that plausibly benefit from physical linear algebra also require batched and
complex matrix products, transformer projections, convolution/correlation,
Fourier operators, structured matrices, RF beamforming, recurrent reservoirs,
and propagation transforms. Treating each as an untyped backend-specific call
would duplicate numerical conventions, obscure calibration inputs, silently
densify structured operators, and make fallback or performance claims
impossible to audit.

`awenBLAS` is the executable semantic boundary between framework/compiler
operations and CPU, GPU, or photonic implementations. It is analogous in role,
not maturity or measured performance, to a combined BLAS/FFT/neural-kernel
library. Every registered operation has one platform-independent definition
against which simulator and hardware implementations can be tested.

## Versioned contracts

The normative schemas are:

- `awen_blas.v1.json` for kernel requests;
- `awen_blas_backend.v1.json` for backend capabilities and cost inputs;
- `awen_blas_result.v1.json` for materialized outputs and descriptors;
- `awen_blas_plan.v1.json` for candidate traces and selection decisions; and
- `awen_blas_benchmark.v1.json` for end-to-end measured conformance evidence.

Unknown fields are rejected. A request uses the exact version
`awen.blas.v1`. Implementations must reject a different major contract rather
than interpreting it approximately. Tensor identifiers and request identifiers
must be non-empty. Tensor dimensions must be positive and their checked product
must equal the number of supplied values. All scalar, real, and complex values
must be finite.

## Tensor, dtype, layout, and accuracy semantics

Each input tensor declares:

- a stable identifier;
- an arbitrary positive shape whose rank is subsequently constrained by its
  kernel;
- one of `f32`, `f16`, `bf16`, `int8`, `int4`, or `complex_f32`;
- `row_major` or `column_major` layout; and
- tagged `real` or `complex` materialized data.

Real data is legal only with a real dtype. Complex data is legal only with
`complex_f32` and stores explicit `{real, imaginary}` pairs. Matrix kernels
honor row-major and column-major indexing and `transpose_lhs` /
`transpose_rhs`; transpose never implies complex conjugation. Results are
materialized in logical row-major order. All inputs to a real v1 call must use
the same dtype; mixed-input and mixed-accumulator precision require the
explicit policy added under the precision work rather than an implicit cast.

Every request carries maximum absolute and relative error tolerances and may
carry a minimum effective-bit requirement. Descriptors repeat those values so
a selected or executed call remains auditable without reconstructing request
defaults. Accumulation is explicitly `optical`, `digital`, or `hybrid`; the CPU
reference evaluates in host `f64` arithmetic as the semantic oracle and
records the requested accumulation mode without pretending to emulate a
physical accumulation mechanism.

## Kernel registry and mathematical semantics

The v1 registry contains the following 22 kinds. An implementation claiming
v1 support for a kind must match these shape, ordering, scaling, and phase
rules.

### GEMM and batched GEMM

`gemm` consumes `A[m,k]` and `B[k,n]`, after applying the two transpose
attributes, and returns `C[m,n] = A B`. `batched_gemm` consumes
`A[b,m,k]` and `B[b,k,n]`, requires equal batch dimensions, and returns
`C[b,m,n]`. Batch broadcasting is not part of v1.

`complex_gemm` applies the same rank-two shape and transpose rules to
`complex_f32` tensors using ordinary complex multiplication and addition.
Transpose is non-conjugating; a future conjugate-transpose operation requires
an explicit contract revision.

### Linear and transformer kernels

`linear` and `mlp_projection` consume `X[m,k]`, `W[k,n]`, and an optional
`bias[n]`. They return `X W`, adding the bias to every output row when present.

`transformer_qkv` consumes one activation matrix `X[m,k]` followed by exactly
three matrices `Wq[k,q]`, `Wk[k,d]`, and `Wv[k,v]`. It returns three separately
identified row-major outputs `X Wq`, `X Wk`, and `X Wv` in Q, K, V order.

`attention_scores` consumes rank-two or equally batched rank-three `Q` and
`K`, computes `Q K^T`, then multiplies every score by the explicit `scale`
attribute. It does not apply masks or softmax. `attention_value` consumes
rank-two or equally batched rank-three attention weights and values and
computes their matrix product. Nonlinear normalization and masking remain
separate digital graph operations so partitioning can expose the boundary.

### Convolution, correlation, and RF filtering

`convolution1d` consumes a real rank-one signal and kernel. It reverses the
kernel, applies explicit non-negative zero padding, positive stride, and
positive dilation, and emits

```text
floor((signal_length + 2*padding - effective_kernel) / stride) + 1
```

samples, where `effective_kernel = dilation*(kernel_length-1)+1`.
`correlation1d` uses the same contract without reversing the kernel. `rf_fir`
uses the convolution definition and is tagged as convolutional structure so a
backend may advertise the RF entry point without losing the underlying FIR
semantics.

### DFT, FFT, and Fourier filtering

`dft` and `fft` consume and return one rank-one `complex_f32` tensor. The
default `negative_exponent` forward transform is

```text
X[k] = sum_n x[n] exp(-i 2 pi k n / N)
```

and its inverse uses the positive exponent and multiplies by `1/N`.
`positive_exponent` reverses both signs while retaining inverse `1/N`
normalization. The FFT uses radix-2 Cooley-Tukey for power-of-two lengths and
the exact DFT definition otherwise. Thus `fft` changes the algorithm, not the
mathematical result.

`fourier_filter` consumes equal-length complex signal and frequency-response
tensors. It computes the forward DFT, elementwise complex multiplication by
the response, and the inverse DFT using the selected phase convention.

### Low-rank and deterministic random projections

`low_rank_gemm` consumes `A[m,k]`, `U[k,r]`, and `V[n,r]` and returns
`(A U) V^T` without constructing the dense `k by n` operator. A non-zero
`rank` attribute must equal the factors' `r`; zero means infer it.

`random_projection` consumes `X[m,k]`, requires a positive `output_size = n`,
and generates a deterministic `k by n` Rademacher matrix from `seed`, with
entries `-1/sqrt(n)` or `+1/sqrt(n)`. It returns `X R`. The seed is part of the
request and execution fingerprint, making reference and replay results stable.

### Toeplitz, circulant, and block-circulant operators

`toeplitz` consumes first column `c[m]`, first row `r[n]`, and vector `x[n]`.
The shared leading elements must agree within the declared absolute tolerance.
It returns `y[m]`, where `T[i,j] = c[i-j]` for `i >= j` and `r[j-i]`
otherwise. The representation remains Toeplitz in descriptors and capability
selection; implementations must not silently replace it with a dense-kernel
claim.

`circulant` consumes generator `g[n]` and vector `x[n]` and returns the
circulant matrix-vector product with coefficient
`g[(j + n - i) mod n]` at output row `i`, input column `j`.

`block_circulant` consumes generators `[p,b,b]` and vector `[p*b]`. The
`block_size` attribute must equal `b`. Output block `i` and input block `j` use
generator block `(j + p - i) mod p`; each generator block is an ordinary dense
`b by b` matrix. The operator is never expanded into a serialized dense
`p*b by p*b` matrix.

### Beamforming, RF, reservoirs, and propagation

`beamforming` consumes a complex rank-two signal matrix and complex rank-two
weight matrix and uses the non-conjugating complex GEMM contract. Applications
requiring conjugate steering weights must provide those conjugated values
explicitly in v1.

`reservoir_step` consumes state `s[n]`, recurrent matrix `W[n,n]`, and input
matrix `U[n,m]`. The scalar external drive is the request's `scale` attribute
and v1 couples it through the first column of `U`. With leakage `a` in `[0,1]`,
the returned state is

```text
s_next[i] = (1-a) s[i] + a tanh(sum_j W[i,j] s[j] + U[i,0]*scale)
```

This primitive makes the nonlinear activation explicit. A vector-valued input
contract is reserved for a later compatible kernel kind rather than being
silently inferred from unused columns.

`propagation` consumes two complex rank-two tensors and applies the complex
GEMM contract. Its descriptor retains `propagation` structure so capability
selection can distinguish propagation engines from general complex matrix
cores.

## Structure preservation

Every descriptor assigns exactly one natural structure: dense, low-rank,
random-projection, Toeplitz, circulant, block-circulant, convolutional,
Fourier, beamforming, reservoir, or propagation. Backend profiles advertise a
set of supported structures independently from their kernel-kind set. A
backend that supports dense GEMM but not Toeplitz is ineligible for a Toeplitz
request even if it could materialize the matrix. Densification is permissible
only as an explicit separately costed compiler transformation outside this
kernel-selection contract.

## Calibration and deterministic simulator

A calibration input declares an identifier, non-zero finite gain, finite bias,
and uncertainty fraction in `[0,1]`. A backend that declares
`requires_calibration` is ineligible when the request contains no calibration
input. The v1 deterministic simulator composes all calibration inputs in their
declared order. For transfers `(g1,b1)` then `(g2,b2)`, the composition is
`gain = g1*g2` and `bias = b1*g2+b2`; longer lists continue identically. The
composed gain and bias must remain finite and the gain must remain non-zero.
The simulator applies `gain*value+bias` to each quantized real result; for
complex results the bias applies to the real component. It then applies inverse
compensation `(measured-bias)/gain`, leaving quantization and optional seeded
noise visible while representing a calibrated transfer boundary.

Simulator input values are block-quantized using the requested effective-bit
count. The optional non-negative noise fraction and seed are explicit simulator
options. Identical request bytes and simulator options must produce identical
outputs and fingerprints. The simulator is a conformance mechanism, not a
claim about shot noise, thermal noise, per-cell loss, drift, or hardware speed.

## Capability and cost-driven selection

A backend profile declares a unique ID, concrete target (`cpu`, `gpu`, or
`photonic`), kernel kinds, dtypes, structures, complex support, maximum tensor
elements, effective bits, calibration requirement, launch latency, throughput,
energy per operation, estimated error, and provenance source. `auto` is not a
concrete backend and is rejected in profiles and results.

A profile that advertises a complex kernel kind must also set
`supports_complex` and include `complex_f32`; advertising the dtype while
denying complex support, or claiming complex support without the dtype, is
invalid rather than a partially usable capability.

Selection always inserts the built-in CPU reference profile before supplied
accelerator profiles. A candidate is ineligible, with a recorded reason, when
any of the following holds:

- the kernel kind is unsupported;
- its natural structure is unsupported;
- any input dtype is unsupported;
- explicit complex semantics are required but unavailable;
- any input or inferred output exceeds the backend element limit;
- effective precision is below the request minimum;
- estimated error exceeds both request tolerances; or
- required calibration is absent.

For an eligible profile with `O` estimated operations:

```text
latency_ns     = launch_latency_ns + O / (throughput_tops * 1000)
energy_uJ      = O * energy_pj_per_operation / 1,000,000
throughput_gops = O / latency_ns
```

The selector deterministically minimizes latency, energy, or error, or
maximizes throughput. Ties are broken by backend ID. The execution plan records
every eligible and ineligible candidate, selected estimate, fallback flag,
rationale, descriptor, and a stable fingerprint over request, profiles, and
objective. Selecting CPU while any non-CPU profile was considered is recorded
as fallback; no accelerator failure is silent.

Profile numbers with `assumed`, `simulated`, or `vendor_specified` provenance
must never be presented as measured AWEN performance. This compact per-kernel
model is a dispatch hook. Whole-graph placement, transfers, residency,
conversion crossings, memory, and queueing remain governed by AEP-0013 and
AEP-0014.

## Reference execution and benchmark evidence

`execute_reference` validates the entire request and executes its CPU semantic
definition. `execute_simulator` validates the same request and a concrete
target, quantizes inputs, executes the same definition, applies calibration and
seeded noise, and marks the result as simulated. Both results contain a
descriptor and stable output fingerprint.

`benchmark_kernel` measures wall-clock time across a positive repetition count
for the full reference and simulator paths, including request validation,
input quantization, kernel execution, calibration transfer, deterministic
noise, and output materialization. It compares every real component or complex
magnitude difference, records maximum absolute and relative error, output
checksum, and the exact measurement boundary, and sets provenance to
`measured`. The measurement is of software running on the host named by the
artifact environment; it is not hardware evidence unless the measured path is
replaced by an identified hardware executor and the contract is versioned
accordingly.

A conformance result is within contract when either its maximum absolute error
or maximum relative error is within the corresponding declared tolerance. This
matches the library's element comparison rule and avoids relative-error
singularities at a zero reference value.

## CLI and Rust API

The Rust surface exports `execute_kernel_reference`,
`execute_kernel_simulator`, `select_kernel`, `benchmark_kernel`, the registry
types, and the version constants from `awen_compiler`.

The runtime CLI provides:

```text
awenctl kernel REQUEST --target cpu|gpu|photonic [--effective-bits N]
                       [--noise-fraction X] [--seed N] --output RESULT

awenctl kernel-plan REQUEST PROFILES
                     [--optimize-for latency|energy|accuracy|throughput]
                     --output PLAN

awenctl kernel-benchmark REQUEST --target gpu|photonic
                                 [--effective-bits N]
                                 [--noise-fraction X] [--seed N]
                                 [--repetitions N] --output REPORT
```

CPU execution uses the reference path. GPU and photonic execution in v1 use
the explicitly marked deterministic simulator; these commands do not claim
that a device was submitted. The plan input is an array of backend profiles;
the selector adds the CPU reference automatically.

## Backwards compatibility

This AEP completes the concrete API and compatibility work left open by
AEP-0003. It does not alter the earlier `awenBLAS` rank-two GEMM helper used by
the tiled compiler and its tests. The new registry is an additive module and
uses independent versioned JSON contracts. Existing Tensor IR, Photonic IR,
Device IR, executable ABI, partition, and benchmark artifacts remain valid.

New optional fields may be added only with defaults and an accompanying schema
revision. New kernel kinds may be added within the v1 family only when older
readers reject them cleanly and their full semantics, reference implementation,
and conformance vectors land together. Any change to an existing kernel's
shape, ordering, phase, normalization, random generation, or accumulation
meaning requires a new major contract.

## Test plan

The implementation is accepted only when all of the following pass:

- exact CPU reference vectors cover every registered kernel kind;
- every registered kernel validates, executes in the CPU reference, executes
  in the deterministic simulator, and produces a measured benchmark record;
- randomized GEMM identity properties cover varied dimensions and layouts;
- FFT forward/inverse round trips cover power-of-two and non-power-of-two
  lengths and both explicit phase conventions;
- complex data representation, finite-value rejection, transpose semantics,
  and phase signs have direct tests;
- low-rank, Toeplitz, circulant, block-circulant, convolutional, Fourier,
  beamforming, reservoir, and propagation structures survive descriptors and
  selection;
- capability, dtype, structure, size, effective-bit, error, complex, and
  calibration failures produce diagnosed CPU fallback candidates;
- repeated simulation and planning with identical seeds and inputs produce
  identical fingerprints and outputs;
- request, backend, result, plan, benchmark, and checked-in example JSON all
  validate against the corresponding schemas;
- runtime format, strict Clippy, unit, integration, doc, CLI execution,
  selection, and benchmark checks pass; and
- documentation labels all reference and simulator results correctly and
  makes no unmeasured hardware-performance claim.
