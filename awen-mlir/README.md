# AWEN MLIR compiler

`awen-mlir` is the production compiler foundation for AWEN. It uses upstream
MLIR ODS/TableGen, registered dialects, operation verifiers, pass management,
text and bytecode parsers/printers, bytecode dialect versioning, and explicit
legality-checked lowering.

The implemented rank-two and equal-batch rank-three GEMM pipeline is:

```text
normalized stablehlo.dot_general
  -> awen_tensor.gemm
  -> awen_photonic.gemm_tile
  -> awen_device.execute_gemm
  -> AWENEXE 1.0 binary artifact
  -> awen_runtime::executable
```

`awen_qphotonic` is registered separately from classical photonics and is
intentionally absent from the classical GEMM pipeline. It defines distinct
Fock-state, Gaussian-state, and seeded sample-stream types; state-space-specific
gates; photon-counting and Gaussian measurements; and narrow measurement-to-
phase, displacement, or squeezing feed-forward. Passing a Fock state to a
Gaussian operation or a classical tensor to a quantum gate fails MLIR
verification. Numeric verifiers also reject invalid seed, shot, confidence,
coherence, calibration, precision, and latency contracts. `awen_photonic`
separately defines calibrated classical transforms, modulation, detection, and
GEMM.

## Build on Ubuntu 24.04

```bash
sudo apt-get install clang-20 cmake libmlir-20-dev lld-20 llvm-20-dev mlir-20-tools ninja-build
cmake -S awen-mlir -B awen-mlir/build -G Ninja \
  -DMLIR_DIR=/usr/lib/llvm-20/lib/cmake/mlir \
  -DLLVM_DIR=/usr/lib/llvm-20/lib/cmake/llvm \
  -DCMAKE_C_COMPILER=clang-20 \
  -DCMAKE_CXX_COMPILER=clang++-20 \
  -DCMAKE_BUILD_TYPE=Release
cmake --build awen-mlir/build --target awen-opt awen-compile -j2
ctest --test-dir awen-mlir/build --output-on-failure
```

## Compile StableHLO GEMM

```bash
awen-mlir/build/tools/awen-opt/awen-opt \
  awen-mlir/test/stablehlo_gemm.mlir \
  --awen-lower-stablehlo-to-device

awen-mlir/build/tools/awen-compile/awen-compile \
  awen-mlir/test/stablehlo_gemm.mlir \
  -o stablehlo_gemm.awenx

cargo run --manifest-path awen-runtime/Cargo.toml \
  --bin awenctl -- execute stablehlo_gemm.awenx
```

## StableHLO import contract

The v1 importer deliberately supports only rank-two or equal-batch rank-three
`stablehlo.dot_general` GEMM. A producer normalization step must materialize the
dimension-number attribute as these four dense i64-array operation attributes:

- `lhs_batching_dimensions`
- `rhs_batching_dimensions`
- `lhs_contracting_dimensions`
- `rhs_contracting_dimensions`

Rank-two batching arrays must be empty and contracting dimensions must be lhs
`[1]` and rhs `[0]`. Rank-three batching dimensions must be lhs `[0]` and rhs
`[0]`, with contracting dimensions lhs `[2]` and rhs `[1]`; this represents
lhs `[B,M,K]`, rhs `[B,K,N]`, and result `[B,M,N]`. Static batch, M, K, and N
dimensions must agree, while dynamic dimensions remain dynamic through the
AWENEXE result-shape contract. F16, BF16, F32, and complex floating-point
element types are preserved. Static and dynamic result dimensions, optional
`awen.minimum_effective_bits`, optional `awen.layout`, and the MLIR source
location are preserved. All other StableHLO operations and layouts fail with a
source-located diagnostic.

This normalized boundary avoids maintaining a fork of the StableHLO dialect.
Framework integrations are responsible for using upstream StableHLO
serialization/versioning and then applying this explicitly documented import
normalization.
