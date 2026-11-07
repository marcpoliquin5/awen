# AWEN-Photon energy-per-token scenario-envelope total (E-036): pre-registration

Registered 2026-11-07 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-035 sourced all 11 system terms from labelled published points but still reports
`total_J = "not defined by this registration"` (no numeric total). Under a rule that respects the laser
caveat (do not sum `laser.published_designs` with `modulator.published_designs` in one architecture total)
and the thermal caveat (heater energy may be included with modulator EO energy, because the registered
modulator points are EO switching energy, not athermal EO compensation), what are the min and max labelled
sums of those published points for each transmitter model, for the stored E-024 counts?

## Disclosure

- The E-024 counts and the E-025 to E-035 results were seen before this registration. E-036 was chosen
  because STATUS next-task 1 asked for a total-defining registration after E-035 sourced every term.
- No new device points are added. The totals, if this registration is later run, will use the existing
  published points already in `awen-spec/hardware_assumptions.yaml`: diamantopoulos2021 (laser),
  timurdogan2014 (modulator), sedighi2012 / palmers2010 / kim2016 (DAC), harris2014 (weight hold),
  lagos2021 / ramkaj2020 (ADC), the shot-noise detection-floor sweep, sun2016 (control), zortman2012
  (thermal), oconnor2017 (memory), li2018 (host transfer).
- The total is **not** one physical device, **not** a bare float, and **not** a speedup or savings figure.
  It is a pair of scenario envelopes: sums of labelled published points within one transmitter model.
- Laser caveat (D-033 / E-032): a DML interconnect-style laser point must not be summed with
  `modulator.published_designs` in one architecture total. Hence two transmitter models:
  `dml_laser` includes laser and excludes modulator; `modulator_eo` includes modulator and excludes laser.
- Thermal caveat (D-034 / E-033): heater energy must not be summed with modulator *athermal EO
  compensation*. The registered modulator points are EO switching energy, not athermal compensation, so
  thermal stabilization **is** included in both transmitter models.
- DAC points in each scenario share the same published source id for `dac_input` and `dac_weight`
  (no mixing sedighi2012 input with palmers2010 weights, and so on).
- e025–e035 stay reproducible as their registrations: e035 still emits the string
  `"not defined by this registration"` when every term is sourced; earlier registrations still emit
  `None` while any term is not sourced.
- During development the accounting code ran only in unit tests on synthetic counts. No E-036 output was
  stored or reported.

## Terms and rules

Identical to E-035 (`research/experiments/awen-photon-host-transfer-energy/PREREGISTRATION.md`) for the
eleven terms (`TERMS_E036 = TERMS_E035`). Accounting version `awen.energy-per-token-accounting.v12`.
The `e025` (default) to `e035` registrations are unchanged.

When `not_sourced` is non-empty: `total_J` is `None` and the reason names the missing terms (same as today).

When `not_sourced` is empty under e036 only, `total_J` is a structured object:

- `rule`: `scenario_envelope_v1`
- `unit`: `J`
- `disclosure`: sums of labelled published points within one transmitter model; not one physical device;
  no speedup/savings claim
- `by_transmitter.dml_laser` / `by_transmitter.modulator_eo`: each has `excluded_terms`, `min_J`, `max_J`,
  `n_scenarios`, and the picks that realize the min and max

Each scenario is one labelled sum over the cartesian product of:

- transmitter model: `dml_laser` or `modulator_eo`
- DAC source: one published DAC design, the same source id for both input and weight DACs
- ADC: each published ADC design
- optical detection floor: each swept effective-bits key
- weight hold: each published phase-shifter design
- control electronics: each published control design
- thermal stabilization: each published thermal design (included in **both** transmitter models)
- memory movement: each (placement × byte width) from the term's `per_token_J_by_placement`
- host transfer: each packing width from `per_token_J_by_width`
- laser (`dml_laser` only): each published laser design/basis
- modulator (`modulator_eo` only): each published modulator design/basis

`min_J` / `max_J` are the min and max of those scenario sums. `n_scenarios` is the count of scenarios
for that transmitter. Contributions are read from the already-built term rows so they stay consistent
with `check()`.

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e036 --counts results/conversions-per-token/results.json --out results/energy-per-token-total/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same term-level checks as E-035, plus: while any term is not sourced, a numeric or structured total is
rejected; when `total_J` is the structured object, `min_J <= max_J`, both finite, `n_scenarios >= 1`,
`excluded_terms` respected, and recomputing the envelope from the term rows matches the stored
`min_J` / `max_J` (tolerance 1e-18 relative or absolute). e025–e035 keep their current totals (`None`
or the string `"not defined by this registration"`). **Pass** when all of this holds at all 40 points;
otherwise **fail** with the failing points listed.

Expected: pass, 0 of 11 terms not sourced, structured envelopes under `scenario_envelope_v1`. Figures
are L1 counts times labelled published points. No speedup, savings or hardware claim follows. This PR
registers only; it does not run E-036.
