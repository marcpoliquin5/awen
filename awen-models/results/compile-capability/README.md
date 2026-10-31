# AWEN-Mini through `torch.compile(backend=awen)`: capability reports (P2-1)

Source label: **measured by us** (simulator output, not hardware), evidence level L1. No energy, latency or
speedup figure is produced; energy needs the full system accounting in `awen-spec/hardware_assumptions.yaml`.

Commit `44009a6` (branch `work/p2-1-compile-backend`), 2026-10-31 ET. Build box: Linux 6.12 x86_64, no GPU,
Python 3.12.15 venv from `requirements/cpu-py312.txt` (torch 2.14.1+cpu, numpy 2.5.4) with `awen_native` 0.1.0
installed. From `awen-models/`:

```
python -m awen.compile_capability --out results/compile-capability/run-44009a6.json
python -m awen.compile_capability --ideal --out results/compile-capability/run-44009a6-ideal.json
```

Input: the module's default text (48 characters, 48 positions), bundled `awen-mini` checkpoint, seed 0.

| File | SHA-256 |
| --- | --- |
| `run-44009a6.json` | `c10abaa0844cfa0e858e779b3eb3aab0f7f94e761bb4cb0ab527c04591e36d59` |
| `run-44009a6-ideal.json` | `e09db72393d4dc8343523c6a4849a1c5687d34781cdc5be4ef2a0e9920aaa9d6` |

Summary (one seed, one input; no confidence interval):

| Contract | logits max abs diff | rms diff | rel. L2 | top-1 agreement |
| --- | --- | --- | --- | --- |
| default (1000 photons/MAC, 8-bit DAC/ADC) | 15.94 | 3.949 | 0.8462 | 0.0208 |
| ideal | 1.43e-05 | 2.06e-06 | 4.4e-07 | 1.0000 |

Both: one FX graph (fullgraph), 21 nodes lowered to the Rust photonic GEMM (20 `AwenLinear` plus the tied output
head `logits`), 47,585,280 lowered MACs, 107 host fallback nodes (5 `scaled_dot_product_attention`, 11
`layer_norm`, 5 `gelu`, 2 `embedding`, the rest layout/indexing/add), 221,184 input DAC, 991,360 weight DAC and
743,520 ADC conversions (counting rule in the JSON).

Not comparable one-to-one with `../photonic-sim-capability/` (P1-5, Runtime path): this path also lowers the
output head, and torch-backend v1 reuses one seed for every lowered GEMM of a call (`seed_note` in the JSON).
Superseded as the CLI default by the P2-2 records below; kept unchanged for comparison.

## P2-2: per-GEMM deterministic seeds

Source label: **measured by us** (simulator output, not hardware), evidence level L1. Same box, venv and input as
above. Commit `a6db2f4` (branch `work/p2-2-per-gemm-seeds`), 2026-10-31 ET. From `awen-models/`:

```
python -m awen.compile_capability --out results/compile-capability/run-a6db2f4.json
python -m awen.compile_capability --ideal --out results/compile-capability/run-a6db2f4-ideal.json
python -m awen.compile_capability --seed-policy shared --out results/compile-capability/run-a6db2f4-shared.json
```

The CLI now defaults to `--seed-policy per_gemm`: lowered GEMM *i* (graph order) gets
`int.from_bytes(sha256(f"awen.torch-backend.gemm-seed.v1:{seed}:{i}")[:8], "big") >> 2`, recorded per row as
`lowered[i].seed`. The backend option `seed_policy` still defaults to `"shared"` (v1 behaviour).

| File | SHA-256 |
| --- | --- |
| `run-a6db2f4.json` | `7a4c39307c48d9b6f180a77e2f9f6327b1b089d0d47e2a92cf89998e60091dbe` |
| `run-a6db2f4-ideal.json` | `7579e88f5dae14da33cb530cb69d715a8a4ce2be744ca00220b7343e490c2629` |
| `run-a6db2f4-shared.json` | `bba4b340430d3529308154e73d6abf7f1eb101b156b6f112f29b220ea3d523c2` |

Summary (one base seed, one input; no confidence interval):

| Contract / seed policy | logits max abs diff | rms diff | rel. L2 | top-1 agreement |
| --- | --- | --- | --- | --- |
| default, per_gemm | 17.07 | 3.457 | 0.7409 | 0.0833 |
| default, shared | 15.94 | 3.949 | 0.8462 | 0.0208 |
| ideal, per_gemm | 1.43e-05 | 2.06e-06 | 4.4e-07 | 1.0000 |

Checks: the `shared` record's deviation, counts and conversions equal `run-44009a6.json` exactly (only
`seed_policy`, per-row `seed`, `seed_note` and the graph fingerprint differ, because the option set is part of the
fingerprint); the ideal deviation equals `run-44009a6-ideal.json`; a second `per_gemm` run gave the same SHA-256.
One seed and one input cannot say whether per-GEMM seeds make the noisy deviation larger or smaller in general.

## P2-3: host fallback MACs

Source label: **measured by us** (simulator output, not hardware) for the deviations; the MAC counts are **counted**
by the report code from FX shape metadata (rule below). Evidence level L1. Same box, venv and input as above.
Commit `a62abd9` (branch `work/p2-3-host-fallback-macs`), 2026-10-31 ET. From `awen-models/`:

```
python -m awen.compile_capability --out results/compile-capability/run-a62abd9.json
python -m awen.compile_capability --ideal --out results/compile-capability/run-a62abd9-ideal.json
python -m awen.compile_capability --seed-policy shared --out results/compile-capability/run-a62abd9-shared.json
```

| File | SHA-256 |
| --- | --- |
| `run-a62abd9.json` | `96eab38caae4a47498392578bf7dd331999ac8740a76649d8121f36c1abde84a` |
| `run-a62abd9-ideal.json` | `86389ecae9ab1031cbf7b1c0945b83633b56bc73121d2ddf8382f9c96a4226d2` |
| `run-a62abd9-shared.json` | `52bca0e4a5ec12f3e5fbfff4741a1784b887434cafd96c84bbbe54af26995992` |

New `counts` fields (additive; `host_macs` stays a string and now points to `host_fallback_macs`):
`host_fallback_macs`, `host_macs_uncounted_nodes`, `host_mac_rule`, `lowered_mac_fraction`; each `fallback` row has
`macs` (`null` if a node in it is uncounted). Rule: contraction MACs only. `scaled_dot_product_attention` counts
QK^T plus AV densely, `batch * L * S * (E + Ev)`, with causal masking not subtracted; `addmm`/`baddbmm` count
`rows * inner * outputs`; elementwise, normalisation, gather, index and layout nodes count 0; anything else is listed
as uncounted and the fraction is then not computed.

All three records: 47,585,280 lowered MACs, 2,949,120 host MACs (5 attention cores x 4 heads x 48 x 48 x (32 + 32)),
no uncounted node, lowered MAC fraction 0.94164 (dense count; with causal masking subtracted the host share would be
smaller). Deviations equal the P2-2 records of the same contract and seed policy, and `lowered`, `conversions` and
`backend` (fingerprint included) in `run-a62abd9.json` equal `run-a6db2f4.json`. A second default run gave the same
SHA-256. The host count equals the Runtime-path count in `awen.capability` (P1-5) minus its unrouted tied head
(test `test_host_fallback_macs_match_the_runtime_path_count`). MAC counts are not time, energy or speedup.
