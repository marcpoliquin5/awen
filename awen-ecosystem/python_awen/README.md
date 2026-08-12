# awen_py

`awen_py` provides AWEN's in-process NumPy, PyTorch, and JAX integration. The
normal path accepts live tensors and does not invoke `awenctl`, create temporary
IR files, or search artifact directories.

## Installation

```bash
pip install .[numpy]
pip install .[torch]
pip install .[jax]
pip install .[frameworks]
```

Supported ranges are Python 3.10+, NumPy 1.26–2.x, PyTorch 2.10–2.13, and JAX
0.9–0.11. The JAX extra requires Python 3.12 or later.

## PyTorch

```python
import torch
from awen_py import awen

model = torch.nn.Sequential(torch.nn.Linear(16, 32), torch.nn.ReLU())
compiled = torch.compile(model, backend=awen, dynamic=True)
x = torch.randn(8, 16, requires_grad=True)
y = compiled(x)
y.sum().backward()
```

Matrix multiplication and linear nodes execute through the AWEN in-process
boundary. Unsupported nodes remain eager PyTorch operations by default. The
compile report exposes every supported and fallback node:

```python
from awen_py.torch_backend import get_last_compile_report

print(get_last_compile_report().to_json())
```

Pass backend options with the `options` argument to `torch.compile`:

```python
compiled = torch.compile(
    model,
    backend=awen,
    options={
        "target": "auto",
        "allow_fallback": True,
        "max_abs_error": 1e-5,
        "max_rel_error": 1e-5,
        "deterministic": True,
        "seed": 7,
        "profile": True,
    },
)
```

Installed packages also register the string backend name, so
`torch.compile(model, backend="awen")` is supported.

## JAX and StableHLO

```python
import jax.numpy as jnp
from awen_py import JaxExecutable, export_jax

def function(left, right):
    return jnp.tanh(left @ right)

left = jnp.ones((2, 3), dtype=jnp.float32)
right = jnp.ones((3, 4), dtype=jnp.float32)
executable = export_jax(
    function,
    left,
    right,
    polymorphic_shapes=("(batch, 3)", "(3, 4)"),
)
result = executable(left, right)
portable = executable.serialize()
restored = JaxExecutable.deserialize(portable)
```

Use `export_jax_value_and_grad` to export analytic gradients. StableHLO text is
available for diagnostics through `stablehlo_text`; serialized execution uses
JAX's portable export bytes, not the diagnostic text.

## NumPy and runtime API

```python
import numpy as np
from awen_py import ExecutionOptions, InProcessRuntime, gemm

left = np.arange(12.0).reshape(3, 4)[:, ::2]
right = np.ones((2, 5))
output = gemm(left, right)

with InProcessRuntime() as runtime:
    future = runtime.execute(
        "gemm",
        left,
        right,
        options=ExecutionOptions(asynchronous=True, seed=42),
    )
    execution = future.wait()
    serialized_plan = execution.plan.to_json()
    replay = runtime.replay(execution.plan, left, right)
```

The runtime also exposes explicit buffers and ownership, devices, streams,
profiling traces, numerical contracts, transfers, deterministic replay, and
typed exceptions.

## C and C++

Build `awen-runtime` as a shared or static library and include either:

```cpp
#include <awen/framework.h>   // C
#include <awen/framework.hpp> // C++20 std::span wrapper
```

The initial compiled ABI provides caller-owned row-major `f32` and `f64` GEMM,
thread-local errors, checked buffer lengths, and panic containment. See
`awen-runtime/examples/framework_cpp.cpp`.

## Legacy CLI diagnostics

Artifact experiments remain available as explicit debug-only functions:

```python
from awen_py.client import run_ir_cli_debug, compute_gradients_cli_debug
```

They invoke `awenctl` and are not used by any normal framework execution path.
