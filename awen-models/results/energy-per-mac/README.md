# E-037: system energy per lowered MAC (scenario envelopes per transmitter model, L1)

Registered run of `research/experiments/awen-photon-energy-per-mac/PREREGISTRATION.md` (#272, `0fc238e`,
D-038), run unchanged on the stored E-036 totals (`results/energy-per-token-total/results.json`, #270) and the
stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `5acc49156f2c973ea49375261b294042b7cfcf7a`, build box: see `env.txt`;
Linux, 8 vCPU, no GPU), started 2026-11-08 12:29 ET, exit 0, about 1.2 s wall:

    PYTHONPATH=. python -m awen_models.energy_per_mac --registration e037 --totals results/energy-per-token-total/results.json --counts results/conversions-per-token/results.json --out results/energy-per-mac/results.json

Input SHA-256 recorded by the command (`input_sha256`) and equal to the registered pins:

| Input | SHA-256 |
| --- | --- |
| E-036 totals | `0f78b7a14dc76f870c0ea7d4108053034a12abd0d3b81a23fcd57bee7f7b7e4b` |
| E-024 counts | `6cec0563175b6c0b5fb07bb068a7eb06338b19a3a7a3f67dd98f869da8432267` |

A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** Both inputs are passing results of the named versions (`awen.energy-per-token-accounting.v12`,
`awen.conversions-per-token-curve.v1`) with the pinned SHA-256s; totals and counts cover the same 40 grid points
(2 architectures x 5 tile sizes x 4 position counts) with equal per-token counts; at every point the denominator
is a positive exact integer, both transmitter models are present, `0 < min_J_per_mac <= max_J_per_mac` (finite),
`n_scenarios` is 288, the per-MAC bounds times the denominator recompute the per-token bounds (relative
tolerance 1e-12), the largest detection floor per MAC is below both `max_J_per_mac`, and conversions per MAC
recompute. `failures` is empty. The two architectures give the same figures at every tile size and position count.

This is **not** one physical device, **not** a measured value, and **not** a speedup or savings claim. No
electronic baseline is compared. Host CPU compute is not a term of the numerator, and host-fallback MACs are not
in the denominator, so this is energy per *lowered* MAC, not per model MAC.

## Energy per lowered MAC (E-036 envelope / E-024 lowered MACs per token)

Lowered MACs per token are 90,112 at all 40 points (`counts.lowered_macs / positions`, exact). Across the grid:

| Transmitter | Excluded term | n_scenarios | min_J_per_mac range (40 points) | max_J_per_mac range (40 points) | Overall span (min of mins to max of maxes) |
| --- | --- | --- | --- | --- | --- |
| dml_laser | modulator | 288 | 3.602 pJ – 145.5 pJ | 7.573 pJ – 296.6 pJ | 3.602 pJ – 296.6 pJ |
| modulator_eo | laser | 288 | 3.600 pJ – 145.5 pJ | 7.569 pJ – 296.6 pJ | 3.600 pJ – 296.6 pJ |

The lowest bounds are at tile 128 and 128 positions; the highest are at tile 8 and 1 position (as in E-036, since
the denominator is the same at every point). The min and max picks are those of E-036 and are the same at all 40
points. Values are rounded from `results.json`.

Per tile size (`dml_laser` min – max per lowered MAC, both architectures equal; `modulator_eo` differs only in
the fourth significant figure or later):

| tile_k | P = 1 | P = 8 | P = 32 | P = 128 |
| --- | --- | --- | --- | --- |
| 8 | 145.5 – 296.6 pJ | 29.95 – 62.74 pJ | 17.57 – 37.68 pJ | 14.47 – 31.42 pJ |
| 16 | 139.4 – 283.2 pJ | 23.82 – 49.29 pJ | 11.44 – 24.23 pJ | 8.341 – 17.97 pJ |
| 32 | 136.3 – 276.4 pJ | 20.76 – 42.57 pJ | 8.371 – 17.51 pJ | 5.275 – 11.24 pJ |
| 64 | 134.8 – 273.1 pJ | 19.22 – 39.20 pJ | 6.838 – 14.14 pJ | 3.742 – 7.879 pJ |
| 128 | 134.7 – 272.8 pJ | 19.08 – 38.90 pJ | 6.698 – 13.84 pJ | 3.602 – 7.573 pJ |

## Optical detection floor per MAC (optics only, a component, not a system figure)

Depends on tile size only. At 4 effective bits 0.4659 aJ (tile 128) to 4.1 aJ (tile 8); at 8 effective bits
0.1192 fJ (tile 128) to 1.049 fJ (tile 8) per lowered MAC. The largest floor is at most 7.25e-5 of the smallest
system lower bound at the same point. It is a lower bound on optical energy and may not stand in for the system
figures above.

## Context: conversions per lowered MAC (E-024)

0.03409 (tile 128, 128 positions) to 1.137 (tile 8, 1 position).

## Source labels

- Lowered MACs and conversions per token: measured by us (stored E-024 simulator counts, L1).
- Per-token envelopes: E-036 stored result (E-024 counts times the labelled published points of E-026 to E-035;
  peer-reviewed measured or estimate, as labelled on each term).
- Per-MAC figures, 40-point ranges and the floor ratio: measured by us (this run's stored `results.json`;
  ratios computed from it).

## What this is not

No accuracy axis (the E-024 graphs are random-init). Not a per-model-MAC figure, not one physical device, not a
speedup, savings or hardware claim.
