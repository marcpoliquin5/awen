# AEP-0011: MLIR embedding and StableHLO GEMM import

Status: Accepted and implemented for the GEMM subset

## Decision

AWEN's production compiler is an out-of-tree MLIR project. The bootstrap Rust
compiler remains a semantic reference and compatibility backend; it is not the
parser or optimization framework for production compiler growth.

The initial supported build pins the Ubuntu 24.04 MLIR/LLVM 20 packages. ODS
and TableGen generate operation, type, parser/printer, verifier, and bytecode
property code for four registered dialects:

- `awen_tensor`: shaped tensor computation and numerical contracts.
- `awen_photonic`: classical analog-photonic physical operations.
- `awen_qphotonic`: quantum state, circuit, measurement, and target boundary.
- `awen_device`: capability-specialized executable commands.

Every dialect owns a custom ABI marker type and an explicit bytecode version.
MLIR source locations are retained during operation replacement.

## StableHLO boundary

The first importer recognizes only rank-two `stablehlo.dot_general` with no
batch dimensions, lhs contraction dimension 1, and rhs contraction dimension
0. Producers normalize StableHLO's dimension-number attribute into four dense
i64-array attributes before invoking the AWEN pass. This keeps AWEN dependent
on StableHLO semantics without maintaining a partial fork of its upstream C++
dialect.

The importer preserves static or dynamic shapes, F16/BF16/F32/complex element
types, layout, minimum-effective-bit contracts, and source locations.
Unsupported operation names, ranks, batching, axes, shapes, and element types
produce source-located errors.

## Conversion legality

The pass sequence is explicit:

1. `awen-import-stablehlo`
2. `awen-lower-tensor-to-photonic`
3. `awen-lower-photonic-to-device`

`awen-lower-stablehlo-to-device` composes these passes deterministically. Each
pass consumes only its declared source operation and produces a registered
target operation with typed properties. The MLIR verifier runs between passes
and after parsing; the standard MLIR canonicalizer runs at every abstraction
boundary.

## Build and distribution

Source builds use CMake and Ninja with `MLIR_DIR` and `LLVM_DIR`. Linux CI
installs the pinned Ubuntu packages, regenerates every TableGen file, builds
both compiler tools, runs parser/verifier/diagnostic/golden/bytecode tests, and
executes the emitted binary artifact through the Rust runtime.

Release distributions should package `awen-opt`, `awen-compile`, the matching
`AWENEXE` ABI documentation, and the dialect version manifest. Generated
TableGen files and CMake build trees are not committed.

## IREE influence

AWEN follows IREE's separation between compiler IR and a stable runtime-facing
executable/command boundary. Capability-specific scheduling and dispatch
metadata are compiled ahead of runtime consumption. AWEN does not copy IREE's
HAL dialect, VM, FlatBuffer schemas, or backend implementation; photonic
calibration, precision, and command semantics remain AWEN-specific.

## CUDA-Q influence

AWEN follows CUDA-Q's explicit target/backend selection model and its
separation between compiler lowering and provider-specific execution. Classical
and quantum photonics remain separate dialects and pipelines even though they
share executable packaging and runtime registration. AWEN does not copy
CUDA-Q's quantum IR, platform APIs, target YAML, or remote-provider protocols.

## Consequences

- Compiler builds require a matching MLIR development distribution.
- StableHLO framework adapters must perform the documented normalization.
- The Rust runtime does not link MLIR and remains small enough for device-side
  deployment.
- Dialect bytecode and the runtime executable ABI evolve independently.
