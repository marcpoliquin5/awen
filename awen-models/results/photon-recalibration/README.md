# E-023: AWEN-Photon accuracy versus recalibration interval (measured by us, L1)

Registered run of `research/experiments/awen-photon-recalibration/PREREGISTRATION.md` (#217, `bf4c08c`, D-024), run
unchanged.

Command (from `awen-models/`, at commit `82b6adc`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-02 12:29 ET, exit 0, 2 min 59 s wall:

    PYTHONPATH=. python -m awen_models.recalibration --seeds 1 2 3 --out results/photon-recalibration

Output: `results.json` (SHA-256 in `SHA256SUMS`). Printed summary:

    standard tolerable budget 0.05
    noise-aware-100 tolerable budget 0.1
    noise-aware-1000 tolerable budget 0.05

## Verdict (registered rule: per-seed paired drop from s = 0, 95% t upper bound at most 0.01)

Drift budget `s` is the rms random-walk offset per recalibration interval in normalized weight units (one 8-bit weight
step is 2/255 = 0.0078, analytic). All budgets are free parameters: there is no sourced drift rate, so no interval in
seconds or hours follows from these numbers.

| Regime | Tolerable budget | Mean drop / 95% upper bound at s = 0.05 | at s = 0.1 | at s = 0.2 |
| --- | --- | --- | --- | --- |
| standard | **0.05** | 0.0028 / 0.0076 | 0.0146 / 0.0231 (fails) | 0.0771 / 0.1149 (fails) |
| noise-aware-100 | **0.1** | 0.0010 / 0.0033 | 0.0034 / 0.0089 | 0.0032 / 0.0122 (fails) |
| noise-aware-1000 | **0.05** | 0.0000 / 0.0012 | 0.0067 / 0.0153 (fails) | 0.0189 / 0.0336 (fails) |

Budgets 0.01 and 0.02 pass for every regime (full rows in `tolerable_budget` in `results.json`).

## Mean interval accuracy over seeds 1-3 (10,000 MNIST test images, model B primary point; descriptive)

| Regime | s = 0 | 0.01 | 0.02 | 0.05 | 0.1 | 0.2 |
| --- | --- | --- | --- | --- | --- | --- |
| standard | 0.9602 | 0.9601 | 0.9594 | 0.9573 | 0.9456 | 0.8830 |
| noise-aware-100 | 0.8697 | 0.8697 | 0.8697 | 0.8687 | 0.8663 | 0.8665 |
| noise-aware-1000 | 0.9520 | 0.9518 | 0.9518 | 0.9520 | 0.9453 | 0.9331 |

## Exploratory: noise-aware minus standard at each budget (E-022 rule)

- noise-aware-100: reverses at s = 0 to 0.1 (mean -0.0905 to -0.0793); no detectable difference at 0.2 (mean -0.0165,
  95% CI [-0.0670, 0.0340]).
- noise-aware-1000: reverses at s = 0, 0.01, 0.02 (mean about -0.008); no detectable difference at 0.05, 0.1 and 0.2
  (at 0.2: mean 0.0501, 95% CI [-0.0007, 0.1008]).

So noise-aware-100 tolerates one grid step more drift than the standard network, but at every budget up to 0.1 it is
less accurate in absolute terms. No regime is more accurate than the standard network at any budget in this grid with
a detectable difference. Flatter is not better here.

## Reproducibility note

The no-drift point is recomputed, as registered. It matches the stored E-022 model-B primary accuracies for 8 of the
9 runs exactly; standard seed 1 gives 0.9575 here and 0.9572 in E-022 (3 of 10,000 images). Same seeds and code;
the Python version differs (3.13.5 here, 3.12.15 recorded for E-022 in its `env.txt`), and multithreaded CPU training
is not bit-reproducible, so this is run-to-run or environment variation, not a design change. It does not change any
E-022 verdict, whose intervals are far from zero.

Limits: one small MLP on MNIST, 3 epochs, CPU, simulated noise and drift only (L1). Independent Gaussian random-walk
drift of programmed weights only; no correlated drift, input-path or detector drift, temperature dependence or
calibration error. No energy, speed, hardware or recalibration-time claim follows from these numbers.
