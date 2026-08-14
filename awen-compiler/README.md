# awen-compiler

`awen-compiler` is the first executable compiler slice for AWEN. It turns a typed rank-2 GEMM program into a capability-specific schedule without assuming that photonics is always faster or accurate enough.

## Implemented contracts

- `awen.tensor.v1`: tensors carry rank-2 shape, dtype, layout, optional literal data, per-operation accuracy requirements, and optional sparsity/structure/input-error cost hints.
- `awen.device-capability.v1`: backends advertise operation/tiling legality, matrix-core shape, wavelengths, rates, coherence, ADC/DAC/effective precision, bit slicing, saturation, dynamic range, loss/power parameters, complex support, accumulation, host/link boundaries, ABI compatibility, and calibration requirements/profile.
- `awen.physical-design.v1`: backends bind immutable PDK, process-corner, component-library, logical-topology, circuit-model, adapter, simulation-settings, and verification identities; mapping requests preserve ports, units, constraints, and candidate topologies without representing layout geometry.
- `awen.backend-health.v1`: a timestamped query result carries availability, temperature, drift, usable channels, disabled components, unavailable resources, and the active calibration identity.
- `awen.photonic.classical.v1`: every selected GEMM is tiled with offsets, edge sizes, precision, wavelength allocation, timing, accumulation, and calibration identity.
- `awen.device.v1`: explicit calibration, configure, upload, execute, accumulate, download, and host-fallback commands.
- `awen.cost-model.v1`: dimensioned end-to-end latency, energy, error, and throughput estimates with provenance, uncertainty, benchmark fitting, and deterministic autotuning.
- `awen.partition-graph.v1`: complete DAGs with legal CPU/GPU/photonic candidates, tensor dependencies, residency, barriers, numerical eligibility, and memory budgets.
- `awen.partition-trace.v1`: deterministic whole-graph assignments, ranked alternatives, fused regions, deduplicated transfers, optical/electrical crossings, memory peaks, profiler events, and visualization edges.
- `awen.blas.v1`: 22 executable dense, complex, transformer, convolution/correlation, Fourier, structured, RF, reservoir, and propagation kernel kinds with explicit shapes, layouts, dtypes, phase, accumulation, accuracy, calibration, and structure.
- `awen.blas.v1` backend/result/plan records: capability and provenance-bearing cost dispatch, every candidate decision, diagnosed CPU fallback, and deterministic fingerprints.
- `awen.blas-benchmark.v1`: measured end-to-end CPU-reference versus deterministic-simulator conformance timing, numerical error, measurement boundary, and output checksum.

The cost model includes scheduling/queueing, host/link/memory movement, layouts,
residency, overlap, conversion boundaries, reconfiguration/calibration,
DAC/modulation/propagation/detection/ADC, accumulation, laser/support power,
SNR/loss/drift, disabled resources, sparsity, batching, and numerical error. It
never compares optical propagation alone. `auto` partitions the complete graph,
so it can keep small, unsupported, inaccurate, isolated, or crossing-heavy GEMMs
on CPU/GPU while grouping linear regions that amortize movement and conversion.

## Library API

```rust
use awen_compiler::{compile_with_backend, BackendSnapshot, CompileOptions, TensorProgram};

let program: TensorProgram = serde_json::from_str(input_json)?;
let snapshot: BackendSnapshot = serde_json::from_str(snapshot_json)?;
let artifact = compile_with_backend(&program, &snapshot, CompileOptions::default())?;
```

`CompileOptions` exposes `optimize_for`, an explicit or automatic CPU/GPU/
photonic target, deterministic `autotune_seed`, CPU/GPU baselines, batch size,
boundary fusion, queue depth, overlap, input residency, transfer bandwidth and
latency, crossing penalties, device memory budgets, search limits, and retained
operation-plan and graph-partition alternatives. The `awenctl compile` and
`awenctl benchmark` commands expose target, transfer, crossing, and memory
controls. Both commands accept `--cost-model`; benchmark additionally accepts a
versioned `--observations` file and writes predicted-versus-observed model
errors.

Every compilation artifact embeds `partition_trace`. Inspect its `nodes` for
local-versus-graph decisions, `regions` for fusion, `transfers` for deduplicated
residency changes, `profiler_events` for explicit boundary timing, and
`visualization_edges` for graph rendering. Fixed graph/device/calibration/model
snapshots and seeds produce byte-identical partition traces.

