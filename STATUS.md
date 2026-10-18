# AWEN status

Updated 2026-10-18 11:15 ET. Governing brief: AWEN Master Prompt v2 (Marc, 2026-10-18).
Main is `e6d4224`; the required quality gate is green on it (run of 2026-10-18 09:21 ET).

## Current phase

**Phase 0 (audit and truth layer), session 1 under v2.** Most of the Phase 1-2
engineering already exists from the earlier directive (see the truth table), but
several earlier agent decisions now need Marc's ratification
([DECISIONS.md](DECISIONS.md), D-004 to D-011), so Phase 0 is not done until they
are settled and `awen-spec/hardware_assumptions.yaml` v0 lands.

## Evidence levels

L0 math reference, L1 photonic simulation, L2 hardware-lowerable (device commands
generated, no device), L3 measured on hardware. **Nothing in AWEN is L3.**

## Truth table (summary)

| Capability | Where | Level | Evidence |
| --- | --- | --- | --- |
| Tensor IR → photonic GEMM plan (tiling, partitioning, calibration-aware compile) | `awen-compiler` | L1 | compiler tests; `docs/EVIDENCE.md` |
| StableHLO GEMM lowering (rank 2, equal-batch rank 3) | `awen-mlir` | L1 | MLIR ctest |
| Virtual photonic ISA v1 (encode, validate, interpret) | `awen-compiler/src/isa` | L2 (virtual ISA only, no vendor command set) | `tests/isa.rs` |
| Photonic GEMM noise model (shot, thermal, RIN, phase, DAC/ADC, crosstalk) | `awen-compiler/src/noise.rs` | L1 | AEP-0022, noise tests |
| Ops beyond GEMM (12 ops, each labeled) | `awen-compiler/src/ops.rs` | L0/L1 per op | ops tests, `awen-spec/specs/ops.md` |
| awenBLAS kernels (22 + extensions) | `awen-runtime` | L0/L1 | runtime tests |
| Runtime, HAL, scheduler, streams, fallback, replay, profiler | `awen-runtime` | L1 (simulated device) | 305 runtime tests |
| C SDK, `awenctl` CLI, model conversion | `awen-runtime`, `awen-models` | L0/L1 | CI C example, CLI tests |
| Python SDK `Runtime(device="photonic-sim")`, PyO3 binding | `awen-ecosystem`, `awen-native` | L1 | 48 + 23 tests |
| PyTorch PrivateUse1 "awen" device (eager GEMM) | `awen-ecosystem` | L1 | tests; no torch.compile backend yet |
| JAX StableHLO `dot_general` on AWEN | `awen-ecosystem` | L1 | `docs/PJRT_BOUNDARY.md`; no PJRT plugin |
| ONNX import with support report | `awen-models` | L0 (report) / L1 (GEMMs) | 7 tests |
| AWEN-Mini (1.0M-param char model, trained on CPU) | `awen-models` | L0 on CPU, L1 on photonic-sim | checkpoint + eval JSON |
| AWEN-Photon noise-aware MNIST MLP | `awen-models` | L1 | `awen-models/results/photon-training/` |
| Physics solvers (TMM, S-matrix, rings, 1-D FDTD, BPM, SHG, adjoint, slab modes, Clements) | `awen-physics` | L0 (validated against analytic results) | 51 tests |
| Quantum-photonic simulators (Fock, boson sampling, Gaussian, kernel) | `awen-qphotonic` | L0/L1, no advantage claimed | 61 tests |
| Memory and fabric models | `awen-systems` | L1 (modeled) | 26 tests |
| Lab Agent (simulation only, refuses hardware) | `awen-lab` | L1 | 11 tests |
| Studio (local web app) | `awen-studio` | L1 (wraps simulators) | 14 tests |
| Benchmark harness with sealed evidence records | `awen-models/awen_models/bench.py` | measured (CPU) / L1 | `benchmarks/results/` |
| Hardware assumptions file | `awen-spec/hardware_assumptions.yaml` | missing | PR in progress (v2 section 9) |

## Blockers

- No photonic hardware: nothing can reach L3.
- No GPU on the build box: CUDA paths are untested; Marc's RTX 5070 Ti laptop would cover this.
- The 72-paper ledger is not in the repo (`research/ledger/` has only 15 checked seed entries).
- Section 16 decisions pending (license, base model, v0.1.0 tag, merge policy): see DECISIONS.md.
- Device energies and noise defaults are still hard-coded in the noise contract until
  `hardware_assumptions.yaml` exists and is wired in.

## In progress (draft, not merged)

- Multiplexing/fractal pilots (H1 butterfly vs Clements meshes, H2 Kronecker/Monarch
  layers, H3 Cantor/Fibonacci stacks): branch `research/fractal-track`; pre-registration
  under `research/experiments/multiplexing-fractal/` comes before any claim.
- Speed program (simulator GEMM, binding copies, KV cache, compile latency): branch
  `perf/hot-paths`; every speedup must be measured with `awen-bench`.

## Next 3 tasks

1. Land `awen-spec/hardware_assumptions.yaml` v0 (cited low/typical/high) and make the noise
   model and cost estimates read it.
2. Pre-register H1-H4 with configs, then run H1 (accuracy per dB, ≥3 tasks, ≥5 fabrication-noise
   seeds) and H3 on CPU; draft PRs only.
3. Get Marc's answers on D-004 to D-011, then plan Phase 1 (fine-tuned open-model AWEN-Mini v0).
