# AWEN-Photon energy-per-token accounting with published thermo-optic weight-hold energy (E-031): pre-registration

Registered 2026-11-06 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-030 left the weight-hold term "not sourced" because `phase_shifter.thermo_optic_power_per_pi` was a power
term with no registered time model (and its value had not yet been read from the paper). With measured
harris2014 values now in the assumption set, and one published π-reconfiguration energy point
(`phase_shifter.published_designs`, Table I Pπ·τ = 66.9 pJ), what is the per-token weight-hold contribution
for the stored E-024 counts under a registered reconfiguration model, and which terms stay not sourced?

## Disclosure

- The E-024 counts and the E-025 to E-030 results were seen before this registration. Weight hold was chosen
  next because STATUS next-task 1 asked for laser / weight-hold / control / thermal, and harris2014 was already
  cited in the assumption set with null values marked "citation checked; value not yet read from the paper".
- Primary text was checked on 2026-11-06 from arXiv:1410.3616 (OSA electronic reprint of Optics Express 22,
  10487-10493, doi:10.1364/OE.22.010487). Measured: Pπ = 24.77 ± 0.43 mW; -3 dB bandwidth = 130.0 ± 5.59 kHz.
  Table I reports τ = 2.69 µs via the paper's single-pole approximation τ = 0.35/f_3dB, and Pπ·τ = 66.9 mW·µs.
- The registered energy model counts **one full-π thermal reconfiguration energy per weight DAC conversion**
  (E-024 `dac_weight`). Continuous hold power during compute is **not** included (that would need a separate
  hold-duration model). Actual programmed phase deltas are not all π; this is a labelled upper-bound-style proxy
  for a programming transient, not a claim about steady-state mesh power.
- During development the accounting code ran on the E-024 file only to confirm that the E-025 to E-030
  registrations still reproduce their sourced/not-sourced structure under the updated assumption file (e030 still
  leaves weight_hold as a power term); no E-031 output was stored or reported. Otherwise the code ran only in
  unit tests on synthetic counts.

## Terms and rules

Identical to E-030 (`research/experiments/awen-photon-memory-energy/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| weight_hold | weight DAC conversions | `phase_shifter.published_designs` | count × each published point (pJ per π reconfiguration), never interpolated |

`awen_models.energy_accounting.TERMS_E031`; accounting version `awen.energy-per-token-accounting.v7`. The `e025`
(default) to `e030` registrations are unchanged: e030 still treats weight_hold as a power term (not sourced).

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e031 --counts results/conversions-per-token/results.json --out results/energy-per-token-weight-hold/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-030, plus: at every point the weight-hold term is sourced from each published design, with
per-token joules equal to `dac_weight` × energy_per_pi, and `check()` rejects a contribution that does not
recompute or a not-sourced row that still carries designs. **Pass** when all of this holds at all 40 points;
otherwise **fail** with the failing points listed.

Expected: pass, 4 of 11 terms not sourced (laser, control electronics, thermal stabilization, host transfer),
so still no total. Weight-hold figures are L1 counts times a peer-reviewed Pπ·τ estimate (labelled). No
speedup, savings or hardware claim follows.
