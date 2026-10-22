# AWEN status

Updated 2026-10-18 13:48 ET (time of this commit). Governing brief: AWEN Master Prompt v2 (Marc, 2026-10-18).
This commit is based on main `01b927f` (#133), after `c1086fc` (#132), `c02eee9` (#131), `132d4d9` (#130),
`75d58f7` (#129), `ee2e8cf` (#127), `840f5c5` (#126) and `0235668` (#125). Truth-table test counts were measured at
`0235668`; the inventory counts at `01b927f` with this change. Gates passed on `0235668` (run 39140149777), `840f5c5`
(run 39141006838), `132d4d9` (run 39143781587), `c02eee9` (run 39147442569) and `c1086fc` (run 39150273629); the gate on `01b927f` (run 39151136678) was running at commit time.

## Current phase

**Phase 0 (audit and truth layer), session 1 under v2.** Most of the Phase 1-2
engineering already exists from the earlier directive (see the truth table), but
several earlier agent decisions now need Marc's ratification
([DECISIONS.md](DECISIONS.md), D-007 to D-012), so Phase 0 is not done until they
are settled. `awen-spec/hardware_assumptions.yaml` v0 is merged (#126, #130); no code reads it yet, and the
device literals still hard-coded in `awen-compiler` are listed in `awen-spec/hardcoded_device_defaults.yaml`.

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
| Hardware assumptions file | `awen-spec/hardware_assumptions.yaml` | L0 (spec data) | v0 merged #126 (`840f5c5`); section 9 follow-up merged #130 (`132d4d9`) |
| Inventory of hard-coded device defaults, checked in CI | `awen-spec/hardcoded_device_defaults.yaml`, `scripts/hardcoded_defaults.py` | L0 (spec data) | non-test `awen-compiler/src` and `awen-runtime/src` code, two literal forms: single-line `field: <number>,` device literals whose field name matches the device-field pattern (energy, power, noise, loss, crosstalk, precision, calibration, temperature, and since #139 also latency, voltage, fidelity, dark count/current, saturation, hysteresis, coherence time; 107 compiler + 40 runtime) and float fallbacks, floors and caps inside expressions (`map_or`/`unwrap_or`/`max`/`min`/`clamp`, 38 compiler + 26 runtime); 211 entries (compiler 145 in 36 functions, runtime 66 in 28 functions; 138 unsourced, 27 numeric guards, 42 definitional, 3 differ from the assumption set, 1 matches). Not covered: `.clamp` upper bounds (only the lower bound is recorded), integer guards, `fold` initial values, arithmetic factors (e.g. `* 0.01`), fields outside the device-field name pattern (e.g. `coupling_q`, `read_efficiency`, `t1_ns`, `photon_count`, `click_probability`), lines with a trailing comment (e.g. `read_latency_ns: 1000, // ...` in `awen-runtime/src/state/memory.rs`), tuple values (e.g. `nominal_voltage_range: (0.0, 10.0)` in `hal_v0.rs`), test modules (e.g. `photons_per_mac: Some(10.0)` in an `executable.rs` test); `awen-native`, `awen-mlir` and the Python packages not inventoried yet; `python scripts/hardcoded_defaults.py --check` |

Count commands (run at `0235668`):

```
C1: rg -c '^\| `[a-z_0-9]+` \| (hardware_lowerable|photonic_sim|math_reference)' awen-spec/specs/ops.md   # 12
C2: sed -n '/fn all_kernel_kinds/,/^}/p' awen-compiler/src/awenblas/kernels.rs | grep -c 'KernelKind::'          # 25
```

## Gap list against Master Prompt v2

| Gap | v2 section | Current level | Needed |
| --- | --- | --- | --- |
| Simulators and cost estimates read `hardware_assumptions.yaml` | 9 | file merged (#126, #130), not read by any code; `awen-compiler` and `awen-runtime` device literals and in-expression fallbacks (211 entries, forms listed in the truth table) inventoried and checked in CI, `awen-native` and Python not yet | loader in `awen-compiler`, then replace inventoried literals one block at a time |
| AWEN-Mini as a fine-tuned open model with model card, domain eval (≥300 Q) | 7 | from-scratch char model only | base model choice (Marc), SFT/KD, eval |
| `Runtime(device="auto").load_model("awen-mini")` | 7 | `device="photonic-sim"` and cpu exist | `auto` + `load_model` API |
| AWEN-Photon curves: bits, energy/MAC, recalibration interval, conversions/token, sim-to-real noise control | 7 | MNIST shot-noise curves only (L1) | four curves + independent noise model |
| OpenXLA PJRT plugin | 8 | StableHLO `dot_general` executor (L1) | plugin |
| torch.compile backend with FX partitioning | 8 | PrivateUse1 eager GEMM (L1) | backend + capability report |
| ONNX Runtime execution provider | 8 | ONNX import report (L0) | EP |
| Adapters to Meep, SAX, gdsfactory, Perceval, MrMustard | 8, 11 | own reference solvers (L0) | adapters + oracle comparisons |
| Reproduce ≥3 published photonic results | 11 | analytic checks only | 3 reproductions |
| Multiplexing-aware lowering pass, H1-H4 keep/kill report | 10 | pilots on a branch; H1-H4 registered (#133) | full runs, lowering pass, report |
| ≥3 seeds on matched experiments | 14 | 2 seeds (E-003, E-005, E-008; stored results, see EXPERIMENTS.md) | rerun with a third seed |
| docs/install.md with pinned CUDA 12.8+ / cu128 lockfile | 4 | none | install doc + lockfile |
| Session reports in `docs/sessions/` | 3, 17 | `2026-10-18-01.md` (#127), `2026-10-18-02.md` | every session |

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
- Device energies and noise defaults are still hard-coded in `awen-compiler` (145 entries) and
  `awen-runtime` (66 entries, mostly `hal_v0.rs` defaults, quantum/state simulator constants and reference-simulator fallbacks), all listed in
  `awen-spec/hardcoded_device_defaults.yaml`, until the noise model and cost estimates read
  `hardware_assumptions.yaml`. 138 have no checked value in the assumption set yet, so wiring them needs
  sourced values first, never guesses. `awen-native` and the Python packages are not inventoried yet.

## In progress (draft, not merged)

- Multiplexing/fractal pilots (H1 butterfly vs Clements meshes, H2 Kronecker/Monarch
  layers, H3 Cantor/Fibonacci stacks): draft #128, branch `research/fractal-track`, exploratory only.
  H1-H4 are registered (`research/experiments/multiplexing-fractal/PREREGISTRATION.md`, #133); full runs not started.
- Speed program (simulator GEMM, binding copies, KV cache, compile latency): branch
  `perf/hot-paths`; every speedup must be measured with `awen-bench`.

## Next 3 tasks

1. Wiring `hardware_assumptions.yaml`, slice 2 (slice 1, the CI-checked inventory, is done): load the file
   in `awen-compiler`, record the assumption-set version in cost estimates, and make the 3 literals that
   differ from it (reference descriptor ADC energy 4.0 pJ, DAC 10 bits, effective bits 8) either read the
   file or carry an explicit "illustrative" label, with compatibility notes.
2. Run H1 and H3 on CPU from the registered configs (`research/experiments/multiplexing-fractal/configs/`,
   `PREREGISTRATION.md`; H1-H4 registered in #133): add the `--config` entry points first, draft PRs only.
3. Get Marc's answers on D-007 to D-012, starting with the AWEN-Mini base model (Phase 1, session P1-1).
