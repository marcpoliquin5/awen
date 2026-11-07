# E-034: energy-per-token accounting with published microring tuner-logic control energy-per-bit (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-control-energy/PREREGISTRATION.md` (#260, `07fbf25`,
D-035), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `e01220ec8818d4af0d5ef6b902759dcef8dd7f42`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-07 08:08 ET, exit 0, about 1.0 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e034 --counts results/conversions-per-token/results.json --out results/energy-per-token-control/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`12fe15c4f7b485c0308a405998656be65980db388e757a1914b178a6f1e6b7d4`; accounting version
`awen.energy-per-token-accounting.v10`. A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): the E-033 rule holds (every required
term listed, every reported contribution recomputes, no not-sourced term carries a contribution, no total), and the
control_electronics term is sourced from each published design with per-token joules equal to `dac_input` ×
energy_per_bit. `failures` is empty. This matches the registered expectation: one of eleven terms is still not
sourced (host transfer), so there is still **no energy per token**.

## Control energy per token (reported, L1 counts x peer-reviewed tuner-logic energy/bit estimate)

Input DAC conversions are independent of tile size and position count under the E-024 closed form (1,088 per
token), so every one of the 40 points gives the same control energy. Energy uses the published point
`sun2016` (`control.published_designs`): 148 fJ/bit estimate (0.74 mW measured tuner logic / 5 Gb/s). Joules =
`dac_input` × energy_per_bit × 1e-15. Values below are per token. Logic only; heater, tuner frontend, modulator
driver, system controller, sequencer, clocking and host interface are excluded. Do **not** treat this as a
whole-accelerator control power.

| Basis | Energy / bit | Input DAC / token | Control energy / token |
| --- | --- | --- | --- |
| tuner_logic_power_over_5Gbps | 148 fJ | 1,088 | 0.1610 nJ (161.024 pJ) |

Range: 0.1610 nJ per token (constant across the 40 points). Source labels: input DAC counts are measured by us
under the E-024 counting rule on stored simulator counts; the 148 fJ/bit figure is a labelled estimate from a
peer-reviewed measured logic power (Sun et al., IEEE JSSC 51(4), 893-907 (2016), doi:10.1109/JSSC.2016.2519390).
Source type in the report is `estimate`. Values in the table are rounded from `results.json`.

## What stays not sourced

One of eleven terms stays "not sourced": host transfer (`system.host_transfer_energy` is still null). ADC, DAC
(per published design), modulator (per published basis), optical detection floor, memory movement (per published
HBM2 point and placement), weight hold (per published π-reconfiguration point), laser (per published DML DC
energy-per-bit point), thermal (per published heater energy-per-bit point), and now control (per published
tuner-logic energy-per-bit point) are filled. There is still **no total**.

## Scope

No total, energy per MAC, speedup, savings or hardware claim follows from this run. Counting one tuner-logic
energy-per-bit contribution per input DAC conversion is a labelled binary optical-input proxy for per-ring
wavelength-lock control logic, not whole-accelerator control power and not heater energy (already sourced under
E-033). Filling the remaining term needs a fitted host-link-only energy per byte.
