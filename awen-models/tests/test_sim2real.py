import json
import math
from pathlib import Path

import numpy as np
import pytest
import torch

yaml = pytest.importorskip("yaml")

from awen_models.sim2real import (  # noqa: E402
    NoiseModelB,
    _quantize_uniform,
    find_assumptions,
    gemm_b,
    load_model_b,
    paired_verdict,
)

ASSUMPTIONS = Path(__file__).resolve().parents[2] / "awen-spec" / "hardware_assumptions.yaml"
IDEALISH = NoiseModelB(system_enob=60.0, weight_dac_bits=30, extinction_ratio_db=300.0)


def test_parameters_come_from_the_assumption_set_with_sources():
    doc = yaml.safe_load(ASSUMPTIONS.read_text())["parameters"]
    m = load_model_b(ASSUMPTIONS)
    assert m.system_enob == doc["system"]["pace_enob"]["typical"]
    assert m.weight_dac_bits == doc["dac"]["weight_dac_bits"]["typical"]
    assert m.extinction_ratio_db == doc["modulator"]["tfln_extinction_ratio"]["typical"]
    prov = dict(m.provenance)
    assert "hua2025, measured" in prov["system_enob"] and "wang2018, measured" in prov["extinction_ratio_db"]
    assert prov["assumption_set"] == "awen.hardware-assumptions.v0"
    assert find_assumptions(Path(__file__).parent) == ASSUMPTIONS.resolve()


def test_null_or_unsourced_values_are_refused(tmp_path):
    doc = yaml.safe_load(ASSUMPTIONS.read_text())
    doc["parameters"]["system"]["pace_enob"]["typical"] = None
    bad = tmp_path / "a.yaml"
    bad.write_text(yaml.safe_dump(doc))
    with pytest.raises(ValueError, match="pace_enob"):
        load_model_b(bad)


def test_near_ideal_model_b_is_exact_matmul():
    rng = np.random.default_rng(0)
    x, w = rng.standard_normal((6, 150)), rng.standard_normal((5, 150))
    assert np.allclose(gemm_b(x, w, IDEALISH, rng), x @ w.T, atol=1e-8)


def test_output_noise_matches_the_enob_formula():
    m = NoiseModelB(system_enob=6.0, weight_dac_bits=30, extinction_ratio_db=300.0)
    assert m.output_sigma == pytest.approx(2.0**-5 / math.sqrt(12.0))
    x, w = np.ones((4000, 64)), np.ones((1, 64))  # one tile, scales 1, so error = 64 * noise
    err = (gemm_b(x, w, m, np.random.default_rng(1)) - 64.0) / 64.0
    assert np.std(err) == pytest.approx(m.output_sigma, rel=0.05)
    assert abs(np.mean(err)) < 0.1 * m.output_sigma


def test_extinction_ratio_leaks_only_for_single_ended_inputs():
    m = NoiseModelB(system_enob=60.0, weight_dac_bits=30, extinction_ratio_db=10.0)  # leakage 0.1
    w = np.full((1, 4), 0.5)
    zero_in = gemm_b(np.zeros((1, 4)) + np.array([[1.0, 0, 0, 0]]), w, m, np.random.default_rng(0))
    # full-scale weight 0.5 -> normalized 1; leak = 0.1 * 4 per tile, times scales
    assert zero_in[0, 0] == pytest.approx(((1 - 0.1) * 1.0 + 0.1 * 4) * 0.5)
    signed = gemm_b(np.array([[1.0, -1.0, 0, 0]]), w, m, np.random.default_rng(0))
    assert signed[0, 0] == pytest.approx(0.0, abs=1e-12)


def test_weight_quantizer_grid_and_independent_rng():
    q = _quantize_uniform(np.linspace(-1, 1, 1001), 3)
    assert len(np.unique(q)) == 8 and q.min() == -1.0 and q.max() == 1.0
    m = load_model_b(ASSUMPTIONS)
    x, w = np.random.default_rng(2).random((3, 70)), np.random.default_rng(3).standard_normal((4, 70))
    a = gemm_b(x, w, m, np.random.default_rng(5))
    assert np.array_equal(a, gemm_b(x, w, m, np.random.default_rng(5)))
    assert not np.array_equal(a, gemm_b(x, w, m, np.random.default_rng(6)))
    assert m.with_enob(None) is m and m.with_enob(4.0).system_enob == 4.0


def test_paired_verdict_rule():
    assert paired_verdict([0.02, 0.03, 0.025])["verdict"] == "transfers"
    assert paired_verdict([-0.02, -0.03, -0.025])["verdict"] == "reverses"
    v = paired_verdict([0.01, -0.01, 0.0])
    assert v["verdict"] == "no detectable difference" and v["ci95"][0] < 0 < v["ci95"][1]


def test_harness_end_to_end_on_synthetic_data(tmp_path):
    pytest.importorskip("awen_native")
    from awen_models.photon_training import PhotonTrainSettings, TrainingRegime
    from awen_models.sim2real import run

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
        enob_sweep=[2.0, None],
    )
    saved = json.loads((tmp_path / "results.json").read_text())
    assert saved["fidelity"] == "simulated" and len(saved["runs"]) == 4 and saved["primary_point_index"] == 1
    assert saved["model_b_points"][1]["system_enob"] == 7.61
    assert len(report["verdicts_model_b"]["noise-aware-100 - standard"]) == 2
    for r in report["runs"]:
        assert r["accuracy_model_b"][1] > 0.6  # chance is 0.1
