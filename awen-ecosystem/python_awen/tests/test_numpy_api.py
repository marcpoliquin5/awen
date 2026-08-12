import numpy as np
import pytest

from awen_py import (
    ContractError,
    ExecutionOptions,
    attention_scores,
    attention_value,
    batched_gemm,
    complex_gemm,
    fft,
    gemm,
    linear,
    mlp_projection,
)


def test_gemm_noncontiguous_out_and_batching():
    lhs = np.arange(24.0).reshape(4, 6)[:, ::2]
    rhs = np.arange(15.0).reshape(3, 5)
    output = np.empty((4, 5), dtype=np.float64)
    assert gemm(lhs, rhs, out=output) is output
    np.testing.assert_array_equal(output, lhs @ rhs)

    batched_lhs = np.arange(24.0).reshape(2, 3, 4)
    batched_rhs = np.arange(40.0).reshape(2, 4, 5)
    np.testing.assert_array_equal(
        batched_gemm(batched_lhs, batched_rhs), batched_lhs @ batched_rhs
    )


def test_complex_linear_fft_and_async():
    lhs = np.array([[1 + 2j, 3 - 1j]])
    rhs = np.array([[2 - 1j], [4 + 3j]])
    np.testing.assert_allclose(complex_gemm(lhs, rhs), lhs @ rhs)

    inputs = np.arange(6.0).reshape(2, 3)
    weight = np.arange(12.0).reshape(3, 4)
    bias = np.arange(4.0)
    np.testing.assert_array_equal(linear(inputs, weight, bias), inputs @ weight + bias)
    np.testing.assert_array_equal(
        mlp_projection(inputs, weight, bias), inputs @ weight + bias
    )
    query = np.arange(12.0).reshape(1, 3, 4)
    key = np.arange(20.0).reshape(1, 5, 4)
    scores = attention_scores(query, key, scale=0.5)
    np.testing.assert_array_equal(scores, (query @ key.swapaxes(-2, -1)) * 0.5)
    value = np.arange(30.0).reshape(1, 5, 6)
    np.testing.assert_array_equal(attention_value(scores, value), scores @ value)
    np.testing.assert_allclose(fft(fft(inputs), inverse=True), inputs)

    future = gemm(np.eye(2), np.eye(2), options=ExecutionOptions(asynchronous=True))
    np.testing.assert_array_equal(future.wait().outputs, np.eye(2))


def test_out_buffer_contract_is_exact_and_not_async():
    value = np.eye(2)
    with pytest.raises(ContractError, match="shape and dtype"):
        gemm(value, value, out=np.empty((2, 2), dtype=np.float32))
    with pytest.raises(ContractError, match="cannot be combined"):
        gemm(
            value,
            value,
            out=np.empty((2, 2)),
            options=ExecutionOptions(asynchronous=True),
        )
