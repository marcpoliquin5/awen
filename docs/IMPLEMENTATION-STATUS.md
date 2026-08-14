# Implementation status

This file is the repository's capability truth table. Source, conformance tests,
and versioned schemas remain authoritative where they are more specific.

| Area | Classification | Evidence or boundary |
| --- | --- | --- |
| Rust tensor-to-photonic compiler | Implemented experimental slice | `awen-compiler`, compiler tests, typed schemas |
| Rust runtime/HAL/scheduler/simulator | Implemented experimental reference | `awen-runtime`, runtime integration tests |
| MLIR StableHLO rank-two GEMM path | Implemented narrow path | `awen-mlir` build and lit/CTest coverage |
| PyTorch/JAX/NumPy/C++ integration | Implemented reference boundaries | framework tests and compiled ABI tests |
| Classical and quantum-photonic dialects | Implemented typed contracts | AEP-0020, schemas, runtime conformance |
| Physical-design integration | Implemented metadata/evidence boundary only | AEP-0021 and physical-design conformance |
| Physical accelerator performance | Not established | No verified physical HIL artifact is published |
| Hosted cloud product | Removed | The README-only nonfunctional directory was removed; no product is offered |
| Desktop Studio product | Removed | The nonfunctional UI directory was removed; no product is offered |
| Datacom kernel package | Removed | The empty documentation-only directory was removed |
| Plugin template | Implemented as guidance | Points to the tested reference manifest and complete acceptance checklist |
| Marketplace | Reference-only metadata | One immutable manifest checksum; no install or commerce service |
| OTLP exporter | Not part of the public API | The panic-only exporter surface was removed; file artifacts are supported |
| Legacy calibration stub API | Removed | Typed calibration/control implementations and tests remain |

New planned work must have an open issue with an owner and measurable acceptance
criteria before a development marker is committed. The repository policy check
rejects unowned development markers.
