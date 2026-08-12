# AEP-0016: In-process framework integration

Status: Accepted and implemented

## Summary

AWEN defines an in-process framework runtime, a genuine PyTorch custom
compiler backend, portable JAX export/import through StableHLO, public NumPy
operations, and a compiled C/C++ ABI. The normal tensor path accepts and
returns framework-owned tensors without serializing values to JSON, discovering
temporary directories, or launching `awenctl`. Supported matrix regions retain
their framework autograd graph. Unsupported graph operations execute through
an explicit framework fallback and produce actionable diagnostics.

The normative serialized contracts are `awen.framework-plan.v1` and
`awen.framework-trace.v1`. The compiled C surface is
`awen.framework-c.v1`. PyTorch and JAX integration reports are diagnostic
records: they are stable within the Python package's 0.2 release line but are
not substitutes for the execution plan and trace contracts.

## Motivation

The previous Python helper was an artifact-oriented experiment. It launched
`awenctl`, scanned the current directory for the newest generated folder,
read JSON results, reduced the output to one scalar, and implemented a manual
finite-difference backward helper. That behavior cannot satisfy framework
compiler semantics:

- concurrent calls race while selecting the newest directory;
- a process boundary loses buffer ownership, device, layout, and stream state;
- scalar extraction loses standard tensor behavior and batching;
- finite differences are neither an analytic autograd definition nor an
  acceptable default for model training;
- the helper bypasses TorchDynamo graph capture and dynamic guards;
- JAX programs have no portable StableHLO representation; and
- C++, NumPy, PyTorch, and JAX do not share one execution contract.

Framework integration must make AWEN usable as a compiler/runtime boundary,
while being honest that the v1 implementation is a semantic reference. It
does not claim that a live accelerator is present or that a simulated
photonic target is measured hardware.

## Architecture

The integration has five layers:

1. `awen_py.runtime` defines buffers, contracts, devices, streams, plans,
   traces, synchronous execution, asynchronous execution, replay, and errors.
2. `awen_py.numpy_api` exposes direct tensor operations over NumPy arrays.
3. `awen_py.torch_backend` implements PyTorch's custom backend callable and
   rewrites supported FX nodes to the in-process runtime boundary.
4. `awen_py.jax_stablehlo` uses `jax.export` to produce, serialize, inspect,
   deserialize, and execute portable StableHLO programs.
5. `awen-runtime/include/awen/framework.h` and `framework.hpp` expose compiled
   row-major GEMM entry points for caller-owned `f32` and `f64` buffers.

The standalone artifact CLI remains available only through explicitly named
`*_cli_debug` helpers. It is outside the normal tensor path.

## Versioned runtime contract

The in-process ABI identifier is `awen.framework-runtime.v1`. A normal call
constructs an `OperationPlan`, executes against live inputs, and may emit an
`ExecutionTrace`. The v1 operation set is:

- `gemm`;
- `batched_gemm`;
- `complex_gemm`;
- `linear`;
- `attention_scores`;
- `attention_value`;
- `mlp_projection`;
- `fft`; and
- `ifft`.

An unknown operation is a contract error. Inputs to one operation must come
from one framework. The implementation recognizes NumPy, PyTorch, and JAX
values without importing optional frameworks until they are used.

`awen.framework-plan.v1` contains exactly:

- the version and deterministic operation identifier;
- the operation;
- selected target and source framework;
- concrete execution-time input shapes and dtypes;
- operation attributes;
- maximum absolute error, maximum relative error, and optional minimum
  effective-bit requirement;
- deterministic seed; and
- a SHA-256 fingerprint over every preceding field.

Unknown serialized fields are rejected. Deserialization recomputes the
fingerprint and rejects modification. The same operation metadata, target,
shapes, dtypes, attributes, contract, and seed produce the same operation
identifier and fingerprint.

## Buffers and ownership

`Buffer.wrap` creates a non-owning description by default. A buffer records:

- the original live value;
- `borrowed`, `runtime`, or `framework` ownership;
- framework;
- device;
- concrete shape;
- element strides; and
- dtype.

Wrapping does not copy a value. Outputs produced through framework operators
are marked framework-owned. NumPy `out` accepts a caller-owned destination
only when shape and dtype exactly match; the implementation copies the result
into that destination and returns the destination object. An `out` buffer is
not legal on an asynchronous NumPy call because ownership for a concurrently
written caller buffer would be ambiguous in v1.

## Devices, targets, and streams

Device strings use `cpu`, `cuda[:index]`, `mps[:index]`, `tpu[:index]`, or
`photonic[:index]`. Execution targets use:

- `auto`, which follows input residency;
- `framework`, which executes with the source framework on its resident
  device;
- `cpu`, which requires CPU-resident inputs;
- `gpu`, which requires CUDA- or MPS-resident inputs; or
- `photonic`, which is explicitly recorded as `simulated:photonic` in v1.

