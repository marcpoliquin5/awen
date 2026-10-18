# Implementation status

This file is the repository's capability truth table. Per-item status with
pull requests is in `docs/DIRECTIVE_TRACKER.md`; commands and test counts are in
`docs/EVIDENCE.md`. Source, conformance tests,
and versioned schemas remain authoritative where they are more specific.

| Area | Classification | Evidence or boundary |
| --- | --- | --- |
| Rust tensor-to-photonic compiler | Implemented experimental slice | `awen-compiler`, compiler tests, typed schemas |
| Rust runtime/HAL/scheduler/simulator | Implemented experimental reference | `awen-runtime`, runtime integration tests |
| MLIR StableHLO rank-two/equal-batch rank-three GEMM path | Implemented narrow path | `awen-mlir` build, deterministic AWENEXE, and lit/CTest coverage |
| PyTorch/JAX/NumPy/C++ integration | Implemented reference boundaries | framework tests and compiled ABI tests |
| Classical and quantum-photonic dialects | Implemented typed contracts | AEP-0020, schemas, runtime conformance |
| Physical-design integration | Implemented metadata/evidence boundary only | AEP-0021 and physical-design conformance |
| AWEN-Mini model, noise-aware training, HF adapter | Implemented, measured on CPU | `awen-models` (trained checkpoint, eval, ablation, capability report) |
| Python native binding to the Rust compiler and noise model | Implemented | `awen-native` (PyO3), `torch.compile` photonic target, PrivateUse1 `awen` device (simulated) |
| Reference physics validation | Implemented, numerically validated | `awen-physics` notebooks (AEP-0023) |
| Virtual Photonic ISA, ops library beyond GEMM, ONNX import/export | Implemented, reference and simulator levels | VPISA v1 (`awen-compiler/src/isa`), 12-op library with per-op levels, `awen-convert`; nothing hardware-measured |
| Runtime streams, residency, leases, remap/fallback, replay, profiling | Implemented on simulated devices | `awen_runtime::devrt`; plugin out-of-process isolation not started |
| C SDK and CLI tools | Implemented | `include/awen/awen.h` (C11 example in CI), `awenctl caps`, `awenctl profile`, `awenctl plugin` |
| Hardware description schemas and reference simulator drivers | Implemented, simulator only | `awen.hardware-description.v1`, `docs/VENDOR_INTEGRATION.md`; no vendor drivers |
| Physics solvers | Partial, numerically validated | `awen-physics` (TMM/S-matrix, rings, 1-D FDTD, BPM, SHG, adjoint); 2-D FDTD, dispersion, thermo/electro-optic, external adapters open |
| Quantum-photonic simulation | Implemented, simulation only | `awen-qphotonic`; no quantum advantage claimed or observed |
| Memory and interconnect models | Implemented, modeled | `awen-systems`; three cited device values, the rest stated parameters |
| Benchmarks, efficiency study, research ledger | Implemented, measured on CPU where stated | `awen-bench`, `awen-models` efficiency results, `research/` |
| Lab Agent and Studio | Implemented, local and simulation only | `awen-lab`, `awen-studio`; every hardware action refused |
| Release artifacts, SBOM, signing | Implemented | `release.yml`, `scripts/build_release_artifacts.sh`, `scripts/verify_release_artifacts.py`, `deny.toml`, plugin signing |
| Physical accelerator performance | Not established | No verified physical HIL artifact is published |
| Hosted cloud product | Not offered | Design document only (`docs/CLOUD_ARCHITECTURE.md`); no service exists |
| Datacom kernel package | Removed | The empty documentation-only directory was removed |
| Plugin template | Implemented as guidance | Points to the tested reference manifest and complete acceptance checklist |
| Marketplace | Reference-only metadata | One immutable manifest checksum; no install or commerce service |
| OTLP exporter | Not part of the public API | The panic-only exporter surface was removed; file artifacts are supported |
| Legacy calibration stub API | Removed | Typed calibration/control implementations and tests remain |

New planned work must have an open issue with an owner and measurable acceptance
criteria before a development marker is committed. The repository policy check
rejects unowned development markers.
