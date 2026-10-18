# AWEN implementation audit

Baseline: `main` at `020be48` (October 18, 2026, after the Step 1 cleanup
pull requests #51–#54). Every statement below is grounded in the source tree,
a local test run, or a dependency scan; the commands are in
`docs/EVIDENCE.md` (added when the directive completes; until then the commands
are in each pull request description).

## Classification used

| Label | Meaning |
| --- | --- |
| implemented | Code exists, is compiled, and is exercised by passing tests |
| partial | Implemented for a documented subset; the rest is missing |
| experimental | Implemented and tested, but its model values are synthetic or assumed |
| broken | Present but fails its own tests or cannot run |
| missing | No code; tracked by an open issue |

Fidelity labels for anything physical: analytic, compact model, numerical
propagation, full-wave, digital twin, measured. Nothing in this repository is
currently labeled *measured* against photonic hardware.

## Inventory

| Component | Language | Size | Tests at baseline |
| --- | --- | --- | --- |
| `awen-compiler` | Rust | ~17k lines | 83 passing, 89.1% line coverage |
| `awen-runtime` (+ `awenctl`) | Rust | ~20k lines | 268 passing, 85.6% line coverage |
| `awen-mlir` | C++ / MLIR 20 | 52 files | 1 CTest wrapping 17 lit-style cases |
| `awen-ecosystem/python_awen` (`awen_py`) | Python | ~3.4k lines | 29 passing on PyTorch 2.14.1 / JAX 0.11, 84% coverage |
| `awen-spec` | Markdown / JSON Schema | 21 AEPs, 24 specs, 33 schemas | schemas meta-validated; 17 instances validated |

## Truth table

| Capability | Status | Fidelity | Evidence | Gap / issue |
| --- | --- | --- | --- | --- |
| Tensor IR validation, precision/scaling/error contracts | implemented | analytic | compiler tests | Contracts are dimensionless fractions, not physical units (#56) |
| Calibration-aware compilation, fault remapping | experimental | compact model | `calibration_aware.rs` tests | Calibration data is synthetic |
| Capability/health negotiation | experimental | n/a | conformance tests | Reference capability files are synthetic |
| Cost model and autotuning | experimental | compact model | `cost_model_conformance` | Energy/latency numbers are estimates, never measured |
| CPU/GPU/photonic partitioning, GEMM tiling | implemented | n/a | partitioner tests | GEMM only (#62) |
| Photonic IR → Device IR | implemented | n/a | pipeline tests | No written ISA (#55) |
| Calibrated transfer simulator (`awenctl benchmark`) | experimental | compact model | CLI smoke test | Noise is fractional, not photon/ENOB based (#57) |
| awenBLAS (22 kernel kinds) | implemented (reference) | analytic | `awenblas_kernels.rs` | GPU/photonic paths are simulated dispatch (#65) |
| MLIR StableHLO `dot_general` → AWENEXE | partial | n/a | CTest | Rank 2 and equal-batch rank 3 only |
| `awenctl execute` | partial | n/a | ABI step in CI | Validates and prepares dispatches; performs no arithmetic (#59) |
| C/C++ GEMM ABI | implemented | analytic | C++20 consumer in CI | f32/f64 GEMM only (#66) |
| HAL | experimental | n/a | unit tests | Only `SimulatedDevice`; no hardware driver (#67) |
| Scheduler, observability, artifacts, plugin registry | implemented | n/a | integration tests | No streams/events, multi-device, plugin isolation (#64) |
| HIL benchmark orchestration and claim generator | implemented | n/a | `hil_benchmark_conformance` | Zero physical artifacts exist |
| Quantum Gaussian/Fock experiments | experimental | numerical | `quantum_integration.rs` | No permanents, boson sampling or resource model (#69) |
| Python `torch.compile` backend, JAX StableHLO, NumPy API | partial | analytic | pytest | Pure Python: GEMMs run on the host framework, not the Rust stack (#59) |
| PyTorch PrivateUse1 device | missing | | | #60 |
| OpenXLA PJRT plugin | missing | | | #61 |
| ONNX import | missing | | | #63 |
| Physics solvers (mode solver, FDTD, BPM, rings, SHG) | missing | | | #68 |
| Physical-unit noise model | missing | | | #56, #57 |
| Physics validation notebooks | missing | | | #58 |
| Memory subsystem / fabric simulators | missing | | | #70, #71 |
| Trained model checkpoints | missing | | | AWEN-Mini (this directive) |
| Studio, Lab Agent | missing (removed earlier as nonfunctional) | | | #75, #76 |
| Hosted cloud | not offered | | | Architecture document only |
| Supply chain (SBOM, binary release artifacts) | partial | | release workflow publishes source only | #78 |

## Dependency and security scan at baseline

- `cargo audit`: 0 vulnerabilities (compiler 27 crates, runtime ~189 crates).
- GitHub Dependabot alerts: 0 open (8 fixed). Secret scanning: 0 open. Code
  scanning: not configured.
- `pip-audit` on the CI environment: clean on the last green run.
- Outdated: `jsonschema` 0.16 (#79), `base64` 0.21, `rand` 0.8.
- MSRV: Rust 1.88, checked in CI; toolchain pinned to 1.99.1.

## Changes made by the Step 1 cleanup

- CI pinned, cached, scheduled weekly, MSRV-checked (#51).
- PyTorch 2.14 supported (#52; superseded Dependabot #49).
- Rust dependencies updated (#53; superseded Dependabot #33, #37–#40, #42, #44).
- ~1,060 lines of never-compiled code and 65 mock-only tests removed; real
  plugin-loader, HAL and CLI tests added (#54).
- Obsolete Dependabot PRs #32, #34, #35, #36 closed with explanations.

## Baseline test results at `020be48`

Run locally on October 18, 2026 (Linux x86-64, Rust 1.99.1, Python 3.12,
PyTorch 2.14.1 CPU):

| Command | Result |
| --- | --- |
| `cargo test --locked` in `awen-compiler` | 83 passed, 0 failed |
| `cargo test --locked` in `awen-runtime` | 268 passed, 0 failed |
| `python -m pytest awen-ecosystem/python_awen/tests` | 29 passed |
| `python scripts/validate_repository.py` | passed (316 files, 3 workflows) |
| `python scripts/validate_json.py` | 60 documents, 33 schemas, 17 instances valid |

The MLIR CTest suite and the C++ ABI consumer run in the required CI gate
(green on #54).
