# AWEN

AWEN is an experimental heterogeneous compiler/runtime for programmable physical linear algebra. The repository contains a Rust runtime, a typed tensor-to-photonic compiler slice, specifications, simulator and calibration components, Python experiments, and early Studio/Cloud/Ecosystem scaffolding.

The implemented compiler paths are deliberately narrow:

```text
awen.tensor graph
  -> shape/layout/precision validation
  -> explicit storage, compute, accumulator, and output precision
  -> scaling, signed bit slicing, saturation, and error-contract validation
  -> calibration ID/fingerprint/topology/environment/health validation
  -> measured wavelength selection and disabled-cell-to-spare remapping
  -> whole-graph CPU, GPU, or photonic region placement
  -> residency-aware transfer, crossing, reuse, and memory optimization
  -> M/N/K tiling for a declared photonic matrix core
  -> classical Photonic IR
  -> Device IR command buffer
  -> seeded analog-noise/calibrated-transfer simulator and attributed CPU comparison

awen.blas kernel request
  -> exact kind/shape/layout/dtype/structure/phase validation
  -> capability, precision, calibration, and cost dispatch
  -> CPU reference or explicitly simulated GPU/photonic execution
  -> versioned result, plan, conformance timing, error, and fingerprint

awen.hil benchmark suite
  -> identical versioned fixture, warmup, repetitions, seed, and tolerances
  -> CPU/simulator runner or timeout-bounded external CUDA/lab/hardware driver
  -> raw full-system latency, energy, power, accuracy, calibration, and environment evidence
  -> recomputed p50/p95/p99 distributions and regression findings
  -> content-addressed artifact set and fail-closed verified claim generation

normalized stablehlo.dot_general
  -> registered MLIR awen_tensor dialect
  -> registered MLIR awen_photonic dialect
  -> registered MLIR awen_device dialect
  -> versioned AWENEXE binary
  -> direct Rust runtime command preparation

typed photonic runtime program
  -> awen.photonic calibrated classical signal/precision/noise contract
     | awen.qphotonic Fock/Gaussian gate/measurement/sampling contract
  -> explicit measurement-readout or classical-control interop only
  -> independent verifier and capability requirements
  -> dialect-specific signed plugin capability and typed artifact bundle

PyTorch / JAX / NumPy / C++
  -> live framework tensors or portable JAX StableHLO
  -> versioned in-process operation plan
  -> framework-resident reference execution
  -> versioned profiling and deterministic replay trace
```

The MLIR path currently accepts only normalized rank-two StableHLO
`dot_general`. The Python integration separately provides a real
`torch.compile` backend for matrix/linear regions, JAX portable StableHLO
export/import, an in-process NumPy runtime, and analytic framework gradients.
The separate awenBLAS semantic library covers 22 executable kernel kinds.
Framework execution is currently a semantic reference and dispatch boundary;
it does not yet connect every captured framework region to generated photonic
device commands or demonstrate validated hardware speedups.
Do not interpret reference capability values or simulator cost estimates as
measured product performance.

## Repository layout

- `awen-compiler`: typed Tensor IR, explicit mixed-precision/scaling/error contracts, immutable calibration-snapshot validation, measured channel/cell routing, fault remapping, drift-triggered artifact refresh, validated capability/health negotiation, full-system cost/autotuning, crossing-aware whole-graph CPU/GPU/photonic partitioning, GEMM tiling, Photonic IR, Device IR, and an executable 22-kind `awenBLAS` reference/simulator/dispatch/conformance library.
- `awen-mlir`: MLIR 20 ODS/TableGen dialects, StableHLO GEMM import passes,
  Device IR bytecode, and the `AWENEXE` emitter.
- `awen-runtime`: CLI, engine, HAL, scheduler, calibration, observability, independent typed classical/quantum-photonic execution and V5 migration, content-addressed HIL benchmark evidence and claims, artifacts, plugins, legacy node IR, quantum experiments, and a compiled C/C++ framework ABI.
- `awen-spec`: schemas, specifications, and AWEN Enhancement Proposals.
- `awen-ecosystem`: in-process PyTorch/JAX/NumPy integration, example PDK data, kernels, marketplace, and plugin templates.
- `awen-studio` and `awen-cloud`: early scaffolding, not shipping products.

## Build and verify

Install the current stable Rust toolchain with `rustfmt` and `clippy`, then run:

```bash
cargo fmt --manifest-path awen-compiler/Cargo.toml --all -- --check
cargo clippy --manifest-path awen-compiler/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path awen-compiler/Cargo.toml --all-features --no-fail-fast

cargo fmt --manifest-path awen-runtime/Cargo.toml --all -- --check
cargo clippy --manifest-path awen-runtime/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path awen-runtime/Cargo.toml --all-features --no-fail-fast

python -m pip install -e "awen-ecosystem/python_awen[frameworks,test]"
python -m pytest awen-ecosystem/python_awen/tests -q

cmake -S awen-mlir -B awen-mlir/build -G Ninja \
  -DMLIR_DIR=/usr/lib/llvm-20/lib/cmake/mlir \
  -DLLVM_DIR=/usr/lib/llvm-20/lib/cmake/llvm \
  -DCMAKE_C_COMPILER=clang-20 \
  -DCMAKE_CXX_COMPILER=clang++-20 \
  -DCMAKE_BUILD_TYPE=Release
cmake --build awen-mlir/build --target awen-opt awen-compile -j2
ctest --test-dir awen-mlir/build --output-on-failure
```

