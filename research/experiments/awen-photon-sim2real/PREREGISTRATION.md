# AWEN-Photon sim-to-real noise control (E-022): pre-registration

Registered 2026-11-01 (commit of this file) under AWEN Master Prompt v2, section 7 ("evaluate under a different,
independently parameterized noise model from the one used in training"). Status: **registered, not run**. A change
after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Disclosure

- E-005 (`awen-models/results/photon-training/results.json`, stored result) already reports model A accuracies for
  seeds 1 and 2. Those numbers were seen before this registration; none of them is reused here.
- Before registration, model B ran only in its unit tests, on random matrices and a synthetic 10-class dataset
  (`awen-models/tests/test_sim2real.py`). No MNIST network has been evaluated under model B.

## Models

- **Model A** (training noise): the AWEN compact noise model, `awen_native.photonic_gemm` with
  `awen_native.default_contract()` (photons per MAC 1000, 8-bit DAC and ADC, phase error, crosstalk; an illustrative
  operating point, not sourced). Training uses its torch surrogate, exactly as E-005.
- **Model B** (evaluation noise): `awen_models/sim2real.py`, NumPy only, its own RNG (PCG64), no shared code or
  parameter with model A. Parameters are read at run time from `awen-spec/hardware_assumptions.yaml` (v0):

| Model B parameter | Assumption-set entry | Value | Source |
| --- | --- | --- | --- |
| Whole-system effective bits (output-referred Gaussian, rms `2^(1-b)/sqrt(12)` on the normalized tile output) | `system.pace_enob.typical` | 7.61 bits | hua2025, measured |
| Weight quantization (uniform grid of `2^b` levels on [-1, 1], one scale per weight tile) | `dac.weight_dac_bits.typical` | 8 bits | hua2025, measured |
| Input extinction ratio (zero input leaks `10^(-ER/10)` of full scale; signed inputs use two subtracted passes; weights signed by balanced differential detection) | `modulator.tfln_extinction_ratio.typical` | 30 dB | wang2018, measured (20-mm TFLN device) |

Not modeled in B (no checked source): phase error, crosstalk, drift, separate input-DAC and ADC quantization (the
system ENOB is a lumped whole-channel figure). Mixing a TFLN modulator figure with a silicon-photonics system ENOB is
a modeling choice, stated here, not a description of one real device. Tile size 64 is the E-005 training setting
(`PhotonTrainSettings.tile_k`), kept equal so the two models differ only in their noise.

Estimate (formula above): model B's output rms at 7.61 bits is 0.00295 of the half-range. For scale, model A's
analytic tile effective bits at 1000 photons per MAC is 5.54 (stored result, E-005 `points`), so the primary point
is likely milder than the training noise; the sweep below covers harsher points.

## Design

- Training: the three E-005 regimes (`standard`, `noise-aware-100`, `noise-aware-1000`) with E-005 settings
  (hidden 128, 3 epochs, batch 128, lr 2e-3, tile 64), retrained with the same code.
- Seeds: 1, 2, 3 (v2 asks for at least 3; E-005 had 2). Model-B evaluation seed `20000 + seed`; model-A evaluation
  seed `10000 + seed` as in E-005. Full 10,000-image MNIST test set. No run is dropped.
- Points: the **primary point** is model B with all three sourced values. **Secondary** (exploratory): system ENOB
  3, 4, 5, 6 and 10 bits with the other two sourced values; these ENOBs are free parameters of the study, not
  device predictions.
- Command (from `awen-models/`, CPU): `python -m awen_models.sim2real --seeds 1 2 3 --out results/photon-sim2real`
  (not run). Output: `results/photon-sim2real/results.json`, stored with its SHA-256, commit and environment.

## Verdict rule (primary point)

For each noise-aware regime R, take per-seed paired differences `acc_B(R, s) - acc_B(standard, s)` for s = 1, 2, 3,
and a two-sided 95% Student t interval (t = 4.303, n = 3):

- lower bound > 0: **transfers** (the noise-aware advantage survives an independent noise model);
- upper bound < 0: **reverses**;
- otherwise: **no detectable difference** (expected if the primary point is mild; reported as is, not as a failure
  or a success of noise-aware training).

The same rule is computed at the secondary points and labeled exploratory. Model-A accuracies and the A-minus-B gap
per regime are reported descriptively. All numbers are simulation (L1); no energy, speed or hardware claim follows.
