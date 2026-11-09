# E-039: recalibration time axis for the E-023 drift budgets (estimate under a stated proxy, L1)

Registered run of `research/experiments/awen-photon-recalibration-time/PREREGISTRATION.md` (#280, `28509a3`,
D-040), run unchanged on the stored E-023 result (`results/photon-recalibration/results.json`, #219). No model was
trained or evaluated; E-023 was not rerun.

Command (from `awen-models/`, at commit `6a043ba7a4ba509df4a311c6c76f7519f56abf45`, build box: see `env.txt`;
Linux, 8 vCPU, no GPU), started 2026-11-09 12:28 ET, exit 0, about 1.4 s wall:

    python -m awen_models.recalibration_time_axis --registration e039 --recalibration results/photon-recalibration/results.json --out results/photon-recalibration-time/results.json

Stdout: `verdict pass`, then one `tolerable_interval_s` line per regime, point and sensitivity rule (12 lines,
same values as the table below).

Inputs recorded by the command (`inputs`):

| Input | SHA-256 | Note |
| --- | --- | --- |
| E-023 result | `12e7db5fa60f594bf4acb0748ea73066b4fc9e9b4b3ffd0f7065040704cc3401` | equal to the registered pin (also checked with `sha256sum` before the run) |
| Assumption set (`awen.hardware-assumptions.v0`) | `69071e4a3aebca31345a6a12eac435d79172fb253631f820dd3879aada097eac` | file at `28509a3`; the `drift.published_designs` points equal the registered table |

The stored `inputs.assumptions.path` is the absolute path on the build box (the registered module records the path
it was given, and the default is resolved to an absolute path). Two more runs from the same worktree gave a
byte-identical `results.json` (SHA-256 in `SHA256SUMS`); a run from another checkout path would differ only in that
string.

## Verdict (registered rule)

**Pass.** The E-023 input matches its pin, the published points match the registered table (elmas2025, sigma 0.25
and 0.15 rad/sqrt(s)), every tolerable budget is copied unchanged (standard 0.05, noise-aware-100 0.1,
noise-aware-1000 0.05), and every interval equals `(s / (k * sigma))^2` within relative tolerance 1e-12.
`failures` is empty. This is a consistency check on an estimate, not a hypothesis test.

## Proxy (read before using any number below)

sigma is the Brownian-model rate the authors of elmas2025 (Applied Optics 64(14) 3995, 2025,
doi:10.1364/AO.560370) matched to the measured **relative phase between two input fibers** of one lab setup
(cooling-fan airflow). It is not a measured drift of programmed on-chip mesh phases or weights. E-039 applies it to
every programmed weight independently, as its registered proxy assumption. k is a registered first-order
phase-to-weight bound (estimate), not a device property. Every interval is therefore an estimate under that proxy.

## Tolerable recalibration interval (seconds)

`T = (s / (k * sigma))^2` at each regime's stored tolerable budget `s`. Values rounded from `results.json`.

| Regime (tolerable `s`) | sigma 0.25, k 1.0 | sigma 0.25, k 0.5 | sigma 0.15, k 1.0 | sigma 0.15, k 0.5 |
| --- | --- | --- | --- | --- |
| standard (0.05) | 0.04 s | 0.16 s | 0.1111 s | 0.4444 s |
| noise-aware-100 (0.1) | 0.16 s | 0.64 s | 0.4444 s | 1.778 s |
| noise-aware-1000 (0.05) | 0.04 s | 0.16 s | 0.1111 s | 0.4444 s |

Across regimes, points and bounds the tolerable interval spans 0.04 s to 1.778 s. The interval for every non-zero
E-023 grid budget (0.01, 0.02, 0.05, 0.1, 0.2) is in `interval_s_by_budget`; it runs from 0.0016 s (s 0.01,
sigma 0.25, k 1.0) to 7.111 s (s 0.2, sigma 0.15, k 0.5).

The tolerable budgets come from E-023's coarse grid, so each interval is bounded by the next grid budget (the
true tolerable budget lies between the stored one and the next grid value; the interval scales with `s^2`).

## Source labels

- Tolerable budgets and grid budgets: measured by us (stored E-023 result, simulator L1, #219).
- sigma 0.25 and 0.15 rad/sqrt(s): peer-reviewed (elmas2025; authors' model values matched to their measurements,
  input-fiber relative phase; separate points, never averaged).
- k 1.0 and 0.5: estimate (formula: |d cos(phi)/d phi| <= 1 and |d cos(theta/2)/d theta| <= 1/2).
- Intervals: estimate (formula `T = (s / (k * sigma))^2`) under the stated proxy; computed by this run.

## What this is not

Not a device prediction, not a measured recalibration time, not a drift rate of any photonic processor's weights,
and not a speed, energy, hardware or model-equivalence claim. The compiler's `drift_rad_per_sqrt_s` default (0.0) is
unchanged.
