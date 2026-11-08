# AWEN-Photon accuracy axis for the energy-per-MAC curve (E-038): pre-registration

Registered 2026-11-08 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

Master Prompt v2 section 7 asks for accuracy against optical energy per MAC. E-037 (#274) stored system energy per
lowered MAC for two random-init graphs that have no meaningful accuracy. On one trained model that has both an
accuracy metric and a lowering count (the bundled AWEN-Mini character model, 1,016,320 parameters, E-001), what is
the held-out accuracy on the photonic simulator at each registered converter precision, and what system energy per
lowered MAC does the registered accounting give for that model's counts?

## Disclosure

- The model, its stored CPU evaluation (E-002) and the E-037 rule were seen before this registration. While writing
  it, the agent hashed the three inputs below and checked that the four registered contracts validate. No window
  was evaluated, no AWEN-Mini count was read from the simulator, and no accuracy or energy figure is reported here.
- Accuracy metric: held-out next-character NLL in nats per character (lower is better), from the capability report
  (`awen.capability.build_report`), simulator against the eager CPU reference on the same characters. Each window
  of `block_size` (128) characters is one forward pass, so the NLL covers the 127 next-character targets inside it.
  The windows are 16 draws from the validation split (last 10% of Tiny Shakespeare, the checkpoint's training split
  rule) with generator seed 2024 (the seed of the stored evaluation).
- Contracts: `ideal` (noise-free), and the simulator default contract with `dac_bits = adc_bits` set to 4, 6 and 8.
  The other default contract values (photons per MAC, phase noise, loss, crosstalk and so on) are the inventoried,
  unsourced simulator defaults in `awen-spec/hardcoded_device_defaults.yaml`. They are not sourced device values,
  so the accuracy figures are a simulation under those defaults, not a prediction for any device. The bits keys are
  the same keys as the E-036 detection-floor sweep. Contract bits are not the effective bits; the report's
  `min_tile_effective_bits` is stored beside each point.
- Seeds: noise seeds 0, 1, 2 (`Runtime(seed=...)`). The executor resets per window, so every window sees the same
  per-seed noise sequence on different characters. Mean and 95% Student-t CI over the 3 seeds.
- Lowering count: the simulator report counts (input DAC, weight DAC, ADC, lowered MACs) per window, checked against
  the closed form from the model's `AwenLinear` weight shapes and the runtime executor's `tile_k` (64, its
  default, an inventoried unsourced value, read and not chosen here). The tied output head and the attention cores
  stay on the host; their MAC share is stored as `host_mac_fraction` and is not charged any energy.
- Energy axis: the E-036 accounting (`awen.energy-per-token-accounting.v12`, `scenario_envelope_v1`, the same 11
  terms and published points) applied unchanged to the AWEN-Mini per-token counts, then the E-037 division by
  lowered MACs per token. Counts do not depend on the contract, so there is one system envelope per transmitter
  model for every bits key; only the optics-only detection floor per MAC is read per bits key. The published DAC and
  ADC points are fixed designs at their own resolution, so the system envelope does not scale with the bits key.
  This limitation is part of the result, not a finding about converter scaling.
- The bounds are sums of labelled published points within one transmitter model. They are not one physical device,
  not a measured energy and not a speedup or savings figure. No electronic baseline is compared.
- During development the code ran only in unit tests on synthetic evaluations. No E-038 output was stored.

## Inputs (pinned)

| Input | Path | SHA-256 |
| --- | --- | --- |
| AWEN-Mini checkpoint | `awen-models/awen_models/checkpoints/awen-mini/model.safetensors` | `3a4e2158c139e86a0905cfc40d063001f09565f09af3e04253840f063a04b003` |
| Assumption set | `awen-spec/hardware_assumptions.yaml` | `e0965ba697e23559d0638fdbf31a405f497f898215ec75a3349a0330651d246f` |
| Corpus | Tiny Shakespeare (`awen_models.data.TINY_SHAKESPEARE`, already pinned there) | `86c4e6aa9db7c042ec79f339dcb96d42b0075e16b8fc2e86bf0ca57e2dc565ed` |

The command hashes all three and records the hashes; a mismatch is a failure (`inputs`).

## Design

Module `awen_models.accuracy_energy_mac`, curve `awen.accuracy-energy-per-mac-curve.v1`. Output:

- `accuracy[label]` for `ideal`, `bits4`, `bits6`, `bits8`: simulator NLL (mean, sd, 95% CI over 3 seeds), CPU NLL,
  NLL difference (mean and CI), `min_tile_effective_bits` per seed, and for the bits keys the detection floor per MAC
- `counts` (one window), `host_mac_fraction`, `energy_per_lowered_mac` (the E-037 point shape for `awen-mini`)
- `input_sha256`, `assumption_set`, `disclosure`, `verdict`, `failures`

Command (from `awen-models/`, CPU; needs `awen_native` and the cached corpus):

    python -m awen_models.accuracy_energy_mac --registration e038 --out results/accuracy-energy-per-mac/results.json

Output stored with its SHA-256, commit and environment.

## Verdict rule

**Pass** when all of this holds: the three input SHA-256s equal the pins; every registered contract and seed was
evaluated on 16 windows; every window's counts equal the closed form; every NLL is finite; under `ideal` the
simulator NLL is within 1e-3 nats of the CPU NLL for every seed; the E-036 accounting check passes with 0 terms not
sourced; and the E-037 per-MAC check passes. Otherwise **fail** with the failures listed.

The verdict checks the pipeline, not a hypothesis about accuracy. Whatever the NLL is at 4, 6 or 8 bits gets
reported as run. Figures are L1. No speedup, savings, hardware or model-equivalence claim follows. This PR
registers only; it does not run E-038.
