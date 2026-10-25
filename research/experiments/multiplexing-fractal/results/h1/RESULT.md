# H1 registered full run

Verdict: **killed** under the registered kill rule (`../../configs/h1.yaml`, `../../PREREGISTRATION.md`): butterfly
does not beat both baselines on `haar_unitary` and `mnist_pca16_linear`. Evidence level L1 (float64 simulation);
loss is bookkeeping with free parameters (no checked insertion-loss values in `hardware_assumptions.yaml` v0), not
a device prediction. Source label for every number below: **measured by us**, stored in
`h1_meshes_registered.json` (SHA-256 in `SHA256SUMS`).

Command (commit `b3f94ff`; environment in `env.txt`: build box Linux 6.12.94+ x86_64, 8 vCPU, no GPU,
Python 3.13.5, torch 2.14.1+cpu, numpy 2.5.4):

```
python research/experiments/multiplexing-fractal/h1_meshes.py --config research/experiments/multiplexing-fractal/configs/h1.yaml
```

`run.log`: started 2026-10-25 00:23:14 ET, `EXIT 0 wall_s 44799` (12.4 h, against a 1-3 h estimate), ended
12:49:53 ET. Run once; not repeated.

## Primary point

N = 16 modes, budget 6.0 dB, 0.25 dB per MZI, 0.05 dB per crossing, phase sigma 0.02 rad, splitter imbalance 0.02.
Deepest configuration within budget: butterfly 3 x log2(N) stages (96 MZIs, 5.4 dB), Clements depth 16 (120 MZIs,
4.0 dB), low-rank SVD rank 4 (112 MZIs, 4.0 dB; rank 8 is 8.0 dB, over budget).

Score = mean over 5 fabrication seeds per init seed; difference = paired t over 5 init seeds (df 4); "beats" means
the 95% CI lower bound of (butterfly - baseline) is > 0.

| Task | Butterfly | Clements | Low-rank | Butterfly - Clements (95% CI) | Beats | Butterfly - low-rank (95% CI) | Beats |
| --- | --- | --- | --- | --- | --- | --- | --- |
| haar_unitary (fidelity) | 0.9632 | 0.9721 | 0.0612 | -0.00882 [-0.00977, -0.00787] | no | 0.90209 [0.90148, 0.90270] | yes |
| mnist_pca16_linear (test accuracy) | 0.8052 | 0.7985 | 0.7210 | 0.00674 [-0.00047, 0.01394] | no | 0.08421 [0.06665, 0.10178] | yes |
| dft_matrix (fidelity) | 0.9833 | 0.9778 | 0.0613 | 0.00542 [0.00399, 0.00684] | yes | 0.92192 [0.92040, 0.92343] | yes |

Means rounded here; the JSON has full precision and every per-seed score at N = 8 and N = 16.

## What this does and does not show

- At this loss model and budget, butterfly is clearly better than a rank-4 SVD mesh on all three tasks, worse than
  a full-depth Clements mesh on Haar-random unitaries, not distinguishable from Clements on MNIST-PCA16 (CI crosses
  0), and better than Clements on the DFT matrix (whose FFT structure butterfly matches).
- Result is conditional on the registered free-parameter point; the other grid points (`free_parameters` in the
  config) were not run. Butterfly's selected depth uses 48 crossings per path, so the verdict is sensitive to the
  crossing loss.
- No speedup, energy or hardware claim.
