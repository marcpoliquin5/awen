# H3 registered run (rule h3-cells-v1)

Verdict: **not kept** under rule `h3-cells-v1` (registered in commit `99ced09` before this run; see
`../../PREREGISTRATION.md` and `../../configs/h3.yaml`). Evidence level L0 (exact lossless transfer-matrix
calculation). Source label for every number below: **measured by us**, stored in `h3_channel_plans.json`
(SHA-256 in `SHA256SUMS`).

Command (commit `99ced09`; build box Linux 6.12.94+, 8 vCPU, no GPU, Python 3.13.5, numpy 2.5.4, pyyaml 6.0.3):

```
python research/experiments/multiplexing-fractal/h3_stacks.py --config research/experiments/multiplexing-fractal/configs/h3.yaml
# {"runs": 45, "uniform_runs": 45, "cells": 9, "verdict": "not kept"}   (12.3 s wall)
```

A second run to another directory gave a byte-identical file (same SHA-256).

## Cells

Nine comparable cells, none "not comparable": 7 "cantor better on both", 2 "mixed", 0 "cantor worse on both".
Six (generation, spacing) pairs had no comb that fits at equal footprint (generation 2: all three spacings;
generation 3: 0.05 and 0.02; generation 4: 0.02), so they form no cell.

| Generation | Spacing | Sigma | Cantor density | Uniform density | Cantor worst xtalk (dB) | Uniform worst xtalk (dB) | Outcome |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 3 | 0.1 | 0.00 | 2.0 | 0.0 | -10.57 | -3.66 | better on both |
| 3 | 0.1 | 0.01 | 2.0 | 0.0 | -10.35 | -3.67 | better on both |
| 3 | 0.1 | 0.02 | 2.0 | 0.0 | -10.06 | -3.68 | better on both |
| 4 | 0.05 | 0.00 | 12.0 | 6.0 | -12.93 | -3.82 | better on both |
| 4 | 0.05 | 0.01 | 9.6 | 6.0 | -6.53 | -3.61 | better on both |
| 4 | 0.05 | 0.02 | 11.4 | 6.0 | -3.22 | -3.51 | mixed |
| 4 | 0.1 | 0.00 | 12.0 | 3.0 | -12.93 | -3.29 | better on both |
| 4 | 0.1 | 0.01 | 9.6 | 3.0 | -6.53 | -3.23 | better on both |
| 4 | 0.1 | 0.02 | 11.4 | 3.0 | -3.22 | -3.37 | mixed |

Density is usable channels per normalized-frequency unit (seed mean, 5 seeds); worst crosstalk is the seed
mean of the worst adjacent-channel value over all channels, usable or not (lower is better). Values are rounded
here; the JSON has full precision.

## What this does and does not show

- At 2% layer-thickness error the generation-4 Cantor stack loses its crosstalk advantage (worst crosstalk
  -3.22 dB vs -3.51 dB for the comb), so H3 is not kept. It is not killed: no cell is worse on both.
- The comparator is one equal-footprint Fabry-Perot comb per spacing with at most 2 mirror pairs (generation 3,
  spacing 0.1: 0 pairs, and no pair count met the design spec, so its usable density is 0). It is not an
  optimized WDM filter bank (cascaded rings, AWGs, or multi-cavity filters), so "better than this comb" is not
  evidence that a Cantor stack beats practical uniform-grid filters.
- The indices (2.3 / 1.38) and the sigma grid are free parameters of the study, not device predictions.
- Lossless, normal incidence only; no absorption, dispersion or angle effects.
