import json
from pathlib import Path

import numpy as np
import pytest
import torch

pytest.importorskip("yaml")

from awen_models.recalibration import (  # noqa: E402
    REGISTERED_BUDGETS,
    drift_path,
    interval_for_budget,
    tolerable_budget,
)
from awen_models.sim2real import NoiseModelB, gemm_b, load_model_b  # noqa: E402

ASSUMPTIONS = Path(__file__).resolve().parents[2] / "awen-spec" / "hardware_assumptions.yaml"
IDEALISH = NoiseModelB(system_enob=60.0, weight_dac_bits=30, extinction_ratio_db=300.0)


def test_gemm_b_without_drift_is_unchanged():
    m = load_model_b(ASSUMPTIONS)
    x, w = np.random.default_rng(2).random((3, 70)), np.random.default_rng(3).standard_normal((4, 70))
    a = gemm_b(x, w, m, np.random.default_rng(5))
    b = gemm_b(x, w, m, np.random.default_rng(5), drift=None)
    c = gemm_b(x, w, m, np.random.default_rng(5), drift=np.zeros_like(w))
    assert np.array_equal(a, b) and np.array_equal(a, c)


def test_drift_is_added_in_normalized_units_and_clipped():
    x, w = np.ones((1, 4)), np.array([[0.5, 0.25, -0.5, 0.0]])  # tile max 0.5 -> normalized [1, .5, -1, 0]
    d = np.array([[0.5, 0.1, -0.2, 0.3]])
    got = gemm_b(x, w, IDEALISH, np.random.default_rng(0), drift=d)
    expected = (1.0 + 0.6 - 1.0 + 0.3) * 0.5  # first and third clip at +-1
    assert got[0, 0] == pytest.approx(expected, abs=1e-9)


def test_drift_path_variance_grows_linearly_with_time():
    path = drift_path((200_000,), 0.1, 4, np.random.default_rng(0))
    for j, offset in enumerate(path):
        assert np.var(offset) == pytest.approx(0.01 * (j + 0.5) / 4, rel=0.02)
    # increments are independent: covariance of first point and next increment ~ 0
    assert abs(np.mean(path[0] * (path[1] - path[0]))) < 1e-4


def test_interval_for_budget():
    assert interval_for_budget(0.02, 0.01) == pytest.approx(4.0)
    with pytest.raises(ValueError):
        interval_for_budget(0.02, 0.0)


def test_tolerable_budget_rule():
    budgets = [0.0, 0.01, 0.05, 0.1]
    curves = [[0.95, 0.949, 0.93, 0.95], [0.96, 0.958, 0.94, 0.96], [0.955, 0.954, 0.935, 0.955]]
    r = tolerable_budget(curves, budgets)
    # 0.05 fails (drop 0.02), so 0.1 does not count even though it passes
    assert r["tolerable_budget"] == 0.01 and [row["passes"] for row in r["rows"]] == [True, True, False, True]
    assert REGISTERED_BUDGETS[0] == 0.0 and sorted(REGISTERED_BUDGETS) == REGISTERED_BUDGETS


def test_harness_end_to_end_on_synthetic_data(tmp_path):
    pytest.importorskip("awen_native")
    from awen_models.photon_training import PhotonTrainSettings, TrainingRegime
    from awen_models.recalibration import run

    g = torch.Generator().manual_seed(0)
    centers = torch.rand(10, 784, generator=g)

    def make(n):
        y = torch.randint(10, (n,), generator=g)
        return (centers[y] + 0.1 * torch.rand(n, 784, generator=g)).clamp(0, 1), y

    (tx, ty), (vx, vy) = make(300), make(150)
    report = run(
        ASSUMPTIONS, seeds=[1, 2], out=tmp_path, data={"train_x": tx, "train_y": ty, "test_x": vx, "test_y": vy},
        settings=PhotonTrainSettings(hidden=16, epochs=3, batch_size=50),
        regimes=[TrainingRegime("standard", False), TrainingRegime("noise-aware-100", True, 100.0)],
        budgets=[0.0, 0.05, 2.0], times=2,
    )
    saved = json.loads((tmp_path / "results.json").read_text())
    assert saved["fidelity"] == "simulated" and len(saved["runs"]) == 4 and saved["budgets"] == [0.0, 0.05, 2.0]
    assert "free parameters" in saved["drift_values"] and saved["model_b"]["system_enob"] == 7.61
    for r in report["runs"]:
        acc = [p["accuracy"] for p in r["curve"]]
        assert acc[0] > 0.6 and acc[2] < acc[0]  # huge drift destroys accuracy; chance is 0.1
    assert set(report["tolerable_budget"]) == {"standard", "noise-aware-100"}
    assert len(report["exploratory_regime_comparisons"]["noise-aware-100 - standard"]) == 3
    with pytest.raises(ValueError, match="start at 0"):
        run(ASSUMPTIONS, seeds=[1], out=tmp_path, data={"train_x": tx, "train_y": ty, "test_x": vx, "test_y": vy}, budgets=[0.1])
