# AWEN-Photon energy-per-token accounting with memory-movement bytes counted (E-029): pre-registration

Registered 2026-11-05 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-025 to E-028 left the memory-movement term "not sourced" because the assumption set has no parameter for it and
no memory model says where weights and activations live. This registration adds a stated memory model for the
lowered GEMMs only, with two placements reported separately, and counts host-memory bytes per token for the stored
E-024 counts at 1 and 2 bytes per element. Does the term stay "not sourced" for energy, and how much of the count
is weight traffic that disappears when weights stay resident on the device?

## Disclosure

- The E-024 counts (#223) and the E-025 to E-028 results (#227, #231, #235, #239) were seen before this
  registration. Memory movement was chosen next because it was the remaining counted-but-uncounted term named in
  STATUS.md; no memory energy source was checked this session.
- No parameter is added to `awen-spec/hardware_assumptions.yaml` and no energy per byte is filled, so the energy
  contribution stays "not sourced" and there is still no total.
- During development the accounting code ran on the E-024 file only to confirm that the E-025 to E-028
  registrations still reproduce their stored outputs byte for byte; no E-029 output was stored or reported.
  Otherwise the code ran only in unit tests on synthetic counts.

## Memory model (counting rule)

Scope: the lowered GEMMs that E-024 counts. Host-side ops (attention softmax, norms, embeddings, KV cache, the
graph's host nodes), accumulator re-reads of partial sums, caches and the simulator's float64 NumPy buffers are not
counted.

| Placement | Read from host memory per token | Written to host memory per token |
|---|---|---|
| `weights_streamed` | input DAC elements + weight DAC elements (weights read once per compiled call, as E-024 programs them) | ADC reads |
| `weights_resident` | input DAC elements (weights held on the device after programming, never re-read) | ADC reads |

Bytes per token = (read + write elements) x bytes per element, for 1 byte and 2 bytes, reported separately. Under
`weights_streamed` the element count equals E-028's host-transfer code count by construction (same rule, different
term: memory reads and writes, not link crossings). Neither placement is chosen as the AWEN default; the weight
hold cost of `weights_resident` belongs to the `weight_hold` term, which stays "not sourced" (power, no time model).

## Terms and rules

Identical to E-028 (`research/experiments/awen-photon-host-transfer-bytes/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| memory_movement | per placement, read and write elements | none | bytes = elements x 1 or 2 bytes per placement; status "not sourced" (no memory energy-per-byte parameter) |

`awen_models.energy_accounting.TERMS_E029`; accounting version `awen.energy-per-token-accounting.v5`. The `e025`
(default) to `e028` registrations are unchanged and reproduce their stored outputs.

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e029 --counts results/conversions-per-token/results.json --out results/energy-per-token-memory-movement/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-028, plus: at every point the memory-movement term lists exactly the two registered placements, each with
read and write elements recomputed from the point's per-token counts and bytes per token equal to elements x 1 and
x 2, no other fields, and status "not sourced". **Pass** when all of this holds at all 40 points; otherwise
**fail** with the failing points listed.

Expected: pass, still 6 of 11 terms not sourced (memory movement stays not sourced for energy), so no total. Byte
counts are L1 counts under a stated memory model. No energy, total, speedup, savings or hardware claim follows.
