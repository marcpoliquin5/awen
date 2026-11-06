# AWEN-Photon energy-per-token accounting with published heater thermal energy-per-bit (E-033): pre-registration

Registered 2026-11-06 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-032 left the thermal-stabilization term "not sourced" because `system.thermal_stabilization_power` is null
(and a power term needs a time model). With a published zortman2012 silicon microdisk heater energy-per-bit
point now in the assumption set (`thermal.published_designs`, 180 fJ/bit), what is the per-token thermal
contribution for the stored E-024 counts under a registered bit-per-conversion model, and which terms stay not
sourced?

## Disclosure

- The E-024 counts and the E-025 to E-032 results were seen before this registration. Thermal was chosen next
  because STATUS next-task 1 asked for control or thermal, and a heater energy-per-bit figure (paper estimate
  from measured efficiency) was preferred over a bare continuous Watt that still needs an invented time model.
  Control-ASIC candidates (e.g. Sun et al. JSSC 2016 0.74 mW tuner logic at 5 Gb/s) were set aside for a later
  slice so this registration stays one concern.
- Primary text was checked on 2026-11-06 from the Sandia open PDF of Zortman et al., OFC/NFOEC 2012 paper
  OW4I.5 (doi:10.1364/ofc.2012.ow4i.5) and Crossref. Measured heater efficiency 7 µW/GHz (stable across ~5–6 nm;
  heater voltage <1 V). Paper estimates 180 fJ/bit for realistic operation at 10 Gbps assuming 80 GHz
  manufacturing variation and a 40 °C environmental swing (~10 GHz/°C), noting that Gaussian variation implies
  about half the span on average. Fig. 3 mid column (535 mV × 3.4 mA) is consistent (~182 fJ/bit at 10 Gbps).
  Heater only; closed-loop control ASIC and package TEC are not included. Modulator switching energy is
  separate (and already covered under `modulator.published_designs` from timurdogan2014 without its athermal
  EO tuning).
- The registered energy model counts **one heater energy-per-bit contribution per input DAC conversion**
  (E-024 `dac_input`) under a binary optical-input proxy. Multi-level DAC codes are not expanded to bits. This
  is resonant-device on-chip heater trimming, **not** a package TEC continuous power and **not** control-loop
  logic energy; joules from this term must not be added to `modulator.published_designs` athermal EO
  compensation in one architecture total without choosing one stabilization model.
- During development the accounting code ran on the E-024 file only to confirm that the E-025 to E-032
  registrations still reproduce their sourced/not-sourced structure under the updated assumption file (e032 still
  leaves thermal as a power term); no E-033 output was stored or reported. Otherwise the code ran only in unit
  tests on synthetic counts.

## Terms and rules

Identical to E-032 (`research/experiments/awen-photon-laser-energy/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| thermal_stabilization | input DAC conversions | `thermal.published_designs` | count × each published point (fJ per bit), never interpolated |

`awen_models.energy_accounting.TERMS_E033`; accounting version `awen.energy-per-token-accounting.v9`. The `e025`
(default) to `e032` registrations are unchanged: e032 still treats thermal_stabilization as a power term (not
sourced).

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e033 --counts results/conversions-per-token/results.json --out results/energy-per-token-thermal/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-032, plus: at every point the thermal_stabilization term is sourced from each published design, with
per-token joules equal to `dac_input` × energy_per_bit, and `check()` rejects a contribution that does not
recompute or a not-sourced row that still carries designs. **Pass** when all of this holds at all 40 points;
otherwise **fail** with the failing points listed.

Expected: pass, 2 of 11 terms not sourced (control electronics, host transfer), so still no total. Thermal
figures are L1 counts times a peer-reviewed heater energy/bit estimate (labelled). No speedup, savings or
hardware claim follows.