PyTorch usage is direct:

```python
import torch
from awen_py import awen

model = torch.compile(model, backend=awen, dynamic=True)
y = model(x)
```

## Compile a 256×256 GEMM

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  compile awen-compiler/examples/gemm_256.json \
  --capabilities awen-compiler/capabilities/pace_like_128.json \
  --health awen-compiler/capabilities/pace_like_128.health.json \
  --target photonic \
  --output awen_compilation.json
```

The example lowers to eight 128×128×128 optical GEMM tiles. The output contains
operation cost decisions, the complete graph partition trace, transfers,
crossings, regions, profiler events, typed classical Photonic IR, immutable
calibration identity/fingerprint/environment/lineage, measured channel choices,
cell remaps, capacity/error impacts, and the executable Device IR command
stream.

## Compile and load StableHLO through MLIR

```bash
awen-mlir/build/tools/awen-compile/awen-compile \
  awen-mlir/test/stablehlo_gemm.mlir \
  -o stablehlo_gemm.awenx

cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  execute stablehlo_gemm.awenx
```

The runtime reads the versioned binary command table directly. It does not
parse compiler JSON or launch an MLIR subprocess.

## Execute the calibrated reference benchmark

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  benchmark awen-compiler/examples/gemm_4x4.json \
  --capabilities awen-compiler/capabilities/reference_2x2.json \
  --health awen-compiler/capabilities/reference_2x2.health.json \
  --target photonic \
  --output awen_benchmark.json
```

This command applies the emitted tensor/channel/block quantization and signed
bit-slice plan, executes the tiles with the selected accumulator, injects
deterministically seeded shot/thermal/phase/detector noise, applies the measured
calibration transfer and emitted inverse rescale, converts to the declared
output dtype, and compares against the digital reference GEMM. Its
`awen.error-report.v1` output separates quantization, analog, calibration,
floating-point accumulation, overflow, clipping, and propagated-input error.
Compilation rejects forced photonic placement when the effective-bit or error
contract cannot be met; automatic placement records a digital fallback.

## Refresh a calibration-bound artifact

`refresh_for_backend` compares an existing compilation artifact with a current
backend snapshot before reuse. Exact source and backend-snapshot fingerprints
return the original artifact. Calibration, health, drift, temperature,
disabled-component, resource, backend, or topology changes produce named
invalidation reasons and deterministic recompilation. If current photonic
hardware cannot satisfy the complete contract, refresh changes the old forced
target to automatic placement and emits a diagnosed digital fallback.

The calibration contract is specified by AEP-0018 and
`awen.calibration-snapshot.v1`. A snapshot carries its ID, exact fingerprint,
parent lineage, backend/topology binding, measured time and environment,
global/per-cell/per-spare/per-channel transfer data, and uncertainty. Health
must confirm both the snapshot ID and fingerprint. Photonic IR and Device IR
record the selected channel IDs, wavelengths, cell remaps, effective transfer,
inverse rescaling, capacity loss, and attributed calibration error.

## Run the comparable full-system benchmark suite

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  benchmark-suite benchmarks/reference_hil_suite.json \
  --output-dir awen_hil_artifacts \
  --commit-sha "$(git rev-parse HEAD)" \
  --runner-id local-reference
```

One command applies the same fixture, warmup, repetitions, seed, and accuracy
contract to every backend configured in the suite. It writes `suite.json`, a
content-addressed `benchmark-<sha256>.json`, and `SHA256SUMS`. Raw evidence
includes full-system component accounting, latency/throughput/energy/power/error
distributions, calibration duration, environment, versions, and output
checksums. The included reference suite measures host wall-clock time but tags
photonic execution as simulated and power/energy as estimated.

CUDA, lab, and accelerator adapters implement the versioned JSON driver
protocol. Physical runs use the manual self-hosted `Manual physical hardware
benchmark` workflow and cannot become noisy required pull-request checks.
`benchmark-claims` refuses mutable, simulated, estimated, vendor-specified,
inaccurate, uncalibrated, or non-accelerating evidence. See AEP-0019 and
`awen-spec/specs/hardware-benchmarking.md`.

## Migrate legacy mixed Photonic IR V5

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  migrate-photonic-v5 legacy-v5.json \
  --output migration-report.json
```

New execution does not accept the mixed V5 string operation space. Classical
programs use calibrated precision/noise/transfer contracts. Quantum programs
use explicit Fock or Gaussian CV state, gates, measurement, shots, seed,
feed-forward, coherence, statistical correctness, and replay identity. Only
typed measurement-readout and classical-control operations cross the boundary.
The migrator classifies allowlisted prefixed legacy operations, preserves a
machine-readable report, and rejects ambiguous or unknown strings without
inventing semantics. See AEP-0020 and
`awen-spec/specs/photonic-dialect-separation.md`.

## Status and evidence

The canonical required check is `AWEN required quality gate`. Public performance claims must be generated from verified immutable end-to-end artifacts that include host transfer, memory, scheduling, reconfiguration, lasers, DAC/ADC, calibration, digital post-processing, and support power. The repository currently ships no measured physical-accelerator artifact and makes no validated hardware-acceleration claim.
