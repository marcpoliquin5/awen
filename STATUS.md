# AWEN status

Updated 2026-11-10 ET (time of this commit). Latest: `f8385da` (#292, measured laser RIN upper bound -150 dB/Hz to 6 GHz and linewidth points 28/440 kHz from vaskasi2022 added as `laser.rin` (high corner only) and `laser.linewidth`, D-044, data only; the bound equals the built-in -150 dB/Hz, so no default or noisy output changed; PR gate run 42924472100 passed on head `eb68924`), after `4f79851` (#291), after `43fee8a` (#290, demarinis2021 per-MZI extinction ratio, D-043). Governing brief: AWEN Master Prompt v2 (Marc, 2026-10-18).
The full merge chain and gate-run list from `0235668` (#125) to `fed7c1a` (#282) moved to [docs/history/status-2026-11-09.md](docs/history/status-2026-11-09.md) (section 1).
Truth-table test counts were measured at `0235668`; the inventory counts at `14385b9` (gate runs for those commits: same file, section 1).

## Current phase

**Phase 0 (audit and truth layer), session 1 under v2.** Most of the Phase 1-2
engineering already exists from the earlier directive (see the truth table), but
several earlier agent decisions now need Marc's ratification
([DECISIONS.md](DECISIONS.md), D-007 to D-012), so Phase 0 is not done until they
are settled. `awen-spec/hardware_assumptions.yaml` v0 is merged (#126, #130). Since #134/#135 `awen-compiler`
loads it and can apply it opt-in to the noise model and cost estimates (defaults unchanged); the device literals still
hard-coded in `awen-compiler`, `awen-runtime`, awen-mlir and `awen-models` are listed in `awen-spec/hardcoded_device_defaults.yaml`.

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
| PyTorch PrivateUse1 "awen" device (eager GEMM) | `awen-ecosystem` | L1 | tests; `torch.compile(backend=awen)` (C3, #84) lowers `linear`/`matmul` to the Rust photonic GEMM; graph capability report for `awen-mini` since P2-1 (#195, `ea3a9e0`): 21 GEMMs lowered, 107 host fallbacks, E-017; opt-in per-GEMM deterministic noise seeds (`seed_policy="per_gemm"`, backend default still `shared`) since P2-2 (#197, `68cdcef`), E-018; host fallback contraction MACs counted since P2-3 (#199, `5690ff9`): 2,949,120 host (5 attention cores, dense count) vs 47,585,280 lowered, lowered MAC fraction 0.94164, E-019; a randomly initialised tiny `transformers` Llama (`LlamaForCausalLM`, built locally, no download) through the same path since P2-4 (#203, `d0d2fe1`): 15 GEMMs lowered, 155 host fallbacks, 0 uncounted, lowered MAC fraction 0.91667, E-020; tiny random-init `transformers` Qwen3 via `python -m awen.compile_llama --arch qwen3` since P2-5 (#211, `7511b3b`), E-021 |
| JAX StableHLO `dot_general` on AWEN | `awen-ecosystem` | L1 | `docs/PJRT_BOUNDARY.md`; no PJRT plugin |
| ONNX import with support report | `awen-models` | L0 (report) / L1 (GEMMs) | 7 tests, measured @ 0235668: `pytest -q awen-models/tests/test_onnx*.py` |
| AWEN-Mini (1,016,320-param char model, trained on CPU; stored result `awen-models/results/awen-mini-eval.json`) | `awen-models` | L0 on CPU, L1 on photonic-sim | checkpoint + eval JSON |
| AWEN-Photon noise-aware MNIST MLP | `awen-models` | L1 | `awen-models/results/photon-training/`; independent noise model B for the sim-to-real control registered in P2-6 (#213, `2f1cf90`, D-023) and run as E-022 in P2-7 (#215, `2ea621c`, `awen-models/results/photon-sim2real/`): at the sourced point (ENOB 7.61) the noise-aware advantage reverses under model B; accuracy-versus-recalibration-interval curve registered as E-023 in P2-8 (#217, `bf4c08c`, D-024) and run in P2-9 (#219, `ba1c57a`, `awen-models/results/photon-recalibration/`): tolerable drift budget standard 0.05, noise-aware-100 0.1, noise-aware-1000 0.05 (free-parameter budgets, no time scale) |
| Physics solvers (TMM, S-matrix, rings, 1-D FDTD, BPM, SHG, adjoint, slab modes, Clements) | `awen-physics` | L0 (validated against analytic results) | 51 tests, measured @ 0235668: `cd awen-physics && pytest -q` |
| Quantum-photonic simulators (Fock, boson sampling, Gaussian, kernel) | `awen-qphotonic` | L0/L1, no advantage claimed | 61 tests, measured @ 0235668: `cd awen-qphotonic && pytest -q` |
| Memory and fabric models | `awen-systems` | L1 (modeled) | 26 tests, measured @ 0235668: `cd awen-systems && pytest -q` |
| Lab Agent (simulation only, refuses hardware) | `awen-lab` | L1 | 11 tests, measured @ 0235668: `cd awen-lab && pytest -q` |
| Studio (local web app) | `awen-studio` | L1 (wraps simulators) | 13 passed + 1 skipped, measured @ 0235668: `cd awen-studio && pytest -q` |
| Benchmark harness with sealed evidence records | `awen-models/awen_models/bench.py` | measured (CPU) / L1 | `benchmarks/results/` |
| Hardware assumptions file | `awen-spec/hardware_assumptions.yaml` | L0 (spec data) | v0 merged #126 (`840f5c5`); section 9 follow-up merged #130 (`132d4d9`); loader `awen_compiler::assumptions` merged #134 (`243acef`); opt-in `with_assumptions` for the noise contract and cost inputs, with before/after outputs in `awen-compiler/results/assumptions-wiring/`, merged #135 (`950a859`); phase-shifter insertion loss (harris2014, per heater, not an MZI or path loss) merged #286 (`2af62e1`); Ge photodetector bandwidth and dark current (chen2016; opt-in typical corner applies 67 GHz) merged #288 (`e8488c9`); per-MZI extinction ratio (demarinis2021; not mapped, not a crosstalk) merged #290 (`43fee8a`); laser RIN upper bound (vaskasi2022, high corner only, equals the built-in value) and linewidth points merged #292 (`f8385da`) |
| Inventory of hard-coded device defaults, checked in CI | `awen-spec/hardcoded_device_defaults.yaml`, `scripts/hardcoded_defaults.py` | L0 (spec data) | 236 entries (compiler 145 in 36 functions, runtime 66 in 28 functions, awen-mlir 2, awen-models 8, awen-systems 5, awen-qphotonic 3, awen-lab 3, awen-studio 1, awen-ecosystem 3; 151 unsourced, 27 numeric guards, 48 definitional, 8 differ from the assumption set, 2 match, as of `f8385da`); scanned forms and the not-covered list are in [docs/history/status-2026-11-09.md](docs/history/status-2026-11-09.md) (section 2); `python scripts/hardcoded_defaults.py --check` |

Count commands (run at `0235668`):

```
C1: rg -c '^\| `[a-z_0-9]+` \| (hardware_lowerable|photonic_sim|math_reference)' awen-spec/specs/ops.md   # 12
C2: sed -n '/fn all_kernel_kinds/,/^}/p' awen-compiler/src/awenblas/kernels.rs | grep -c 'KernelKind::'          # 25
```

## Gap list against Master Prompt v2

| Gap | v2 section | Current level | Needed |
| --- | --- | --- | --- |
| Simulators and cost estimates read `hardware_assumptions.yaml` | 9 | file merged (#126, #130); loaded by `awen-compiler` and applied opt-in to the noise contract and cost inputs (#134, #135), defaults unchanged; `awen-compiler`, `awen-runtime`, `awen-native` and awen-mlir device literals, in-expression fallbacks and C++ named defaults, and `awen-models`, `awen-physics`, `awen-systems`, `awen-qphotonic`, `awen-lab` `awen-studio` and `awen-ecosystem` Python defaults (236 entries, forms listed in the truth table) inventoried and checked in CI; all Python packages now covered | replace inventoried literals one block at a time, each with a sourced value; runtime and Python readers |
| AWEN-Mini as a fine-tuned open model with model card, domain eval (≥300 Q) | 7 | from-scratch char model only | base model choice (Marc), SFT/KD, eval |
| `Runtime(device="auto").load_model("awen-mini")` | 7 | API exists for the bundled from-scratch char model (`awen-models/awen/runtime.py`; `auto`, `cpu`, `cuda`, `cuda:N`, `photonic-sim`); CPU tests in `awen-models/tests/test_runtime.py` and, since P1-3 (#187, `d65c1be`), `test_runtime_devices.py` (`auto` falls back to CPU with CUDA hidden, explicit `cuda` without CUDA raises, tiny-config checkpoint loads by path and generates); the CUDA test is skipped here (no GPU); on `photonic-sim`, `capability_report(text)` since P1-5 (#189, `dcfdaf0`) reports lowered GEMMs, host fallbacks and deviation from CPU (L1) | CUDA run on Marc's laptop (P1-7; the CPU half runs in the fresh-clone gate via `scripts/phase1_devices.py`, #193); the same API for the fine-tuned AWEN-Mini (P1-1, P1-4) |
| AWEN-Photon curves: bits, energy/MAC, recalibration interval, conversions/token, sim-to-real noise control | 7 | all L1, none a device prediction or a speedup or savings figure: MNIST shot-noise curves; sim-to-real control E-022 (#215: the noise-aware advantage reverses at the sourced point, ENOB 7.61); recalibration budgets E-023 (#219: 0.05 standard, 0.1 noise-aware-100, 0.05 noise-aware-1000, normalized weight units) with time axis E-039 (#282: 0.04 s to 1.778 s, an estimate under the elmas2025 input-fiber phase proxy); conversions per token E-024 (#223); energy terms E-025 to E-035 (0 of 11 terms not sourced since E-035, #266); `scenario_envelope_v1` totals E-036 (#270: `dml_laser` 0.3246 µJ to 26.73 µJ per token); energy per lowered MAC E-037 (#274); AWEN-Mini accuracy axis E-038 (#278: held-out NLL CPU 1.4295, bits8 4.256, bits6 4.603, bits4 7.136 nats/char under the unsourced default contract; `dml_laser` 3.009 pJ to 6.367 pJ per lowered MAC). Per-experiment detail: EXPERIMENTS.md and [docs/history/status-2026-11-09.md](docs/history/status-2026-11-09.md) (section 3) | a verified on-chip phase-shifter drift rate (none found and verified so far); sourced values for the default contract's noise terms |
| OpenXLA PJRT plugin | 8 | StableHLO `dot_general` executor (L1) | plugin |
| torch.compile backend with FX partitioning | 8 | backend exists (C3, #84) and, since P2-1 (#195), a graph capability report (per-GEMM seeds since #197, host fallback MAC count since #199) and `python -m awen.compile_capability` for `awen-mini` with CPU CI tests (L1); an external architecture (tiny random-init `transformers` Llama, `python -m awen.compile_llama`) since P2-4 (#203); the Qwen3 architecture (per-head q/k RMS norm, `--arch qwen3`: 15 GEMMs lowered, 187 host nodes, all counted, E-021) since P2-5 (#211) | same path for a fine-tuned AWEN-Mini; an external open model with real weights (waits on Marc: download, model choice); backend default `seed_policy` (per-GEMM seeds are opt-in since #197, default in the AWEN-Mini driver; flipping the default is Marc's decision) |
| ONNX Runtime execution provider | 8 | ONNX import report (L0) | EP |
| Adapters to Meep, SAX, gdsfactory, Perceval, MrMustard | 8, 11 | own reference solvers (L0) | adapters + oracle comparisons |
| Reproduce ≥3 published photonic results | 11 | analytic checks only | 3 reproductions |
| Multiplexing-aware lowering pass, H1-H4 keep/kill report | 10 | pilots on a branch; H1-H4 registered (#133) | full runs, lowering pass, report |
| ≥3 seeds on matched experiments | 14 | 2 seeds (E-003, E-005, E-008; stored results, see EXPERIMENTS.md) | rerun with a third seed |
| docs/install.md with pinned CUDA 12.8+ / cu128 lockfile | 4 | `docs/install.md` and hashed CPU lockfile (#179), fresh-clone install from it in the required gate (#181); hashed CUDA 12.9 lockfile `requirements/cu129-py312.txt` (#183, resolved only, not run; cu129 instead of cu128, see D-022); CPU fallback check for every example, `scripts/examples_cpu.py --run`, run by the fresh-clone gate step (#185: 6 of 6 Python examples pass with `CUDA_VISIBLE_DEVICES` empty; the 5 Rust, C, C++ and JSON examples are covered by named gate steps) | logged laptop run from the cu129 lockfile |
| Session reports in `docs/sessions/` | 3, 17 | one report per session since 2026-10-18 (`ls docs/sessions/`); late filings and the one missing report (2026-10-28-04, slice shipped as #177) are listed in [docs/history/status-2026-11-09.md](docs/history/status-2026-11-09.md) (section 4) | every session |

## Phase 1 plan (one task per session)

| Session | Task | Done when |
| --- | --- | --- |
| P1-1 | Marc picks the AWEN-Mini base model (recommendation: Qwen3-0.6B, Apache-2.0; fallback SmolLM2-360M) | decision logged in DECISIONS.md |
| P1-2 | `docs/install.md` + pinned CPU and CUDA (cu129, D-022) lockfiles; CPU fallback for every example | fresh-clone install test in CI |
| P1-3 | `Runtime(device="auto"\|"cpu"\|"cuda").load_model("awen-mini").generate()` with a tiny CI config | CI test on CPU (done: `awen-models/tests/test_runtime_devices.py`, #187 `d65c1be`) |
| P1-4 | AWEN-Mini v0 fine-tune script, model card, checkpoint SHA-256 (CPU smoke test; laptop GPU run plan for Marc) | smoke test passes; run plan written |
| P1-5 | Same model on `device="photonic-sim"`: linear layers through the GEMM lowering, capability report, deviation from CPU | CI test with tiny config (done: `LoadedModel.capability_report`, `awen-models/tests/test_photonic_capability_report.py`, #189 `dcfdaf0`) |
| P1-6 | Three validation notebooks re-checked under v2 (slab modes, MZI mesh vs Clements, shot-noise bits) | notebooks run in CI (done: evidence levels and source labels in each notebook, photons-per-extra-bit check in (c), `scripts/run_validation_notebooks.py --require-native` in the gate, record `awen-physics/results/notebooks/run-f00120b.json`, #191 `5ae1ab6`) |
| P1-7 | Laptop CUDA run logged by Marc for all three devices | log committed (CPU half done: `scripts/phase1_devices.py` runs `cpu`, `auto`, `photonic-sim` in the fresh-clone gate step and uploads the run records as `awen-phase1-devices-<sha>`, #193 `0b1c3ae`, record `awen-models/results/phase1-devices/run-8d1bff4.json`; laptop command `python scripts/phase1_devices.py --require-cuda --out phase1-devices-cuda.json`, not run) |

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
- Device energies and noise defaults are still hard-coded: 236 inventoried entries across the Rust crates, awen-mlir and
  the Python packages (counts in the truth table), all listed in `awen-spec/hardcoded_device_defaults.yaml`. The compiler
  can read `hardware_assumptions.yaml` opt-in (#135), but the defaults themselves are unchanged. 133 of the 211 compiler
  and runtime entries have no checked value in the assumption set yet, so wiring them needs sourced values first, never
  guesses. Per-package detail: [docs/history/status-2026-11-09.md](docs/history/status-2026-11-09.md) (section 5).

## In progress (draft, not merged)

- Multiplexing/fractal research drafts (exploratory, not on main; full detail in [docs/history/status-2026-11-09.md](docs/history/status-2026-11-09.md), section 6). H1-H4 are
  registered (`research/experiments/multiplexing-fractal/PREREGISTRATION.md`, #133) and their full runs are done on
  drafts: H1 butterfly meshes **killed** (#150, L1); H2 `kron3` and tensor-train **killed** by the conversions rule
  (#153, CPU); H3 Cantor stacks **not kept** (#144, L0; the comparator is a weak single-cavity comb); H4 **kept**
  (#161: FNO beats dense at matched FLOPs on all 3 tasks, CPU). Draft chain: #128, #136/#144 (H3), #137/#148/#150 (H1),
  #153 (H2), #161 (H4), #140 (box-counting). The H2 `kron3` layer is on main in `awen_models.efficiency`. No photonic
  speed or energy claim.
- PACE reference-descriptor defaults: draft #138 is a proposal for Marc only (do not merge).
- Speed program (simulator GEMM, binding copies, KV cache, compile latency): branch
  `perf/hot-paths`; every speedup must be measured with `awen-bench`.

## Next 3 tasks

CI note (2026-11-01, #201): the MLIR install step now has its own 15-min limit and a bounded, retried `apt-get update`, so a stalled mirror fails that step instead of cancelling the whole gate.

1. Phase 2 (needs no decision from Marc): E-039 is run (#282, pass). Next, source a verified on-chip phase-shifter
   drift rate (programmed mesh phases, not input-path phase; read the full text and check Crossref before use), or
   sourced values for the simulator default contract's noise terms (or a registered rule that maps the stored per-MZI
   extinction ratio to `crosstalk_db`), and register its use before any run. Lead not
   yet verified: Chaboyer et al., Optics Express 25(26) 33056 (2017), doi:10.1364/OE.25.033056 (full text still
   not read, publisher captcha again 2026-11-10 06:24 ET, no arXiv or repository copy version; abstract gives a bound, no drift over more than 12 h and short-term stability better than 6 mrad, no
   rate). Checked and not usable: Ceccarelli et al., J. Lightwave Technol. 37(17) 4275 (2019), doi:10.1109/JLT.2019.2923126 (Crossref checked),
   arXiv:2006.08707 (full text read 2026-11-09; heater resistance stability only, no phase drift rate). Sourced so far for the
   default contract: per-heater insertion loss only (#286; `mzi.insertion_loss` and the total path loss stay null) and the detector 3-dB bandwidth (#288, chen2016, typical 67 GHz; default stays 10 GHz); a per-MZI extinction ratio is stored but not mapped (#290, D-043; mapping it to `crosstalk_db` needs a registered composition rule over mesh depth and programmed phases); laser RIN has a measured upper bound only (#292, D-044: below -150 dB/Hz to 6 GHz, equal to the default; no level). Still unsourced: total path loss, phase sigma, thermal noise, crosstalk, ADC bits, QE. The real-weight open-model run waits on Marc (download, model choice). P1-7's CUDA
   log is still Marc's laptop run: `python scripts/phase1_devices.py --require-cuda --out phase1-devices-cuda.json` (not run).
2. Get Marc's answers on D-007 to D-012 (starting with the AWEN-Mini base model, Phase 1 session P1-1, which also
   unblocks P1-4), on the PACE reference-descriptor proposal in draft #138 (DAC 10 to 8 bits, effective bits 8 to 7,
   ADC 4.0 to 6.35 pJ), and on the agent decisions D-018 to D-044 (H2 counting rule, amendments A1 and A2, H4
   definitions, cu129 lockfile, sim-to-real model B, recalibration drift model, conversions counting rule,
   energy accounting rule, DAC published designs, modulator points, host-transfer byte rule, memory-movement byte
   rule, HBM2 memory energy point, harris2014 weight-hold energy, diamantopoulos2021 laser energy-per-bit,
   zortman2012 thermal heater energy-per-bit, sun2016 tuner-logic control energy-per-bit, li2018 host-link PHY
   energy-per-bit, scenario_envelope_v1 total rule, energy per lowered MAC,
   AWEN-Mini accuracy axis, recalibration time axis, phase-shifter insertion loss as a per-heater value, chen2016 detector bandwidth applied opt-in, demarinis2021 extinction ratio stored unmapped, vaskasi2022 RIN bound at the high corner).
3. Get Marc's answer on landing the research stacks on main as research code: H3 (#136, then #144), H1 (#137,
   #148, #150), H2 (#153) and H4 (#161, verdict kept).

Wiring slices 2a (#134) and 2b (#135) are merged; replacing further literals waits on sourced values.
