# AWEN-Photon energy-per-token accounting with published DML laser energy-per-bit (E-032): pre-registration

Registered 2026-11-06 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-031 left the laser term "not sourced" because `laser.wall_plug_efficiency` was null (and a power term needs a
time model). With measured diamantopoulos2021 DML DC energy-per-bit points now in the assumption set
(`laser.published_designs`, 0.17 pJ/bit at 25 °C and 0.34 pJ/bit at 50 °C), what is the per-token laser
contribution for the stored E-024 counts under a registered bit-per-conversion model, and which terms stay not
sourced?

## Disclosure

- The E-024 counts and the E-025 to E-031 results were seen before this registration. Laser was chosen next
  because STATUS next-task 1 asked for laser / control / thermal, and a laser-link-only energy-per-bit figure
  was preferred over a full-system SoA that mixes terms (PACE including-vs-excluding lasers was rejected).
- Primary text was checked on 2026-11-06 from the MDPI open-access PDF (doi:10.3390/photonics8020031) and
  Crossref. Measured at the 60 GBaud PAM-4 / KP4-FEC operating points: DC 19.565 mW at 25 °C (9.1 mA, 2.15 V)
  and 38.656 mW at 50 °C (15.1 mA, 2.56 V); net rate 113.42 Gbps; paper estimates DC energy/bit 0.17 pJ/bit and
  0.34 pJ/bit. Wall-plug efficiency 6.44% at 25 °C (wide-apparatus photodetection without fiber). RF driver
  energy is excluded.
- The registered energy model counts **one DML DC energy-per-bit contribution per input DAC conversion**
  (E-024 `dac_input`) under a binary optical-input proxy. Multi-level DAC codes are not expanded to bits. This
  is an interconnect-style DML (laser and modulation are the same device), **not** a CW external-laser power for
  an external-modulator photonic MAC; joules from this term must not be added to `modulator.published_designs`
  in one architecture total without choosing one transmitter model.
- During development the accounting code ran on the E-024 file only to confirm that the E-025 to E-031
  registrations still reproduce their sourced/not-sourced structure under the updated assumption file (e031 still
  leaves laser as a power term); no E-032 output was stored or reported. Otherwise the code ran only in unit
  tests on synthetic counts.

## Terms and rules

Identical to E-031 (`research/experiments/awen-photon-weight-hold-energy/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| laser | input DAC conversions | `laser.published_designs` | count × each published point (pJ per bit), never interpolated |

`awen_models.energy_accounting.TERMS_E032`; accounting version `awen.energy-per-token-accounting.v8`. The `e025`
(default) to `e031` registrations are unchanged: e031 still treats laser as a power term (not sourced).

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e032 --counts results/conversions-per-token/results.json --out results/energy-per-token-laser/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-031, plus: at every point the laser term is sourced from each published design, with per-token joules
equal to `dac_input` × energy_per_bit, and `check()` rejects a contribution that does not recompute or a
not-sourced row that still carries designs. **Pass** when all of this holds at all 40 points; otherwise **fail**
with the failing points listed.

Expected: pass, 3 of 11 terms not sourced (control electronics, thermal stabilization, host transfer), so still
no total. Laser figures are L1 counts times a peer-reviewed DML DC energy/bit estimate (labelled). No speedup,
savings or hardware claim follows.
