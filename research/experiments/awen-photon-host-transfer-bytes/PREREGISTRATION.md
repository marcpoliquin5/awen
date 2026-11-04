# AWEN-Photon energy-per-token accounting with host-transfer bytes counted (E-028): pre-registration

Registered 2026-11-04 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-025 to E-027 left the host-transfer term "not sourced" for two reasons: the bytes moved per token were not
counted, and `system.host_transfer_energy` has no checked value. This registration removes the first reason only.
Under the simulator's counting rule, how many bytes cross the host boundary per token for the stored E-024 counts,
at 1 and 2 bytes per code, and does the term stay "not sourced" for energy?

## Disclosure

- The E-024 counts (#223) and the E-025, E-026 and E-027 results (#227, #231, #235) were seen before this
  registration. Host transfer was chosen next because its byte count follows from counts E-024 already reports;
  memory movement needs a memory model (where weights and activations live), which is not registered.
- No new source is added and no energy per byte is filled: no peer-reviewed or measured host-link energy was checked
  this session, so `system.host_transfer_energy` stays null and the energy contribution stays "not sourced".
- During development the accounting code ran on the E-024 file only to confirm that the E-025, E-026 and E-027
  registrations still reproduce their stored outputs byte for byte; no E-028 output was stored or reported.
  Otherwise the code ran only in unit tests on synthetic counts.

## Byte-counting rule

Every input DAC code, weight DAC code and ADC read crosses the host boundary exactly once per compiled call
(the simulator programs weights once per call and accumulates partial sums on the host, E-024's counting rule).
Codes per token = input DAC + weight DAC + ADC conversions per token (the E-024 `per_token.total`). Bytes per token =
codes per token x bytes per code, for 1 byte (codes of up to 8 bits) and 2 bytes (9 to 16 bits), reported separately.
No headers, framing, compression, protocol overhead or host-side copies are counted. This is a counting rule about
the simulator's partitioning, not a measurement of any link or device.

## Terms and rules

Identical to E-027 (`research/experiments/awen-photon-energy-accounting-modulator/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| host_transfer | input DAC + weight DAC + ADC codes | `system.host_transfer_energy` | bytes = codes x 1 or 2 bytes; energy = bytes x pJ/byte only if the value has a source, else "not sourced" |

`awen_models.energy_accounting.TERMS_E028`; accounting version `awen.energy-per-token-accounting.v4`. The `e025`
(default), `e026` and `e027` registrations are unchanged and reproduce their stored outputs.

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e028 --counts results/conversions-per-token/results.json --out results/energy-per-token-host-transfer/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-027, plus: at every point the host-transfer term lists its codes per token and its bytes per token at
1 and 2 bytes per code, each equal to codes x width, and it carries no energy contribution while
`system.host_transfer_energy` is null. **Pass** when all of this holds at all 40 points; otherwise **fail** with the
failing points listed.

Expected: pass, still 6 of 11 terms not sourced (host transfer stays not sourced for energy), so no total. Byte
counts are L1 counts under a stated packing rule. No energy, total, speedup, savings or hardware claim follows.
