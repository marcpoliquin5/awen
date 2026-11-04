# E-027: energy-per-token accounting with a measured published modulator (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-energy-accounting-modulator/PREREGISTRATION.md` (#233, `d71e7f4`,
D-028), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `afa8656`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-04 12:24 ET, exit 0, about 1.3 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e027 --counts results/conversions-per-token/results.json --out results/energy-per-token-modulator/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`0b7ed6abec9637b0eef875020a572be30ba13b7be9576cac4116f07952ecb930`. A second run gave a byte-identical
`results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): every required term is listed, every
reported contribution recomputes to count x value x unit, no not-sourced term carries a contribution, no total is
emitted (`total_J` is null, 6 terms not sourced), and the modulator term carries one contribution per published
point (`timurdogan2014`, two bases), each with its basis. `failures` is empty. This matches the registered
expectation.

## What could be filled (reported, no test)

Five of the eleven terms now have a sourced per-event value: input DAC and weight DAC (per published design, as in
E-026), ADC and the optical detection floor (as in E-025), and the modulator. Six stay "not sourced": laser, weight
hold, control electronics, thermal stabilization, memory movement and host transfer. So there is still **no energy
per token**, only labelled partial terms.

The modulator is counted once per input DAC conversion, which is 1,088 per token at every point, so the modulator
term is the same for both architectures, every tile size and every position count.

| Term | Basis (`timurdogan2014`) | Per event | Events per token | Per token |
| --- | --- | --- | --- | --- |
| modulator | per NRZ on-off-keyed bit | 0.91 fJ | 1,088 | 0.990 pJ |
| modulator | per full-swing 0-to-1 transition | 3.65 fJ | 1,088 | 3.97 pJ |

The DAC, ADC and optical-floor rows are unchanged from E-026 and E-025 (`results/energy-per-token-dac/README.md`,
`results/energy-per-token/README.md`).

Source labels: modulator events per token are measured by us (simulator counts, E-024). Both per-event values are
peer-reviewed measured values (`timurdogan2014`, Nature Communications 2014, time-domain reflectometry, 25 Gb/s
NRZ-OOK, 0.5 Vpp, as listed in `hardware_assumptions.yaml`); the stored derivation relates them (3.65 fJ / 4,
equiprobable NRZ transitions, only 0-to-1 draws energy). The device is one binary on-off-keyed silicon microdisk
measured without its driver, bias-lock or monitor circuitry, and without the paper's athermal tuning (0.24 to 2 fJ per
bit). It is not an analog photonic input symbol at the simulated contract's resolution, so the rows are labelled
partial terms, not the modulator energy of an AWEN system. Values in the table are rounded from `results.json`.

## Scope

No total, energy per MAC, speedup, savings or hardware claim follows from this run. The modulator row excludes its
driver, and the laser, weight-hold, control, thermal, memory-movement and host-transfer terms have no sourced value.
Filling them needs checked sources for laser and phase-shifter hold power, control and thermal power, a
memory-movement parameter, a byte count for host transfer, and a registered time model for the power terms.
