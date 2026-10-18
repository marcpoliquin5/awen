# Benchmarks and evidence records

`awen-bench` (`awen-models/awen_models/bench.py`) measures CPU, CUDA, the AWEN
software reference, the photonic simulator and a hybrid placement on the same
workloads and writes one **immutable evidence record** per session:
`benchmarks/results/evidence-<date>-<hash12>.json`, schema
`awen-spec/schemas/awen_benchmark_evidence.v1.json`.

```bash
pip install ./awen-native "./awen-models[test]"
awen-bench --runs 5 --repeats 20 --label "my machine" --out-dir benchmarks/results
awen-bench --verify benchmarks/results/evidence-2026-10-18-b65a849f9579.json
```

## What the backends mean

| backend | what runs | fidelity label |
|---|---|---|
| `cpu` | PyTorch float32 on the host CPU | measured |
| `cuda` | PyTorch float32 on a CUDA GPU; recorded as `skipped` with the reason when no GPU is visible | measured |
| `awen-reference` | AWEN Rust GEMM executor, ideal (noise-free) contract | software reference (exact math) |
| `photonic-sim` | AWEN Rust photonic GEMM model, default noise contract (1000 photons/MAC, 8-bit DAC/ADC, 6 dB loss) | simulated (compact model) |
| `hybrid` | AWEN-Mini only: feed-forward GEMMs on `photonic-sim`, attention GEMMs and everything else on the CPU | simulated (compact model) + measured host |

**Wall-clock numbers on `awen-reference`, `photonic-sim` and `hybrid` are the
cost of running the simulator on the host.** They say nothing about the speed
of photonic hardware and must not be quoted as device latency or throughput.
Perplexity and error on those backends are simulated outcomes of the stated
noise contract.

## Metrics and statistics

- GEMM (shapes M×K×N 64×128×128, 128×256×256, 256×512×512): p50/p95/p99
  latency, GFLOP/s at p50, relative RMS error against a float64 oracle,
  working-set bytes, process peak RSS, and for the AWEN paths the Rust
  compile time of an equivalent `awen.tensor.v1` program against the
  `pace_like_128` capability file.
- AWEN-Mini inference: time to first token (prompt `ROMEO:`), decode latency
  per token and tokens/s over 64 new tokens, validation loss and perplexity on
  32 fixed, evenly spaced 128-token validation windows of Tiny Shakespeare.
- `runs` independent runs × `repeats` timed calls after `warmup` calls.
  Percentiles use all samples. 95% CIs are percentile bootstraps (2000
  resamples, fixed seed) of the median over samples and of the mean of run
  medians; perplexity CIs bootstrap the per-window mean loss.

The record stores git SHA and dirty flag, CPU model, core count, memory, load
average, Python/PyTorch/NumPy/awen-native versions, all settings including
both noise contracts, and `evidence_sha256`, the SHA-256 of its own canonical
JSON. The writer opens files in exclusive-create mode, so an existing record
is never overwritten; CI re-verifies every committed record's hash and schema.

## Recorded run: 2026-10-18 06:27 ET, box CPU (Intel Xeon, 8 vCPU, no GPU)

`benchmarks/results/evidence-2026-10-18-b65a849f9579.json`, harness at
`d9ade4c` (clean tree), 5 runs × 20 repeats, 199 s wall. Load average at the
time was about 3.7 on 8 vCPUs (other jobs were running), which widens the CPU
tails. CUDA was skipped: no GPU on this host.

GEMM p50 latency in ms, with the bootstrap 95% CI of the median:

| M×K×N | cpu | awen-reference | photonic-sim | photonic-sim rel. RMS error |
|---|---|---|---|---|
| 64×128×128 | 0.014 [0.012, 6.5] | 1.27 [1.25, 1.31] | 2.24 [2.18, 2.29] | 0.167 |
| 128×256×256 | 0.022 [0.020, 0.022] | 9.96 [9.27, 10.4] | 20.1 [19.7, 20.8] | 0.157 |
| 256×512×512 | 0.121 [0.117, 0.124] | 77.9 [73.5, 81.9] | 135 [133, 137] | 0.127 |

Rust compile time of the GEMM program: p50 0.64–0.80 ms for every shape.
The `awen-reference` error against float64 is below 1e-8 (exact math).

AWEN-Mini (1,016,320 parameters):

| backend | TTFT p50 (ms) | decode tok/s (95% CI) | perplexity (95% CI) |
|---|---|---|---|
| cpu | 1.18 | 479 [217, 741] | 4.31 [3.89, 4.82] |
| awen-reference | 12.9 | 21.4 [21.0, 22.0] | 4.31 [3.89, 4.82] |
| photonic-sim | 38.8 | 9.9 [9.8, 10.1] | 67.6 [61.8, 73.8] |
| hybrid | 27.7 | 14.4 [14.0, 14.8] | 45.2 [42.2, 48.2] |

How to read it: the software reference reproduces the CPU perplexity exactly,
so the AWEN execution path is numerically correct. At the default contract of
1000 photons per MAC the simulated model degrades strongly (perplexity 67.6
vs 4.31), and moving only the feed-forward layers to the simulator (`hybrid`)
halves that loss of quality. This matches the photon sweep in
`awen-models/results/photonic-sim-demo.json` (1000 photons/MAC: 4.24 nats) and
is why noise-aware training (AWEN-Photon) matters.
