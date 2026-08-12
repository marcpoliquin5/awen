import numpy as np
import pytest

from awen_py import (
    JaxExecutable,
    NumericalContract,
    assert_numerical_contract,
    export_jax,
    export_jax_value_and_grad,
    get_runtime,
)
from awen_py.errors import UnsupportedGraphError

jax = pytest.importorskip("jax")
jnp = pytest.importorskip("jax.numpy")


def test_stablehlo_export_execution_serialization_and_contract():
    def function(left, right):
        return jnp.tanh(left @ right + 0.25)

    left = jnp.arange(6.0, dtype=jnp.float32).reshape(2, 3)
    right = jnp.arange(12.0, dtype=jnp.float32).reshape(3, 4)
    executable = export_jax(function, left, right)
    result = executable(left, right)
    assert_numerical_contract(
        function(left, right), result, NumericalContract(1.0e-5, 1.0e-5)
    )
    assert "dot_general" in executable.report.supported_operations
    assert "tanh" in executable.report.fallback_operations
    assert any(item.action == "framework_fallback" for item in executable.report.diagnostics)
    assert executable.report.portable_fingerprint.startswith("sha256:")
    assert "stablehlo.dot_general" in executable.stablehlo_text

    restored = JaxExecutable.deserialize(executable.serialize())
    np.testing.assert_allclose(restored(left, right), result, atol=1.0e-5, rtol=1.0e-5)
    with pytest.raises(UnsupportedGraphError, match="tanh"):
        export_jax(function, left, right, allow_fallback=False)


def test_live_jax_runtime_preserves_array_device_and_transfer():
    left = jnp.arange(6.0, dtype=jnp.float32).reshape(2, 3)
    right = jnp.arange(12.0, dtype=jnp.float32).reshape(3, 4)
    runtime = get_runtime()
    execution = runtime.execute_with_trace("gemm", left, right)
    assert isinstance(execution.outputs, jax.Array)
    np.testing.assert_allclose(execution.outputs, left @ right)
    assert execution.plan.framework == "jax"
    assert execution.plan.target.startswith("framework:cpu")
    transferred = runtime.transfer(execution.outputs, "cpu")
    np.testing.assert_allclose(transferred, execution.outputs)


def test_dynamic_shape_export_and_analytic_gradients():
    def loss(left, right):
        return jnp.sum((left @ right) ** 2)

    left = jnp.arange(6.0, dtype=jnp.float32).reshape(2, 3)
    right = jnp.arange(12.0, dtype=jnp.float32).reshape(3, 4)
    executable = export_jax_value_and_grad(
        loss,
        left,
        right,
        argnums=(0, 1),
        polymorphic_shapes=("(batch, 3)", "(3, 4)"),
    )
    expected_value, expected_gradients = jax.value_and_grad(loss, argnums=(0, 1))(left, right)
    actual_value, actual_gradients = executable(left, right)
    np.testing.assert_allclose(actual_value, expected_value)
    np.testing.assert_allclose(actual_gradients[0], expected_gradients[0])
    np.testing.assert_allclose(actual_gradients[1], expected_gradients[1])
    assert "batch" in executable.stablehlo_text

    larger_left = jnp.arange(15.0, dtype=jnp.float32).reshape(5, 3)
    larger_expected = jax.value_and_grad(loss, argnums=(0, 1))(larger_left, right)
    larger_actual = executable(larger_left, right)
    np.testing.assert_allclose(larger_actual[0], larger_expected[0])
    np.testing.assert_allclose(larger_actual[1][0], larger_expected[1][0])
    np.testing.assert_allclose(larger_actual[1][1], larger_expected[1][1])
