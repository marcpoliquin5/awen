import json

import numpy as np
import pytest

from awen_py import (
    Buffer,
    BufferOwner,
    ContractError,
    Device,
    DeviceError,
    ExecutionError,
    ExecutionOptions,
    ExecutionTrace,
    InProcessRuntime,
    NumericalContract,
    OperationPlan,
    Stream,
)
from awen_py.runtime import FRAMEWORK_RUNTIME_VERSION, debug_finite_difference


def test_buffer_ownership_device_and_noncontiguous_strides():
    source = np.arange(24.0).reshape(4, 6)[:, ::2]
    buffer = Buffer.wrap(source, BufferOwner.BORROWED)
    assert buffer.owner is BufferOwner.BORROWED
    assert buffer.framework == "numpy"
    assert str(buffer.device) == "cpu"
    assert buffer.shape == (4, 3)
    assert buffer.strides == (6, 2)


def test_deterministic_plan_serialization_and_replay():
    runtime = InProcessRuntime()
    lhs = np.arange(6.0).reshape(2, 3)
    rhs = np.arange(12.0).reshape(3, 4)
    first = runtime.plan("gemm", (lhs, rhs), options=ExecutionOptions(seed=91))
    second = runtime.plan("gemm", (lhs.copy(), rhs.copy()), options=ExecutionOptions(seed=91))
    assert first.operation_id == second.operation_id
    assert first.fingerprint == second.fingerprint
    serialized = first.to_json()
    restored = OperationPlan.from_json(serialized)
    assert restored == first
    replay = runtime.replay(restored, lhs, rhs)
    np.testing.assert_array_equal(replay.outputs, lhs @ rhs)
    assert replay.trace.plan_fingerprint == first.fingerprint
    assert replay.trace.replay_fingerprint.startswith("sha256:")
    with pytest.raises(ContractError, match="shapes"):
        runtime.replay(restored, lhs[:, :2], rhs[:2])


def test_async_execution_profiling_and_last_trace():
    runtime = InProcessRuntime(max_workers=1)
    lhs = np.eye(4)
    rhs = np.arange(16.0).reshape(4, 4)
    future = runtime.execute(
        "gemm", lhs, rhs, options=ExecutionOptions(asynchronous=True, profile=True)
    )
    result = future.wait(timeout=5)
    np.testing.assert_array_equal(result.outputs, rhs)
    assert result.trace.events[0].asynchronous is True
    assert result.trace.events[0].operation == "gemm"
    assert result.trace.events[0].duration_ns >= 0
    assert runtime.last_trace == result.trace
    assert ExecutionTrace.from_json(result.trace.to_json()) == result.trace


def test_transfer_contract_errors_and_abi_version():
    runtime = InProcessRuntime()
    value = np.ones((2, 2), dtype=np.float32)
    assert runtime.abi_version == FRAMEWORK_RUNTIME_VERSION
    assert runtime.transfer(value, "cpu") is value
    with pytest.raises(DeviceError, match="cannot be transferred"):
        runtime.transfer(value, "cuda:0")
    with pytest.raises(DeviceError, match="does not match"):
        runtime.execute(
            "gemm",
            value,
            value,
            options=ExecutionOptions(stream=Stream(Device("cuda", 0), "test")),
        )
    with pytest.raises(ContractError, match="effective-bit"):
        runtime.execute(
            "gemm",
            value,
            value,
            contract=NumericalContract(minimum_effective_bits=25),
        )
    with pytest.raises(ExecutionError, match="failed"):
        runtime.execute("gemm", np.ones((2, 3)), np.ones((4, 2)))
    with pytest.raises(ContractError, match="unsupported"):
        runtime.execute("convolution", value)


def test_plan_rejects_tampering_and_unknown_fields():
    runtime = InProcessRuntime()
    value = np.eye(2)
    plan = runtime.plan("gemm", (value, value))
    payload = json.loads(plan.to_json())
    payload["seed"] += 1
    with pytest.raises(Exception, match="fingerprint"):
        OperationPlan.from_json(json.dumps(payload))
    payload = json.loads(plan.to_json())
    payload["surprise"] = True
    with pytest.raises(Exception, match="fields"):
        OperationPlan.from_json(json.dumps(payload))


def test_finite_difference_is_explicit_debug_only():
    def function(value):
        return value * value

    with pytest.raises(ContractError, match="disabled"):
        debug_finite_difference(function, (np.array([2.0]),))
    gradient = debug_finite_difference(
        function, (np.array([2.0]),), epsilon=1.0e-5, enabled=True
    )
    np.testing.assert_allclose(gradient[0], [4.0], atol=1.0e-8)
