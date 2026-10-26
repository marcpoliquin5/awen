# H2 registered full run

Verdict: **killed** for both candidates (`kron3` and `tensor-train`) under the registered conversions rule
(`../../configs/h2.yaml`, `../../PREREGISTRATION.md`, counting rule D-018, amendments A1 D-019 and A2 D-020).
Neither candidate is distinguishable from the dense baseline on validation loss. Evidence level L0 (CPU
training) plus an analytic conversion count; no device, energy or speedup claim. Source label for every
number below: **measured by us**, stored in `h2_registered.json` (SHA-256 in `SHA256SUMS`), except the
conversion counts, which are **analytic** (formula in PREREGISTRATION.md, H2 counting rule).

Command (commit `3170440`; environment in `env.txt`: build box Linux 6.12.94+ x86_64, 8 vCPU, no GPU,
Python 3.13.5, torch 2.14.1+cpu, numpy 2.5.4, 4 torch threads):

```
PYTHONPATH=awen-models python research/experiments/multiplexing-fractal/h2_structured_weights.py \
    --config research/experiments/multiplexing-fractal/configs/h2.yaml \
    --out research/experiments/multiplexing-fractal/results/h2
```

Started 2026-10-26 06:26:43 ET, `EXIT 0` at 07:02:11 ET (about 35 min). Run once; not repeated. Task: Tiny
Shakespeare character LM (harness `awen_models.efficiency`, checksum-verified download), 1500 steps, batch 32,
seeds 101, 102, 103.

## Results (means over 3 seeds; per-seed values in the JSON and `run.log`)

| Variant | Stored params | Val loss (nats/char) | Candidate - baseline, 95% paired-t CI | Conversions per useful MAC (primary) | Lower bound (report only) | Batch-1 latency, verdict path (ms) |
| --- | --- | --- | --- | --- | --- | --- |
| baseline (dense) | 818,048 | 1.71949 | n/a | 0.009766 | 0.009766 | 1.281 (native) |
| kron3 (A1 shape, A2 paths) | 820,032 | 1.71136 | -0.00813 [-0.02980, 0.01354] | 1.045898 | 0.008301 | 77.209 (contraction) |
| tensor-train | 818,048 | 1.70879 | -0.01070 [-0.07210, 0.05071] | 0.747070 | 0.008789 | 2.410 (native) |

`kron3` batch-1 latency on the `dense_weight` path (same trained model, report only, not used by the verdict):
4.256 ms mean, 3.32x baseline.

## Rules

| Candidate | Accuracy rule | Conversions rule | Latency rule (v2 section 7) | Verdict |
| --- | --- | --- | --- | --- |
| kron3 | passed (CI crosses 0) | killed (1.0459 >= 0.0098) | passed: not applied, candidate does not win on parameters (ratio 60.3x) | **killed** |
| tensor-train | passed (CI crosses 0) | killed (0.7471 >= 0.0098) | passed: not applied, equal parameters (ratio 1.88x) | **killed** |

## What this does and does not show

- At matched stored parameters (within 0.24%), neither factorized FFN loses accuracy against dense on this task
  at 3 seeds; neither shows a gain either (both CIs cross 0).
- Under the registered primary count (one DAC per input and one ADC per output for each rank term), rank-126
  `kron3` needs about 107x and rank-`R` tensor-train about 76x the dense conversions per useful MAC. This was
  stated in amendment A1 before the run for `kron3`.
- The report-only lower bound (optical fan-out of the input and photocurrent summation across rank terms) is
  below dense for both (0.0083 and 0.0088 vs 0.0098), but the registered mapping does not describe that
  hardware, so it does not change the verdict. Whether such a mapping is physically realizable at these channel
  counts is an open question, not a result.
- The latency rule only applies when a candidate wins on parameters; neither does, so it did not fire even though
  both are slower on CPU (60.3x and 1.88x). CPU latency says nothing about photonic latency.
- `kron3` training wall times varied (316 to 528 s per seed) with other load on the build box; wall time is not a
  registered metric.
- One task, one model size, 3 seeds; not tested at other sizes or on other tasks.
