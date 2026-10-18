import numpy as np
import pytest

from awen_py.errors import ContractError, UnsupportedGraphError
from awen_py.stablehlo_exec import execute_stablehlo, run_jax_function

DOT = """module @m {
  func.func public @main(%arg0: tensor<2x3xf32>, %arg1: tensor<3x4xf32>) -> (tensor<2x4xf32>) {
    %0 = stablehlo.dot_general %arg0, %arg1, contracting_dims = [1] x [0], precision = [DEFAULT, DEFAULT] : (tensor<2x3xf32>, tensor<3x4xf32>) -> tensor<2x4xf32>
    %cst = stablehlo.constant dense<2.500000e-01> : tensor<f32>
    %1 = stablehlo.broadcast_in_dim %cst, dims = [] : (tensor<f32>) -> tensor<2x4xf32>
    %2 = stablehlo.add %0, %1 : tensor<2x4xf32>
    return %2 : tensor<2x4xf32>
  }
}
"""


def test_handwritten_module_reference():
    a = np.arange(6, dtype=np.float32).reshape(2, 3)
    b = np.arange(12, dtype=np.float32).reshape(3, 4)
    (out,), report = execute_stablehlo(DOT, a, b)
    np.testing.assert_allclose(out, a @ b + 0.25, rtol=1e-6)
    assert out.dtype == np.float32
    assert [r.op for r in report.ops] == ["dot_general", "constant", "broadcast_in_dim", "add"]
    assert report.photonic_macs == 2 * 3 * 4


def test_unsupported_ops_and_contracts_are_refused():
    bad = DOT.replace("stablehlo.add %0, %1", "stablehlo.tanh %0")
    with pytest.raises(UnsupportedGraphError, match="tanh"):
        execute_stablehlo(bad, np.zeros((2, 3)), np.zeros((3, 4)))
    with pytest.raises(ContractError):
        execute_stablehlo(DOT, np.zeros((2, 2)), np.zeros((3, 4)))
    with pytest.raises(ContractError):
        execute_stablehlo(DOT, np.zeros((2, 3)), np.zeros((3, 4)), target="gpu")


jax = pytest.importorskip("jax")
jnp = pytest.importorskip("jax.numpy")


def _cases():
    rng = np.random.default_rng(0)
    yield (lambda a, b: jnp.dot(a, b)), (rng.standard_normal((5, 7)).astype(np.float32), rng.standard_normal((7, 3)).astype(np.float32))
    yield (lambda a, b: jnp.einsum("bij,bjk->bik", a, b)), (rng.standard_normal((2, 4, 6)).astype(np.float32), rng.standard_normal((2, 6, 3)).astype(np.float32))
    yield (lambda a, b: a.T @ b + 1.0), (rng.standard_normal((6, 4)).astype(np.float32), rng.standard_normal((6, 2)).astype(np.float32))
    yield (lambda x, w, c: jnp.maximum(x @ w + c, 0.0)), (
        rng.standard_normal((8, 5)).astype(np.float32), rng.standard_normal((5, 4)).astype(np.float32), rng.standard_normal((4,)).astype(np.float32))


@pytest.mark.parametrize("case", list(range(4)))
def test_jax_lowered_matches_jax_cpu_reference(case):
    fn, args = list(_cases())[case]
    expected = np.asarray(jax.jit(fn)(*args))
    (out,), report = run_jax_function(fn, *args)
    np.testing.assert_allclose(out, expected, rtol=1e-5, atol=1e-5)
    assert any(r.op == "dot_general" for r in report.ops)


def test_jax_unsupported_op_reported():
    with pytest.raises(UnsupportedGraphError, match="tanh"):
        run_jax_function(lambda a, b: jnp.tanh(a @ b), np.ones((2, 3), np.float32), np.ones((3, 2), np.float32))


def test_jax_dot_on_photonic_simulator():
    awen_native = pytest.importorskip("awen_native")
    rng = np.random.default_rng(1)
    a = rng.standard_normal((4, 16)).astype(np.float32)
    b = rng.standard_normal((16, 3)).astype(np.float32)
    expected = np.asarray(jax.jit(jnp.dot)(a, b))
    (ideal,), rep = run_jax_function(jnp.dot, a, b, target="photonic-sim", contract=awen_native.ideal_contract())
    np.testing.assert_allclose(ideal, expected, rtol=1e-5, atol=1e-5)
    assert rep.ops[0].placement == "photonic-sim" and "not a hardware measurement" in rep.evidence
    (noisy,), _ = run_jax_function(jnp.dot, a, b, target="photonic-sim", contract=awen_native.default_contract(), seed=3)
    err = np.abs(noisy - expected).max()
    assert err > 0  # the default contract injects simulated noise
    (noisy2,), _ = run_jax_function(jnp.dot, a, b, target="photonic-sim", contract=awen_native.default_contract(), seed=3)
    np.testing.assert_array_equal(noisy, noisy2)  # seeded and reproducible
