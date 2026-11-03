# AWEN-Photon energy-per-token accounting with published DAC designs (E-026): pre-registration

Registered 2026-11-03 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-025 (#227) left the input and weight DAC terms "not sourced" because `dac.energy_per_conversion` had no checked value.
With three measured DAC designs now in the assumption set (`dac.published_designs`), what are the per-token DAC
contributions for the stored E-024 counts, per design, and which terms remain "not sourced"?

## Disclosure

- The E-024 counts (#223) and the E-025 result (#227) were seen before this registration. The weight DAC is the
  largest E-024 count, which is why the DAC term was chosen first.
- The three DAC designs were chosen before any E-026 output existed, from a Crossref search for high-speed DACs whose
  power and sample rate are stated in the published title or abstract of a measured chip:
  `sedighi2012` (8 bit, 5 GS/s, 26 mW, 0.13 um SiGe BiCMOS), `palmers2010` (10 bit, 1.6 GS/s, 27 mW, 130 nm CMOS) and
  `kim2016` (6 bit, 3.1 GS/s, 17.7 mW, 90 nm CMOS). Each energy per conversion is power / sample rate (estimate,
  formula in the assumption set). They are standalone DACs, not modulator or phase-shifter drivers.
- During development the E-026 command was smoke-run once on the E-024 file to a scratch path, and the output was
  deleted unread. Otherwise the code ran only in its unit tests on one synthetic point.

## Terms and rules

Identical to E-025 (`research/experiments/awen-photon-energy-accounting/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| dac_input | input DAC conversions | `dac.published_designs` | count x each published design, never interpolated |
| dac_weight | weight DAC conversions | `dac.published_designs` | count x each published design, never interpolated |

`awen_models.energy_accounting.TERMS_E026`; accounting version `awen.energy-per-token-accounting.v2`. The default
registration (`e025`) is unchanged and still reproduces the stored E-025 file byte for byte.

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e026 --counts results/conversions-per-token/results.json --out results/energy-per-token-dac/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-025: **pass** when, at every point, every required term is listed, every reported contribution
recomputes to count x value x unit, no not-sourced term carries a contribution, and no total is emitted while any
term is not sourced. In addition, both DAC terms must carry one contribution per published design. Otherwise it
**fails** and the failing points are listed.

Expected: pass with 7 of 11 terms still not sourced (laser, modulator, weight hold, control electronics, thermal
stabilization, memory movement, host transfer), so still no total. Every figure is a partial, labelled term (L1
counts times an AWEN estimate from a measured standalone DAC). No total, speedup, savings or hardware claim follows.
