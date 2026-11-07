# AWEN-Photon energy-per-token accounting with published microring tuner-logic energy-per-bit (E-034): pre-registration

Registered 2026-11-07 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-033 left the control-electronics term "not sourced" because `system.control_electronics_power` is null
(and a power term needs a time model). With one published closed-loop microring tuner-logic energy-per-bit
point now in the assumption set (`control.published_designs`, sun2016, 148 fJ/bit), what is the per-token
control contribution for the stored E-024 counts under a registered bit-per-conversion model, and which terms
stay not sourced?

## Disclosure

- The E-024 counts and the E-025 to E-033 results were seen before this registration. Control electronics was
  chosen because STATUS next-task 1 named it; host-transfer energy still has no fitted host-link-only source.
- Source checked on 2026-11-07: Sun et al., IEEE JSSC 51(4), 893-907 (2016), doi:10.1109/JSSC.2016.2519390,
  metadata on Crossref and the published abstract (full text is closed access; OpenAlex reports no open copy).
  The abstract states that the bit-statistical tuner "consumes 0.74 mW in the logic", with 3.8 µW/GHz heater
  efficiency, and that it integrates with a 5 Gb/s 30 fJ/bit monolithic microring transmitter.
- The registered point is an **estimate**: 0.74 mW / 5 Gb/s = 148 fJ/bit. It assumes the logic power is spent
  at the 5 Gb/s transmitter rate the abstract names. Cross-check (not used as a value): the first author's open
  MIT PhD thesis (C. Sun, *Silicon-Photonics for VLSI Systems*, MIT 2015, hdl:1721.1/99784, read in full on
  2026-11-07), Fig. 6-16, breaks down the 5 Gb/s link per bit: tuner backend 152 fJ/bit, tuner frontend
  (integrator, SAR ADC) 39 fJ/bit, heater 290 fJ/bit, modulator 30 fJ/bit, receiver 374 fJ/bit. 152 fJ/bit at
  5 Gb/s is about 0.76 mW (our arithmetic), close to 0.74 mW, so the JSSC logic figure likely excludes the
  39 fJ/bit frontend; this term may therefore be low by about that much. The VLSI 2015 conference abstract
  (doi:10.1109/VLSIC.2015.7231348) gives 275 fJ/bit for "the thermal tuner" with no breakdown; not used.
- The registered energy model counts **one tuner-logic energy-per-bit contribution per input DAC conversion**
  (E-024 `dac_input`) under the same binary optical-input proxy as E-032 and E-033. This is per-ring
  wavelength-lock control logic only. It is **not** a whole-accelerator control-electronics power: system
  controller, sequencer, clock distribution, DAC/ADC drivers and host interface are not included, and the
  heater is the separate thermal term (E-033, zortman2012). The Sun heater (290 fJ/bit in the thesis) is not
  used, so heater energy is not counted twice.
- During development the accounting code ran only in unit tests on synthetic counts and in a structural check
  (e033 still leaves control as a power term; e034 leaves only host transfer not sourced). No E-034 output was
  stored or reported.

## Terms and rules

Identical to E-033 (`research/experiments/awen-photon-thermal-energy/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| control_electronics | input DAC conversions | `control.published_designs` | count × each published point (fJ per bit), never interpolated |

`awen_models.energy_accounting.TERMS_E034`; accounting version `awen.energy-per-token-accounting.v10`. The `e025`
(default) to `e033` registrations are unchanged: e033 still treats control_electronics as a power term (not
sourced).

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e034 --counts results/conversions-per-token/results.json --out results/energy-per-token-control/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-033, plus: at every point the control_electronics term is sourced from each published design, with
per-token joules equal to `dac_input` × energy_per_bit, and `check()` rejects a contribution that does not
recompute or a not-sourced row that still carries designs. **Pass** when all of this holds at all 40 points;
otherwise **fail** with the failing points listed.

Expected: pass, 1 of 11 terms not sourced (host transfer), so still no total. Control figures are L1 counts
times a labelled estimate from a peer-reviewed measured logic power. No speedup, savings or hardware claim
follows.
