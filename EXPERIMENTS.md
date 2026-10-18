# AWEN experiments

Index of every experiment run so far. Status: **done** (result committed), **pilot**
(run before pre-registration; not evidence for a hypothesis), **registered** (configs committed, full runs not started), **planned**. Levels and
source labels follow Master Prompt v2 section 2. All runs so far were on the build box:
Intel Xeon 8 vCPU, 15 GiB, no GPU (measured on CPU unless labeled simulated).

| ID | Hypothesis / question | Config / command | Seeds | Status | Result |
| --- | --- | --- | --- | --- | --- |
| E-001 | A ~1M-param char model trains to a usable loss on CPU | `python -m awen_models.train` (settings in checkpoint metadata) | 1 | done | [checkpoint](awen-models/awen_models/checkpoints/awen-mini/), val 1.4795 nats |
| E-002 | int8 weights keep AWEN-Mini quality | `python -m awen_models.evaluate` | 1 | done | [awen-mini-eval.json](awen-models/results/awen-mini-eval.json) |
| E-003 | Nonlinear FFN variants beat a GELU baseline at matched params | `python -m awen_models.nonlinear` | 2 (v2 needs ≥3) | done | [nonlinear/results.json](awen-models/results/nonlinear/results.json) |
| E-004 | AWEN-Mini on photonic-sim matches CPU under the ideal contract | `python -m awen_models.photonic_demo` | 1 | done (L1) | [photonic-sim-demo.json](awen-models/results/photonic-sim-demo.json) |
| E-005 | Noise-aware training keeps accuracy under shot noise (MNIST MLP) | `python -m awen_models.photon_training` | 2 (v2 needs ≥3) | done (L1) | [photon-training/results.json](awen-models/results/photon-training/results.json) |
| E-006 | LoRA lowers SmolLM2-135M val loss on CPU | `python -m awen_models.finetune lora` | 1 | done | [lora-SmolLM2-135M.json](awen-models/results/finetune/lora-SmolLM2-135M.json) |
| E-007 | KD from SmolLM2-135M beats CE-only for a 7.1M student | `python -m awen_models.finetune distill` | 2 | done | [seed0](awen-models/results/finetune/distill-SmolLM2-135M-seed0.json), [seed1](awen-models/results/finetune/distill-SmolLM2-135M-seed1.json) |
| E-008 | Efficiency variants (MoE, TT, QAT, sparse, low-rank, recursive) at matched params | `python -m awen_models.efficiency --steps 1500 --seeds 1 2` | 2 (v2 needs ≥3) | done | [efficiency/results.json](awen-models/results/efficiency/results.json) |
| E-009 | Photonic quantum kernel vs classical RBF kernel | `awen-qphotonic` kernel study | 3 | done (L1, no advantage) | [kernel_vs_rbf.json](awen-qphotonic/results/kernel_vs_rbf.json) |
| E-010 | Optical vs electrical fabric on identical workloads | `awen-systems` fabric study | 1 | done (modeled) | [fabric_study.json](awen-systems/results/fabric_study.json) |
| E-011 | KV-cache placement across memory technologies | `awen-systems` KV-cache study | 1 | done (modeled) | [kvcache_study.json](awen-systems/results/kvcache_study.json) |
| E-012 | Backend benchmark (CPU, AWEN reference, photonic-sim, hybrid) | `python -m awen_models.bench --runs 5` | 5 runs | done | [evidence record](benchmarks/results/evidence-2026-10-18-b65a849f9579.json) |
| E-013 | Which SmolLM2-135M layers AWEN can lower | `awen_models.hf_adapter` report | n/a | done | [capability report](awen-models/results/smollm2-135m-capability-report.txt) |
| P-001 | H1 pilot: butterfly vs Clements expressivity vs depth (N = 8, 16) | branch `research/fractal-track` | 1 | pilot | draft #128 @ `0974f38`: `meshes_n8.json`, `meshes_n16.json` |
| P-002 | H3 pilot: Fibonacci/Cantor stacks (trace map, quarter-wave 6-cycle, scaling scan) | branch `research/fractal-track` | n/a | pilot | draft #128 @ `0974f38`: `stacks.json` |
| P-003 | H2 pilot: Monarch / 3-factor Kronecker FFNs vs dense at matched params | branch `research/fractal-track` | 2 | pilot (baseline, monarch, kron3, recursive stored; recursive-monarch not run to completion) | draft #128 @ `0974f38`: `h2_pilot_runs.jsonl` |
| H1-H4 | Multiplexing/fractal hypotheses (v2 section 10) | [`configs/h1.yaml`-`h4.yaml`](research/experiments/multiplexing-fractal/configs/) | H1: 5 init x 5 fab; H2-H4: 3 (all from 101, disjoint from pilot seeds) | registered (full runs not started) | [PREREGISTRATION.md](research/experiments/multiplexing-fractal/PREREGISTRATION.md) |

Gaps against v2: E-003, E-005 and E-008 have 2 seeds, not 3; E-005 does not yet evaluate under
an independently parameterized noise model (sim-to-real control); `awen-spec/hardware_assumptions.yaml`
exists (merged in #126 and #130) but no experiment reads it yet; the device literals the compiler
still hard-codes instead are listed in `awen-spec/hardcoded_device_defaults.yaml` (133 entries in `awen-compiler`: 95 field literals and 38 in-expression fallbacks, checked in CI;
`awen-runtime` and the Python packages not yet).