AWEN never silently transfers input buffers to satisfy an explicit target.
`runtime.transfer` is the explicit transfer operation. NumPy can remain on
CPU. PyTorch uses `Tensor.to` with the requested non-blocking setting. JAX
selects a matching registered device and uses `jax.device_put`.

A `Stream` associates a device, stable identifier, and optional native stream.
For a supplied PyTorch CUDA stream, the operation executes inside
`torch.cuda.stream`. Without an explicit stream, native framework operations
use the framework's current/default stream semantics.

## Synchronous and asynchronous execution

`execute` returns the standard framework tensor for a synchronous call.
Setting `ExecutionOptions.asynchronous` returns `AwenFuture`. `wait`/`result`
returns the complete `ExecutionResult`, including output, plan, and trace.
The v1 reference runtime uses a bounded thread pool. `close`, the context
manager exit method, and interpreter shutdown release workers.

Async trace events record that the call was reported asynchronous. A future
propagates the same typed execution exception that a synchronous call would
raise. It never encodes failure in an output tensor.

## Errors and numerical contracts

All public framework errors derive from `AWENError`:

- `ContractError` for invalid operations, options, shapes, dtypes, contracts,
  or replay inputs;
- `UnsupportedGraphError` for strict graph compilation without fallback;
- `ExecutionError` for a framework operation failure;
- `DeviceError` for invalid residency or transfer; and
- `SerializationError` for malformed or modified portable records.

Numerical tolerances must be finite and non-negative. Minimum effective bits,
when supplied, must be positive and must not exceed the effective mantissa or
integer width of any input dtype. Framework integrations transport the same
contract values to the operation plan. JAX exposes
`assert_numerical_contract` to compare a candidate result with a reference.

The reference runtime uses the source framework's matrix and FFT definitions,
so it introduces no alternate approximation. Later physical backends must
validate produced outputs against the same declared contract before returning
success.

## Profiling and deterministic replay

With profiling enabled, an `awen.framework-trace.v1` event records:

- operation identifier and kind;
- selected target;
- framework, device, and stream;
- input and output shapes;
- monotonic start time and duration in nanoseconds; and
- whether execution was reported asynchronous.

The trace references the plan fingerprint and carries a replay fingerprint
over the plan plus output shape/dtype/device metadata. A replay accepts a
deserialized plan only when framework, shapes, and dtypes match exactly. Replay
uses the recorded attributes, contract, selected target class, and seed.

The fingerprint is an integrity and reproducibility identifier, not a digital
signature and not a hash of secret tensor contents.

## PyTorch compiler backend

`awen_py.torch_backend.awen_backend` implements the documented custom-backend
contract `(GraphModule, example_inputs) -> Callable`. The package registers
the `awen` name in the `torch_dynamo_backends` entry-point group and also
exports the callable as `awen`, enabling:

```python
from awen_py import awen

compiled_model = torch.compile(model, backend=awen)
```

The backend recognizes FX `call_function`, `call_method`, and `call_module`
forms for:

- `matmul`, `mm`, and `bmm`;
- the matrix multiplication operator; and
- functional or module `linear`.

Recognized nodes are rewritten to the AWEN in-process boundary. Other nodes
remain in the same FX graph and execute eagerly through PyTorch. The compile
report lists supported nodes, fallback nodes, contiguous AWEN regions, and one
diagnostic per fallback node. Each diagnostic names the node and target,
states the v1 supported set, and records `framework_fallback` as the action.
`allow_fallback=False` rejects the graph with all accumulated diagnostics.

TorchDynamo remains the owner of graph breaks, guards, and recompilation.
AWEN does not freeze dynamic dimensions in its operation plans; a plan is made
from each guarded concrete execution. Compile reports label symbolic scalar
inputs separately and state that dynamic dimensions are guarded by
TorchDynamo. Rank, dtype, device, layout, and stride metadata are diagnostic
only.

Supported functions return ordinary PyTorch tensors. The implementation uses
PyTorch transpose, matrix multiplication, and addition directly on live
tensors. Consequently, PyTorch autograd records analytic operations and
computes gradients for inputs, weights, and bias. No detach, NumPy conversion,
temporary file, scalar reconstruction, custom finite difference, or
subprocess occurs.

The backend emits `awen::<operation>` profiler ranges when profiling is
enabled. Runtime execution traces remain accessible after each compiled
call.

## JAX and StableHLO

`export_jax` invokes the supported `jax.export.export(jax.jit(function))`
interface. It retains:

- the `Exported` callable;
- portable serialized bytes, including the configured VJP order;
- textual StableHLO for inspection;
- calling-convention version;
- target platforms;
- input and output abstract values; and
- a SHA-256 fingerprint of the portable bytes.

