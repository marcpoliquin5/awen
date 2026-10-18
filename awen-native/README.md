# awen-native

Python bindings (PyO3 + maturin) for the AWEN Rust compiler and the photonic
GEMM noise model (AEP-0022). Everything this package computes is **simulated**
at compact-model fidelity. Nothing it returns is a hardware measurement.

```bash
pip install ./awen-native          # needs a Rust toolchain (rust-toolchain.toml)
```

```python
import numpy as np
import awen_native

x = np.random.default_rng(0).uniform(-1, 1, (4, 128))
w = np.random.default_rng(1).uniform(-1, 1, (8, 128))
y, report = awen_native.photonic_gemm(x, w, tile_k=64, seed=0)   # x @ w.T with noise
print(report["fidelity"], report["tile_budget"]["effective_bits"])

exact, _ = awen_native.photonic_gemm(x, w, awen_native.ideal_contract())
```

| Function | Purpose |
| --- | --- |
| `photonic_gemm(x, w, contract=None, tile_k=64, seed=0)` | `x @ w.T` on the noise model; returns `(y, report)` |
| `default_contract()` / `ideal_contract()` / `validate_contract(c)` | `awen.photonic-noise-contract.v1` documents |
| `predicted_noise(k, signal, contract)` | Analytic per-term noise budget for one tile |
| `effective_bits_from_sigma`, `shot_limited_effective_bits` | IEEE 1241 ENOB helpers |
| `compile_program(program, capabilities, options=None)` | Rust compiler: TensorProgram → compilation artifact |

`awen_py` uses this extension when an operation targets `photonic`, which
includes `torch.compile(..., backend=awen)` with `options={"target": "photonic"}`.
That target is inference-only.

The default contract is an illustrative operating point and is not a measured
device. Random-data dot products sit well below the full scale of the converters,
so the default contract gives low effective precision on them. This is a real
dynamic-range property of analog accumulation, which the model reproduces.

Tests: `python -m pytest awen-native/tests`.
