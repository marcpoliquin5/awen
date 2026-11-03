# AWEN-Photon conversions per token (E-024): pre-registration

Registered 2026-11-02 (commit of this file) under AWEN Master Prompt v2. Status: **registered 2026-11-02; run 2026-11-03 at `d6dfa80` without change** (result: `awen-models/results/conversions-per-token/`; verdict: pass, all 40 points equal the closed form).
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

How many DAC and ADC conversions per token does the AWEN simulator path need for the tiny random-init Llama (E-020)
and Qwen3 (E-021) models through `torch.compile(backend=awen)`, as a function of the simulator tile size and the
sequence length, and does the capability report's count agree exactly with the closed form derived from the model
config?

## Disclosure

- E-020 and E-021 (`awen-models/results/compile-capability/`, stored results) already report the conversion totals
  at one point (tile size 64, 32 positions): DAC input 34,816, DAC weight 90,112, ADC 45,056 for both models. Those
  numbers were seen before this registration; one unit test checks the closed form against them.
- Before registration the curve code ran only in its unit tests (`awen-models/tests/test_conversions_curve.py`), on
  grids of at most 2 positions and 2 tile sizes.

## Definitions (counting rule of the simulator, not of any device)

- Lowered nodes: every `linear` node (q, k, v, o, gate, up, down per layer, and the untied output head). Attention
  cores, norms, embeddings and elementwise nodes stay on the host and make no conversions in this count.
- Per lowered GEMM with `rows` token rows, `inner` input width and `outputs` output width: input DAC conversions
  `rows * inner`; weight DAC conversions `inner * outputs`, once per compiled call; ADC reads
  `rows * outputs * ceil(inner / tile_k)` (each tile's partial sum is read and accumulated on the host).
- One call processes all `P` positions of one sequence with no KV cache (prefill). **Per token** means the call's
  total divided by `P`. Weight conversions per token therefore fall as `1/P`; input DAC and ADC per token do not
  depend on `P`. Decode with a KV cache is not covered.

## Design

- Architectures: `llama`, `qwen3` (`awen.compile_llama`, seeded random weights, built locally, nothing downloaded).
- Tile sizes `tile_k`: 8, 16, 32, 64, 128. Positions `P`: 1, 8, 32, 128. Full grid, 40 points, no point dropped.
- Ideal (noise-free) contract; counts do not depend on noise or seed.
- Command (from `awen-models/`, CPU):
  `python -m awen.conversions_curve --out results/conversions-per-token/results.json`
  Output: `results/conversions-per-token/results.json`, stored with its SHA-256, commit and environment.

## Verdict rule

Primary: the curve **passes** when, at every grid point, the report's DAC input, DAC weight, ADC and lowered-MAC
totals equal `expected_counts` exactly. Any mismatch is a **fail** and is reported with the differing point; the
counting code is then fixed in a separate PR and the run repeated as a new registration.

Reported, no test: per-token counts and conversions per lowered MAC at each point.

All numbers are simulation counts (L1). No energy, latency, speedup or hardware claim follows; energy per MAC needs
the full system accounting in `awen-spec/hardware_assumptions.yaml` and is the next slice.