Use `benchmark(&program, &artifact)` only when the input tensors include literal
data. It is a deterministic reference/conformance path, not a hardware-performance
measurement. Hardware and external-simulator measurements enter through
`benchmark_with_observations`; its predicted-versus-observed reports can fit a
new model with `CostModelInputs::calibrated_from_reports`.

`compile` remains the deterministic offline convenience API and constructs a
snapshot at the embedded calibration timestamp. Runtime execution should query
health and call `compile_with_backend`. Calibration freshness is evaluated
against the supplied health observation, never the compiler wall clock.

## Physical-design boundary

`PhysicalDesignBinding` is a required part of `DeviceCapabilities`. It validates
immutable SHA-256 identities, logical port/topology consistency, exact topology
content identity, explicit units, passed connectivity evidence, closed adapter
kinds, and Circulax/circuit-adapter compatibility. It deliberately has no GDS,
polygon, route, rule-deck, foundry-source, solver-state, or raw-result fields.

`MappingRequest` exports logical photonic operations, required ports, scalar
layout constraints, and candidate logical topologies. `MappingResponse` imports
a gdsfactory-selected binding only after checking the request identity,
candidate name, cross-unit port compatibility, constraint compliance, topology
digest, adapter set, and verification evidence. Use
`import_mapping_response` as the fail-closed import chokepoint.

Compilation artifacts contain identity-only `PhysicalDesignProvenance`. The
complete binding affects both the backend snapshot and calibrated topology
fingerprints. `refresh_for_backend` reports PDK, process-corner, or general
physical-binding changes and recompiles instead of reusing a stale artifact.

The open fixture is
`awen-ecosystem/pdks/example_silicon_pdk.json`; the matching exported request is
`awen-spec/fixtures/physical_design_mapping_request.v1.json`. Neither
gdsfactory nor Circulax is linked into this crate. See AEP-0021 and the physical-
design boundary specification.

## awenBLAS kernel API

The kernel registry is separate from the narrow tiled-GEMM compiler frontend.
It supplies executable CPU references, a deterministic quantization/calibration/
noise simulator, backend selection, and measured software-conformance reports
for every registered kind:

```rust
use awen_compiler::{
    benchmark_kernel, execute_kernel_reference, execute_kernel_simulator,
    select_kernel, KernelRequest, KernelSimulatorOptions, OptimizationObjective,
};

let request: KernelRequest = serde_json::from_str(input_json)?;
let reference = execute_kernel_reference(&request)?;
let simulated = execute_kernel_simulator(&request, KernelSimulatorOptions::default())?;
let plan = select_kernel(&request, &backend_profiles, OptimizationObjective::Latency)?;
let report = benchmark_kernel(&request, KernelSimulatorOptions::default(), 10)?;
```

The versioned CLI surfaces are:

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  kernel awen-compiler/kernels/transformer_qkv.json \
  --target cpu --output awen_blas_result.json

cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  kernel-plan awen-compiler/kernels/transformer_qkv.json \
  awen-compiler/kernels/reference_kernel_backends.json \
  --optimize-for latency --output awen_blas_plan.json

cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  kernel-benchmark awen-compiler/kernels/transformer_qkv.json \
  --target photonic --effective-bits 12 --repetitions 10 \
  --output awen_blas_benchmark.json
```

GPU and photonic `kernel`/`kernel-benchmark` targets are explicitly simulated
in this version. Backend profile numbers in the repository are assumed or
simulated dispatch inputs. Only the benchmark's host wall-clock intervals are
measured, and they measure the complete software reference/simulator boundary,
not accelerator hardware.

## Relationship to the MLIR compiler

The JSON Tensor IR remains a bootstrap representation and a Rust semantic
reference. The production foundation is now under `awen-mlir`, with registered
TableGen dialects and a normalized StableHLO `dot_general` to Device IR path.
Do not extend the JSON parser into a parallel general-purpose compiler
infrastructure.

This crate also owns the platform-independent decoder for `AWENEXE` 1.x so the
runtime can consume the MLIR compiler's command table without linking MLIR or
using JSON shell-out glue.

Framework-native lowering into these operations remains tracked under
[the compiler epic](https://github.com/marcpoliquin5/awen/issues/5).
