# E-033: energy-per-token accounting with published heater thermal energy-per-bit (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-thermal-energy/PREREGISTRATION.md` (#256, `91458fe`,
D-034), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `d1bfcdc`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-07 00:28 ET, exit 0, about 1.0 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e033 --counts results/conversions-per-token/results.json --out results/energy-per-token-thermal/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`0214ea6ef0b53bd591729f1c40aa2b158ddf7e5b2c06f29e407975a49e0d7154`; accounting version
`awen.energy-per-token-accounting.v9`. A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): the E-032 rule holds (every required
term listed, every reported contribution recomputes, no not-sourced term carries a contribution, no total), and the
thermal_stabilization term is sourced from each published design with per-token joules equal to `dac_input` ×
energy_per_bit. `failures` is empty. This matches the registered expectation: two of eleven terms are still not
sourced (control electronics, host transfer), so there is still **no energy per token**.

## Thermal energy per token (reported, L1 counts x peer-reviewed heater energy/bit estimate)

Input DAC conversions are independent of tile size and position count under the E-024 closed form (1,088 per
token), so every one of the 40 points gives the same thermal energy. Energy uses the published point
`zortman2012` (`thermal.published_designs`): 180 fJ/bit at 10 Gbps (paper estimate for 80 GHz manufacturing +
40 °C environmental swing from measured 7 µW/GHz heater efficiency). Joules = `dac_input` × energy_per_bit ×
1e-15. Values below are per token. Heater only; closed-loop control ASIC and package TEC are excluded. Do
**not** sum with `modulator.published_designs` athermal EO compensation in one architecture total.

| Basis | Energy / bit | Input DAC / token | Thermal energy / token |
| --- | --- | --- | --- |
| realistic_trim_plus_thermal_10Gbps | 180 fJ | 1,088 | 0.1958 nJ (195.84 pJ) |

Range: 0.1958 nJ per token (constant across the 40 points). Source labels: input DAC counts are measured by us
under the E-024 counting rule on stored simulator counts; the 180 fJ/bit figure is a peer-reviewed estimate
(Zortman et al., OFC/NFOEC 2012 OW4I.5 / doi:10.1364/ofc.2012.ow4i.5, from measured 7 µW/GHz). Source type in
the report is `estimate`. Values in the table are rounded from `results.json`.

## What stays not sourced

Two of eleven terms stay "not sourced": control electronics and host transfer (`system.host_transfer_energy` is
still null). ADC, DAC (per published design), modulator (per published basis), optical detection floor, memory
movement (per published HBM2 point and placement), weight hold (per published π-reconfiguration point), laser
(per published DML DC energy-per-bit point), and now thermal (per published heater energy-per-bit point) are
filled. There is still **no total**.

## Scope

No total, energy per MAC, speedup, savings or hardware claim follows from this run. Counting one heater
energy-per-bit contribution per input DAC conversion is a labelled binary optical-input proxy for resonant-device
on-chip heater trimming, not a package TEC continuous power and not control-loop logic energy. Filling the
remaining terms needs a checked source for control electronics (or a registered time model) and a fitted
host-link-only energy per byte.
