# AWEN status

Updated 2026-10-18 11:37 ET (time of this commit). Governing brief: AWEN Master Prompt v2 (Marc, 2026-10-18).
Main is `840f5c5` (#126), after `0235668` (#125). Truth-table counts were measured at `0235668`; the
gate on `0235668` passed (CI run 39140149777); the gate on `840f5c5` (run 39141006838) was running at this commit.

## Current phase

**Phase 0 (audit and truth layer), session 1 under v2.** Most of the Phase 1-2
engineering already exists from the earlier directive (see the truth table), but
several earlier agent decisions now need Marc's ratification
([DECISIONS.md](DECISIONS.md), D-007 to D-012), so Phase 0 is not done until they
are settled. `awen-spec/hardware_assumptions.yaml` v0 is merged (#126); nothing reads it yet.

## Evidence levels

L0 math reference, L1 photonic simulation, L2 hardware-lowerable (device commands
generated, no device), L3 measured on hardware. **Nothing in AWEN is L3.**

## Truth table (summary)

Source labels: **measured @ 0235668** = run by the agent on the build box at commit `0235668`
(Linux 6.12, Intel Xeon 8 vCPU, no GPU, rustc 1.99.1, Python 3.13.5) with the command shown;
**stored result** = read from the named committed file; **counted @ 0235668** = exact count
command shown.

| Capability | Where | Level | Evidence |
| --- | --- | --- | --- |
| Tensor IR → photonic GEMM plan (tiling, partitioning, calibration-aware compile) | `awen-compiler` | L1 | compiler tests; `docs/EVIDENCE.md` (no count stated) |
| StableHLO GEMM lowering (rank 2, equal-batch rank 3) | `awen-mlir` | L1 | MLIR ctest |
| Virtual photonic ISA v1 (encode, validate, interpret) | `awen-compiler/src/isa` | L2 (virtual ISA only, no vendor command set) | `tests/isa.rs` |
| Photonic GEMM noise model (shot, thermal, RIN, phase, DAC/ADC, crosstalk) | `awen-compiler/src/noise.rs` | L1 | AEP-0022, noise tests |
| Ops beyond GEMM (12 ops, each labeled) | `awen-compiler/src/ops.rs` | L0/L1 per op | 12 counted @ 0235668 (command C1 below); ops tests |
| awenBLAS kernels (25 kernel kinds; C ABI in `awen-runtime`) | `awen-compiler/src/awenblas` | L0/L1 | 25 counted @ 0235668 (command C2 below); runtime tests |
| Runtime, HAL, scheduler, streams, fallback, replay, profiler | `awen-runtime` | L1 (simulated device) | 305 tests, measured @ 0235668: `cd awen-runtime && cargo test -q` |
| C SDK, `awenctl` CLI, model conversion | `awen-runtime`, `awen-models` | L0/L1 | CI C example, CLI tests |
| Python SDK `Runtime(device="photonic-sim")`, PyO3 binding | `awen-ecosystem`, `awen-native` | L1 | 48 + 23 tests, measured @ 0235668: `pytest -q` in `awen-ecosystem/python_awen` and `awen-native/tests` |
| PyTorch PrivateUse1 "awen" device (eager GEMM) | `awen-ecosystem` | L1 | tests; no torch.compile backend yet |
| JAX StableHLO `dot_general` on AWEN | `awen-ecosystem` | L1 | `docs/PJRT_BOUNDARY.md`; no PJRT plugin |
| ONNX import with support report | `awen-models` | L0 (report) / L1 (GEMMs) | 7 tests, measured @ 0235668: `pytest -q awen-models/tests/test_onnx*.py` |
| AWEN-Mini (1,016,320-param char model, trained on CPU; stored result `awen-models/results/awen-mini-eval.json`) | `awen-models` | L0 on CPU, L1 on photonic-sim | checkpoint + eval JSON |
| AWEN-Photon noise-aware MNIST MLP | `awen-models` | L1 | `awen-models/results/photon-training/` |
| Physics solvers (TMM, S-matrix, rings, 1-D FDTD, BPM, SHG, adjoint, slab modes, Clements) | `awen-physics` | L0 (validated against analytic results) | 51 tests, measured @ 0235668: `cd awen-physics && pytest -q` |
| Quantum-photonic simulators (Fock, boson sampling, Gaussian, kernel) | `awen-qphotonic` | L0/L1, no advantage claimed | 61 tests, measured @ 0235668: `cd awen-qphotonic && pytest -q` |
| Memory and fabric models | `awen-systems` | L1 (modeled) | 26 tests, measured @ 0235668: `cd awen-systems && pytest -q` |
| Lab Agent (simulation only, refuses hardware) | `awen-lab` | L1 | 11 tests, measured @ 0235668: `cd awen-lab && pytest -q` |
| Studio (local web app) | `awen-studio` | L1 (wraps simulators) | 13 passed + 1 skipped, measured @ 0235668: `cd awen-studio && pytest -q` |
| Benchmark harness with sealed evidence records | `awen-models/awen_models/bench.py` | measured (CPU) / L1 | `benchmarks/results/` |
| Hardware assumptions file | `awen-spec/hardware_assumptions.yaml` | L0 (spec data) | v0 merged #126 (`840f5c5`); section 9 follow-up PR #130 in review |

Count commands (run at `0235668`):

```
C1: rg -c '^\| `[a-z_0-9]+` \| (hardware_lowerable|photonic_sim|math_reference)' awen-spec/specs/ops.md   # 12
C2: sed -n '/fn all_kernel_kinds/,/^}/p' awen-compiler/src/awenblas/kernels.rs | grep -c 'KernelKind::'          # 25
```

## Gap list against Master Prompt v2

| Gap | v2 section | Current level | Needed |
| --- | --- | --- | --- |
| Simulators and cost estimates read `hardware_assumptions.yaml` | 9 | file merged (#126), not read by any code | wiring, no hard-coded device energies |
| AWEN-Mini as a fine-tuned open model with model card, domain eval (≥300 Q) | 7 | from-scratch char model only | base model choice (Marc), SFT/KD, eval |
| `Runtime(device="auto").load_model("awen-mini")` | 7 | `device="photonic-sim"` and cpu exist | `auto` + `load_model` API |
| AWEN-Photon curves: bits, energy/MAC, recalibration interval, conversions/token, sim-to-real noise control | 7 | MNIST shot-noise curves only (L1) | four curves + independent noise model |
| OpenXLA PJRT plugin | 8 | StableHLO `dot_general` executor (L1) | plugin |
| torch.compile backend with FX partitioning | 8 | PrivateUse1 eager GEMM (L1) | backend + capability report |
| ONNX Runtime execution provider | 8 | ONNX import report (L0) | EP |
| Adapters to Meep, SAX, gdsfactory, Perceval, MrMustard | 8, 11 | own reference solvers (L0) | adapters + oracle comparisons |
| Reproduce ≥3 published photonic results | 11 | analytic checks only | 3 reproductions |
| Multiplexing-aware lowering pass, H1-H4 keep/kill report | 10 | pilots on a branch | pre-registration, runs, report |
| ≥3 seeds on matched experiments | 14 | 2 seeds (E-003, E-005, E-008; stored results, see EXPERIMENTS.md) | rerun with a third seed |
| docs/install.md with pinned CUDA 12.8+ / cu128 lockfile | 4 | none | install doc + lockfile |
| Session reports in `docs/sessions/` | 3, 17 | 2026-10-18-01 in review (#127) | every session |

## Phase 1 plan (one task per session)

| Session | Task | Done when |
| --- | --- | --- |
| P1-1 | Marc picks the AWEN-Mini base model (recommendation: Qwen3-0.6B, Apache-2.0; fallback SmolLM2-360M) | decision logged in DECISIONS.md |
| P1-2 | `docs/install.md` + pinned CPU and cu128 lockfiles; CPU fallback for every example | fresh-clone install test in CI |
| P1-3 | `Runtime(device="auto"\|"cpu"\|"cuda").load_model("awen-mini").generate()` with a tiny CI config | CI test on CPU |
| P1-4 | AWEN-Mini v0 fine-tune script, model card, checkpoint SHA-256 (CPU smoke test; laptop GPU run plan for Marc) | smoke test passes; run plan written |
| P1-5 | Same model on `device="photonic-sim"`: linear layers through the GEMM lowering, capability report, deviation from CPU | CI test with tiny config |
| P1-6 | Three validation notebooks re-checked under v2 (slab modes, MZI mesh vs Clements, shot-noise bits) | notebooks run in CI |
| P1-7 | Laptop CUDA run logged by Marc for all three devices | log committed |

## Blockers

- No photonic hardware: nothing can reach L3.
- No GPU on the build box: CUDA paths are untested; Marc's RTX 5070 Ti laptop would cover this.
- **Marc's 72-paper ledger export is missing.** `research/ledger/ledger.json` is an empty index; the 15
  agent-written entries live separately in `research/reference/agent-sourced.json`.
- Section 16 decisions pending Marc: D-007 to D-012 (v0.1.0 tag, AWEN-Mini design, SmolLM2 base,
  physics solvers, Studio location, frontier directory); see DECISIONS.md. Settled by Marc: MIT
  license (D-001), agent self-merge after green checks (D-002), v0.1.0 marked pre-release (D-015).
- Release tag signing (v2 section 15): the v0.1.0 tag is unsigned. It needs a signing key created
  and held by Marc only; the agent must never create or hold it. Blocked on Marc.
- Release artifacts have no cryptographic signatures, only `SHA256SUMS`.
- Device energies and noise defaults are still hard-coded in the noise contract until the
  noise model and cost estimates read `hardware_assumptions.yaml`.

## In progress (draft, not merged)

- Multiplexing/fractal pilots (H1 butterfly vs Clements meshes, H2 Kronecker/Monarch
  layers, H3 Cantor/Fibonacci stacks): draft #128, branch `research/fractal-track`; pre-registration
  under `research/experiments/multiplexing-fractal/` comes before any claim.
- Speed program (simulator GEMM, binding copies, KV cache, compile latency): branch
  `perf/hot-paths`; every speedup must be measured with `awen-bench`.

## Next 3 tasks

1. Make the noise model and cost estimates read `awen-spec/hardware_assumptions.yaml`, with no
   hard-coded device energies.
2. Pre-register H1-H4 with configs, then run H1 (accuracy per dB, ≥3 tasks, ≥5 fabrication-noise
   seeds) and H3 on CPU; draft PRs only.
3. Get Marc's answers on D-007 to D-012, starting with the AWEN-Mini base model (Phase 1, session P1-1).
