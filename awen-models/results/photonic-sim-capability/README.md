# AWEN-Mini on `photonic-sim`: capability reports (P1-5)

Source label: **measured by us** (simulator output, not hardware), evidence level L1. No energy, latency or
speedup figure is produced; energy needs the full system accounting in `awen-spec/hardware_assumptions.yaml`.

Commit `8dfad1c` (branch `feat/p1-5-photonic-sim-report`), 2026-10-30 12:33 ET. Build box: Linux 6.12 x86_64,
8 vCPU, no GPU, Python 3.12.15 venv from `requirements/cpu-py312.txt` (torch 2.14.1+cpu) with `awen_native`
installed. From `awen-models/`:

```
PYTHONPATH=. python -m awen.capability --seed 0 --out results/photonic-sim-capability/awen-mini-default.json
PYTHONPATH=. python -m awen.capability --seed 0 --ideal --out results/photonic-sim-capability/awen-mini-ideal.json
```

Input: the module's default text (48 characters, 48 positions), bundled `awen-mini` checkpoint.

| File | SHA-256 |
| --- | --- |
| `awen-mini-default.json` | `1cea50488f256078c5376b27d4fa8998770f8948180c857a65991ba138a89868` |
| `awen-mini-ideal.json` | `a61a233313834a5aa48d4efa00e401b9be791d1ebbbb2ceb68f41574f1a09e5b` |

Summary (one seed, one input; no confidence interval):

| Contract | mean NLL sim / CPU (nats) | NLL diff | logits rel. L2 | top-1 agreement |
| --- | --- | --- | --- | --- |
| default (`awen_native.default_contract()`, 1000 photons/MAC, 8-bit DAC/ADC) | 4.1710 / 1.2882 | +2.8828 | 0.7146 | 0.1667 |
| ideal (`awen_native.ideal_contract()`) | 1.2882 / 1.2882 | -8.1e-08 | 4.8e-07 | 1.0000 |

Both: 20 GEMMs lowered (every `AwenLinear`), 47,185,920 MACs on the simulator and 3,348,480 on the host
(attention core and tied output head; dense count), 215,040 input DAC, 983,040 weight DAC and 737,280 ADC
conversions under the simulator's tiling rule. The default contract's smallest per-tile effective bits is 5.54
(simulator noise budget). Under the default contract the bundled model, which was trained without noise, loses
most of its next-character accuracy; that is a negative result for running it unmodified, not a hardware finding.
The default contract values are unsourced (see `awen-spec/hardcoded_device_defaults.yaml`).
