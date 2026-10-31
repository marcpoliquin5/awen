# AWEN status

Updated 2026-10-31 06:50 ET (time of this commit). Governing brief: AWEN Master Prompt v2 (Marc, 2026-10-18).
This commit is based on main `ea3a9e0` (#195, PR gate run 41193265627 passed on head `ded7c94`), after `cdad05c` (#194, gate run 41171087531 passed), after `0b1c3ae` (#193, PR gate run 41168894778 passed on head `18a0db5`), after `5da183c` (#192, gate run 41142393036 passed), after `5ae1ab6` (#191, PR gate run 41138332030 passed on head `96fcc29`), after `c9aa78d` (#190, gate run 41097481004 passed), after `dcfdaf0` (#189, PR gate run 41091038534 passed on head `7f119d9`), after `3241052` (#188, gate run 41043245527 passed), after `d65c1be` (#187, PR gate run 41040938526 passed on head `07fdb0c`), after `0c7f757` (#186, gate run 41006898097 passed), after `798d3fa` (#185, PR gate run 41003390165 passed on head `acf311a`), after `6432479` (#184, gate run 40969775066 passed), after `bc3fd6c` (#183, PR gate run 40965792269 passed on head `2aef40f`), after `2672556` (#182, gate run 40917749158 passed), after `69c2a20` (#181, PR gate run 40912079353 passed on head `4669265`), after `698d1c2` (#180, gate run 40861558068 passed), after `0d41214` (#179, PR gate run 40858468897 passed on head `a2dd49e`), after `3a5c3bc` (#178, gate run 40819858857 passed), after `2b78fed` (#177, PR gate run 40817654678 passed on head `bff99e1`), after `332d4a7` (#176, gate run 40734046127 passed), after `5dde85c` (#175, PR gate run 40726725442 passed on head `e51ac97`), after `2fa522e` (#174, gate run 40674705701 passed), after `a7fd20e` (#173, PR gate run 40671584588 passed on head `a82b8a1`), after `04085c2` (#172, gate run 40634872813 passed), after `5dd19c4` (#171, PR gate run 40632619182 passed on head `e1f1dc5`), after `f3dfa08` (#170, gate run 40598529173 passed), after `9902d84` (#169, PR gate run 40595490211 passed on head `0a685a7`), after `41e224b` (#168, gate run 40544601082 passed), after `09beddb` (#167, gate run 40490344117 passed), after `213ce2b` (#166, gate run 40452031650 passed), after `3a562c9` (#165, PR gate run 40450135645 passed on head `c270358`), after `09390a2` (#164, gate run 40415390257 passed), `917f656` (#163, PR gate run 40412651063 passed on head `e04a9e7`), after `52e5821` (#162, gate run 40363060478 passed), `00ef6b7` (#160), `acb3298` (#159, gate run 40271853808 passed), `42f03e3` (#158, PR gate run 40269929547 passed on head `080fe5e`), `abd7abd` (#157, gate run 40242575808 passed), `7267774` (#156, gate run 40213918425 passed), `cf60185` (#155, PR gate run 40212149057 passed on head `a563254`), `17e3cad` (#154, gate run 40185998015 passed), `46f6394` (#152, gate run 40162396875 passed), `73e9780` (#151, gate run 40140095232 passed), `c294212` (#149, gate run 40112995081 passed), `122f1b5` (#147, gate run 40086556965 passed), `dac9b90` (#146, gate run 40063420420 passed), `e90414a` (#145, gate run 40034904339 passed), `c468e28` (#143, gate run 39988618058 passed), `87c3ee2` (#142), `14385b9` (#139), `c59586b` (#141), `950a859` (#135), `243acef` (#134), `01b927f` (#133),
`c1086fc` (#132), `c02eee9` (#131), `132d4d9` (#130), `75d58f7` (#129), `ee2e8cf` (#127), `840f5c5` (#126) and `0235668` (#125).
Truth-table test counts were measured at `0235668`; the inventory counts at `14385b9`. Gates passed on `0235668` (run 39140149777),
`840f5c5` (run 39141006838), `132d4d9` (run 39143781587), `c02eee9` (run 39147442569), `c1086fc` (run 39150273629),
`01b927f` (run 39151136678), `243acef` (run 39868419379) and `950a859` (run 39870091723); PR gates passed for #141
(run 39903934485, head `5179d23`) and #139 (run 39938827763, head `7d2b985`). The main gates on `c59586b` (run 39938808307)
and `14385b9` (run 39939861563) were cancelled when newer pushes superseded them; the main gate on `87c3ee2`
(run 39941103445) passed.

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
| PyTorch PrivateUse1 "awen" device (eager GEMM) | `awen-ecosystem` | L1 | tests; `torch.compile(backend=awen)` (C3, #84) lowers `linear`/`matmul` to the Rust photonic GEMM; graph capability report for `awen-mini` since P2-1 (#195, `ea3a9e0`): 21 GEMMs lowered, 107 host fallbacks, E-017 |
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
| Hardware assumptions file | `awen-spec/hardware_assumptions.yaml` | L0 (spec data) | v0 merged #126 (`840f5c5`); section 9 follow-up merged #130 (`132d4d9`); loader `awen_compiler::assumptions` merged #134 (`243acef`); opt-in `with_assumptions` for the noise contract and cost inputs, with before/after outputs in `awen-compiler/results/assumptions-wiring/`, merged #135 (`950a859`) |
| Inventory of hard-coded device defaults, checked in CI | `awen-spec/hardcoded_device_defaults.yaml`, `scripts/hardcoded_defaults.py` | L0 (spec data) | non-test `awen-compiler/src`, `awen-runtime/src` and `awen-native/src` Rust code, two literal forms: single-line `field: <number>,` device literals whose field name matches the device-field pattern (energy, power, noise, loss, crosstalk, precision, calibration, temperature, and since #139 also latency, voltage, fidelity, dark count/current, saturation, hysteresis, coherence time; 107 compiler + 40 runtime) and float fallbacks, floors and caps inside expressions (`map_or`/`unwrap_or`/`max`/`min`/`clamp`, 38 compiler + 26 runtime; `awen-native` has none), plus, since 2026-10-26, awen-mlir C++ (`include`, `lib`, `tools`) file-scope named defaults `constexpr ... kDefault<Name> = <number>;` (2: `kDefaultEffectiveBits` 8, `kDefaultTile` 128 in `Passes.cpp`), plus, since 2026-10-27 (#165), the `awen-models` Python packages `awen` and `awen_models` (module constants, class-body fields and parameter defaults with a literal number and a device name, plus `wavelength` and `tile_*`; 8: `tile_k` 64 seven times and `DEFAULT_WAVELENGTH_M` 1550e-9), plus, since #169, the `awen-physics` and `awen-systems` Python packages with the same forms (5, all in `awen_systems`; `awen_physics` has none), plus, since #171, `awen-qphotonic` (3: two zero dark-count defaults and the synthetic-dataset `noise` 0.15 in `kernel.run_experiment`), plus, since #173, `awen-lab` (3: the `OperatingBounds` request limits `min_photons_per_mac` 1.0, `max_photons_per_mac` 1e9 and `max_photons_quantum` 4, policy bounds rather than device values), plus, since #175, `awen-studio` (1: `CompareRequest.tile_k` 64, read from a pydantic `Field` default, a form the scanner now reads in class bodies along with dataclass `field(default=...)`), plus, since #177, `awen-ecosystem/python_awen/awen_py` (3: `tile_k` 64 in `device.configure` and `photonic_tile_k` 64 in `ExecutionOptions` and `TorchBackendOptions`; the Python tile pattern now also matches prefixed names like `photonic_tile_k`); 236 entries (compiler 145 in 36 functions, runtime 66 in 28 functions, awen-mlir 2, awen-models 8, awen-systems 5, awen-qphotonic 3, awen-lab 3, awen-studio 1, awen-ecosystem 3; 156 unsourced, 27 numeric guards, 48 definitional, 4 differ from the assumption set, 1 matches). Not covered: `.clamp` upper bounds (only the lower bound is recorded), integer guards, `fold` initial values, arithmetic factors (e.g. `* 0.01`), fields outside the device-field name pattern (e.g. `coupling_q`, `read_efficiency`, `t1_ns`, `photon_count`, `click_probability`), lines with a trailing comment (e.g. `read_latency_ns: 1000, // ...` in `awen-runtime/src/state/memory.rs`), tuple values (e.g. `nominal_voltage_range: (0.0, 10.0)` in `hal_v0.rs`), test modules (e.g. `photons_per_mac: Some(10.0)` in an `executable.rs` test), C++ validation bounds and constants not named `kDefault*`, awen-mlir lit tests, PyO3 signature defaults (e.g. `signal = 1.0` and `tile_k = 64` in `awen-native`), Python sampling `temperature`, nested functions, numbers inside Python expressions, `awen-models/examples` and tests, the `awen_systems/memory.py` technology table (`P(...)` calls with their own source labels), the illustrative numbers in `fabric.study()`, zero defaults outside the pattern (`loss_db`, `reconfig_delay_s`), the `awen_qphotonic` `ExperimentSpec` efficiencies (1.0) and `trigger_rate_hz` 1e6, the other `awen_lab` `OperatingBounds` limits (`max_matrix_elements`, `max_inner_dim`, `max_seeds`, `max_modes`, `max_shots`) and its `tile_k` fallback `exp.get("tile_k", 64)`; the `awen-studio` request limits not matching the pattern (`MAX_GEMM_DIM` 512, `MAX_NEW_TOKENS` 400, sampling `temperature` 0.8) and its TypeScript frontend; in `awen-ecosystem` the `_STATE` dict value, the 1e-5 tolerances, the 1e-4 finite-difference epsilon, a positional 0.0 offset, and the `python_awen/examples` scripts and package tests; `python scripts/hardcoded_defaults.py --check` |

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
| AWEN-Photon curves: bits, energy/MAC, recalibration interval, conversions/token, sim-to-real noise control | 7 | MNIST shot-noise curves only (L1) | four curves + independent noise model |
| OpenXLA PJRT plugin | 8 | StableHLO `dot_general` executor (L1) | plugin |
| torch.compile backend with FX partitioning | 8 | backend exists (C3, #84) and, since P2-1 (#195), a graph capability report and `python -m awen.compile_capability` for `awen-mini` with CPU CI tests (L1) | independent per-GEMM noise seeds (v1 reuses one seed per call); host fallback MAC count; same path for a fine-tuned AWEN-Mini |
| ONNX Runtime execution provider | 8 | ONNX import report (L0) | EP |
| Adapters to Meep, SAX, gdsfactory, Perceval, MrMustard | 8, 11 | own reference solvers (L0) | adapters + oracle comparisons |
| Reproduce ≥3 published photonic results | 11 | analytic checks only | 3 reproductions |
| Multiplexing-aware lowering pass, H1-H4 keep/kill report | 10 | pilots on a branch; H1-H4 registered (#133) | full runs, lowering pass, report |
| ≥3 seeds on matched experiments | 14 | 2 seeds (E-003, E-005, E-008; stored results, see EXPERIMENTS.md) | rerun with a third seed |
| docs/install.md with pinned CUDA 12.8+ / cu128 lockfile | 4 | `docs/install.md` and hashed CPU lockfile (#179), fresh-clone install from it in the required gate (#181); hashed CUDA 12.9 lockfile `requirements/cu129-py312.txt` (#183, resolved only, not run; cu129 instead of cu128, see D-022); CPU fallback check for every example, `scripts/examples_cpu.py --run`, run by the fresh-clone gate step (#185: 6 of 6 Python examples pass with `CUDA_VISIBLE_DEVICES` empty; the 5 Rust, C, C++ and JSON examples are covered by named gate steps) | logged laptop run from the cu129 lockfile |
| Session reports in `docs/sessions/` | 3, 17 | `2026-10-18-01.md` to `-03.md` (`-03` filed late), `2026-10-23-01.md` (covers the Oct 22 and Oct 23 runs), `2026-10-23-02.md`, `2026-10-23-03.md`, `2026-10-24-01.md`, `2026-10-24-02.md`, `2026-10-24-03.md`, `2026-10-24-04.md`, `2026-10-25-01.md`, `2026-10-25-02.md`, `2026-10-25-03.md`, `2026-10-25-04.md`, `2026-10-26-01.md` to `-04.md`, `2026-10-27-01.md` to `-04.md`, `2026-10-28-01.md` to `-03.md` (no `-04`: that run never opened its PR; its slice shipped as #177), `2026-10-29-01.md` to `-04.md`, `2026-10-30-01.md` to `-04.md`, `2026-10-31-01.md`, `2026-10-31-02.md` | every session |

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
- Device energies and noise defaults are still hard-coded in `awen-compiler` (145 entries) and
  `awen-runtime` (66 entries, mostly `hal_v0.rs` defaults, quantum/state simulator constants and reference-simulator fallbacks), all listed in
  `awen-spec/hardcoded_device_defaults.yaml`. The compiler can read `hardware_assumptions.yaml` opt-in (#135), but the
  defaults themselves are unchanged. 138 of those 211 have no checked value in the assumption set yet, so wiring them needs
  sourced values first, never guesses. awen-mlir hard-codes 2 more (`kDefaultEffectiveBits` 8, `kDefaultTile` 128 in
  `Passes.cpp`, #163; the tile size is unsourced, the 8 bits differ from `system.pace_enob`); `awen-native` has none. `awen-models` hard-codes 8 more (#165): `tile_k` 64 in 7 places (same as the awen-native
  PyO3 default, different from awen-mlir's 128) and `DEFAULT_WAVELENGTH_M` 1550e-9, all unsourced. `awen-physics` and `awen-systems` add 5 (#169): in `awen_systems/fabric.py` the `Link` fields `latency_s` and
  `energy_j_per_bit` 0.0 and `FabricConfig.static_power_w` 0.0 (disabled terms, definitional) and `FabricConfig.tx_power_dbm` 0.0 dBm
  (unsourced), and `MemoryStats.energy_j` 0.0 (accumulator, definitional); `awen_physics` has none in the scanned forms. `awen-qphotonic`
  adds 3 (#171): `dark_count` 0.0 in `threshold_detection` and `ExperimentSpec.dark_count_per_gate` 0.0 (ideal detector,
  definitional) and `run_experiment` `noise` 0.15 (jitter on the synthetic dataset features, not a device value; unsourced).
  `awen-lab` adds 3 (#173): the `OperatingBounds` request limits `min_photons_per_mac` 1.0, `max_photons_per_mac` 1e9
  and `max_photons_quantum` 4 (policy bounds on accepted simulation requests, not device values; unsourced).
  `awen-studio` adds 1 (#175): `CompareRequest.tile_k` 64 in `app.py` (passed to `awen_native.photonic_gemm`; unsourced, same as
  the awen-native and awen-models default, different from awen-mlir's 128).
  `awen-ecosystem` adds 3 (#177): `device.configure` `tile_k` 64, `ExecutionOptions.photonic_tile_k` 64 and
  `TorchBackendOptions.photonic_tile_k` 64 (all passed down to `awen_native.photonic_gemm`; unsourced). Every Python package is now
  inventoried; example scripts and tests stay out of scope.

## In progress (draft, not merged)

- Multiplexing/fractal pilots (H1 butterfly vs Clements meshes, H2 Kronecker/Monarch
  layers, H3 Cantor/Fibonacci stacks): draft #128, branch `research/fractal-track`, exploratory only; split
  into per-hypothesis drafts #137 (H1), #136 (H3) and #140 (box-counting), H2 still in #128. The H2 `kron3`
  layer is on main in `awen_models.efficiency` (2026-10-23, no results). The H3 `--config` entry point (Cantor side:
  config checks, seeded thickness perturbation, channel metrics) is draft #144, stacked on #136. Since 2026-10-24
  #144 also has the equal-footprint uniform-grid comparator (Fabry-Perot comb, `2c291a9`) and per-cell comparison.
  On 2026-10-24 the cell-to-verdict rule `h3-cells-v1` was registered (`99ced09`) and then the registered run was done
  (`5902799`, stored with SHA-256 in `results/h3/` on #144): **H3 not kept** (9 comparable cells: 7 Cantor better on
  both, 2 mixed at sigma 0.02, 0 worse on both; L0, measured by us). The comparator is a weak single-cavity comb, not an
  optimized WDM filter bank, so this is not evidence against practical uniform-grid filters.
  The H1 `--config` entry point is draft #148 (`b1353fe`, stacked on #137; main merged into #137 as `4cc749d`):
  config checks, loss-budget depth selection, a two-coupler fabrication model (phase error and splitter imbalance),
  the Haar and DFT tasks, butterfly and Clements meshes, and the paired-t analysis. Draft #150 (`587c873`, stacked
  on #148) adds the low-rank SVD mesh baseline (ranks 2, 4, 8, own component and worst-path count) and the
  MNIST-PCA16 task (one photonic layer plus detection, N = 16 only), compares butterfly with both baselines and
  applies the registered kill rule; their definitions were fixed in PREREGISTRATION.md on 2026-10-24 before any
  H1 full run. On 2026-10-25 the first full-run launch stopped before any score (registered rank 8 at N = 8 is full
  rank); `b3f94ff` on #150 excludes ranks >= N at that N and reports them (primary point N = 16 keeps all ranks;
  deviation logged in PREREGISTRATION.md). The relaunched full run at `b3f94ff` finished at 12:49 ET on 2026-10-25 (exit 0, 44,799 s wall, about
  12.4 h against a 1-3 h estimate). Results are committed to draft #150 as `13a4cc0` (`results/h1/` with SHA-256,
  `RESULT.md`): **H1 killed** under the registered rule (L1, measured by us). At N = 16 and a 6 dB budget, butterfly
  beats the rank-4 low-rank mesh on all three tasks, loses to Clements on Haar unitaries, is not distinguishable
  from Clements on MNIST-PCA16 (95% CI crosses 0) and beats Clements on the DFT matrix. Loss values are free
  parameters, not device data, and only the registered grid point was run.
  The H2 `--config` entry point is draft #153 (`d360467`, based on main): config checks, the registered variants, an
  analytic conversions-per-useful-MAC counter with mapping legality, the paired-t verdict and latency. The counting rule
  (`R * (d_in + d_out)` conversions per layer against `d_in * d_out` useful MACs) was fixed in PREREGISTRATION.md
  and D-018 before any full run. Two findings block the H2 full run. First, the registered `kron3` shape puts 4 -> 16
  and 16 -> 4 factors on polarization, which is illegal under the 2 x 2 mapping. Second, the registered batch size 32
  ran out of memory for `kron3` on the 16 GB build box (measured by us; OOM kill at about 13.7 GB). The memory
  finding is addressed on main by #155 (`cf60185`): opt-in `rank_chunk` on `Kron3Linear` (checkpointed rank chunks,
  default unchanged, forward and gradient equivalence tested). One batch-32 training step peaked at 1365.9 MB RSS
  with `rank_chunk` 17 (measured by us, `awen-models/results/kron3-chunk/`). On 2026-10-25 (session -04) amendment A1 was registered on #153 (`c5eb38f`, D-019) before any H2 run (D-019 is on #153, not on main yet):
  `kron3` becomes `(8, 2, 8) -> (32, 2, 32)`, rank 126, `rank_chunk` 18 (polarization 2 x 2, legal; stored parameters
  820,032 vs 818,048 baseline, +0.24%). The harness default build is unchanged. A 20-step probe at batch 32 peaked at
  1250.0 MB RSS but took 642.0 s of training (about 32 s per step; measured by us, one run), so the registered `kron3`
  arm would need roughly 40 h on this box (estimate: 4500 steps x 32 s). On 2026-10-26 #158 (`42f03e3`) added an opt-in
  `dense_weight` path to `Kron3Linear` (dense-equivalent weight built once per forward, one matrix product; forward
  and gradient equivalence tested, default unchanged). At the A1 shape, batch 32, 4 threads, the median training step
  went from 32.8607 s (`rank_chunk` 18) to 0.2565 s, peak RSS 1239.7 MB to 729.1 MB, same loss after 6 steps
  (measured by us, one run, `awen-models/results/kron3-dense/`), so the `kron3` arm is estimated at about 20 min of
  training steps (4500 x 0.26 s, estimate). On 2026-10-26 amendment A2 was registered on #153 (`3170440`, D-020)
  before any H2 full run: `kron3` trains on `dense_weight`; the batch-1 latency rule uses the contraction path,
  `dense_weight` latency is report-only. The H2 registered full run at `3170440` finished 2026-10-26 07:02 ET
  (EXIT 0, about 35 min; results `58bc548` on draft #153, `results/h2/RESULT.md`): **killed** for both candidates.
  Val loss (3 seeds, measured by us): baseline 1.71949, `kron3` 1.71136 (diff 95% CI [-0.02980, 0.01354]),
  tensor-train 1.70879 (CI [-0.07210, 0.05071]), so the accuracy rule passed for both; conversions per useful MAC
  (analytic, D-018): baseline 0.009766, `kron3` 1.045898, tensor-train 0.747070, so the conversions rule killed both;
  the latency rule did not apply (neither wins on parameters). CPU only; no photonic latency or energy claim.
  H1-H4 are registered (`research/experiments/multiplexing-fractal/PREREGISTRATION.md`, #133); the H3 (on #144), H1 (on #150) and H2 (on #153) full runs are done.
  H4 `--config` entry point opened 2026-10-26 as draft #161 (`e9cf511`): definitions fixed before any run
  (PREREGISTRATION.md "H4 definitions", D-021), widths matched to dense FLOPs within +0.25% to +3.93% (analytic,
  `--flops-only`). The H4 full run at `e9cf511` (launched 2026-10-26 12:38 ET in a detached worktree) finished 2026-10-27 01:00 ET
  (EXIT 0, about 12 h 22 min against the ~1.5 h estimate; cause not measured). Results `ae3956f` on draft #161
  (`results/h4/RESULT.md`): **kept**. FNO beats dense at matched FLOPs on all 3 tasks (improvement 95% CIs, measured by us:
  Burgers rel. L2 0.1214 [0.1032, 0.1396], denoise 0.1367 [0.1253, 0.1482], MNIST accuracy 0.0044 [0.0013, 0.0076]);
  Haar beats dense only on denoise (0.1129 [0.1089, 0.1169]) and is far worse on MNIST (accuracy 0.4126 vs 0.9827),
  so a defect in the Haar layer's 2D use was possible. On 2026-10-27 `9b30f08` on #161 checked `haar` and `haar_bands`
  against an independent NumPy reference (11 tests, all pass, measured by us): forward, inverse, gradient, energy and
  band labels are right at the registered shapes, so the gap is not a transform defect. Untested hypothesis: the layer's
  one mixing matrix per band is shared by every position in that band. The verdict stays as run. CPU only; no photonic speed or energy claim.
- PACE reference-descriptor defaults: draft #138 is a proposal for Marc only (do not merge).
- Speed program (simulator GEMM, binding copies, KV cache, compile latency): branch
  `perf/hot-paths`; every speedup must be measured with `awen-bench`.

## Next 3 tasks

1. Phase 2 (needs no decision from Marc): give each GEMM lowered by `torch.compile(backend=awen)` its own noise seed
   (derived deterministically from the call seed and node index), so that layer noise is independent. Today torch-backend
   v1 passes one seed to every GEMM of a call (`seed_note` in the P2-1 report). This changes default-contract outputs, so
   the report version and E-017 records must be regenerated. P2-1 is done (#195, `ea3a9e0`):
   `python -m awen.compile_capability` compiles `awen-mini` with `fullgraph=True`, lowers 21 GEMMs (20 `AwenLinear` plus the
   tied head) and lists 107 host fallback nodes with reasons (L1, E-017). P1-7's CUDA log is still Marc's laptop run:
   `python scripts/phase1_devices.py --require-cuda --out phase1-devices-cuda.json` (not run).
2. Get Marc's answers on D-007 to D-012 (starting with the AWEN-Mini base model, Phase 1 session P1-1, which also
   unblocks P1-4), on the PACE reference-descriptor proposal in draft #138 (DAC 10 to 8 bits, effective bits 8 to 7,
   ADC 4.0 to 6.35 pJ), and on the agent decisions D-018 to D-022 (H2 counting rule, amendments A1 and A2, H4
   definitions, cu129 lockfile).
3. Get Marc's answer on landing the research stacks on main as research code: H3 (#136, then #144), H1 (#137,
   #148, #150), H2 (#153) and H4 (#161, verdict kept).

Wiring slices 2a (#134) and 2b (#135) are merged; replacing further literals waits on sourced values.
