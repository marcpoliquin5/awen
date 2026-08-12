# AWEN

AWEN is an experimental heterogeneous compiler/runtime for programmable physical linear algebra. The repository contains a Rust runtime, a typed tensor-to-photonic compiler slice, specifications, simulator and calibration components, Python experiments, and early Studio/Cloud/Ecosystem scaffolding.

The implemented compiler paths are deliberately narrow:

```text
awen.tensor GEMM
  -> shape/layout/precision validation
  -> whole-graph CPU, GPU, or photonic region placement
  -> residency-aware transfer, crossing, reuse, and memory optimization
  -> M/N/K tiling for a declared photonic matrix core
  -> classical Photonic IR
  -> Device IR command buffer
  -> calibrated reference simulator and CPU comparison

normalized stablehlo.dot_general
  -> registered MLIR awen_tensor dialect
  -> registered MLIR awen_photonic dialect
  -> registered MLIR awen_device dialect
  -> versioned AWENEXE binary
  -> direct Rust runtime command preparation
```

The MLIR path currently accepts only normalized rank-two StableHLO
`dot_general`; it does not yet provide a real `torch.compile` backend, broad
`awenBLAS` coverage, or validated hardware speedups. Those are tracked
explicitly in the [compiler roadmap](https://github.com/marcpoliquin5/awen/issues/5).
Do not interpret reference capability values or simulator cost estimates as
measured product performance.

## Repository layout

- `awen-compiler`: typed Tensor IR, validated capability/health negotiation, full-system cost/autotuning, crossing-aware whole-graph CPU/GPU/photonic partitioning, GEMM tiling, Photonic IR, Device IR, `awenBLAS` reference GEMM, and calibrated benchmark simulator.
- `awen-mlir`: MLIR 20 ODS/TableGen dialects, StableHLO GEMM import passes,
  Device IR bytecode, and the `AWENEXE` emitter.
- `awen-runtime`: CLI, engine, HAL, scheduler, calibration, observability, artifacts, plugins, legacy node IR, and quantum experiments.
- `awen-spec`: schemas, specifications, and AWEN Enhancement Proposals.
- `awen-ecosystem`: Python experiments, example PDK data, kernels, marketplace, and plugin templates.
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

cmake -S awen-mlir -B awen-mlir/build -G Ninja \
  -DMLIR_DIR=/usr/lib/llvm-20/lib/cmake/mlir \
  -DLLVM_DIR=/usr/lib/llvm-20/lib/cmake/llvm \
  -DCMAKE_C_COMPILER=clang-20 \
  -DCMAKE_CXX_COMPILER=clang++-20 \
  -DCMAKE_BUILD_TYPE=Release
cmake --build awen-mlir/build --target awen-opt awen-compile -j2
ctest --test-dir awen-mlir/build --output-on-failure
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
crossings, regions, profiler events, typed classical Photonic IR, a calibration
reference, and the Device IR command stream.

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

This command quantizes literal inputs to the backend's advertised effective precision, executes the emitted tiles, applies the calibration transfer function and inverse compensation, compares against the digital reference GEMM, and fails when the declared accuracy contract is exceeded.

## Status and evidence

The canonical required check is `AWEN required quality gate`. Public performance claims must cite immutable end-to-end benchmark artifacts that include host transfer, reconfiguration, lasers, DAC/ADC, calibration, and digital post-processing. The repository currently makes no validated hardware-acceleration claim.
