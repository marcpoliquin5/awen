"""Route real-valued matrix products through the AWEN Rust photonic simulator.

Used when an operation's target is ``simulated:photonic``. Arithmetic runs in
``awen_native.photonic_gemm`` (compact noise model, float64); results are
simulated, never hardware measurements. Install the extension with
``pip install ./awen-native``.
"""

from __future__ import annotations

from typing import Any, Mapping, Optional, Tuple

from .errors import ContractError, DeviceError

PHOTONIC_OPERATIONS = frozenset(
    {"gemm", "batched_gemm", "linear", "mlp_projection", "attention_scores", "attention_value"}
)


def native_available() -> bool:
    try:
        import awen_native  # noqa: F401
    except ImportError:
        return False
    return True


def _native():
    try:
        import awen_native
    except ImportError as error:
        raise DeviceError(
            "target 'photonic' runs on the AWEN Rust simulator, which is not installed; "
            "build it with `pip install ./awen-native`"
        ) from error
    return awen_native


def _to_numpy(value: Any, framework: str):
    import numpy as np

    if framework == "torch":
        return value.detach().to("cpu").double().numpy()
    return np.asarray(value, dtype=np.float64)


def _from_numpy(array: Any, like: Any, framework: str):
    if framework == "torch":
        import torch

        return torch.from_numpy(array).to(dtype=like.dtype, device=like.device)
    if framework == "jax":
        import jax.numpy as jnp

        return jnp.asarray(array, dtype=like.dtype)
    return array.astype(like.dtype, copy=False)


def _matmul(lhs, rhs, noise: Optional[Mapping[str, Any]], tile_k: int, seed: int):
    """``lhs @ rhs`` for float64 NumPy arrays with NumPy broadcasting rules on batch dims."""
    import numpy as np

    native = _native()
    if rhs.ndim == 2:
        flat = lhs.reshape(-1, lhs.shape[-1])
        y, _ = native.photonic_gemm(flat, rhs.T, noise, tile_k=tile_k, seed=seed)
        return y.reshape(*lhs.shape[:-1], rhs.shape[-1])
    batch = np.broadcast_shapes(lhs.shape[:-2], rhs.shape[:-2])
    lb = np.broadcast_to(lhs, batch + lhs.shape[-2:]).reshape(-1, *lhs.shape[-2:])
    rb = np.broadcast_to(rhs, batch + rhs.shape[-2:]).reshape(-1, *rhs.shape[-2:])
    out = np.empty((lb.shape[0], lhs.shape[-2], rhs.shape[-1]))
    for index in range(lb.shape[0]):
        out[index], _ = native.photonic_gemm(lb[index], rb[index].T, noise, tile_k=tile_k, seed=seed + index)
    return out.reshape(*batch, lhs.shape[-2], rhs.shape[-1])


def dispatch_photonic(
    operation: str,
    inputs: Tuple[Any, ...],
    attributes: Mapping[str, Any],
    framework: str,
    *,
    noise: Optional[Mapping[str, Any]],
    tile_k: int,
    seed: int,
) -> Any:
    """Execute ``operation`` with every matrix product on the photonic simulator."""
    if operation not in PHOTONIC_OPERATIONS:
        raise ContractError(f"operation '{operation}' has no photonic noise model")
    if framework == "torch":
        import torch

        if torch.is_grad_enabled() and any(getattr(v, "requires_grad", False) for v in inputs if v is not None):
            raise ContractError("the simulated photonic target is inference-only; run under torch.no_grad()")
        if any(v.is_complex() for v in inputs if v is not None):
            raise ContractError("complex operands have no photonic noise model yet")
    arrays = [None if v is None else _to_numpy(v, framework) for v in inputs]
    lhs, rhs = arrays[0], arrays[1]
    if operation == "attention_scores":
        rhs = rhs.swapaxes(-2, -1)
    result = _matmul(lhs, rhs, noise, tile_k, seed)
    if operation == "attention_scores":
        result = result * float(attributes.get("scale", 1.0))
    if operation in {"linear", "mlp_projection"} and len(arrays) == 3 and arrays[2] is not None:
        result = result + arrays[2]
    return _from_numpy(result, inputs[0], framework)
