awen_py
=======

Python convenience wrapper for AWEN runtime CLI. Provides `compute_gradients` and `run_ir` helpers that invoke `awenctl` and return parsed artifact contents.

Usage:

- Install locally: `pip install .` from the `python_awen` directory (after building runtime or installing `awenctl`).
- Example:

```py
from awen_py import compute_gradients
res = compute_gradients('example_ir.json', ['mzi_0:phase', 'mzi_1:phase'], seed=42)
print(res)
```

Notes:
- This is a thin wrapper for integration with PyTorch/JAX workflows. For deeper integration, build native bindings or RPC APIs in `awen-runtime`.
