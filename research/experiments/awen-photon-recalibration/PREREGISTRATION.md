# AWEN-Photon accuracy versus recalibration interval (E-023): pre-registration

Registered 2026-11-02 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run**. A change
after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

How much weight drift between calibrations can the E-005 MNIST MLP take before its test accuracy under the
independent noise model B (E-022, `awen_models/sim2real.py`, sourced primary point: system ENOB 7.61, weight DAC
8 bits, extinction ratio 30 dB) drops by more than one percentage point, and does noise-aware training move that point?

## Disclosure

- E-022 (`awen-models/results/photon-sim2real/results.json`, stored result) already reports model-B accuracies without
  drift for the same regimes, seeds and training code. Those numbers were seen before this registration; the
  no-drift point here is recomputed, not copied.
- Before registration the drift code ran only in its unit tests (`awen-models/tests/test_recalibration.py`), on
  random matrices and a synthetic 10-class dataset. No MNIST network has been evaluated with drift.

## Drift model (all drift values are free parameters)

`hardware_assumptions.yaml` v0 has no checked source for any drift rate (`thermal_tuning_power` and
`thermal_stabilization_power` are null; no phase-drift entry), so no number below is a device prediction.

- After each calibration, every programmed weight (normalized units: quantized weight divided by its tile's maximum,
  in [-1, 1]) drifts as an independent Gaussian random walk, added after quantization and clipped to [-1, 1].
- The **drift budget** `s` is the rms offset accumulated over one full recalibration interval. A recalibration resets
  the offset to zero.
- Interval accuracy is the mean of the accuracies at `t/T = 0.125, 0.375, 0.625, 0.875` along one random-walk path
  (midpoint rule, 4 times). The model-B noise seed is the same at every time, so only the drift differs.
- For a drift rate `D` (rms per square root of time) the interval is `T = (s / D)^2` (`interval_for_budget`). Results
  are reported against `s` only; turning them into hours needs a sourced `D`, which we do not have.
- Not modeled: correlated drift across weights, drift of the input path or detectors, temperature dependence,
  calibration error itself. These are stated limits, not omissions to fix silently.

## Design

- Training: the E-005 regimes (`standard`, `noise-aware-100`, `noise-aware-1000`) with E-005 settings, retrained with
  the E-022 code and seeds 1, 2, 3. Full 10,000-image MNIST test set. No run is dropped.
- Budget grid `s`: 0, 0.01, 0.02, 0.05, 0.1, 0.2 (for scale, one 8-bit weight step is 2/255 = 0.0078, analytic).
- Seeds: model-B noise `20000 + seed`; drift `30000 + 100 * seed + budget index` (NumPy PCG64).
- Command (from `awen-models/`, CPU): `python -m awen_models.recalibration --seeds 1 2 3 --out results/photon-recalibration`
  (not run). Output: `results/photon-recalibration/results.json`, stored with its SHA-256, commit and environment.

## Verdict rule

Primary, per regime: per-seed paired drop `acc(s=0) - acc(s)`, two-sided 95% Student t interval (t = 4.303, n = 3).
A budget passes when the upper bound is at most 0.01. The **tolerable budget** is the largest grid budget such that it
and every smaller budget pass (reported as 0 if only the reference passes).

Exploratory: at each budget, noise-aware minus standard paired differences with the E-022 rule (transfers, reverses,
no detectable difference). All numbers are simulation (L1); no energy, speed, hardware or recalibration-time claim
follows.
