# E-036: energy-per-token scenario-envelope totals (all terms sourced, scenario_envelope_v1, L1)

Registered run of `research/experiments/awen-photon-energy-total/PREREGISTRATION.md` (#268, `cbdaee8`,
D-037), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `5578fd072934142412a62b07e757255e265e90e2`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-08 00:29 ET, exit 0, about 1.3 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e036 --counts results/conversions-per-token/results.json --out results/energy-per-token-total/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`e0965ba697e23559d0638fdbf31a405f497f898215ec75a3349a0330651d246f`; accounting version
`awen.energy-per-token-accounting.v12`. A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): every required term is listed,
every reported contribution recomputes, no not-sourced term carries a contribution, `not_sourced` is empty, and
`total_J` is a `scenario_envelope_v1` object whose `min_J` / `max_J` match a recomputation from the term rows
(tolerance 1e-18 relative or absolute). `failures` is empty. The two architectures give the same envelopes at
every tile size and position count.

This is **not** one physical device, **not** a bare energy-per-token figure, and **not** a speedup or savings
claim. It is a pair of labelled scenario envelopes per point.

## Scenario envelopes per token (L1 counts x labelled published points)

Each point reports two transmitter models. `dml_laser` includes the laser term and excludes the modulator.
`modulator_eo` includes the modulator and excludes the laser. Thermal stabilization is included in both (the
registered modulator points are EO switching energy, not athermal EO compensation). Input and weight DAC
contributions in any one scenario share the same published DAC source. `n_scenarios` is 288 for each transmitter
at every point.

Across the 40-point grid, the stored envelopes span:

| Transmitter | Excluded term | n_scenarios | min_J range (across 40 points) | max_J range (across 40 points) | Overall span (min of mins to max of maxes) |
| --- | --- | --- | --- | --- | --- |
| dml_laser | modulator | 288 | 0.3246 µJ – 13.11 µJ | 0.6824 µJ – 26.73 µJ | 0.3246 µJ – 26.73 µJ |
| modulator_eo | laser | 288 | 0.3244 µJ – 13.11 µJ | 0.6821 µJ – 26.73 µJ | 0.3244 µJ – 26.73 µJ |

The lowest envelopes are at tile 128 and 128 positions; the highest are at tile 8 and 1 position. Values in the
table are rounded from `results.json`.

The picks that realize the min and max are the same at all 40 points:

- Minimum scenario: ADC `lagos2021`; both DACs `sedighi2012`; host transfer `li2018` at 1 byte/code; memory
  `oconnor2017` weights-resident at 1 byte/element; optical detection floor at 4 effective bits; plus
  `sun2016` control, `zortman2012` thermal and `harris2014` weight hold. The `dml_laser` min also picks
  `diamantopoulos2021` at 25 °C; the `modulator_eo` min picks `timurdogan2014` per NRZ bit.
- Maximum scenario: ADC `ramkaj2020`; both DACs `palmers2010`; host transfer `li2018` at 2 bytes/code; memory
  `oconnor2017` weights-streamed at 2 bytes/element; optical detection floor at 8 effective bits; same control,
  thermal and weight-hold sources. The `dml_laser` max also picks `diamantopoulos2021` at 50 °C; the
  `modulator_eo` max picks `timurdogan2014` per full-swing transition.

Source labels: the per-token counts are measured by us under the E-024 counting rules on stored simulator
counts; each energy point is the published design already used by E-026 through E-035 (peer-reviewed measured
or estimate, as labelled on that term). Source types are not mixed inside one scenario sum beyond what those
term rows already report.

## What this is not

Zero of eleven terms stay "not sourced". The registration reports structured envelopes, not a single summed
energy per token and not a comparison against an electronic baseline. No energy per MAC, speedup, savings or
hardware claim follows from this run.
