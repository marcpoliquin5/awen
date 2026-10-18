"""The PyTorch 'awen' PrivateUse1 device: data movement, photonic GEMMs, electronic add/relu."""

import pytest

torch = pytest.importorskip("torch")
awen_native = pytest.importorskip("awen_native")

from awen_py import device as awen_device  # noqa: E402
from awen_py.errors import DeviceError  # noqa: E402


@pytest.fixture(autouse=True)
def ideal_device():
    awen_device.register()
    awen_device.configure(awen_native.ideal_contract())
    yield


def test_round_trip_to_and_from_device():
    x = torch.randn(3, 5)
    on_device = x.to("awen")
    assert on_device.device.type == "awen" and on_device.is_awen
    back = on_device.cpu()
    assert back.device.type == "cpu"
    torch.testing.assert_close(back, x, rtol=0, atol=0)
    assert on_device.to("awen", torch.float64).dtype == torch.float64


def test_matmul_and_linear_run_on_the_photonic_simulator():
    torch.manual_seed(0)
    x, w = torch.randn(4, 16), torch.randn(16, 6)
    with torch.no_grad():
        product = x.to("awen") @ w.to("awen")
        assert awen_device.gemm_calls() == 1
        torch.testing.assert_close(product.cpu(), x @ w, rtol=1e-5, atol=1e-5)
        layer = torch.nn.Linear(16, 6)
        reference = layer(x)
        on_device = layer.to("awen")
        assert on_device.weight.device.type == "awen"
        torch.testing.assert_close(on_device(x.to("awen")).cpu(), reference, rtol=1e-5, atol=1e-5)
        assert awen_device.gemm_calls() == 2
        batched = torch.randn(2, 3, 16)
        torch.testing.assert_close((batched.to("awen") @ w.to("awen")).cpu(), batched @ w, rtol=1e-5, atol=1e-5)


def test_mlp_with_electronic_add_and_relu_matches_cpu():
    torch.manual_seed(1)
    model = torch.nn.Sequential(torch.nn.Linear(8, 12), torch.nn.ReLU(), torch.nn.Linear(12, 4))
    x = torch.randn(5, 8)
    with torch.no_grad():
        expected = model(x) + model(x)
        model = model.to("awen")
        xa = x.to("awen")
        got = model(xa) + model(xa)
    torch.testing.assert_close(got.cpu(), expected, rtol=1e-5, atol=1e-5)


def test_noise_contract_applies_and_is_reproducible():
    x, w = torch.rand(8, 64) * 2 - 1, torch.rand(64, 8) * 2 - 1
    awen_device.configure(None, seed=3)  # simulator default (illustrative) contract
    first = (x.to("awen") @ w.to("awen")).cpu()
    awen_device.configure(None, seed=3)
    second = (x.to("awen") @ w.to("awen")).cpu()
    torch.testing.assert_close(first, second, rtol=0, atol=0)
    assert (first - x @ w).abs().max() > 0


def test_unsupported_ops_mixed_devices_and_autograd_fail_clearly():
    xa = torch.randn(2, 2).to("awen")
    with pytest.raises(NotImplementedError, match=r"\.cpu\(\)"):
        torch.sin(xa)
    with pytest.raises(DeviceError, match="expected all tensors"):
        xa + torch.randn(2, 2)
    weight = torch.randn(2, 2).to("awen").requires_grad_(True)
    with pytest.raises(NotImplementedError, match="sum"):
        (xa @ weight).sum().backward()


def test_device_module_reports_one_device():
    assert torch.awen.is_available() and torch.awen.device_count() == 1
