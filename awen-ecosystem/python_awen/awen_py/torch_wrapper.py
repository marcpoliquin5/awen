"""Compatibility helpers backed by live PyTorch analytic operations.

New code should use ``torch.compile(..., backend=awen_py.awen)``. This module
contains no artifact, temporary-file, subprocess, scalar, or finite-difference
path.
"""

from __future__ import annotations

from typing import Any, Optional, Sequence

from .errors import ContractError
from .runtime import ExecutionOptions, NumericalContract, get_runtime


def awen_forward(
    lhs: Any,
    rhs: Any,
    bias: Optional[Any] = None,
    *,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
) -> Any:
    """Execute live tensors while preserving the native PyTorch autograd graph."""

    _require_torch_tensor(lhs)
    _require_torch_tensor(rhs)
    inputs = (lhs, rhs) if bias is None else (lhs, rhs, bias)
    operation = "gemm" if bias is None else "linear"
    return get_runtime().execute(operation, *inputs, contract=contract, options=options)


def awen_backward(
    output: Any,
    inputs: Sequence[Any],
    *,
    grad_output: Optional[Any] = None,
    retain_graph: bool = False,
) -> Any:
    """Return analytic gradients using ``torch.autograd.grad``."""

    try:
        import torch
    except ImportError as error:
        raise ContractError("PyTorch is required for awen_backward") from error
    _require_torch_tensor(output)
    if grad_output is None and output.numel() != 1:
        grad_output = torch.ones_like(output)
    return torch.autograd.grad(
        output,
        tuple(inputs),
        grad_outputs=grad_output,
        retain_graph=retain_graph,
    )


def _require_torch_tensor(value: Any) -> None:
    if type(value).__module__.split(".", 1)[0] != "torch":
        raise ContractError("awen_forward requires live PyTorch tensors")
