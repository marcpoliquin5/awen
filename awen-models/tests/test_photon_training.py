import json

import numpy as np
import pytest
import torch

awen_native = pytest.importorskip("awen_native")

from awen_models.photon_training import (  # noqa: E402
    PhotonTrainSettings,
    TrainingRegime,
    contract_with_photons,
    optical_energy_per_mac_j,
    run,
    surrogate_photonic_linear,
    wilson_interval,
)


def _no_static_terms(photons):
    c = contract_with_photons(awen_native.default_contract(), photons)
    c["phase_sigma_rad"] = 0.0
    c["crosstalk_db"] = None
    return c


def test_ideal_surrogate_is_exact_matmul():
    x = torch.rand(8, 150, dtype=torch.float64)
    w = torch.randn(5, 150, dtype=torch.float64)
    y = surrogate_photonic_linear(x, w, awen_native.ideal_contract(), tile_k=64)
    assert torch.allclose(y, x @ w.T, atol=1e-12)


@pytest.mark.parametrize("photons", [30.0, 1000.0])
def test_surrogate_error_statistics_match_rust_simulator(photons):
    torch.manual_seed(0)
    x = torch.rand(128, 256, dtype=torch.float64)
    w = torch.randn(64, 256, dtype=torch.float64) * 0.1
    contract = _no_static_terms(photons)
    exact = (x @ w.T).numpy()
    ys = surrogate_photonic_linear(x, w, contract, 64, torch.Generator().manual_seed(1)).numpy()
    yr, _ = awen_native.photonic_gemm(x.numpy(), w.numpy(), contract, tile_k=64, seed=7)
    s_sur, s_rust = np.std(ys - exact), np.std(yr - exact)
    assert abs(s_sur - s_rust) / s_rust < 0.05
    assert abs(np.mean(ys - exact) - np.mean(yr - exact)) < 0.05 * s_rust


def test_quantizers_pass_straight_through_gradients():
    x = torch.rand(4, 70, dtype=torch.float64, requires_grad=True)
    w = torch.randn(3, 70, dtype=torch.float64, requires_grad=True)
    surrogate_photonic_linear(x, w, contract_with_photons(awen_native.default_contract(), None)).sum().backward()
    assert torch.isfinite(x.grad).all() and x.grad.abs().sum() > 0
    assert torch.isfinite(w.grad).all() and w.grad.abs().sum() > 0


def test_energy_and_interval_helpers():
    assert optical_energy_per_mac_j(1.0) == pytest.approx(1.2816e-19, rel=1e-3)
    lo, hi = wilson_interval(95, 100)
    assert lo < 0.95 < hi and 0.88 < lo < 0.90


def test_harness_end_to_end_on_synthetic_data(tmp_path):
    g = torch.Generator().manual_seed(0)
    centers = torch.rand(10, 784, generator=g)
    def make(n):
        y = torch.randint(10, (n,), generator=g)
        return (centers[y] + 0.1 * torch.rand(n, 784, generator=g)).clamp(0, 1), y
    tx, ty = make(400)
    vx, vy = make(200)
    data = {"train_x": tx, "train_y": ty, "test_x": vx, "test_y": vy}
    report = run(
        seeds=[1, 2], out=tmp_path,
        settings=PhotonTrainSettings(hidden=16, epochs=4, batch_size=50),
        regimes=[TrainingRegime("standard", False), TrainingRegime("noise-aware-100", True, 100.0)],
        sweep=[10.0, 1e5], data=data,
    )
    saved = json.loads((tmp_path / "results.json").read_text())
    assert saved["fidelity"] == "simulated" and len(saved["runs"]) == 4
    assert (tmp_path / "accuracy_curves.png").stat().st_size > 1000
    enob = [p["tile_effective_bits"] for p in report["points"]]
    assert enob[0] < enob[1]
    for r in report["runs"]:
        assert r["ideal_accuracy"] > 0.6  # chance is 0.1
        assert r["sweep"][1]["accuracy"] >= r["sweep"][0]["accuracy"]