AWEN recognizes `stablehlo.dot_general` plus the initial surrounding element,
shape, constant, conversion, and transpose operations. Other StableHLO
operations are listed as framework fallbacks. With fallback enabled, JAX
executes the complete portable program. With fallback disabled, any unknown
operation produces `UnsupportedGraphError`.

Dynamic dimensions use `jax.export.symbolic_args_specs`. The symbolic shape
specification is part of the exported StableHLO constraints, and one portable
executable can accept multiple conforming concrete shapes. Deserialization
uses `jax.export.deserialize`; it does not rebuild a program from diagnostic
text.

`export_jax_value_and_grad` applies `jax.value_and_grad` before export. The
result is an analytic differentiated StableHLO program. This is the supported
gradient path for v1.

## NumPy API

The NumPy surface exports `gemm`, `batched_gemm`, `complex_gemm`, `linear`,
`attention_scores`, `fft`, and `compile_plan`. Inputs are converted with
`numpy.asarray` without forcing contiguous layout. Matrix semantics therefore
support normal strided NumPy views. Operations return NumPy arrays, a supplied
valid `out` object, or `AwenFuture` for async execution.

NumPy has no automatic differentiation facility. `debug_finite_difference`
exists only for explicit diagnostics and requires `enabled=True`. It is not
imported as a normal execution or training path.

## C and C++ ABI

The runtime builds `rlib`, `cdylib`, and `staticlib` artifacts. The C header
defines `awen.framework-c.v1`, a fixed `awen_status` enum, thread-local error
retrieval, and caller-owned row-major GEMM functions for `f32` and `f64`.

Each call receives pointer and element count for every buffer plus `m`, `n`,
and `k`. It rejects null pointers, zero dimensions, checked-size overflow, and
undersized buffers before constructing slices. No allocation crosses the ABI.
The caller owns all buffers before, during, and after the call. Panics are
caught at the boundary and converted to `AWEN_STATUS_INTERNAL_ERROR`.

`awen_last_error_message` is thread-local. It returns the required byte count,
including the terminating NUL, and may copy into a caller-owned buffer. The
C++20 header wraps buffers in `std::span` and converts non-success status to
`std::runtime_error`.

The v1 C ABI intentionally starts with synchronous dense GEMM. Async,
device-specific buffers, and streams remain available through the in-process
framework ABI and must receive new ABI structs rather than incompatible
changes to these functions.

## Supported versions and packaging

The Python package requires Python 3.10 or later. Optional dependency groups
are:

- NumPy `>=1.26,<3`;
- PyTorch `>=2.10,<2.14`; and
- JAX `>=0.9,<0.12` on Python 3.12 or later.

The release gate tests PyTorch 2.13.0 and JAX/JAXlib 0.11.0 together on Python
3.12 and retains a minimum-version job for PyTorch 2.10 and JAX 0.9. Runtime
version checks reject an installed PyTorch or JAX outside the declared range
instead of silently relying on an untested extension contract.

## Security and process boundaries

Normal framework and NumPy modules do not call `subprocess`, create temporary
files, inspect the current directory for results, or interpret user-provided
shell commands. Portable JAX bytes and JSON plans are parsed by their
versioned deserializers and checked before execution.

The old artifact CLI behavior lives in `awen_py.client` under
`compute_gradients_cli_debug` and `run_ir_cli_debug`. Compatibility aliases
remain local to that module. They are not exported from `awen_py`, used by the
compiler backend, or invoked by runtime execution.

## Backwards compatibility

Existing code that explicitly imports `compute_gradients` or `run_ir` from
`awen_py.client` continues to work as a debug path. Those names are removed
from the package root because a root-level tensor API must not imply that a
filesystem/subprocess operation is normal execution.

Plan and trace readers must reject unknown major versions. Minor additions
require a new schema identifier because v1 rejects unknown fields. New
framework operations may be added to a later contract without changing the
mathematical meaning of the v1 set. C ABI functions are never changed in
place; additions use new functions or versioned descriptor structs.

## Test plan

The conformance suite must cover:

- installation and backend discovery through package metadata;
- mixed supported/fallback PyTorch graphs;
- eager-versus-compiled outputs and analytic gradients;
- TorchDynamo dynamic shapes and recompilation;
- non-contiguous PyTorch and NumPy inputs;
- batched and complex matrix multiplication;
- strict fallback errors containing node and target information;
- ordinary tensor serialization;
- portable JAX serialization/deserialization;
- symbolic JAX dimensions with more than one concrete batch size;
- JAX analytic value-and-gradient results;
- numerical-contract comparison and effective-bit rejection;
- explicit device transfers and invalid-residency errors;
- synchronous and asynchronous execution;
- buffer ownership, `out` validation, streams, and profiling metadata;
- deterministic plan identity and replay;
- schema validation and tamper rejection;
- proof that the public normal path does not invoke a subprocess;
- Rust unit tests for success and error handling at the C boundary; and
- compilation and execution of the C++20 example against the produced shared
  library.
