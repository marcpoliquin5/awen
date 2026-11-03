# AWEN-Photon energy-per-token accounting (E-025): pre-registration

Registered 2026-11-03 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

For the stored E-024 per-token conversion counts (tiny Llama and Qwen3 through `torch.compile(backend=awen)`, 40 grid
points), which terms of a system-level energy account can be filled from `awen-spec/hardware_assumptions.yaml`
(assumption set `awen.hardware-assumptions.v0`) with a source label, and which stay "not sourced"?

## Disclosure

- The E-024 counts (`awen-models/results/conversions-per-token/results.json`, #223) were seen before this registration.
- The assumption set was read before this registration: today `dac.energy_per_conversion`, `modulator.energy_per_symbol`,
  `laser.wall_plug_efficiency`, `phase_shifter.thermo_optic_power_per_pi`, `system.control_electronics_power`,
  `system.thermal_stabilization_power` and `system.host_transfer_energy` are null, and there is no memory-movement
  parameter. So the expected outcome is a partial table with no total.
- Before registration the code ran only in its unit tests (`awen-models/tests/test_energy_accounting.py`), on one
  synthetic point, not on E-024 output.

## Terms and rules (`awen_models.energy_accounting.TERMS`)

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| laser | none | `laser.wall_plug_efficiency` | power term, needs a time model |
| modulator | input DAC conversions (one symbol each) | `modulator.energy_per_symbol` | count x value |
| dac_input | input DAC conversions | `dac.energy_per_conversion` | count x value |
| dac_weight | weight DAC conversions | `dac.energy_per_conversion` | count x value |
| weight_hold | none | `phase_shifter.thermo_optic_power_per_pi` | power term, needs a time model |
| adc | ADC reads | `adc.published_designs` | count x each published design, never interpolated |
| optical_detection_floor | ADC reads (one detection each) | `shot_noise.shot_noise_energy_per_detection_J` | count x each swept bit depth; optics only |
| control_electronics | none | `system.control_electronics_power` | power term, needs a time model |
| thermal_stabilization | none | `system.thermal_stabilization_power` | power term, needs a time model |
| memory_movement | none | none in the assumption set | not sourced |
| host_transfer | bytes (not counted) | `system.host_transfer_energy` | not sourced until bytes are counted |

A per-event term with a null value or no source is "not sourced". No time model is registered, so power terms are
never converted to energy. `system.efficiency_including_lasers` and `system.on_chip_efficiency_reported` are
whole-system figures of other devices and are excluded: they cannot be composed with per-term counts.

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --counts results/conversions-per-token/results.json --out results/energy-per-token/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

The accounting **passes** when, at every point, (a) every required term (lasers, modulators, input and weight DAC,
ADC, control electronics, memory movement, host transfer) is listed, (b) every reported contribution recomputes to
count x value x unit, (c) no not-sourced term carries a contribution, and (d) no total is emitted while any term is
not sourced. Otherwise it **fails** and the failing points are listed.

Reported, no test: the per-term table at each point. Every figure is a partial, labelled term (L1 counts times a
sourced or AWEN-estimated per-event energy). No total, speedup, savings or hardware claim follows from this run.
