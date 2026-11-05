# AWEN-Photon energy-per-token accounting with published HBM2 memory energy (E-030): pre-registration

Registered 2026-11-05 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-029 counted memory-movement bytes per token under two placements but left the energy "not sourced" because the
assumption set had no memory energy. With one peer-reviewed HBM2 access-energy point now in
`memory.published_designs` (O'Connor et al., MICRO-50 2017, doi:10.1145/3123939.3124545, Section 1 / Fig. 1b:
approximately 3.97 pJ/bit), what are the per-token memory-movement contributions at each placement and byte width
for the stored E-024 counts, and which terms stay not sourced?

## Disclosure

- The E-024 counts (#223) and the E-025 to E-029 results were seen before this registration. Memory energy was
  chosen next because STATUS next-task 1 asked for a checked, sourced memory or host-link energy per byte, and a
  verified in-repo citation already existed in `awen-systems` for HBM2.
- Primary text was re-checked on 2026-11-05 from the NVIDIA open PDF of the MICRO-50 paper; the quote matches the
  existing `awen-systems` citation. Host-link (PCIe/CXL) candidates checked the same day did not fit a host-only
  per-byte term (PHY TX-only ~1.93 pJ/bit; SerDes ~11.4 pJ/bit; a system measurement ~167.9 pJ/bit mixes host and
  device), so `system.host_transfer_energy` stays null.
- During development the accounting code ran on the E-024 file only to confirm that the E-025 to E-029
  registrations still reproduce their stored outputs under the prior assumption SHA where applicable, and that
  e029 still reports memory movement as not sourced; no E-030 output was stored or reported. Otherwise the code
  ran only in unit tests on synthetic counts.

## Terms and rules

Identical to E-029 (`research/experiments/awen-photon-memory-movement-bytes/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| memory_movement | per placement, read and write elements | `memory.published_designs` | bytes as in E-029; energy = bytes x 8 bits/byte x energy_per_bit_pJ per published point, reported per placement and per byte width; never interpolated |

`awen_models.energy_accounting.TERMS_E030`; accounting version `awen.energy-per-token-accounting.v6`. The `e025`
(default) to `e029` registrations are unchanged: e029 still does not apply memory energy.

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e030 --counts results/conversions-per-token/results.json --out results/energy-per-token-memory-energy/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-029, plus: at every point the memory-movement term is sourced from each published design, with
per-token joules equal to bytes x 8 x energy_per_bit for each placement and each of 1 and 2 bytes per element, and
`check()` rejects a contribution that does not recompute or a not-sourced row that still carries designs. **Pass**
when all of this holds at all 40 points; otherwise **fail** with the failing points listed.

Expected: pass, 5 of 11 terms not sourced (laser, weight hold, control electronics, thermal stabilization, host
transfer), so still no total. Memory figures are L1 counts times a peer-reviewed HBM2 access energy (labelled). No
speedup, savings or hardware claim follows.
