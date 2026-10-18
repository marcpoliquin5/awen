# PJRT plugin (C5, issue #61): what runs today and the remaining boundary

## What runs today

`awen_py.stablehlo_exec` executes the StableHLO that JAX itself produces
(`jax.jit(f).lower(*args).as_text()`) on AWEN:

- `stablehlo.dot_general` (any batching and contracting dimensions) runs on
  the f64 math reference (`target="reference"`) or through
  `awen_native.photonic_gemm`, the Rust compact noise model
  (`target="photonic-sim"`). Photonic-sim results are **simulated**, never
  measured; the report says so in its `evidence` field.
- `add`, `subtract`, `multiply`, `negate`, `maximum`, `transpose`, `reshape`,
  `broadcast_in_dim`, `constant` (decimal dense literals) and `convert` run
  electronically in NumPy.
- Anything else (for example `stablehlo.tanh`, `func.call`, multi-result
  ops, hex-encoded constants, `dot_general` with an `algorithm` attribute)
  raises `UnsupportedGraphError` naming the operation. Nothing is executed
  silently wrong.

Tests (`awen-ecosystem/python_awen/tests/test_stablehlo_exec.py`) lower
`jnp.dot`, a batched `einsum`, a transposed matmul with a bias, and a
dense + ReLU layer with JAX, run them on AWEN, and compare with JAX's own CPU
result (rtol 1e-5). With `awen_native.ideal_contract()` the photonic-sim
path matches JAX CPU; with the default contract it injects seeded,
reproducible simulated noise.

```python
import jax.numpy as jnp, numpy as np, awen_native
from awen_py.stablehlo_exec import run_jax_function
a, b = np.ones((4, 16), np.float32), np.ones((16, 3), np.float32)
(out,), report = run_jax_function(jnp.dot, a, b, target="photonic-sim",
                                  contract=awen_native.default_contract())
```

## What this is not

This is **not** a PJRT plugin. JAX does not see AWEN as a device: there is no
`jax.devices("awen")`, no device buffers, and JAX does not dispatch to AWEN
on its own. The caller lowers the function and hands the text to AWEN.

## Remaining boundary for a real PJRT C-API plugin

JAX loads PJRT plugins as shared libraries (`jax._src.xla_bridge.register_plugin`,
or the `PJRT_NAMES_AND_LIBRARY_PATHS` environment variable) that export
`GetPjrtApi()`, returning the function table defined in OpenXLA's
`xla/pjrt/c/pjrt_c_api.h`. A minimal plugin has to provide, at least:

1. Plugin and client lifecycle, platform name/version, device and memory
   enumeration with descriptions and attributes.
2. Host-to-device and device-to-host buffer transfer, buffer metadata
   (element type, dimensions, layout) and deletion, plus ready events.
3. Compilation: the program arrives as serialized MLIR bytecode (StableHLO,
   typically a versioned portable artifact). Parsing it needs the MLIR and
   StableHLO C++ libraries built against a matching LLVM revision. That is
   the main blocker: this repository's MLIR build (`awen-mlir`) does not
   build StableHLO, and vendoring StableHLO at a pinned LLVM commit is a
   multi-hour build that has not been set up in CI.
4. Execution of the compiled program on the simulator, with outputs as
   device buffers, and error objects with codes and messages.
5. API-version negotiation: the header's major/minor version must match
   what the installed jaxlib expects; this pins the plugin to jaxlib
   releases.

The executor above is the intended compile/execute core: once a plugin can
deserialize the bytecode (step 3), it can print or walk the same op set and
reuse the same dot_general mapping and refusal rules.

Next actions: add StableHLO (pinned to the LLVM revision used by
`awen-mlir`) as an optional CMake dependency; implement the C-API table in
C++ under `awen-mlir/` with the executor logic ported to the runtime; test
with `jax.devices("awen")` and `jnp.dot` against CPU.
