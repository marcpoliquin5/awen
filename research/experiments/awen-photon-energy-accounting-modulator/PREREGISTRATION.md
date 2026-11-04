# AWEN-Photon energy-per-token accounting with a published measured modulator (E-027): pre-registration

Registered 2026-11-04 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-026 (#231) left the modulator term "not sourced" because `modulator.energy_per_symbol` had no checked value. With
one measured, peer-reviewed silicon modulator now in the assumption set (`modulator.published_designs`), what is the
per-token modulator contribution for the stored E-024 counts, per published point, and which terms remain
"not sourced"?

## Disclosure

- The E-024 counts (#223), the E-025 result (#227) and the E-026 result (#231) were seen before this registration.
  The modulator was chosen next because it is driven once per input DAC conversion, a count E-024 already reports.
- The source was chosen before any E-027 output existed: `timurdogan2014` (Timurdogan et al., Nature Communications
  5, 4008, 2014, doi 10.1038/ncomms5008), checked on 2026-11-04 on Crossref and in the open-access article. It is a
  4.8 um vertical-junction silicon microdisk from a 65 nm CMOS foundry, driven with 25 Gb/s NRZ-OOK at 0.5 Vpp.
  The paper measures 3.65 fJ per 0-to-1 transition by time-domain reflectometry and states 0.91 fJ per bit
  (switching energy / 4 for equiprobable NRZ data). Both are stored as separate points; neither is interpolated.
- Not included: the driver, bias lock and monitor circuitry, and the paper's athermal tuning energy (0.24 fJ per bit
  at the compensated temperature, more over a 10 C range). It is binary on-off keying, not an analog multi-level
  photonic input symbol, so the term is a labelled lower-level proxy, not the energy of a MAC input.
- During development the accounting code ran on the E-024 file only to confirm that the E-026 registration still
  reproduces its stored points; no E-027 output was stored or reported. Otherwise the code ran only in unit tests.

## Terms and rules

Identical to E-026 (`research/experiments/awen-photon-energy-accounting-dac/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| modulator | input DAC conversions | `modulator.published_designs` | count x each published point (fJ), never interpolated |

`awen_models.energy_accounting.TERMS_E027`; accounting version `awen.energy-per-token-accounting.v3`. The `e025`
(default) and `e026` registrations are unchanged and give the same points as their stored runs.

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e027 --counts results/conversions-per-token/results.json --out results/energy-per-token-modulator/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-026: **pass** when, at every point, every required term is listed, every reported contribution recomputes
to count x value x unit, no not-sourced term carries a contribution, and no total is emitted while any term is not
sourced. In addition, the modulator term must carry one contribution per published point, each with its basis.
Otherwise it **fails** and the failing points are listed.

Expected: pass with 6 of 11 terms still not sourced (laser, weight hold, control electronics, thermal stabilization,
memory movement, host transfer), so still no total. Every figure is a partial, labelled term (L1 counts times a
measured standalone binary modulator, driver excluded). No total, speedup, savings or hardware claim follows.
