# E-038: accuracy axis for the energy-per-MAC curve on AWEN-Mini (L1)

Registered run of `research/experiments/awen-photon-accuracy-energy-per-mac/PREREGISTRATION.md` (#276, `52c82d0`,
D-039), run unchanged.

Command (from `awen-models/`, at commit `926cef9fa8e3c64039d5c0820c8c75e6b8b064c7`, build box: see `env.txt`;
Linux, 8 vCPU, no GPU), started 2026-11-09 00:31 ET, exit 0, about 46.5 s wall, no stdout:

    PYTHONPATH=<awen_native built at 926cef9>:.:../awen-ecosystem/python_awen python -m awen_models.accuracy_energy_mac --registration e038 --out results/accuracy-energy-per-mac/results.json

`awen_native` was built from `awen-native/` at the same commit (`maturin build --release`) and put first on the
path; `awen_py` was read from the source tree because the build box's installed copy is older than main.

Input SHA-256 recorded by the command (`input_sha256`) and equal to the registered pins (also checked with
`sha256sum` before the run):

| Input | SHA-256 |
| --- | --- |
| AWEN-Mini checkpoint | `3a4e2158c139e86a0905cfc40d063001f09565f09af3e04253840f063a04b003` |
| Assumption set | `e0965ba697e23559d0638fdbf31a405f497f898215ec75a3349a0330651d246f` |
| Corpus (Tiny Shakespeare) | `86c4e6aa9db7c042ec79f339dcb96d42b0075e16b8fc2e86bf0ca57e2dc565ed` |

A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** Input hashes equal the pins; all 4 contracts x 3 seeds were evaluated on 16 windows; every window's
counts equal the closed form; every NLL is finite; under `ideal` the simulator NLL equals the CPU NLL within
1e-3 nats for every seed (difference -7.4e-9 nats); the E-036 accounting check passes with 0 terms not sourced;
the E-037 per-MAC check passes. `failures` is empty. The verdict checks the pipeline, not a hypothesis about accuracy.

## Accuracy (held-out next-character NLL, nats per character, lower is better)

16 validation windows of 128 characters (seed 2024), mean over the windows per seed, then mean and 95% Student-t CI
over noise seeds 0, 1, 2. The CPU reference is 1.4295 nats on the same windows.

| Contract | Simulator NLL (mean ± 95% CI half-width) | Simulator − CPU (mean ± CI) | `min_tile_effective_bits` (all 3 seeds) | Detection floor per MAC (optics only) |
| --- | --- | --- | --- | --- |
| ideal | 1.4295 ± 0 | -7.4e-9 ± 0 | n/a | n/a |
| bits8 | 4.256 ± 0.231 | 2.827 ± 0.231 | 5.544 | 0.1311 fJ |
| bits6 | 4.603 ± 0.050 | 3.173 ± 0.050 | 5.236 | 8.203 aJ |
| bits4 | 7.136 ± 0.021 | 5.707 ± 0.021 | 3.748 | 0.5125 aJ |

The bits contracts are the simulator default contract with only `dac_bits = adc_bits` changed. Its other values
(photons per MAC, phase noise, loss, crosstalk and so on) are the inventoried, **unsourced** simulator defaults in
`awen-spec/hardcoded_device_defaults.yaml`, so these figures describe the simulator under those defaults, not any
device. At contract 8 bits the smallest tile effective resolution is 5.544 bits, so the converters are not the only
limit there. For scale only: a uniform guess over the model's 65-character vocabulary has NLL ln 65 = 4.174 nats
(estimate, formula). The bits6 and bits4 means are above it, and the bits8 CI (4.025 to 4.487) includes it. The
model was trained without noise (E-001); no noise-aware training is part of E-038.

## Energy per lowered MAC (E-036 accounting on AWEN-Mini counts, E-037 division)

One 128-character window: 573,440 input DAC, 983,040 weight DAC and 1,966,080 ADC conversions and 125,829,120
lowered MACs (simulator counts, equal to the closed form at runtime `tile_k` 64). Per token: 983,040 lowered MACs
and 27,520 conversions (0.02799 conversions per lowered MAC). Host-fallback MACs (attention cores, tied output
head) are 0.1490 of all MACs and are not charged.

| Transmitter | Excluded term | n_scenarios | Per lowered MAC (min – max) | Per token (min – max) |
| --- | --- | --- | --- | --- |
| dml_laser | modulator | 288 | 3.009 pJ – 6.367 pJ | 2.958 µJ – 6.259 µJ |
| modulator_eo | laser | 288 | 3.008 pJ – 6.366 pJ | 2.957 µJ – 6.258 µJ |

Counts do not depend on the contract, so this envelope is the same for every bits key. The published DAC and ADC
points are fixed designs at their own resolution, so the envelope does not scale with the bits key; that is a
limitation of this curve, not a finding about converter scaling. Values are rounded from `results.json`.

## Source labels

- NLL, effective bits, counts, host MAC fraction: measured by us (this run's stored `results.json`, simulator L1
  and CPU reference).
- Per-MAC and per-token envelopes: the E-036 accounting applied to those counts, i.e. counts times the labelled
  published points of E-026 to E-035 (peer-reviewed measured or estimate, as labelled on each term), divided by
  lowered MACs per token (E-037 rule).
- ln 65: estimate (formula), for scale only.

## What this is not

Not a device prediction (the contract values besides the bits are unsourced defaults), not one physical device,
not a measured energy, not per model MAC, and not a speedup, savings, hardware or model-equivalence claim. No
electronic baseline is compared.
