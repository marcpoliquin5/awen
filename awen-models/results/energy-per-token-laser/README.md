# E-032: energy-per-token accounting with published DML laser energy-per-bit (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-laser-energy/PREREGISTRATION.md` (#252, `9f0326a`,
D-033), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `1cc6f98`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-06 15:21 ET, exit 0, about 1.1 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e032 --counts results/conversions-per-token/results.json --out results/energy-per-token-laser/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`b035fd84ff5a12e82df01eabe25674a9616ed5f9c6d712d02d8efc680b2e1cbb`; accounting version
`awen.energy-per-token-accounting.v8`. A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): the E-031 rule holds (every required
term listed, every reported contribution recomputes, no not-sourced term carries a contribution, no total), and the
laser term is sourced from each published design with per-token joules equal to `dac_input` × energy_per_bit.
`failures` is empty. This matches the registered expectation: three of eleven terms are still not sourced (control
electronics, thermal stabilization, host transfer), so there is still **no energy per token**.

## Laser energy per token (reported, L1 counts x peer-reviewed DML DC energy/bit estimate)

Input DAC conversions are independent of tile size and position count under the E-024 closed form (1,088 per
token), so every one of the 40 points gives the same laser energy. Energy uses the two published points
`diamantopoulos2021` (`laser.published_designs`): 0.17 pJ/bit at 25 °C and 0.34 pJ/bit at 50 °C. Joules =
`dac_input` × energy_per_bit × 1e-12. Values below are per token. RF driver energy is excluded. This is an
interconnect-style DML (laser and modulation are one device); do **not** sum with `modulator.published_designs`
in one architecture total.

| Basis | Energy / bit | Input DAC / token | Laser energy / token |
| --- | --- | --- | --- |
| dc_energy_per_bit_25C | 0.17 pJ | 1,088 | 0.1850 nJ (184.96 pJ) |
| dc_energy_per_bit_50C | 0.34 pJ | 1,088 | 0.3699 nJ (369.92 pJ) |

Range: 0.1850 nJ to 0.3699 nJ per token. Source labels: input DAC counts are measured by us under the E-024
counting rule on stored simulator counts; the 0.17 / 0.34 pJ/bit figures are peer-reviewed estimates
(Diamantopoulos et al., Photonics 8, 31 (2021) / doi:10.3390/photonics8020031, DC power / net rate at the
60 GBaud PAM-4 KP4-FEC operating points). Source type in the report is `estimate`. Values in the table are
rounded from `results.json`.

## What stays not sourced

Three of eleven terms stay "not sourced": control electronics, thermal stabilization, and host transfer
(`system.host_transfer_energy` is still null). ADC, DAC (per published design), modulator (per published basis),
optical detection floor, memory movement (per published HBM2 point and placement), weight hold (per published
π-reconfiguration point), and now laser (per published DML DC energy-per-bit point) are filled. There is still
**no total**.

## Scope

No total, energy per MAC, speedup, savings or hardware claim follows from this run. Counting one DML DC
energy-per-bit contribution per input DAC conversion is a labelled binary optical-input proxy, not a CW
external-laser power for an external-modulator photonic MAC. Filling the remaining terms needs checked sources
for control and thermal power (or a registered time model) and a fitted host-link-only energy per byte.
