"""GEMMs on the photonic target run in the Rust simulator (awen_native)."""

import numpy as np
import pytest

from awen_py import awen
from awen_py.runtime import ExecutionOptions, get_runtime

awen_native = pytest.importorskip("awen_native")
torch = pytest.importorskip("torch")


@pytest.fixture
def native_calls(monkeypatch):
    calls = []
    original = awen_native.photonic_gemm

    def spy(x, w, contract=None, tile_k=64, seed=0):
        calls.append((np.shape(x), np.shape(w), tile_k))
        return original(x, w, contract, tile_k=tile_k, seed=seed)

    monkeypatch.setattr(awen_native, "photonic_gemm", spy)
    return calls


def photonic(noise=None, **kwargs):
    return ExecutionOptions(target="photonic", photonic_noise=noise, **kwargs)


def test_numpy_gemm_runs_in_rust_and_matches_exactly_without_noise(native_calls):
    rng = np.random.default_rng(0)
    a, b = rng.normal(size=(6, 40)), rng.normal(size=(40, 5))
    out = get_runtime().execute("gemm", a, b, options=photonic(awen_native.ideal_contract(), photonic_tile_k=16))
    np.testing.assert_allclose(out, a @ b, rtol=0, atol=1e-10)
    assert native_calls == [((6, 40), (5, 40), 16)]


def test_batched_and_attention_operations_route_through_rust(native_calls):
    rng = np.random.default_rng(1)
    q, k = rng.normal(size=(2, 3, 8)), rng.normal(size=(2, 4, 8))
    ideal = photonic(awen_native.ideal_contract())
    scores = get_runtime().execute("attention_scores", q, k, attributes={"scale": 0.5}, options=ideal)
    np.testing.assert_allclose(scores, 0.5 * q @ k.swapaxes(-1, -2), atol=1e-10)
    assert len(native_calls) == 2


def test_default_noise_contract_perturbs_results_reproducibly():
    rng = np.random.default_rng(2)
    a, b = rng.uniform(-1, 1, size=(8, 64)), rng.uniform(-1, 1, size=(64, 8))
    first = get_runtime().execute("gemm", a, b, options=photonic(seed=4))
    second = get_runtime().execute("gemm", a, b, options=photonic(seed=4))
    np.testing.assert_array_equal(first, second)
    error = np.abs(first - a @ b).max()
    assert 0 < error < 0.25 * np.abs(a @ b).max() * 10


class Mlp(torch.nn.Module):
    def __init__(self):
        super().__init__()
        self.fc1 = torch.nn.Linear(12, 16)
        self.fc2 = torch.nn.Linear(16, 4)

    def forward(self, x):
        return self.fc2(torch.relu(self.fc1(x)))


def test_torch_compile_routes_linear_layers_through_rust(native_calls):
    torch.manual_seed(0)
    model = Mlp().eval()
    x = torch.randn(5, 12)

    def backend(graph, inputs, **kwargs):
        return awen(graph, inputs, options={"target": "photonic", "photonic_noise": awen_native.ideal_contract()})

    compiled = torch.compile(model, backend=backend)
    with torch.no_grad():
        expected = model(x)
        got = compiled(x)
    torch.testing.assert_close(got, expected, rtol=1e-5, atol=1e-5)
    assert got.dtype == torch.float32
    assert len(native_calls) == 2  # fc1 and fc2


def test_photonic_target_is_inference_only():
    weight = torch.randn(3, 3, requires_grad=True)
    with pytest.raises(Exception, match="inference-only"):
        get_runtime().execute("gemm", torch.randn(2, 3), weight, options=photonic())
