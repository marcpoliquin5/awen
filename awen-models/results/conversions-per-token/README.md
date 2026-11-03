# E-024: DAC and ADC conversions per token, tiny Llama and Qwen3 (measured by us, simulator counts, L1)

Registered run of `research/experiments/awen-photon-conversions/PREREGISTRATION.md` (#221, `9063aa9`, D-025), run
unchanged.

Command (from `awen-models/`, at commit `d6dfa80`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-03 00:40 ET, exit 0, 28 s wall:

    PYTHONPATH=/workspace/e024-native:../awen-ecosystem/python_awen python -m awen.conversions_curve --out results/conversions-per-token/results.json

`PYTHONPATH` points at `awen_native` built from `awen-native/` at this commit (`maturin build --release`) and at
this commit's `awen_py`, so no older install on the build box is used. An earlier run with the box's older
`awen_native` build gave a byte-identical `results.json`. Output: `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule: report totals equal the closed form at every grid point)

**Pass.** All 40 points (2 architectures x 5 tile sizes x 4 position counts, none dropped) have DAC input, DAC
weight, ADC and lowered-MAC totals equal to `expected_counts` exactly. The tile-64, 32-position point reproduces the
stored E-020/E-021 totals (DAC input 34,816, DAC weight 90,112, ADC 45,056).

## Conversions per token (reported, no test)

The two tiny configs have the same projection shapes, so Llama and Qwen3 give identical counts at every point.
Per token is the one-call total over `P` positions divided by `P` (prefill, no KV cache). Input DAC per token is
1,088 at every point; ADC per token depends only on the tile size; weight DAC per token is 90,112 / `P`.

| Tile size | ADC per token | Total per token, P = 1 | P = 8 | P = 32 | P = 128 | Conversions per lowered MAC, P = 128 |
| --- | --- | --- | --- | --- | --- | --- |
| 8 | 11,264 | 102,464 | 23,616 | 15,168 | 13,056 | 0.1449 |
| 16 | 5,632 | 96,832 | 17,984 | 9,536 | 7,424 | 0.0824 |
| 32 | 2,816 | 94,016 | 15,168 | 6,720 | 4,608 | 0.0511 |
| 64 | 1,408 | 92,608 | 13,760 | 5,312 | 3,200 | 0.0355 |
| 128 | 1,280 | 92,480 | 13,632 | 5,184 | 3,072 | 0.0341 |

Lowered MACs per token are 90,112 at every point. At one position, weight programming dominates (90,112 of at
least 92,480 conversions); by 128 positions it is 704 per token and ADC reads dominate at small tile sizes. Going
from tile 64 to 128 saves little ADC (1,408 to 1,280) because only the MLP down projection (inner width 128) splits into more than one tile at tile size 64.

## Scope

Counts under the simulator's counting rule for random-init tiny models, not a statement about any device. No
energy, latency or speedup figure follows; energy per MAC needs the full system accounting in
`awen-spec/hardware_assumptions.yaml` and is the next slice. Decode with a KV cache and real-weight models are not
covered.
