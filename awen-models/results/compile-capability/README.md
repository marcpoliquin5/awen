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
