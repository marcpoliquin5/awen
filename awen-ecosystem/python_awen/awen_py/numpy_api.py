"""NumPy entry points for the AWEN in-process runtime."""

from __future__ import annotations

from typing import Any, Mapping, Optional

from .errors import ContractError
from .runtime import (
    AwenFuture,
    ExecutionOptions,
    InProcessRuntime,
    NumericalContract,
    OperationPlan,
    get_runtime,
)


def gemm(
    lhs: Any,
    rhs: Any,
    *,
    out: Optional[Any] = None,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
):
    return _execute("gemm", lhs, rhs, out=out, contract=contract, options=options, runtime=runtime)


def batched_gemm(
    lhs: Any,
    rhs: Any,
    *,
    out: Optional[Any] = None,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
):
    return _execute(
        "batched_gemm", lhs, rhs, out=out, contract=contract, options=options, runtime=runtime
    )


def complex_gemm(
    lhs: Any,
    rhs: Any,
    *,
    out: Optional[Any] = None,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
):
    return _execute(
        "complex_gemm", lhs, rhs, out=out, contract=contract, options=options, runtime=runtime
    )


def linear(
    inputs: Any,
    weight: Any,
    bias: Optional[Any] = None,
    *,
    out: Optional[Any] = None,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
):
    operands = (inputs, weight) if bias is None else (inputs, weight, bias)
    return _execute(
        "linear", *operands, out=out, contract=contract, options=options, runtime=runtime
    )


def attention_scores(
    query: Any,
    key: Any,
    *,
    scale: float = 1.0,
    out: Optional[Any] = None,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
):
    return _execute(
        "attention_scores",
        query,
        key,
        attributes={"scale": scale},
        out=out,
        contract=contract,
        options=options,
        runtime=runtime,
    )


def attention_value(
    probabilities: Any,
    value: Any,
    *,
    out: Optional[Any] = None,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
):
    return _execute(
        "attention_value",
        probabilities,
        value,
        out=out,
        contract=contract,
        options=options,
        runtime=runtime,
    )


def mlp_projection(
    inputs: Any,
    weight: Any,
    bias: Optional[Any] = None,
    *,
    out: Optional[Any] = None,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
):
    operands = (inputs, weight) if bias is None else (inputs, weight, bias)
    return _execute(
        "mlp_projection",
        *operands,
        out=out,
        contract=contract,
        options=options,
        runtime=runtime,
    )


def fft(
    value: Any,
    *,
    inverse: bool = False,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
):
    return _execute(
        "ifft" if inverse else "fft",
        value,
        contract=contract,
        options=options,
        runtime=runtime,
    )


def ifft(
    value: Any,
    *,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
):
    return fft(
        value,
        inverse=True,
        contract=contract,
        options=options,
        runtime=runtime,
    )


def compile_plan(
    operation: str,
    *inputs: Any,
    attributes: Optional[Mapping[str, Any]] = None,
    contract: NumericalContract = NumericalContract(),
    options: ExecutionOptions = ExecutionOptions(),
    runtime: Optional[InProcessRuntime] = None,
) -> OperationPlan:
    return (runtime or get_runtime()).plan(
        operation,
        inputs,
        attributes=attributes,
        contract=contract,
        options=options,
    )


def _execute(
    operation: str,
    *inputs: Any,
    attributes: Optional[Mapping[str, Any]] = None,
    out: Optional[Any] = None,
    contract: NumericalContract,
    options: ExecutionOptions,
    runtime: Optional[InProcessRuntime],
):
    import numpy as np

    arrays = tuple(np.asarray(value) for value in inputs)
    selected_runtime = runtime or get_runtime()
    result = selected_runtime.execute(
        operation,
        *arrays,
        attributes=attributes,
        contract=contract,
        options=options,
    )
    if isinstance(result, AwenFuture):
        if out is not None:
            raise ContractError("an out buffer cannot be combined with asynchronous execution")
        return result
    if out is None:
        return result
    destination = np.asarray(out)
    if destination.shape != result.shape or destination.dtype != result.dtype:
        raise ContractError("out buffer shape and dtype must exactly match the result")
    np.copyto(destination, result)
    return out
